#![allow(dead_code)]

use std::sync::mpsc::Sender;
use std::time::Instant;

use crossterm::event::KeyEvent;

#[derive(Debug)]
pub struct ControlState {
    pub throttle: i16,
    pub steering: i16,
}

impl ControlState {
    pub fn as_u32(&self) -> u32 {
        let high = self.throttle as u32;
        let low = self.steering as u32;
        (high << 16) | low
    }

    pub fn trim(&mut self) {
        if self.throttle == i16::MIN {
            self.throttle += 1;
        }
        if self.steering == i16::MIN {
            self.steering += 1;
        }
    }
}

impl From<u32> for ControlState {
    fn from(value: u32) -> Self {
        Self {
            throttle: (value >> 16) as i16,
            steering: (value & 0xffff) as i16,
        }
    }
}

#[derive(Debug)]
pub struct ThreadMsg {
    pub name: String,
    pub message: String,
}

#[derive(Debug)]
pub struct StickPosition {
    pub x: i16,
    pub y: i16,
}

#[derive(Debug)]
pub enum Action {
    Message(ThreadMsg),
    Error(ThreadMsg),
    Fatal(ThreadMsg),
    KeyPress(KeyEvent),
    StickUpdate(StickPosition),
}

pub fn record_ticks_for_period(
    tx: &Sender<Action>,
    name: &str,
    ticks: u32,
    prev_time: Instant,
    curr_time: Instant,
) {
    // TODO: probably need to handle this more gracefully
    let ms_since = curr_time
        .checked_duration_since(prev_time)
        .unwrap()
        .as_millis();
    let msg = format!("looped {0} times in {1}ms", ticks, ms_since);
    let msg = ThreadMsg {
        name: name.to_owned(),
        message: msg,
    };
    tx.send(Action::Message(msg)).unwrap();
}
