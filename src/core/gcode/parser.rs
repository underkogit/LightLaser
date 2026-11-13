use std::fmt;
use std::path::Path;

use crate::core::gcode::command::{GcodeCommand, MotionKind};

#[derive(Debug)]
pub enum ParseGcodeError {
    Io(std::io::Error),
}

impl fmt::Display for ParseGcodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseGcodeError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for ParseGcodeError {}

impl From<std::io::Error> for ParseGcodeError {
    fn from(e: std::io::Error) -> Self {
        ParseGcodeError::Io(e)
    }
}

pub fn parse_file(path: &Path) -> Result<Vec<GcodeCommand>, ParseGcodeError> {
    let content = std::fs::read_to_string(path)?;
    Ok(content
        .lines()
        .map(|l| parse_line(l))
        .filter(|c| !c.is_comment_only())
        .collect())
}

pub fn parse_line(line: &str) -> GcodeCommand {
    let raw = line.trim().to_string();

    let (code_owned, comment) = match raw.split_once(';') {
        Some((c, com)) => (c.trim().to_string(), Some(com.trim().to_string())),
        None => (raw.clone(), None),
    };

    let mut cmd = GcodeCommand {
        raw,
        line_number: None,
        motion: None,
        x: None,
        y: None,
        z: None,
        i: None,
        j: None,
        feed: None,
        spindle: None,
        comment,
    };

    for token in code_owned.split_whitespace() {
        let mut chars = token.chars();
        let Some(letter) = chars.next() else { continue };
        let value = chars.as_str();
        if value.is_empty() {
            continue;
        }

        match letter.to_ascii_uppercase() {
            'N' => cmd.line_number = value.parse().ok(),
            'G' => {
                if let Ok(g) = value.parse::<u32>() {
                    cmd.motion = match g {
                        0 => Some(MotionKind::Rapid),
                        1 => Some(MotionKind::Linear),
                        2 => Some(MotionKind::ArcCw),
                        3 => Some(MotionKind::ArcCcw),
                        28 => Some(MotionKind::Home),
                        _ => cmd.motion,
                    };
                }
            }
            'X' => cmd.x = value.parse().ok(),
            'Y' => cmd.y = value.parse().ok(),
            'Z' => cmd.z = value.parse().ok(),
            'I' => cmd.i = value.parse().ok(),
            'J' => cmd.j = value.parse().ok(),
            'F' => cmd.feed = value.parse().ok(),
            'S' => cmd.spindle = value.parse().ok(),
            _ => {}
        }
    }

    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_move() {
        let c = parse_line("G1 X10.5 Y-3.2 F500 S300");
        assert_eq!(c.motion, Some(MotionKind::Linear));
        assert_eq!(c.x, Some(10.5));
        assert_eq!(c.y, Some(-3.2));
        assert_eq!(c.feed, Some(500.0));
        assert_eq!(c.spindle, Some(300.0));
    }

    #[test]
    fn parse_with_comment() {
        let c = parse_line("G0 X0 Y0 ; go home");
        assert_eq!(c.motion, Some(MotionKind::Rapid));
        assert_eq!(c.comment.as_deref(), Some("go home"));
    }

    #[test]
    fn parse_comment_only() {
        let c = parse_line("; header line");
        assert!(c.is_comment_only());
    }
}
