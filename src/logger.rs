use log::{Level, Log, Metadata, Record};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: Level,
    pub message: String,
}

pub struct MultiLogger {
    pub console: env_logger::Logger,
    pub buffer: Arc<Mutex<VecDeque<LogEntry>>>,
}

impl MultiLogger {
    pub fn new(buffer: Arc<Mutex<VecDeque<LogEntry>>>) -> Self {
        let console = env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("info"),
        )
            .build();
        Self { console, buffer }
    }
}

impl Log for MultiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.console.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        self.console.log(record);

        if let Ok(mut buf) = self.buffer.lock() {
            buf.push_back(LogEntry {
                level: record.level(),
                message: format!("[{}] {}", record.target(), record.args()),
            });
            if buf.len() > 2000 {
                buf.pop_front();
            }
        }
    }

    fn flush(&self) {
        self.console.flush();
    }
}