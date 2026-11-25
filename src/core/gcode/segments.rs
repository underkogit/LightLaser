use std::path::Path;

pub fn parse_gcode_segments(content: &str) -> Vec<(f64, f64, f64, f64)> {
    let mut raw = Vec::new();
    let mut cur = (0.0f64, 0.0f64);
    let mut mode_rapid = false;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        let mut nx = None;
        let mut ny = None;
        let mut seen_g = None;

        for tok in line.split_whitespace() {
            if let Some(rest) = tok.strip_prefix('G') {
                if let Ok(g) = rest.parse::<u32>() {
                    seen_g = Some(g);
                }
            } else if let Some(rest) = tok.strip_prefix('X') {
                nx = rest.parse::<f64>().ok();
            } else if let Some(rest) = tok.strip_prefix('Y') {
                ny = rest.parse::<f64>().ok();
            }
        }

        if let Some(g) = seen_g {
            if g == 0 {
                mode_rapid = true;
            } else if g == 1 || g == 2 || g == 3 {
                mode_rapid = false;
            }
        }

        let px = nx.unwrap_or(cur.0);
        let py = ny.unwrap_or(cur.1);

        if !mode_rapid && (px != cur.0 || py != cur.1) {
            raw.push((cur.0, cur.1, px, py));
        }

        cur = (px, py);
    }

    raw
}

pub fn parse_gcode_file_segments(path: &Path) -> Vec<(f64, f64, f64, f64)> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_gcode_segments(&content)
}

