#![allow(clippy::explicit_write)]

use std::error::Error;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent};
use crossterm::terminal;
use dbus::blocking::Connection;

mod actions;
mod joystick;
mod radio;
mod term;
mod ui;

use actions::{Action, ControlState, StickValues};
use ui::UIUpdate;

struct ToggleButtons {
    r#move: bool,
    view: bool,
}

fn main() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    write!(io::stdout(), "Starting up...\r\n")?;

    // Prevent screen blanking and locking via d-bus call
    let mut dbus_conn: Option<Connection> = None;
    let mut dbus_cookie: Option<u32> = None;
    match Connection::new_session() {
        Ok(conn) => {
            dbus_conn = Some(conn);
        }
        Err(e) => {
            write!(io::stderr(), "Error creating D-Bus connection: {}\r\n", e)?;
        }
    }
    if let Some(ref conn) = dbus_conn {
        let proxy = conn.with_proxy(
            "org.freedesktop.ScreenSaver",
            "/org/freedesktop/ScreenSaver",
            Duration::from_millis(5_000),
        );
        let result: Result<(u32,), dbus::Error> = proxy.method_call(
            "org.freedesktop.ScreenSaver",
            "Inhibit",
            ("controller.rs", "wake lock enabled"),
        );
        match result {
            Ok((cookie,)) => {
                dbus_cookie = Some(cookie);
                write!(io::stdout(), "Screensaver inhibited\r\n")?;
            }
            Err(e) => {
                write!(io::stderr(), "Error inhibiting screensaver: {}\r\n", e)?;
            }
        }
    }

    let control_state_mutex = Arc::new(Mutex::new(ControlState::new()));
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
            radio::radio_comms(r_tx, Arc::clone(&control_state_mutex), &exit_flag);
        });
        s.spawn(|| {
            term::collect_terminal_events(t_tx, &exit_flag);
        });
        s.spawn(|| {
            ui::draw_ui(ui_rx, u_tx, &exit_flag);
        });

        // Loop over channel rx and process events
        // Set error message and exit flag on any error, then allow threads to end
        if let Err(e) = handle_actions(rx, ui_tx, &exit_flag, Arc::clone(&control_state_mutex)) {
            err_msg = Some(format!("{}", e));
            exit_flag.store(true, Ordering::Relaxed);
        }
    });

    if let Some(msg) = err_msg {
        write!(io::stderr(), "{0}\r\n", msg)?;
    }

    if let Some(ref conn) = dbus_conn {
        if let Some(cookie) = dbus_cookie {
            let proxy = conn.with_proxy(
                "org.freedesktop.ScreenSaver",
                "/org/freedesktop/ScreenSaver",
                Duration::from_millis(5_000),
            );
            let result: Result<(), dbus::Error> =
                proxy.method_call("org.freedesktop.ScreenSaver", "UnInhibit", (cookie,));
            if let Err(e) = result {
                write!(io::stderr(), "Error uninhibiting screensaver: {}\r\n", e)?;
            }
        } else {
            write!(
                io::stderr(),
                "DBus connection present but no cookie for uninhibit call\r\n"
            )?;
        }
    }

    write!(io::stdout(), "Shutting down...\r\n")?;
    terminal::disable_raw_mode()
}

fn handle_actions(
    rx: Receiver<Action>,
    ui_tx: Sender<UIUpdate>,
    exit_flag: &AtomicBool,
    control_state_mutex: Arc<Mutex<ControlState>>,
) -> Result<(), Box<dyn Error>> {
    let max_wait = Duration::from_millis(20);
    let mut buttons = ToggleButtons {
        r#move: false,
        view: false,
    };

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
                        let prev_state = {
                            let prev_state = control_state_mutex.lock().unwrap();
                            prev_state.clone()
                        };
                        match handle_keypress_event(&prev_state, key_event) {
                            Some(control_state) => {
                                if control_state != prev_state {
                                    let mut stored_state = control_state_mutex.lock().unwrap();
                                    *stored_state = control_state;
                                }
                                ui_tx.send(UIUpdate::Control(control_state))?;
                                // write!(io::stdout(), "Control state: {:?}\r\n", control_state)?;
                            }
                            None => {
                                exit_flag.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                    Action::StickUpdate(stick_pos) => {
                        let prev_state = {
                            let prev_state = control_state_mutex.lock().unwrap();
                            prev_state.clone()
                        };
                        let control_state =
                            handle_stick_positions(&prev_state, &mut buttons, stick_pos);
                        if control_state != prev_state {
                            let mut stored_state = control_state_mutex.lock().unwrap();
                            *stored_state = control_state;
                        }
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
fn handle_keypress_event(prev_state: &ControlState, key_event: KeyEvent) -> Option<ControlState> {
    let mut control_state = prev_state.clone();
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
        // Cycle through speed modes
        KeyCode::Char('m') => {
            control_state.move_speed = control_state.move_speed.toggle();
        }
        // Ignore others
        _ => {}
    }
    Some(control_state.trim())
}

/// Converts a joystick position to a new control state
fn handle_stick_positions(
    prev_state: &ControlState,
    buttons: &mut ToggleButtons,
    stick_pos: StickValues,
) -> ControlState {
    // Convert stick position to control state
    // At present this is a simple mapping of Y axis to throttle
    // and X axis to steering, except the movement speed toggle
    let StickValues(move_pos, view_pos) = stick_pos;
    let mut move_speed = prev_state.move_speed;

    if move_pos.button && !buttons.r#move {
        move_speed = move_speed.toggle();
    }
    buttons.r#move = move_pos.button;
    buttons.view = view_pos.button;

    let control_state = ControlState {
        throttle: move_pos.y,
        steering: move_pos.x,
        pan: view_pos.x,
        tilt: view_pos.y,
        move_speed: move_speed,
    };
    control_state.trim()
}
