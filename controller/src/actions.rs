use std::sync::mpsc::Sender;
use std::time::Instant;

pub enum Action {
    Message(String),
    Error(String),
}

pub fn record_ticks_for_period(tx: &Sender<Action>, name: &str, ticks: u32, prev_time: Instant, curr_time: Instant) {
    // TODO: probably need to handle this more gracefully
    let ms_since = curr_time.checked_duration_since(prev_time).unwrap().as_millis();
    let msg = format!("{0} looped {1} times in {2}ms", name, ticks, ms_since);
    tx.send(Action::Message(msg)).unwrap();
}