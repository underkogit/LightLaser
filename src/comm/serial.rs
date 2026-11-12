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
            .timeout(Duration::from_millis(50))
            .open()?;

        let _ = port.write_data_terminal_ready(false);
        let _ = port.write_request_to_send(false);
        std::thread::sleep(Duration::from_millis(500));

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
        let mut buf = String::new();
        match self.reader.read_line(&mut buf) {
            Ok(0) => Ok(None),
            Ok(_) => Ok(Some(buf.trim_end_matches(['\r', '\n']).to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn close(&mut self) {
        let _ = self.port.write_data_terminal_ready(false);
        let _ = self.port.clear(serialport::ClearBuffer::All);
    }
}
