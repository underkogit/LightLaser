 
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

use serialport::SerialPort;

pub struct SerialConnection {
    port: Box<dyn SerialPort>,
    reader: BufReader<Box<dyn SerialPort>>,
}

impl SerialConnection {
    pub fn open(port_name: &str, baudrate: u32) -> Result<Self, serialport::Error> {
        let mut port = serialport::new(port_name, baudrate)
            .timeout(Duration::from_millis(100))
            .open()?;

        let _ = port.write_data_terminal_ready(true);
        std::thread::sleep(Duration::from_millis(100));
        let _ = port.write_data_terminal_ready(false);
        std::thread::sleep(Duration::from_millis(50));
        let _ = port.write_data_terminal_ready(true);
        let _ = port.write_request_to_send(true);
        std::thread::sleep(Duration::from_millis(2000));

        let reader = BufReader::new(port.try_clone()?);
        Ok(Self { port, reader })
    }

    pub fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.port.write_all(line.as_bytes())?;
        self.port.write_all(b"\n")?;
        self.port.flush()?;
        Ok(())
    }

    pub fn write_raw(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.port.write_all(bytes)?;
        self.port.flush()?;
        Ok(())
    }

    pub fn read_line(&mut self) -> std::io::Result<Option<String>> {
        let mut buf: Vec<u8> = Vec::with_capacity(128);
        loop {
            match self.reader.read_until(b'\n', &mut buf) {
                Ok(0) => {
                    if buf.is_empty() {
                        return Ok(None);
                    }
                    break;
                }
                Ok(_) => break,
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    if buf.is_empty() {
                        return Ok(None);
                    }
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    if buf.is_empty() {
                        return Err(e);
                    }
                    break;
                }
            }
        }

        while buf.last() == Some(&b'\n') || buf.last() == Some(&b'\r') {
            buf.pop();
        }

        if buf.is_empty() {
            return Ok(Some(String::new()));
        }

        Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
    }

    pub fn close(&mut self) {
        let _ = self.port.write_data_terminal_ready(false);
        let _ = self.port.clear(serialport::ClearBuffer::All);
    }
}
