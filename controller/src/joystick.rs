use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::sleep;
use std::time::{Duration, Instant};

use evdev::{AbsoluteAxisCode, Device, EventSummary, InputEvent};
use nix::sys::epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags, EpollTimeout};

use crate::actions::{
    record_ticks_for_period, send_error_message, send_message, Action, StickPosition,
    RECORD_TICKS_INTERVAL,
};

const MAX_WAIT: Duration = Duration::from_millis(100);
const DEAD_ZONE: i32 = (i16::MAX as i32) / 10;

pub fn collect_joystick_events(tx: Sender<Action>, exit_flag: &AtomicBool) {
    let mut prev_marker = Instant::now();
    let mut next_marker = prev_marker + RECORD_TICKS_INTERVAL;
    let mut ticks = 0_u32;
    let mut advance_ticks = || {
        ticks += 1;

        let curr_time = Instant::now();
        if curr_time >= next_marker {
            // Send message with loop count for period
            record_ticks_for_period(&tx, "Joystick", ticks, prev_marker, curr_time);

            // Set next marker, ensuring in the future
            ticks = 0;
            prev_marker = next_marker;
            while next_marker < curr_time {
                next_marker += RECORD_TICKS_INTERVAL;
            }
        }
    };

    let mut device: Option<StickDevice> = None;

    'outer: loop {
        // Try to find an appropriate joystick device
        if device.is_none() {
            match StickDevice::find() {
                Ok(maybe_device) => {
                    device = maybe_device;
                    if let Some(ref dev) = device {
                        let msg = format!("opened joystick device \"{}\"", dev.get_path());
                        send_message(&tx, "Joystick", &msg);
                    }
                }
                Err(e) => {
                    let msg = format!("couldn't open joystick device: {}", e);
                    send_error_message(&tx, "Joystick", &msg);
                }
            }
        }
        // If a device is already open, process any events, otherwise wait
        match device {
            Some(ref mut dev) => {
                match dev.update_position() {
                    Ok(pos) => {
                        if let Err(_) = tx.send(Action::StickUpdate(pos)) {
                            // Can happen during shutdown
                            device = None;
                        }
                    }
                    Err(e) => {
                        let msg = format!("error updating position: {}", e);
                        send_error_message(&tx, "Joystick", &msg);
                        // Clear device so we can try reopening
                        device = None;
                    }
                };
            }
            None => {
                // Sleep for a while, and we'll look for another joystick
                sleep(MAX_WAIT);
            }
        };

        advance_ticks();
        if exit_flag.load(Ordering::Relaxed) {
            break 'outer;
        }
    }
}

struct StickDevice {
    device: Device,
    epoll: Epoll,
    pos: StickPosition,
}

impl StickDevice {
    pub fn find() -> Result<Option<Self>, io::Error> {
        for dev_file in glob::glob("/dev/input/by-id/*-event-joystick").unwrap() {
            let dev_file = match dev_file {
                Ok(dev_file) => dev_file,
                Err(_) => {
                    continue;
                }
            };
            let device = Device::open(dev_file)?;
            // Check that X and Y axes are supported
            let supported = device.supported_absolute_axes().map_or(false, |axes| {
                axes.contains(AbsoluteAxisCode::ABS_X) && axes.contains(AbsoluteAxisCode::ABS_Y)
            });
            if !supported {
                continue;
            }
            // Set up epoll for non-blocking access
            device.set_nonblocking(true)?;
            let epoll = Epoll::new(EpollCreateFlags::EPOLL_CLOEXEC)?;
            let event = EpollEvent::new(EpollFlags::EPOLLIN, 0);
            epoll.add(&device, event)?;

            return Ok(Some(StickDevice {
                device,
                epoll,
                pos: StickPosition { x: 0, y: 0 },
            }));
        }
        Ok(None)
    }

    pub fn get_path(&self) -> String {
        self.device
            .physical_path()
            .unwrap_or("<unknown>")
            .to_owned()
    }

    pub fn update_position(&mut self) -> Result<StickPosition, io::Error> {
        let mut events = [EpollEvent::empty(); 2];
        let max_wait = EpollTimeout::try_from(MAX_WAIT).unwrap();
        self.epoll.wait(&mut events, max_wait)?;

        match self.device.fetch_events() {
            Ok(iterator) => {
                for ev in iterator {
                    Self::process_event(&mut self.pos, ev);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No events
            }
            Err(e) => {
                return Err(e);
            }
        }

        Ok(self.pos.clone())
    }

    fn process_event(pos: &mut StickPosition, event: InputEvent) {
        match event.destructure() {
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_X, value) => {
                // Use X axis as-is
                pos.x = clamp_with_deadzone(value);
            }
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_Y, value) => {
                // Invert Y axis
                pos.y = clamp_with_deadzone(value).saturating_neg();
            }
            _ => {}
        }
    }
}

fn clamp_with_deadzone(value: i32) -> i16 {
    if value > -DEAD_ZONE && value < DEAD_ZONE {
        return 0;
    }
    if value > i16::MAX as i32 {
        return i16::MAX;
    }
    if value < i16::MIN as i32 {
        return i16::MIN;
    }
    return value as i16;
}
