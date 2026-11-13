use std::fmt;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use crate::comm::serial::SerialConnection;
use crate::comm::status::GrblStatus;

#[derive(Debug)]
pub enum GrblError {
    Serial(serialport::Error),
    Io(std::io::Error),
    NoResponse(String),
    Alarm(String),
}

impl fmt::Display for GrblError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GrblError::Serial(e) => write!(f, "Serial error: {}", e),
            GrblError::Io(e) => write!(f, "IO error: {}", e),
            GrblError::NoResponse(ctx) => write!(f, "No response from GRBL: {}", ctx),
            GrblError::Alarm(msg) => write!(f, "Alarm: {}", msg),
        }
    }
}

impl std::error::Error for GrblError {}

impl From<serialport::Error> for GrblError {
    fn from(e: serialport::Error) -> Self {
        GrblError::Serial(e)
    }
}

impl From<std::io::Error> for GrblError {
    fn from(e: std::io::Error) -> Self {
        GrblError::Io(e)
    }
}

pub struct GrblConnection {
    conn: SerialConnection,
}

impl GrblConnection {
    pub fn open(port: &str, baudrate: u32) -> Result<Self, GrblError> {
        let mut conn = SerialConnection::open(port, baudrate)?;

        let _ = conn.write_raw(b"\r\n\r\n");
        std::thread::sleep(Duration::from_millis(300));

        let mut saw_banner: Option<String> = None;
        let mut saw_reply = false;
        let mut asked = false;
        let start = Instant::now();
        let hard_deadline = start + Duration::from_secs(5);

        while Instant::now() < hard_deadline {
            if !asked && start.elapsed() > Duration::from_millis(700) {
                let _ = conn.write_raw(b"?");
                asked = true;
            }

            match conn.read_line()? {
                Some(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if trimmed.starts_with("Grbl") || trimmed.contains("Grbl") {
                        saw_banner = Some(trimmed.to_string());
                        saw_reply = true;
                        break;
                    }
                    if trimmed.starts_with('<') && trimmed.ends_with('>') {
                        saw_reply = true;
                        break;
                    }
                    if trimmed == "ok"
                        || trimmed.starts_with("error:")
                        || trimmed.starts_with("ALARM:")
                    {
                        saw_reply = true;
                        break;
                    }
                }
                None => {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }

        if !saw_reply {
            return Err(GrblError::NoResponse(
                "no reply from controller (banner/status/ok)".to_string(),
            ));
        }

        if let Some(b) = saw_banner {
            log::info!("GRBL banner: {}", b);
        } else {
            log::info!("GRBL banner not received, connection confirmed by status reply");
        }

        Ok(Self { conn })
    }

    pub fn fetch_work_area(&mut self) -> Result<Option<(f64, f64)>, GrblError> {
        while let Ok(Some(_)) = self.conn.read_line() {}

        self.conn.write_line("$$")?;

        let mut max_x: Option<f64> = None;
        let mut max_y: Option<f64> = None;
        let timeout = Instant::now() + Duration::from_secs(3);

        while Instant::now() < timeout {
            match self.conn.read_line()? {
                Some(line) => {
                    let line = line.trim();
                    if line == "ok" {
                        break;
                    }
                    if let Some((key, val)) = line.split_once('=') {
                        let key = key.trim();
                        let val = val.trim();
                        match key {
                            "$130" => max_x = val.parse::<f64>().ok(),
                            "$131" => max_y = val.parse::<f64>().ok(),
                            _ => {}
                        }
                    }
                }
                None => {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }

        if let (Some(x), Some(y)) = (max_x, max_y) {
            if x > 0.0 && y > 0.0 {
                return Ok(Some((x, y)));
            }
        }

        Ok(None)
    }

    pub fn send_and_wait_ok(&mut self, line: &str) -> Result<(), GrblError> {
        self.conn.write_line(line)?;

        for _ in 0..50 {
            match self.conn.read_line()? {
                Some(resp) if resp == "ok" => return Ok(()),
                Some(resp) if resp.starts_with("error:") => {
                    return Err(GrblError::NoResponse(resp));
                }
                Some(resp) if resp.starts_with("ALARM:") => {
                    return Err(GrblError::Alarm(resp));
                }
                Some(_) => continue,
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        }

        Err(GrblError::NoResponse(format!(
            "timeout waiting ok for '{}'",
            line
        )))
    }

    pub fn soft_reset(&mut self) -> Result<(), GrblError> {
        self.conn.write_raw(&[0x18])?;
        Ok(())
    }

    pub fn feed_hold(&mut self) -> Result<(), GrblError> {
        self.conn.write_raw(&[b'!'])?;
        Ok(())
    }

    pub fn cycle_start(&mut self) -> Result<(), GrblError> {
        self.conn.write_raw(&[b'~'])?;
        Ok(())
    }

    pub fn unlock(&mut self) -> Result<(), GrblError> {
        self.send_and_wait_ok("$X")
    }

    pub fn home(&mut self) -> Result<(), GrblError> {
        self.send_and_wait_ok("$H")
    }

    pub fn jog(&mut self, dx: f64, dy: f64, feed: f64) -> Result<(), GrblError> {
        let cmd = format!("$J=G91 X{:.3} Y{:.3} F{:.0}", dx, dy, feed);
        self.send_and_wait_ok(&cmd)
    }

    pub fn query_status(&mut self) -> Result<GrblStatus, GrblError> {
        self.conn.write_raw(b"?")?;
        for _ in 0..10 {
            if let Some(line) = self.conn.read_line()? {
                if let Some(s) = GrblStatus::parse(&line) {
                    return Ok(s);
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(GrblError::NoResponse("no status".to_string()))
    }

    pub fn write_line_raw(&mut self, line: &str) -> Result<(), GrblError> {
        self.conn.write_line(line)?;
        Ok(())
    }

    pub fn read_line(&mut self) -> Result<Option<String>, GrblError> {
        Ok(self.conn.read_line()?)
    }

    pub fn close(&mut self) {
        self.conn.close();
    }
}

pub type CommSender = Sender<GrblEvent>;
pub type CommReceiver = Receiver<GrblEvent>;

#[derive(Debug, Clone)]
pub enum GrblEvent {
    Connected,
    Disconnected,
    Status(GrblStatus),
    WorkArea(f64, f64),
    Ok,
    Error(String),
    Alarm(String),
    JobStarted { total_lines: usize },
    JobProgress { sent: usize, total: usize },
    JobFinished { success: bool },
}
