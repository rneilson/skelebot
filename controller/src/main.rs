use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent};

mod actions;
mod joystick;
mod radio;
mod term;
mod ui;

use actions::{Action, ControlState, StickPosition};

fn main() {
    let exit_flag = AtomicBool::new(false);
    let mut err_msg: Option<String> = None;

    let (tx, rx) = mpsc::channel::<Action>();
    let j_tx = tx.clone();
    let r_tx = tx.clone();
    let t_tx = tx.clone();
    let u_tx = tx.clone();
    drop(tx);

    let control_state_atomic = AtomicU32::new(0);

    println!("Starting up...");
    let exit_at = Instant::now() + Duration::from_secs(5);

    thread::scope(|s| {
        s.spawn(|| {
            joystick::collect_joystick_events(j_tx, &exit_flag);
        });
        s.spawn(|| {
            radio::radio_comms(r_tx, &exit_flag);
        });
        s.spawn(|| {
            term::collect_terminal_events(t_tx, &exit_flag);
        });
        s.spawn(|| {
            ui::draw_ui(u_tx, &exit_flag);
        });

        // Loop over channel rx and process events
        // TEMP: until quittin' time
        let max_wait = Duration::from_millis(20);
        'listener: loop {
            match rx.recv_timeout(max_wait) {
                Ok(action) => {
                    match action {
                        Action::Message(msg) => {
                            println!("{0}: {1}", msg.name, msg.message);
                        }
                        Action::Error(err) => {
                            eprintln!("Error from {0}: {1}", err.name, err.message);
                        }
                        Action::Fatal(err) => {
                            err_msg =
                                Some(format!("Fatal error from {0}: {1}", err.name, err.message));
                            exit_flag.store(true, Ordering::Relaxed);
                        }
                        Action::KeyPress(key_event) => {
                            let control_state =
                                ControlState::from(control_state_atomic.load(Ordering::Relaxed));
                            match handle_keypress_event(control_state, key_event) {
                                Some(control_state) => {
                                    control_state_atomic
                                        .store(control_state.as_u32(), Ordering::Relaxed);
                                    // TODO: send update message to UI thread
                                    println!("Control state: {:?}", control_state);
                                }
                                None => {
                                    exit_flag.store(true, Ordering::Relaxed);
                                }
                            }
                        }
                        Action::StickUpdate(stick_pos) => {
                            let control_state = handle_stick_position(stick_pos);
                            control_state_atomic.store(control_state.as_u32(), Ordering::Relaxed);
                            // TODO: send update message to UI thread
                            println!("Control state: {:?}", control_state);
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
            if Instant::now() >= exit_at {
                exit_flag.store(true, Ordering::Relaxed);
            }
        }
    });

    if let Some(msg) = err_msg {
        eprintln!("{0}", msg);
    }

    println!("Shutting down...");
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
            control_state.throttle = control_state.throttle.saturating_add(8_192);
        }
        KeyCode::Down => {
            control_state.throttle = control_state.throttle.saturating_sub(8_192);
        }
        KeyCode::Right => {
            control_state.steering = control_state.steering.saturating_add(8_192);
        }
        KeyCode::Left => {
            control_state.steering = control_state.steering.saturating_sub(8_192);
        }
        // Reset control state to center on spacebar
        KeyCode::Char(' ') => {
            control_state.throttle = 0;
            control_state.steering = 0;
        }
        // Ignore others
        _ => {}
    }
    control_state.trim();
    Some(control_state)
}

/// Converts a joystick position to a new control state
fn handle_stick_position(stick_pos: StickPosition) -> ControlState {
    // Convert stick position to control state
    // At present this is a simple mapping of Y axis to throttle
    // and X axis to steering
    let mut control_state = ControlState {
        throttle: stick_pos.y,
        steering: stick_pos.x,
    };
    control_state.trim();
    control_state
}
