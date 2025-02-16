use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crossterm::event::{poll, read, Event};

use crate::actions::{record_ticks_for_period, send_error_message, Action, RECORD_TICKS_INTERVAL};

pub fn collect_terminal_events(tx: Sender<Action>, exit_flag: &AtomicBool) {
    let mut prev_marker = Instant::now();
    let mut next_marker = prev_marker + RECORD_TICKS_INTERVAL;
    let mut ticks = 0_u32;

    'outer: loop {
        match poll(Duration::from_millis(20)) {
            Ok(available) => {
                if available {
                    match read() {
                        Ok(event) => {
                            if let Event::Key(event) = event {
                                tx.send(Action::KeyPress(event)).unwrap();
                            }
                        }
                        Err(e) => {
                            let msg = format!("{}", e);
                            send_error_message(&tx, "Terminal", &msg);
                        }
                    }
                }
            }
            Err(e) => {
                let msg = format!("{}", e);
                send_error_message(&tx, "Terminal", &msg);
            }
        }

        ticks += 1;

        let curr_time = Instant::now();
        if curr_time >= next_marker {
            // Send message with loop count for period
            record_ticks_for_period(&tx, "Terminal", ticks, prev_marker, curr_time);

            // Set next marker, ensuring in the future
            ticks = 0;
            prev_marker = next_marker;
            while next_marker < curr_time {
                next_marker += RECORD_TICKS_INTERVAL;
            }
        }

        if exit_flag.load(Ordering::Relaxed) {
            break 'outer;
        }
    }
}
