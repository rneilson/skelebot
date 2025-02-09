use std::thread;
// use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

mod actions;
mod joystick;
mod radio;
mod term;
mod ui;

fn main() {
    let exit_flag = AtomicBool::new(false);
    // TODO: create channel

    println!("Starting up...");

    thread::scope(|s| {
        s.spawn(|| {joystick::collect_joystick_events(&exit_flag);});
        s.spawn(|| {radio::radio_comms(&exit_flag);});
        s.spawn(|| {term::collect_terminal_events(&exit_flag);});
        s.spawn(|| {ui::draw_ui(&exit_flag);});

        // TODO: loop over channel rx

        // Placeholder
        thread::sleep(Duration::from_secs(5));

        exit_flag.store(true, Ordering::Relaxed);
    });

    println!("Shutting down...");
}
