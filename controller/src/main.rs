#![allow(clippy::explicit_write)]

use std::error::Error;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent};
use crossterm::terminal;

mod actions;
mod joystick;
mod radio;
mod term;
mod ui;

use actions::{Action, ControlState, StickPosition};
use ui::UIUpdate;

fn main() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    write!(io::stdout(), "Starting up...\r\n")?;

    let control_state_atomic = AtomicU64::new(0);
    let exit_flag = AtomicBool::new(false);
    let mut err_msg: Option<String> = None;

    let (tx, rx) = mpsc::channel::<Action>();
    let j_tx = tx.clone();
    let r_tx = tx.clone();
    let t_tx = tx.clone();
    let u_tx = tx.clone();
    drop(tx);

    let (ui_tx, ui_rx) = mpsc::channel::<UIUpdate>();

    thread::scope(|s| {
        s.spawn(|| {
            joystick::collect_joystick_events(j_tx, &exit_flag);
        });
        s.spawn(|| {
            radio::radio_comms(r_tx, &control_state_atomic, &exit_flag);
        });
        s.spawn(|| {
            term::collect_terminal_events(t_tx, &exit_flag);
        });
        s.spawn(|| {
            ui::draw_ui(ui_rx, u_tx, &exit_flag);
        });

        // Loop over channel rx and process events
        // Set error message and exit flag on any error, then allow threads to end
        if let Err(e) = handle_actions(rx, ui_tx, &exit_flag, &control_state_atomic) {
            err_msg = Some(format!("{}", e));
            exit_flag.store(true, Ordering::Relaxed);
        }
    });

    if let Some(msg) = err_msg {
        write!(io::stderr(), "{0}\r\n", msg)?;
    }

    write!(io::stdout(), "Shutting down...\r\n")?;
    terminal::disable_raw_mode()
}

fn handle_actions(
    rx: Receiver<Action>,
    ui_tx: Sender<UIUpdate>,
    exit_flag: &AtomicBool,
    control_state_atomic: &AtomicU64,
) -> Result<(), Box<dyn Error>> {
    let max_wait = Duration::from_millis(20);
    'listener: loop {
        match rx.recv_timeout(max_wait) {
            Ok(action) => {
                match action {
                    Action::Message(msg) => {
                        ui_tx.send(UIUpdate::Message(msg))?;
                        // write!(io::stdout(), "{0}: {1}\r\n", msg.name, msg.message)?;
                    }
                    Action::Error(err) => {
                        ui_tx.send(UIUpdate::Error(err))?;
                        // write!(
                        //     io::stderr(),
                        //     "Error from {0}: {1}\r\n",
                        //     err.name,
                        //     err.message
                        // )?;
                    }
                    Action::Fatal(err) => {
                        return Err(
                            format!("Fatal error from {0}: {1}", err.name, err.message).into()
                        );
                    }
                    Action::KeyPress(key_event) => {
                        let control_state =
                            ControlState::from(control_state_atomic.load(Ordering::Relaxed));
                        match handle_keypress_event(control_state, key_event) {
                            Some(control_state) => {
                                control_state_atomic
                                    .store(control_state.as_u64(), Ordering::Relaxed);
                                ui_tx.send(UIUpdate::Control(control_state))?;
                                // write!(io::stdout(), "Control state: {:?}\r\n", control_state)?;
                            }
                            None => {
                                exit_flag.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                    Action::StickUpdate(stick_pos) => {
                        let control_state = handle_stick_positions(stick_pos.0, stick_pos.1);
                        control_state_atomic.store(control_state.as_u64(), Ordering::Relaxed);
                        ui_tx.send(UIUpdate::Control(control_state))?;
                        // write!(io::stdout(), "Control state: {:?}\r\n", control_state)?;
                    }
                    Action::BatteryUpdate(voltage) => {
                        ui_tx.send(UIUpdate::Battery(voltage))?;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // If we've timed out after signalling exit, just break
                if exit_flag.load(Ordering::Relaxed) {
                    break 'listener;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Disconnected implies all senders dropped
                break 'listener;
            }
        }
    }

    Ok(())
}

/// Returns a modified control state if arrow keys are pressed, or None if the quit
/// key ('q' at present) is pressed
fn handle_keypress_event(
    mut control_state: ControlState,
    key_event: KeyEvent,
) -> Option<ControlState> {
    match key_event.code {
        // Quit on 'q'
        KeyCode::Char('q') => {
            return None;
        }
        // Manipulate control state on arrow keys
        KeyCode::Up => {
            let mut step = 8_192;
            if control_state.throttle <= (i16::MIN + 1) {
                step = 8_191;
            }
            control_state.throttle = control_state.throttle.saturating_add(step);
        }
        KeyCode::Down => {
            let mut step = 8_192;
            if control_state.throttle == i16::MAX {
                step = 8_191;
            }
            control_state.throttle = control_state.throttle.saturating_sub(step);
        }
        KeyCode::Right => {
            let mut step = 8_192;
            if control_state.steering <= (i16::MIN + 1) {
                step = 8_191;
            }
            control_state.steering = control_state.steering.saturating_add(step);
        }
        KeyCode::Left => {
            let mut step = 8_192;
            if control_state.steering == i16::MAX {
                step = 8_191;
            }
            control_state.steering = control_state.steering.saturating_sub(step);
        }
        // Reset control state to center on spacebar
        KeyCode::Char(' ') => {
            control_state.throttle = 0;
            control_state.steering = 0;
        }
        // Ignore others
        _ => {}
    }
    Some(control_state.trim())
}

/// Converts a joystick position to a new control state
fn handle_stick_positions(move_pos: StickPosition, view_pos: StickPosition) -> ControlState {
    // Convert stick position to control state
    // At present this is a simple mapping of Y axis to throttle
    // and X axis to steering
    let control_state = ControlState {
        throttle: move_pos.y,
        steering: move_pos.x,
        pan: view_pos.x,
        tilt: view_pos.y,
    };
    control_state.trim()
}
