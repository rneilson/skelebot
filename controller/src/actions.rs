#![allow(dead_code)]

use std::i16;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crossterm::event::KeyEvent;

pub const RECORD_TICKS_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ControlSpeed {
    Slow,
    Fast,
}

impl ToString for ControlSpeed {
    fn to_string(&self) -> String {
        match self {
            Self::Fast => String::from("Fast"),
            Self::Slow => String::from("Slow"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlState {
    pub throttle: i16,
    pub steering: i16,
    pub pan: i16,
    pub tilt: i16,
    pub move_speed: ControlSpeed,
}

impl ControlState {
    pub fn new() -> Self {
        Self {
            throttle: 0,
            steering: 0,
            pan: 0,
            tilt: 0,
            move_speed: ControlSpeed::Slow,
        }
    }

    pub fn trim(mut self) -> Self {
        if self.throttle == i16::MIN {
            self.throttle += 1;
        }
        if self.steering == i16::MIN {
            self.steering += 1;
        }
        if self.pan == i16::MIN {
            self.pan += 1;
        }
        if self.tilt == i16::MIN {
            self.tilt += 1;
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
        let max = f64::max(left.abs(), right.abs()).max(1.0);
        let factor = match self.move_speed {
            ControlSpeed::Slow => 50.0,
            ControlSpeed::Fast => 100.0,
        };

        let left = (factor * left / max).clamp(-factor, factor) as i8;
        let right = (factor * right / max).clamp(-factor, factor) as i8;

        (left, right)
    }

    // Convert pan and tilt values to angular values in +/- degrees (max 90°)
    pub fn as_camera_angles(&self) -> (i8, i8) {
        let pan = (self.pan as f64) / (i16::MAX as f64);
        let tilt = (self.tilt as f64) / (i16::MAX as f64);

        let pan = (90.0 * pan).clamp(-90.0, 90.0) as i8;
        let tilt = (90.0 * tilt).clamp(-90.0, 90.0) as i8;

        (pan, tilt)
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
    pub toggle: bool,
}

#[derive(Clone, Debug)]
pub struct StickValues(pub StickPosition, pub StickPosition);

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
    StickUpdate(StickValues),
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
