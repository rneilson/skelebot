use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

use crazyradio::{self, Channel, Crazyradio, Datarate};

use crate::actions::{
    record_ticks_for_period, send_error_message, send_message, Action, BatteryVoltage,
    ControlState, RECORD_TICKS_INTERVAL,
};

enum SendStateType {
    DRIVE,
    CAMERA,
}

const RADIO_LOOP_INTERVAL: Duration = Duration::from_millis(10);

pub fn radio_comms(
    tx: Sender<Action>,
    control_state_mutex: Arc<Mutex<ControlState>>,
    exit_flag: &AtomicBool,
) {
    let mut prev_marker = Instant::now();
    let mut next_marker = prev_marker + RECORD_TICKS_INTERVAL;
    let mut ticks = 0_u32;

    let channel: u8 = 76; // Later make this mutable
    let mut radio: Option<Crazyradio> = None;
    let mut state_type: SendStateType = SendStateType::DRIVE;

    'outer: loop {
        // Attempt finding crazyradio device
        if radio.is_none() {
            match init_crazyradio(channel) {
                Ok(cr) => {
                    if let Ok(serial) = cr.serial() {
                        let msg = format!("initialized radio, serial {}", serial);
                        send_message(&tx, "Radio", &msg);
                    }
                    radio = Some(cr);
                }
                Err(e) => {
                    let msg = format!("couldn't open radio device: {}", e);
                    send_error_message(&tx, "Radio", &msg);
                }
            }
        }
        if let Some(ref mut cr) = radio {
            let control_state = {
                let control_state = control_state_mutex.lock().unwrap();
                control_state.clone()
            };
            match send_state_update(cr, control_state, &state_type) {
                Ok(ack_data) => {
                    receive_ack_data(&tx, ack_data);
                }
                Err(e) => {
                    let msg = format!("couldn't transmit update: {}", e);
                    send_error_message(&tx, "Radio", &msg);
                }
            }
            // Alternate state updates
            match state_type {
                SendStateType::DRIVE => {
                    state_type = SendStateType::CAMERA;
                }
                SendStateType::CAMERA => {
                    state_type = SendStateType::DRIVE;
                }
            }
        }

        // TODO: switch to timerfd
        sleep(RADIO_LOOP_INTERVAL);
        ticks += 1;

        let curr_time = Instant::now();
        if curr_time >= next_marker {
            // Send message with loop count for period
            record_ticks_for_period(&tx, "Radio", ticks, prev_marker, curr_time);

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

fn init_crazyradio(channel: u8) -> Result<Crazyradio, crazyradio::Error> {
    let channel = Channel::from_number(channel)?;
    let mut cr = Crazyradio::open_first()?;
    cr.set_datarate(Datarate::Dr250K)?;
    cr.set_channel(channel)?;
    Ok(cr)
}

// Maps signed +/- 100 to 0-200 for transmission
fn map_percent_value(value: i8) -> u8 {
    if value <= -100_i8 {
        return 0_u8;
    }
    if value >= 100_i8 {
        return 200_u8;
    }
    if value < 0_i8 {
        return (value + 100_i8) as u8;
    }
    return (value as u8) + 100_u8;
}

// Maps signed +/- 90 to 0-180 for transmission
fn map_angular_value(value: i8) -> u8 {
    if value <= -90_i8 {
        return 0_u8;
    }
    if value >= 90_i8 {
        return 180_u8;
    }
    if value < 0_i8 {
        return (value + 90_i8) as u8;
    }
    return (value as u8) + 90_u8;
}

fn send_state_update(
    cr: &mut Crazyradio,
    control_state: ControlState,
    state_type: &SendStateType,
) -> Result<[u8; 4], crazyradio::Error> {
    let mut command: [u8; 3] = [0; 3];
    let mut command_len: usize = 1;

    match state_type {
        SendStateType::DRIVE => {
            let (left_val, right_val) = control_state.as_tank_drive();
            if left_val == 0 && right_val == 0 {
                command[0] = 0xF3; // Stop
            } else {
                command[0] = 0xF4; // Drive
                command[1] = map_percent_value(left_val);
                command[2] = map_percent_value(right_val);
                command_len = 3;
            }
        }
        SendStateType::CAMERA => {
            let (pan_val, tilt_val) = control_state.as_camera_angles();
            if pan_val == 0 && tilt_val == 0 {
                command[0] = 0xF5; // Center camera
            } else {
                command[0] = 0xF6; // Look
                command[1] = map_angular_value(pan_val);
                command[2] = map_angular_value(tilt_val);
                command_len = 3;
            }
        }
    }

    let cmd_slice = &command[..command_len];
    let mut ack_data: [u8; 4] = [0; 4];
    let _ack = cr.send_packet(cmd_slice, &mut ack_data)?;
    // TODO: check ack properties

    Ok(ack_data)
}

fn receive_ack_data(tx: &Sender<Action>, ack_data: [u8; 4]) {
    match ack_data[0] {
        // No-op, 0 bytes
        0xF8 => {}
        // 0xF9, 0xFA reserved
        // Battery voltage, 2 bytes
        0xFB => {
            let voltage = BatteryVoltage(u16::from_be_bytes([ack_data[1], ack_data[2]]));
            if let Err(_) = tx.send(Action::BatteryUpdate(voltage)) {
                // Can happen during shutdown
            }
        }
        // Left RPM, 2 bytes
        0xFC => {
            // TODO
        }
        // Right RPM, 2 bytes
        0xFD => {
            // TODO
        }
        // 0xFE, 0xFF reserved
        _ => {
            // Send error message?
        }
    }
}
