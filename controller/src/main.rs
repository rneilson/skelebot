use std::thread;
use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

mod actions;
mod joystick;
mod radio;
mod term;
mod ui;

use actions::Action;

fn main() {
    let exit_flag = AtomicBool::new(false);
    let mut err_msg: Option<String> = None;
    let (tx, rx) = mpsc::channel::<Action>();
    let j_tx = tx.clone();
    let r_tx = tx.clone();
    let t_tx = tx.clone();
    let u_tx = tx.clone();
    drop(tx);

    println!("Starting up...");
    let exit_at = Instant::now() + Duration::from_secs(5);

    thread::scope(|s| {
        s.spawn(|| {joystick::collect_joystick_events(j_tx, &exit_flag);});
        s.spawn(|| {radio::radio_comms(r_tx, &exit_flag);});
        s.spawn(|| {term::collect_terminal_events(t_tx, &exit_flag);});
        s.spawn(|| {ui::draw_ui(u_tx, &exit_flag);});

        // Loop over channel rx and process events
        // TEMP: until quittin' time
        let max_wait = Duration::from_millis(20);
        'listener: loop {
            match rx.recv_timeout(max_wait) {
                Ok(action) => {
                    match action {
                        Action::Message(msg) => {
                            println!("{0}", msg);
                        },
                        Action::Error(err) => {
                            eprintln!("Error: {0}", err);
                        },
                        Action::Fatal(err) => {
                            err_msg = Some(format!("Fatal error: {0}", err));
                            exit_flag.store(true, Ordering::Relaxed);
                        },
                    }
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // If we've timed out after signalling exit, just break
                    if exit_flag.load(Ordering::Relaxed) {
                        break 'listener;
                    }
                },
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // Disconnected implies all senders dropped
                    break 'listener;
                },
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
