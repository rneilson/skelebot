use std::time::Instant;

pub fn record_ticks_for_period(name: &str, ticks: u32, prev_time: Instant, curr_time: Instant) {
    // TODO: probably need to handle this more gracefully
    let ms_since = curr_time.checked_duration_since(prev_time).unwrap().as_millis();
    println!("{0} looped {1} times in {2}ms", name, ticks, ms_since);
}