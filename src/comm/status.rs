#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrblState {
    Idle,
    Run,
    Hold,
    Jog,
    Alarm,
    Door,
    Check,
    Home,
    Sleep,
    Unknown,
}

impl GrblState {
    pub fn from_str(s: &str) -> Self {
        match s {
            "Idle" => GrblState::Idle,
            "Run" => GrblState::Run,
            "Hold" => GrblState::Hold,
            "Jog" => GrblState::Jog,
            "Alarm" => GrblState::Alarm,
            "Door" => GrblState::Door,
            "Check" => GrblState::Check,
            "Home" => GrblState::Home,
            "Sleep" => GrblState::Sleep,
            _ => GrblState::Unknown,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            GrblState::Idle => "Idle",
            GrblState::Run => "Run",
            GrblState::Hold => "Hold",
            GrblState::Jog => "Jog",
            GrblState::Alarm => "Alarm",
            GrblState::Door => "Door",
            GrblState::Check => "Check",
            GrblState::Home => "Home",
            GrblState::Sleep => "Sleep",
            GrblState::Unknown => "?",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GrblStatus {
    pub state: GrblState,
    pub mpos: Option<(f64, f64, f64)>,
    pub wpos: Option<(f64, f64, f64)>,
    pub feed: Option<f64>,
    pub spindle: Option<f64>,
}

impl GrblStatus {
    pub fn parse(line: &str) -> Option<Self> {
        let inner = line.trim();
        let inner = inner.strip_prefix('<')?.strip_suffix('>')?;

        let mut parts = inner.split('|');
        let state = GrblState::from_str(parts.next()?);

        let mut status = GrblStatus {
            state,
            mpos: None,
            wpos: None,
            feed: None,
            spindle: None,
        };

        for part in parts {
            if let Some(rest) = part.strip_prefix("MPos:") {
                status.mpos = parse_xyz(rest);
            } else if let Some(rest) = part.strip_prefix("WPos:") {
                status.wpos = parse_xyz(rest);
            } else if let Some(rest) = part.strip_prefix("FS:") {
                let mut it = rest.split(',');
                status.feed = it.next().and_then(|v| v.parse().ok());
                status.spindle = it.next().and_then(|v| v.parse().ok());
            }
        }

        Some(status)
    }
}

fn parse_xyz(s: &str) -> Option<(f64, f64, f64)> {
    let mut it = s.split(',');
    let x = it.next()?.parse().ok()?;
    let y = it.next()?.parse().ok()?;
    let z = it.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    Some((x, y, z))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_idle_status() {
        let s = GrblStatus::parse("<Idle|MPos:10.0,20.0,0.0|FS:0,0>").unwrap();
        assert_eq!(s.state, GrblState::Idle);
        assert_eq!(s.mpos, Some((10.0, 20.0, 0.0)));
        assert_eq!(s.feed, Some(0.0));
    }

    #[test]
    fn parse_run_with_wpos() {
        let s = GrblStatus::parse("<Run|WPos:1.5,-2.5,0.0|FS:500,300>").unwrap();
        assert_eq!(s.state, GrblState::Run);
        assert_eq!(s.wpos, Some((1.5, -2.5, 0.0)));
        assert_eq!(s.feed, Some(500.0));
        assert_eq!(s.spindle, Some(300.0));
    }

    #[test]
    fn reject_non_status() {
        assert!(GrblStatus::parse("ok").is_none());
        assert!(GrblStatus::parse("error:1").is_none());
    }
}