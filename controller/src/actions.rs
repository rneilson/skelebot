#![allow(dead_code)]

use std::i16;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crossterm::event::KeyEvent;

pub const RECORD_TICKS_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub struct ControlState {
    pub throttle: i16,
    pub steering: i16,
}

impl ControlState {
    pub fn as_u32(&self) -> u32 {
        let high = (self.throttle as u16) as u32;
        let low = (self.steering as u16) as u32;
        (high << 16) | low
    }

    pub fn trim(mut self) -> Self {
        if self.throttle == i16::MIN {
            self.throttle += 1;
        }
        if self.steering == i16::MIN {
            self.steering += 1;
        }
        self
    }

    // Convert throttle and steering values to left/right tank-drive values,
    // as expressed in +/- %
    // Constant curvature drive logic from https://ewpratten.com/blog/joystick-to-voltage
    // except straight tank drive when no throttle component
    pub fn as_tank_drive(&self) -> (i8, i8) {
        let t = (self.throttle as f64) / (i16::MAX as f64);
        let s = (self.steering as f64) / (i16::MAX as f64);

        let (left, right) = if t == 0.0 {
            // Use tank drive when no throttle applied to allow turning in-place
            let left = t + s;
            let right = t - s;
            (left, right)
        } else {
            // Use constant curvature when throttle is applied
            let left = t + (t.abs() * s);
            let right = t - (t.abs() * s);
            (left, right)
        };
        let m = f64::max(left.abs(), right.abs()).max(1.0);

        let left = (100.0 * left / m).clamp(-100.0, 100.0) as i8;
        let right = (100.0 * right / m).clamp(-100.0, 100.0) as i8;

        (left, right)
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

#[derive(Clone, Debug)]
pub struct StickPosition {
    pub x: i16,
    pub y: i16,
}

#[derive(Debug)]
pub struct BatteryVoltage(pub u16);

impl BatteryVoltage {
    pub fn as_float(&self) -> f32 {
        f32::from(self.0) / 1024.0
    }
}

#[derive(Debug)]
pub enum Action {
    Message(ThreadMsg),
    Error(ThreadMsg),
    Fatal(ThreadMsg),
    KeyPress(KeyEvent),
    StickUpdate(StickPosition),
    BatteryUpdate(BatteryVoltage),
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

pub fn send_message(tx: &Sender<Action>, name: &str, msg: &str) {
    let msg = ThreadMsg {
        name: name.to_owned(),
        message: msg.to_owned(),
    };
    tx.send(Action::Message(msg)).unwrap();
}

pub fn send_error_message(tx: &Sender<Action>, name: &str, msg: &str) {
    let msg = ThreadMsg {
        name: name.to_owned(),
        message: msg.to_owned(),
    };
    tx.send(Action::Error(msg)).unwrap();
}
