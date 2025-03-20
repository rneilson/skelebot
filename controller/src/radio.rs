use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crazyradio::{self, Channel, Crazyradio, Datarate};

use crate::actions::{
    record_ticks_for_period, send_error_message, send_message, Action, ControlState,
    RECORD_TICKS_INTERVAL,
};

pub fn radio_comms(tx: Sender<Action>, control_state_atomic: &AtomicU32, exit_flag: &AtomicBool) {
    let mut prev_marker = Instant::now();
    let mut next_marker = prev_marker + RECORD_TICKS_INTERVAL;
    let mut ticks = 0_u32;

    let channel: u8 = 76; // Later make this mutable
    let mut radio: Option<Crazyradio> = None;

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
            match send_control_state(cr, control_state_atomic) {
                Ok(_ack_data) => {
                    // TODO: process ack data
                }
                Err(e) => {
                    let msg = format!("couldn't transmit update: {}", e);
                    send_error_message(&tx, "Radio", &msg);
                }
            }
        }

        // TODO: switch to timerfd
        sleep(Duration::from_millis(20));
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

fn send_control_state(
    cr: &mut Crazyradio,
    state: &AtomicU32,
) -> Result<[u8; 4], crazyradio::Error> {
    let control_state = ControlState::from(state.load(Ordering::Relaxed));
    let (left_val, right_val) = control_state.as_tank_drive();
    let mut command: [u8; 3] = [0; 3];

    if left_val == 0 && right_val == 0 {
        command[0] = 0xF3; // Stop
    } else if left_val >= 0 {
        if right_val >= 0 {
            command[0] = 0xF4; // Forward
        } else {
            command[0] = 0xF5; // Turn right
        }
    } else {
        if right_val >= 0 {
            command[0] = 0xF6; // Turn left
        } else {
            command[0] = 0xF7; // Backward
        }
    }
    command[1] = left_val.unsigned_abs();
    command[2] = right_val.unsigned_abs();

    let mut ack_data: [u8; 4] = [0; 4];
    let _ack = cr.send_packet(&command, &mut ack_data)?;
    // TODO: check ack properties

    Ok(ack_data)
}
