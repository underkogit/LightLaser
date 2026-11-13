use crate::core::gcode::command::GcodeCommand;

pub struct GcodeStream {
    lines: Vec<String>,
    index: usize,
}

impl GcodeStream {
    pub fn from_commands(commands: &[GcodeCommand]) -> Self {
        let lines = commands
            .iter()
            .filter(|c| !c.is_comment_only())
            .map(|c| c.raw.clone())
            .collect();
        Self { lines, index: 0 }
    }

    pub fn from_lines(lines: Vec<String>) -> Self {
        let lines = lines
            .into_iter()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with(';'))
            .collect();
        Self { lines, index: 0 }
    }

    pub fn total(&self) -> usize {
        self.lines.len()
    }

    pub fn sent(&self) -> usize {
        self.index
    }

    pub fn is_finished(&self) -> bool {
        self.index >= self.lines.len()
    }

    pub fn next_line(&mut self) -> Option<String> {
        if self.is_finished() {
            return None;
        }
        let line = self.lines[self.index].clone();
        self.index += 1;
        Some(line)
    }

    pub fn reset(&mut self) {
        self.index = 0;
    }
}
