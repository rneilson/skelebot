use std::collections::VecDeque;
use std::io::stdout;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use crossterm::ExecutableCommand;
use ratatui::crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::prelude::*;
use ratatui::widgets::{
    Axis, Bar, BarChart, BarGroup, Block, Chart, Dataset, GraphType, Paragraph, Wrap,
};

use crate::actions::{record_ticks_for_period, Action, ControlState, ThreadMsg};

const MESSAGE_LINES: u16 = 5;

pub enum UIUpdate {
    Control(ControlState),
    Message(ThreadMsg),
    Error(ThreadMsg),
}

pub fn draw_ui(rx: Receiver<UIUpdate>, tx: Sender<Action>, exit_flag: &AtomicBool) {
    let mut prev_marker = Instant::now();
    let mut next_marker = prev_marker + Duration::from_secs(10);
    let mut ticks = 0_u32;

    let mut control_state = ControlState::from(0);
    let mut messages: VecDeque<String> = vec![].into();

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(e) => {
            let msg = ThreadMsg {
                name: "UI".to_owned(),
                message: format!("couldn't initialize terminal: {:?}", e),
            };
            tx.send(Action::Fatal(msg)).unwrap();
            return;
        }
    };

    if let Err(e) = stdout().execute(EnterAlternateScreen) {
        let msg = ThreadMsg {
            name: "UI".to_owned(),
            message: format!("couldn't initialize terminal: {:?}", e),
        };
        tx.send(Action::Fatal(msg)).unwrap();
        return;
    }
    if let Err(e) = terminal.draw(|frame| render_ui(frame, &control_state, &messages)) {
        let msg = ThreadMsg {
            name: "UI".to_owned(),
            message: format!("couldn't draw frame: {:?}", e),
        };
        tx.send(Action::Fatal(msg)).unwrap();
        return;
    }

    let max_wait = Duration::from_millis(20);
    'listener: loop {
        match rx.recv_timeout(max_wait) {
            Ok(update) => {
                match update {
                    UIUpdate::Control(new_state) => {
                        control_state = new_state;
                    }
                    UIUpdate::Message(msg) => {
                        messages.push_back(format!("{0}: {1}\r\n", msg.name, msg.message));
                        if messages.len() > MESSAGE_LINES.into() {
                            _ = messages.pop_front();
                        }
                    }
                    UIUpdate::Error(err_msg) => {
                        messages.push_back(format!(
                            "Error from {0}: {1}\r\n",
                            err_msg.name, err_msg.message
                        ));
                        if messages.len() > MESSAGE_LINES.into() {
                            _ = messages.pop_front();
                        }
                    }
                }
                match terminal.draw(|frame| render_ui(frame, &control_state, &messages)) {
                    Ok(_) => {}
                    Err(e) => {
                        let msg = ThreadMsg {
                            name: "UI".to_owned(),
                            message: format!("couldn't draw frame: {:?}", e),
                        };
                        tx.send(Action::Fatal(msg)).unwrap();
                        return;
                    }
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
                next_marker += Duration::from_secs(10);
            }
        }

        if exit_flag.load(Ordering::Relaxed) {
            break 'listener;
        }
    }

    stdout().execute(LeaveAlternateScreen).unwrap();
}

fn render_ui(frame: &mut Frame, control_state: &ControlState, messages: &VecDeque<String>) {
    let outer_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Min(11),
            Constraint::Length(MESSAGE_LINES + 2),
        ])
        .split(frame.area());
    let upper_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Min(11), Constraint::Length(5)])
        .split(outer_layout[0]);
    let lower_layout = outer_layout[1];
    let upper_left = upper_layout[0];
    let upper_right = upper_layout[1];

    // Joystick position
    let positions = [(control_state.steering.into(), control_state.throttle.into())];
    let labels = [Line::from("-32767"), Line::from("0"), Line::from("32767")];
    let ul_data = vec![Dataset::default()
        .marker(symbols::Marker::Dot)
        .graph_type(GraphType::Scatter)
        .style(Style::default().cyan())
        .data(&positions)];
    let ul_x_axis = Axis::default()
        .style(Style::default().white())
        .bounds([(i16::MIN + 1).into(), i16::MAX.into()])
        .labels(labels.clone());
    let ul_y_axis = Axis::default()
        .style(Style::default().white())
        .bounds([(i16::MIN + 1).into(), i16::MAX.into()])
        .labels(labels.clone());
    let ul_chart = Chart::new(ul_data)
        .block(Block::bordered())
        .x_axis(ul_x_axis)
        .y_axis(ul_y_axis);

    // Tank drive
    // TODO: extract from joystick position
    let ur_data = &[Bar::default().value(100), Bar::default().value(100)];
    // TODO: style according to values
    let ur_chart = BarChart::default()
        .block(Block::bordered())
        .bar_width(1)
        .bar_gap(1)
        .bar_style(Style::default().green())
        .data(BarGroup::default().bars(ur_data))
        .max(200);

    // Message list
    let msg_data: Vec<Line<'_>> = messages.iter().map(|s| Line::from(s.to_owned())).collect();
    let msg_para = Paragraph::new(msg_data)
        .block(Block::bordered())
        .style(Style::new().white().on_black())
        .left_aligned()
        .wrap(Wrap { trim: true });

    frame.render_widget(ul_chart, upper_left);
    frame.render_widget(ur_chart, upper_right);
    frame.render_widget(msg_para, lower_layout);
}
