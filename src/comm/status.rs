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
        match s.trim() {
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
            let part = part.trim();
            if let Some(rest) = part.strip_prefix("MPos:") {
                status.mpos = parse_xyz(rest);
            } else if let Some(rest) = part.strip_prefix("WPos:") {
                status.wpos = parse_xyz(rest);
            } else if let Some(rest) = part.strip_prefix("FS:") {
                let mut it = rest.split(',');
                status.feed = it.next().and_then(|v| v.trim().parse().ok());
                status.spindle = it.next().and_then(|v| v.trim().parse().ok());
            }
        }

        Some(status)
    }
}

fn parse_xyz(s: &str) -> Option<(f64, f64, f64)> {
    let mut it = s.split(',');
    let x = it.next()?.trim().parse().ok()?;
    let y = it.next()?.trim().parse().ok()?;
    let z = it.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
    Some((x, y, z))
}
