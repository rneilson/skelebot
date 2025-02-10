use std::sync::mpsc::Sender;
use std::time::Instant;

pub struct ThreadMsg {
    pub name: String,
    pub message: String,
}

pub enum Action {
    Message(ThreadMsg),
    Error(ThreadMsg),
    Fatal(ThreadMsg),
}

pub fn record_ticks_for_period(tx: &Sender<Action>, name: &str, ticks: u32, prev_time: Instant, curr_time: Instant) {
    // TODO: probably need to handle this more gracefully
    let ms_since = curr_time.checked_duration_since(prev_time).unwrap().as_millis();
    let msg = format!("looped {0} times in {1}ms", ticks, ms_since);
    let msg = ThreadMsg { name: name.to_owned(), message: msg };
    tx.send(Action::Message(msg)).unwrap();
}