use std::collections::VecDeque;
use std::io::stdout;
use std::panic::{set_hook, take_hook};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use crossterm::terminal::{disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use ratatui::widgets::{
    Axis, Bar, BarChart, BarGroup, Block, Chart, Dataset, GraphType, Paragraph, Wrap,
};

use crate::actions::{
    record_ticks_for_period, Action, BatteryVoltage, ControlSpeed, ControlState, ThreadMsg,
    RECORD_TICKS_INTERVAL,
};

const MESSAGE_LINES: u16 = 5;

pub enum UIUpdate {
    Control(ControlState),
    Battery(BatteryVoltage),
    Message(ThreadMsg),
    Error(ThreadMsg),
}

struct UIState {
    control_state: ControlState,
    battery_voltage: BatteryVoltage,
    messages: VecDeque<String>,
}

impl UIState {
    fn new() -> Self {
        Self {
            control_state: ControlState::new(),
            battery_voltage: BatteryVoltage(0),
            messages: vec![].into(),
        }
    }
}

pub fn draw_ui(rx: Receiver<UIUpdate>, tx: Sender<Action>, exit_flag: &AtomicBool) {
    let mut prev_marker = Instant::now();
    let mut next_marker = prev_marker + RECORD_TICKS_INTERVAL;
    let mut ticks = 0_u32;

    let mut ui_state = UIState::new();

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(e) => {
            send_io_error(tx, e, "couldn't initialize terminal");
            return;
        }
    };

    // Switch to alternate buffer and setup panic handler
    if let Err(e) = stdout().execute(EnterAlternateScreen) {
        send_io_error(tx, e, "couldn't initialize terminal");
        return;
    }
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        // intentionally ignore errors here since we're already in a panic
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
        original_hook(panic_info);
    }));

    // Draw initial frame
    if let Err(e) = terminal.draw(|frame| render_ui(frame, &ui_state)) {
        send_io_error(tx, e, "couldn't draw frame");
        return;
    }

    // Update as messages come in
    let max_wait = Duration::from_millis(20);
    'listener: loop {
        match rx.recv_timeout(max_wait) {
            Ok(update) => {
                match update {
                    UIUpdate::Control(new_state) => {
                        ui_state.control_state = new_state;
                    }
                    UIUpdate::Battery(new_voltage) => {
                        ui_state.battery_voltage = new_voltage;
                    }
                    UIUpdate::Message(msg) => {
                        ui_state
                            .messages
                            .push_back(format!("{0}: {1}\r\n", msg.name, msg.message));
                        if ui_state.messages.len() > MESSAGE_LINES.into() {
                            _ = ui_state.messages.pop_front();
                        }
                    }
                    UIUpdate::Error(err_msg) => {
                        ui_state.messages.push_back(format!(
                            "Error from {0}: {1}\r\n",
                            err_msg.name, err_msg.message
                        ));
                        if ui_state.messages.len() > MESSAGE_LINES.into() {
                            _ = ui_state.messages.pop_front();
                        }
                    }
                }
                if let Err(e) = terminal.draw(|frame| render_ui(frame, &ui_state)) {
                    send_io_error(tx, e, "couldn't draw frame");
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Exit flag checked below after match block and tick handling
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Disconnected implies all senders dropped
                break 'listener;
            }
        }
        // TODO: render UI here instead, ie every tick?

        ticks += 1;
        let curr_time = Instant::now();
        if curr_time >= next_marker {
            // Send message with loop count for period
            record_ticks_for_period(&tx, "UI", ticks, prev_marker, curr_time);

            // Set next marker, ensuring in the future
            ticks = 0;
            prev_marker = next_marker;
            while next_marker < curr_time {
                next_marker += RECORD_TICKS_INTERVAL;
            }
        }

        if exit_flag.load(Ordering::Relaxed) {
            break 'listener;
        }
    }

    let _ = stdout().execute(LeaveAlternateScreen);
}

fn render_ui(frame: &mut Frame, ui_state: &UIState) {
    // Extracted from joystick position
    let (left_val, right_val) = ui_state.control_state.as_tank_drive();
    let move_speed = ui_state.control_state.move_speed;
    let (pan_val, tilt_val) = ui_state.control_state.as_camera_angles();
    let voltage = ui_state.battery_voltage.as_float();

    let outer_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Min(23),
            Constraint::Length(MESSAGE_LINES + 2),
        ])
        .split(frame.area());
    let upper_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![
            Constraint::Length(14),
            Constraint::Min(23),
            Constraint::Length(14),
        ])
        .split(outer_layout[0]);
    let lower_layout = outer_layout[1];
    let upper_left = upper_layout[0];
    let upper_mid = upper_layout[1];
    let upper_right = upper_layout[2];

    // Summary values
    let sum_data = vec![
        Line::from("Throttle"),
        Line::from(ui_state.control_state.throttle.to_string()),
        Line::from(""),
        Line::from("Steering"),
        Line::from(ui_state.control_state.steering.to_string()),
        Line::from(""),
        Line::from("Mode"),
        Line::from(move_speed.to_string()).style(move_speed_style(move_speed)),
        Line::from(""),
        Line::from("Left"),
        Line::from(format!("{}%", left_val)).style(tank_drive_style(left_val)),
        Line::from(""),
        Line::from("Right"),
        Line::from(format!("{}%", right_val)).style(tank_drive_style(right_val)),
        Line::from(""),
        Line::from("Pan"),
        Line::from(format!("{}°", pan_val)).style(camera_angle_style(pan_val)),
        Line::from(""),
        Line::from("Tilt"),
        Line::from(format!("{}°", tilt_val)).style(camera_angle_style(tilt_val)),
        Line::from(""),
        Line::from("Battery"),
        Line::from(format!("{:.2}V", voltage)),
        // TODO: left RPM
        // TODO: right RPM
    ];
    let sum_para = Paragraph::new(sum_data)
        .block(Block::bordered())
        .style(Style::new().white().on_black())
        .left_aligned()
        .wrap(Wrap { trim: true });

    // Joystick position
    let drive_positions = [
        (0.0, 0.0),
        (
            ui_state.control_state.steering.into(),
            ui_state.control_state.throttle.into(),
        ),
    ];
    let camera_positions = [
        (0.0, 0.0),
        (
            ui_state.control_state.pan.into(),
            ui_state.control_state.tilt.into(),
        ),
    ];
    let labels = [Line::from("-32767"), Line::from("0"), Line::from("32767")];
    let um_data = vec![
        Dataset::default()
            .marker(symbols::Marker::Dot)
            .graph_type(GraphType::Line)
            .style(Style::default().cyan().bold())
            .data(&drive_positions),
        Dataset::default()
            .marker(symbols::Marker::Dot)
            .graph_type(GraphType::Line)
            .style(Style::default().yellow().bold())
            .data(&camera_positions),
    ];
    let um_x_axis = Axis::default()
        .style(Style::default().white())
        .bounds([(i16::MIN + 1).into(), i16::MAX.into()])
        .labels(labels.clone());
    let um_y_axis = Axis::default()
        .style(Style::default().white())
        .bounds([(i16::MIN + 1).into(), i16::MAX.into()])
        .labels(labels.clone());
    let um_chart = Chart::new(um_data)
        .block(Block::bordered())
        .x_axis(um_x_axis)
        .y_axis(um_y_axis);

    // Tank drive
    let ur_data = &[
        Bar::default()
            .value(((left_val as i16) + 100) as u64)
            .label("L".into())
            .style(tank_drive_style(left_val))
            .value_style(tank_drive_value_style(left_val)),
        Bar::default()
            .value(((right_val as i16) + 100) as u64)
            .label("R".into())
            .style(tank_drive_style(right_val))
            .value_style(tank_drive_value_style(right_val)),
    ];
    // TODO: style according to values
    let ur_chart = BarChart::default()
        .block(Block::bordered())
        .bar_width(5)
        .bar_gap(2)
        .value_style(Style::new().black().on_black())
        .data(BarGroup::default().bars(ur_data))
        .max(200);

    // Message list
    let msg_data: Vec<Line<'_>> = ui_state
        .messages
        .iter()
        .map(|s| Line::from(s.to_owned()))
        .collect();
    let msg_para = Paragraph::new(msg_data)
        .block(Block::bordered())
        .style(Style::new().white().on_black())
        .left_aligned()
        .wrap(Wrap { trim: true });

    frame.render_widget(sum_para, upper_left);
    frame.render_widget(um_chart, upper_mid);
    frame.render_widget(ur_chart, upper_right);
    frame.render_widget(msg_para, lower_layout);
}

fn tank_drive_style(val: i8) -> Style {
    if val > 0 {
        Style::default().green()
    } else if val == 0 {
        Style::default().white()
    } else {
        Style::default().cyan()
    }
}

fn tank_drive_value_style(val: i8) -> Style {
    if val > 0 {
        Style::default().green().on_green()
    } else if val == 0 {
        Style::default().white().on_white()
    } else {
        Style::default().cyan().on_cyan()
    }
}

fn move_speed_style(val: ControlSpeed) -> Style {
    match val {
        ControlSpeed::Fast => Style::default().green(),
        ControlSpeed::Slow => Style::default().light_yellow(),
    }
}

fn camera_angle_style(val: i8) -> Style {
    if val > 0 {
        Style::default().light_yellow()
    } else if val == 0 {
        Style::default().white()
    } else {
        Style::default().yellow()
    }
}

fn send_io_error(tx: Sender<Action>, err: std::io::Error, err_desc: &str) {
    let msg = ThreadMsg {
        name: "UI".to_owned(),
        message: format!("{}: {:?}", err_desc, err),
    };
    tx.send(Action::Fatal(msg)).unwrap();
}
