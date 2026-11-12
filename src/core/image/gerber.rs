use std::path::Path;

pub struct GerberPath {
    pub points: Vec<(f64, f64)>,
    pub closed: bool,
}

pub fn parse_gerber(path: &Path) -> Result<Vec<GerberPath>, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("read: {}", e))?;
    let mut paths = Vec::new();
    let mut current_path = Vec::new();
    let mut cur_x = 0.0;
    let mut cur_y = 0.0;
    let mut in_region = false;
    let mut scale = 1.0;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("G04") {
            continue;
        }

        if line.starts_with("%FSLAX") {
            if let Some(rest) = line.strip_prefix("%FSLAX") {
                if let Some(comma_idx) = rest.find('Y') {
                    let x_part = &rest[..comma_idx];
                    let y_part = &rest[comma_idx + 1..];
                    if x_part.ends_with('*') {
                        continue;
                    }
                    let x_digits = x_part.chars().filter(|c| c.is_digit(10)).count();
                    let _ = x_digits;
                    let _ = y_part;
                }
            }
        }

        if line.starts_with("%MOMM") {
            scale = 1.0;
        } else if line.starts_with("%MOIN") {
            scale = 25.4;
        }

        if line == "G36*" {
            in_region = true;
            current_path.clear();
            continue;
        }
        if line == "G37*" {
            in_region = false;
            if current_path.len() > 1 {
                paths.push(GerberPath {
                    points: current_path.clone(),
                    closed: true,
                });
            }
            current_path.clear();
            continue;
        }

        let mut cmd = line;
        if let Some(pos) = cmd.find('*') {
            cmd = &cmd[..pos];
        }

        let mut is_d01 = false;
        let mut is_d02 = false;
        let mut is_d03 = false;
        let mut x = None;
        let mut y = None;

        let chars: Vec<char> = cmd.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                'X' => {
                    let start = i + 1;
                    let mut end = start;
                    while end < chars.len()
                        && (chars[end].is_digit(10) || chars[end] == '.' || chars[end] == '-')
                    {
                        end += 1;
                    }
                    if start < end {
                        x = chars[start..end]
                            .iter()
                            .collect::<String>()
                            .parse::<f64>()
                            .ok();
                    }
                    i = end;
                }
                'Y' => {
                    let start = i + 1;
                    let mut end = start;
                    while end < chars.len()
                        && (chars[end].is_digit(10) || chars[end] == '.' || chars[end] == '-')
                    {
                        end += 1;
                    }
                    if start < end {
                        y = chars[start..end]
                            .iter()
                            .collect::<String>()
                            .parse::<f64>()
                            .ok();
                    }
                    i = end;
                }
                'D' => {
                    let start = i + 1;
                    let mut end = start;
                    while end < chars.len() && chars[end].is_digit(10) {
                        end += 1;
                    }
                    if start < end {
                        let d_val: u32 = chars[start..end]
                            .iter()
                            .collect::<String>()
                            .parse()
                            .unwrap_or(0);
                        if d_val == 1 {
                            is_d01 = true;
                        }
                        if d_val == 2 {
                            is_d02 = true;
                        }
                        if d_val == 3 {
                            is_d03 = true;
                        }
                    }
                    i = end;
                }
                _ => {
                    i += 1;
                }
            }
        }

        let tx = x.unwrap_or(cur_x);
        let ty = y.unwrap_or(cur_y);

        if in_region {
            if is_d02 {
                if current_path.len() > 1 {
                    paths.push(GerberPath {
                        points: current_path.clone(),
                        closed: true,
                    });
                }
                current_path.clear();
                current_path.push((tx * scale, ty * scale));
            } else if is_d01 {
                current_path.push((tx * scale, ty * scale));
            }
        } else {
            if is_d01 {
                if current_path.is_empty() {
                    current_path.push((cur_x * scale, cur_y * scale));
                }
                current_path.push((tx * scale, ty * scale));
            } else if is_d02 {
                if current_path.len() > 1 {
                    paths.push(GerberPath {
                        points: current_path.clone(),
                        closed: false,
                    });
                }
                current_path.clear();
            } else if is_d03 {
                let cx = tx * scale;
                let cy = ty * scale;
                let r = 0.5;
                let mut circle_pts = Vec::new();
                for k in 0..16 {
                    let a = k as f64 / 16.0 * std::f64::consts::TAU;
                    circle_pts.push((cx + a.cos() * r, cy + a.sin() * r));
                }
                paths.push(GerberPath {
                    points: circle_pts,
                    closed: true,
                });
            }
        }

        if is_d01 || is_d02 || is_d03 {
            cur_x = tx;
            cur_y = ty;
        }
    }

    if current_path.len() > 1 {
        paths.push(GerberPath {
            points: current_path,
            closed: false,
        });
    }

    if paths.is_empty() {
        return Err("no paths found in gerber".to_string());
    }

    Ok(paths)
}
