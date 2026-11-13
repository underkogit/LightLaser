use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crate::comm::grbl::{GrblConnection, GrblEvent};
use crate::comm::status::GrblStatus;

pub enum StreamerCommand {
    Connect { port: String, baudrate: u32 },
    Disconnect,
    Start { lines: Vec<String> },
    Pause,
    Resume,
    Stop,
    Jog { dx: f64, dy: f64, feed: f64 },
    Home,
    Unlock,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamerState {
    Idle,
    Running,
    Paused,
    Stopping,
}

const RX_BUFFER_SIZE: usize = 120;
const OK_TIMEOUT: Duration = Duration::from_secs(3);

pub struct Streamer {
    cmd_tx: Sender<StreamerCommand>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Streamer {
    pub fn spawn(events_tx: Sender<GrblEvent>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<StreamerCommand>();

        let handle = thread::spawn(move || {
            run_loop(cmd_rx, events_tx);
        });

        Self {
            cmd_tx,
            handle: Some(handle),
        }
    }

    pub fn send(&self, cmd: StreamerCommand) {
        if self.cmd_tx.send(cmd).is_err() {
            log::error!("streamer thread is dead");
        }
    }

    pub fn shutdown(mut self) {
        let _ = self.cmd_tx.send(StreamerCommand::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn run_loop(cmd_rx: Receiver<StreamerCommand>, events_tx: Sender<GrblEvent>) {
    let mut grbl: Option<GrblConnection> = None;
    let mut state = StreamerState::Idle;

    let mut in_flight: VecDeque<String> = VecDeque::new();
    let mut current_bytes: usize = 0;

    let mut pending: VecDeque<String> = VecDeque::new();
    let mut sent_total: usize = 0;
    let mut total: usize = 0;

    let mut last_ok_at = Instant::now();
    let mut last_status_poll = Instant::now();

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                StreamerCommand::Connect { port, baudrate } => {
                    let _ = events_tx.send(GrblEvent::Error(format!("Connecting to {}...", port)));
                    match GrblConnection::open(&port, baudrate) {
                        Ok(mut conn) => {
                            if let Ok(Some((w, h))) = conn.fetch_work_area() {
                                println!("Work area from controller: {:.1} x {:.1} mm", w, h);
                                log::info!("Work area from controller: {:.1} x {:.1} mm", w, h);
                                let _ = events_tx.send(GrblEvent::WorkArea(w, h));
                            } else {
                                println!("Failed to retrieve work area from controller");
                                log::warn!("Failed to retrieve work area from controller");
                            }
                            grbl = Some(conn);
                            let _ = events_tx.send(GrblEvent::Connected);
                        }
                        Err(e) => {
                            let _ = events_tx.send(GrblEvent::Error(e.to_string()));
                            let _ = events_tx.send(GrblEvent::Disconnected);
                        }
                    }
                }
                StreamerCommand::Disconnect => {
                    if let Some(mut g) = grbl.take() {
                        g.close();
                    }
                    in_flight.clear();
                    pending.clear();
                    current_bytes = 0;
                    state = StreamerState::Idle;
                    let _ = events_tx.send(GrblEvent::Disconnected);
                }
                StreamerCommand::Start { lines } => {
                    if grbl.is_none() {
                        let _ = events_tx.send(GrblEvent::Error("not connected".into()));
                        continue;
                    }
                    pending = lines.into_iter().collect();
                    total = pending.len();
                    sent_total = 0;
                    in_flight.clear();
                    current_bytes = 0;
                    last_ok_at = Instant::now();
                    state = StreamerState::Running;
                    let _ = events_tx.send(GrblEvent::JobStarted { total_lines: total });
                }
                StreamerCommand::Pause => {
                    if state == StreamerState::Running {
                        if let Some(g) = grbl.as_mut() {
                            let _ = g.feed_hold();
                        }
                        state = StreamerState::Paused;
                    }
                }
                StreamerCommand::Resume => {
                    if state == StreamerState::Paused {
                        if let Some(g) = grbl.as_mut() {
                            let _ = g.cycle_start();
                        }
                        state = StreamerState::Running;
                        last_ok_at = Instant::now();
                    }
                }
                StreamerCommand::Stop => {
                    if matches!(state, StreamerState::Running | StreamerState::Paused) {
                        state = StreamerState::Stopping;
                    }
                }
                StreamerCommand::Jog { dx, dy, feed } => {
                    if let Some(g) = grbl.as_mut() {
                        let _ = g.jog(dx, dy, feed);
                    }
                }
                StreamerCommand::Home => {
                    if let Some(g) = grbl.as_mut() {
                        let _ = g.home();
                    }
                }
                StreamerCommand::Unlock => {
                    if let Some(g) = grbl.as_mut() {
                        let _ = g.unlock();
                    }
                }
                StreamerCommand::Shutdown => {
                    if let Some(mut g) = grbl.take() {
                        g.close();
                    }
                    return;
                }
            }
        }

        if state == StreamerState::Stopping {
            if let Some(g) = grbl.as_mut() {
                let _ = g.soft_reset();
            }
            pending.clear();
            in_flight.clear();
            current_bytes = 0;
            state = StreamerState::Idle;
            let _ = events_tx.send(GrblEvent::JobFinished { success: false });
        }

        if let Some(g) = grbl.as_mut() {
            for _ in 0..8 {
                match g.read_line() {
                    Ok(Some(line)) => {
                        if let Some(st) = GrblStatus::parse(&line) {
                            let _ = events_tx.send(GrblEvent::Status(st));
                        } else if line == "ok" {
                            if let Some(sent) = in_flight.pop_front() {
                                current_bytes = current_bytes.saturating_sub(sent.len() + 1);
                                sent_total += 1;
                            }
                            last_ok_at = Instant::now();
                            let _ = events_tx.send(GrblEvent::Ok);
                        } else if line.starts_with("error:") {
                            let _ = events_tx.send(GrblEvent::Error(line));
                            pending.clear();
                            in_flight.clear();
                            current_bytes = 0;
                            state = StreamerState::Idle;
                            let _ = events_tx.send(GrblEvent::JobFinished { success: false });
                        } else if line.starts_with("ALARM:") {
                            let _ = events_tx.send(GrblEvent::Alarm(line));
                            pending.clear();
                            in_flight.clear();
                            current_bytes = 0;
                            state = StreamerState::Idle;
                            let _ = events_tx.send(GrblEvent::JobFinished { success: false });
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let _ = events_tx.send(GrblEvent::Error(e.to_string()));
                        break;
                    }
                }
            }

            if state == StreamerState::Running {
                while let Some(line) = pending.front() {
                    let bytes = line.len() + 1;
                    if current_bytes + bytes > RX_BUFFER_SIZE && !in_flight.is_empty() {
                        break;
                    }
                    let line = pending.pop_front().unwrap();
                    if g.write_line_raw(&line).is_err() {
                        let _ = events_tx.send(GrblEvent::Error("write failed".into()));
                        break;
                    }
                    current_bytes += bytes;
                    in_flight.push_back(line);
                    let _ = events_tx.send(GrblEvent::JobProgress {
                        sent: sent_total + in_flight.len(),
                        total,
                    });
                }

                if pending.is_empty() && in_flight.is_empty() {
                    state = StreamerState::Idle;
                    let _ = events_tx.send(GrblEvent::JobFinished { success: true });
                }
            }

            if state == StreamerState::Running
                && !in_flight.is_empty()
                && last_ok_at.elapsed() > OK_TIMEOUT
            {
                let _ = events_tx.send(GrblEvent::Error("timeout waiting for ok".into()));
                let _ = g.soft_reset();
                pending.clear();
                in_flight.clear();
                current_bytes = 0;
                state = StreamerState::Idle;
                let _ = events_tx.send(GrblEvent::JobFinished { success: false });
            }

            if state == StreamerState::Idle
                && in_flight.is_empty()
                && last_status_poll.elapsed() > Duration::from_millis(250)
            {
                if let Ok(st) = g.query_status() {
                    let _ = events_tx.send(GrblEvent::Status(st));
                }
                last_status_poll = Instant::now();
            }
        }

        thread::sleep(Duration::from_millis(2));
    }
}
