use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::thread::sleep;

use crate::actions::record_ticks_for_period;

pub fn collect_terminal_events(exit_flag: &AtomicBool) {
    let mut prev_marker = Instant::now();
    let mut next_marker = prev_marker + Duration::from_secs(1);
    let mut ticks = 0_u32;

    'outer: loop {
        // TODO: actual work
        sleep(Duration::from_millis(20));
        ticks += 1;

        let curr_time = Instant::now();
        if curr_time >= next_marker {
            // Send message with loop count for period
            record_ticks_for_period("Terminal", ticks, prev_marker, curr_time);

            // Set next marker, ensuring in the future
            ticks = 0;
            prev_marker = next_marker;
            while next_marker < curr_time {
                next_marker += Duration::from_secs(1);
            }
        }

        if exit_flag.load(Ordering::Relaxed) {
            break 'outer;
        }
    }
}
