 

use std::fmt;
use std::sync::mpsc::{Receiver, Sender};

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
        std::thread::sleep(std::time::Duration::from_millis(500));

        let mut welcome = None;
        for _ in 0..20 {
            if let Some(line) = conn.read_line()? {
                if line.starts_with("Grbl") {
                    welcome = Some(line);
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        if welcome.is_none() {
            return Err(GrblError::NoResponse(
                "did not receive Grbl banner".to_string(),
            ));
        }

        Ok(Self { conn })
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
                None => std::thread::sleep(std::time::Duration::from_millis(20)),
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
            std::thread::sleep(std::time::Duration::from_millis(50));
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
    Ok,
    Error(String),
    Alarm(String),
    JobStarted { total_lines: usize },
    JobProgress { sent: usize, total: usize },
    JobFinished { success: bool },
}