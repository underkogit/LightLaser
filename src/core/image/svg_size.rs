//! Physical size of an SVG file, taken from its root element.
//!
//! A board export states what it is: `<svg width="63.9826mm" height="59.9948mm"
//! viewBox="0.0000 0.0000 63.9826 59.9948">` is a 63.9826 by 59.9948 mm board, and it has to
//! land on the workbed at exactly that size. The renderer works in user units (CSS pixels at
//! 96 dpi), so its size alone loses the authored millimetres and rounds through `f32`; reading
//! the attributes directly keeps them.
//!
//! Numbers without a unit are user units, i.e. CSS pixels. Board exports mean millimetres by
//! them, which is what the [`SvgImportProfile::PcbMillimetres`] profile is for.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// How an SVG's numbers are read when they do not state a physical unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SvgImportProfile {
    /// Read the file as written: a physical unit means that unit, everything else means CSS
    /// pixels at 96 dpi.
    #[default]
    Auto,
    /// PCB exports (KiCad, Altium, ...): unitless numbers are millimetres.
    PcbMillimetres,
}

impl SvgImportProfile {
    /// Both profiles, in the order menus and settings show them.
    pub const ALL: [SvgImportProfile; 2] = [
        SvgImportProfile::Auto,
        SvgImportProfile::PcbMillimetres,
    ];

    /// Name shown in menus and in the settings window.
    pub fn label(&self) -> &'static str {
        match self {
            SvgImportProfile::Auto => "Auto",
            SvgImportProfile::PcbMillimetres => "SCG (PCB)",
        }
    }

    /// Whether numbers without a unit are read as millimetres.
    fn unitless_is_mm(&self) -> bool {
        matches!(self, SvgImportProfile::PcbMillimetres)
    }
}

/// Length unit of an SVG attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    /// No unit, or `px`: user units, which are CSS pixels.
    Px,
    Mm,
    Cm,
    In,
    Pt,
    Pc,
}

/// A number with a unit, as written in an SVG attribute.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Length {
    value: f64,
    unit: Unit,
}

impl Length {
    fn px(value: f64) -> Self {
        Self {
            value,
            unit: Unit::Px,
        }
    }

    /// Converts to millimetres; unitless numbers count as millimetres when asked.
    fn to_mm(self, unitless_is_mm: bool) -> f64 {
        match self.unit {
            Unit::Px => {
                if unitless_is_mm {
                    self.value
                } else {
                    self.value * 25.4 / 96.0
                }
            }
            Unit::Mm => self.value,
            Unit::Cm => self.value * 10.0,
            Unit::In => self.value * 25.4,
            Unit::Pt => self.value * 25.4 / 72.0,
            Unit::Pc => self.value * 25.4 / 6.0,
        }
    }

    /// A length that names a physical unit, so it means the same in every profile.
    fn is_physical(self) -> bool {
        self.unit != Unit::Px
    }
}

/// Size declared by the root element.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct SvgHeader {
    width: Option<Length>,
    height: Option<Length>,
    view_box: Option<[f64; 4]>,
}

/// Size of the SVG in millimetres, as the layer should be placed on the workbed.
///
/// Parsing the file through the renderer also validates it, so a broken SVG is rejected before
/// a layer is created.
pub fn svg_size_mm(path: &Path, profile: SvgImportProfile) -> Result<(f64, f64), String> {
    // The renderer's own size is the last resort, and it fails on files that cannot be drawn.
    let render_size = crate::core::image::svg_raster::svg_user_size_px(path)?;
    let data = std::fs::read(path).map_err(|e| format!("svg read: {}", e))?;
    // Lossy on purpose: the attributes being read are ASCII, and a stray byte elsewhere in the
    // file should not stop an otherwise drawable drawing from being imported.
    let text = String::from_utf8_lossy(&data);
    svg_size_mm_from_str(&text, render_size, profile)
}

/// Resolves the size from the file's text and the size the renderer resolved.
fn svg_size_mm_from_str(
    data: &str,
    render_size: (f64, f64),
    profile: SvgImportProfile,
) -> Result<(f64, f64), String> {
    let header = parse_svg_header(data);
    let unitless_is_mm = profile.unitless_is_mm();

    let width = header.width.filter(|l| l.value > 0.0);
    let height = header.height.filter(|l| l.value > 0.0);

    // A width and height that name a physical unit are the size the file was authored at, so
    // they win in every profile.
    if let (Some(w), Some(h)) = (width, height) {
        if w.is_physical() && h.is_physical() {
            return finish(w.to_mm(unitless_is_mm), h.to_mm(unitless_is_mm));
        }
    }

    // With the PCB profile the drawing's own coordinates are millimetres, and board exports
    // carry the real board there.
    if unitless_is_mm {
        if let Some([_, _, w, h]) = header.view_box {
            if w > 0.0 && h > 0.0 {
                return finish(w, h);
            }
        }
    }

    // A declared viewport, in CSS pixels or millimetres according to the profile.
    if let (Some(w), Some(h)) = (width, height) {
        return finish(w.to_mm(unitless_is_mm), h.to_mm(unitless_is_mm));
    }

    // A viewBox on its own: its numbers are user units, which are CSS pixels at 96 dpi, or
    // millimetres with the PCB profile.
    if let Some([_, _, w, h]) = header.view_box {
        if w > 0.0 && h > 0.0 {
            let scale = Length::px(1.0).to_mm(unitless_is_mm);
            return finish(w * scale, h * scale);
        }
    }

    // Nothing usable in the text: take what the renderer resolved.
    let (w, h) = render_size;
    finish(
        Length::px(w).to_mm(unitless_is_mm),
        Length::px(h).to_mm(unitless_is_mm),
    )
}

/// Rejects sizes that cannot be drawn.
fn finish(width: f64, height: f64) -> Result<(f64, f64), String> {
    if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
        Ok((width, height))
    } else {
        Err("svg has no usable size".to_string())
    }
}

/// Reads width, height and viewBox from the root `<svg>` element.
fn parse_svg_header(data: &str) -> SvgHeader {
    let Some(start) = data.find("<svg") else {
        return SvgHeader::default();
    };
    let rest = &data[start..];
    let end = start_tag_end(rest).unwrap_or(rest.len());
    let tag = &rest[..end];

    let mut header = SvgHeader::default();
    for (name, value) in parse_attributes(tag) {
        if name.eq_ignore_ascii_case("width") {
            header.width = parse_length(value);
        } else if name.eq_ignore_ascii_case("height") {
            header.height = parse_length(value);
        } else if name.eq_ignore_ascii_case("viewbox") {
            header.view_box = parse_view_box(value);
        }
    }
    header
}

/// Byte index of the `>` that closes the start tag, ignoring `>` inside quoted values.
fn start_tag_end(text: &str) -> Option<usize> {
    let mut quote = None;
    for (index, c) in text.char_indices() {
        match (quote, c) {
            (None, '"') | (None, '\'') => quote = Some(c),
            (Some(open), c) if open == c => quote = None,
            (None, '>') => return Some(index),
            _ => {}
        }
    }
    None
}

/// Reads `name="value"` (or `name='value'`) pairs from a start tag.
///
/// Names and quotes are ASCII, and the scan only ever slices at those, so values holding any
/// character stay on character boundaries.
fn parse_attributes(tag: &str) -> Vec<(&str, &str)> {
    let bytes = tag.as_bytes();
    let mut attributes = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        if !is_name_byte(bytes[i]) {
            i += 1;
            continue;
        }

        let name_start = i;
        while i < bytes.len() && is_name_byte(bytes[i]) {
            i += 1;
        }
        let name = &tag[name_start..i];

        let mut j = i;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'=' {
            i = j;
            continue;
        }

        j += 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || (bytes[j] != b'"' && bytes[j] != b'\'') {
            i = j;
            continue;
        }

        let quote = bytes[j];
        j += 1;
        let value_start = j;
        while j < bytes.len() && bytes[j] != quote {
            j += 1;
        }
        attributes.push((name, &tag[value_start..j]));
        i = j.saturating_add(1);
    }

    attributes
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
}

/// Parses `63.9826mm`, `100`, `10.5px`, `8in`, `72pt`, `6pc`.
///
/// `None` for percentages and for anything that is not a plain length.
fn parse_length(value: &str) -> Option<Length> {
    let text = value.trim();
    if text.is_empty() || text.ends_with('%') {
        return None;
    }

    let lowered = text.to_ascii_lowercase();
    let (number, unit) = if let Some(number) = lowered.strip_suffix("mm") {
        (number, Unit::Mm)
    } else if let Some(number) = lowered.strip_suffix("cm") {
        (number, Unit::Cm)
    } else if let Some(number) = lowered.strip_suffix("in") {
        (number, Unit::In)
    } else if let Some(number) = lowered.strip_suffix("pt") {
        (number, Unit::Pt)
    } else if let Some(number) = lowered.strip_suffix("pc") {
        (number, Unit::Pc)
    } else if let Some(number) = lowered.strip_suffix("px") {
        (number, Unit::Px)
    } else {
        (lowered.as_str(), Unit::Px)
    };

    let value = number.trim().parse::<f64>().ok()?;
    Some(Length { value, unit })
}

/// Parses `min-x min-y width height`, separated by whitespace or commas.
fn parse_view_box(value: &str) -> Option<[f64; 4]> {
    let mut numbers = [0.0; 4];
    let mut count = 0;

    for part in value.split(|c: char| c.is_whitespace() || c == ',') {
        if part.is_empty() {
            continue;
        }
        if count == numbers.len() {
            return None;
        }
        numbers[count] = part.parse::<f64>().ok()?;
        count += 1;
    }

    (count == numbers.len()).then_some(numbers)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The header a PCB export writes: board size in millimetres, viewBox in the same units.
    const PCB_MM: &str = r#"<svg xmlns:svg="http://www.w3.org/2000/svg" xmlns="http://www.w3.org/2000/svg" version="1.1" width="63.9826mm" height="59.9948mm" viewBox="0.0000 0.0000 63.9826 59.9948"><g/></svg>"#;

    /// A drawing whose numbers carry no unit, which is what the PCB profile is for.
    const PCB_UNITLESS: &str = r#"<svg width="63.9826" height="59.9948" viewBox="0 0 63.9826 59.9948"></svg>"#;

    /// A drawing with only a viewBox, as Inkscape writes documents without a document size.
    const VIEW_BOX_ONLY: &str = r#"<svg viewBox="0 0 210 297" xmlns="http://www.w3.org/2000/svg"></svg>"#;

    fn size(data: &str, profile: SvgImportProfile) -> (f64, f64) {
        svg_size_mm_from_str(data, (100.0, 100.0), profile).expect("size")
    }

    #[test]
    fn millimetres_are_read_exactly() {
        let (w, h) = size(PCB_MM, SvgImportProfile::Auto);
        assert!((w - 63.9826).abs() < 1e-9, "width was {}", w);
        assert!((h - 59.9948).abs() < 1e-9, "height was {}", h);
    }

    #[test]
    fn a_pcb_profile_reads_the_same_header_the_same_way() {
        let auto = size(PCB_MM, SvgImportProfile::Auto);
        let pcb = size(PCB_MM, SvgImportProfile::PcbMillimetres);
        assert_eq!(auto, pcb);
    }

    #[test]
    fn physical_units_are_converted() {
        let data = r#"<svg width="2in" height="25.4mm"></svg>"#;
        let (w, h) = size(data, SvgImportProfile::Auto);
        assert!((w - 50.8).abs() < 1e-9, "width was {}", w);
        assert!((h - 25.4).abs() < 1e-9, "height was {}", h);
    }

    #[test]
    fn unitless_numbers_are_pixels_by_default_and_millimetres_for_pcb() {
        let (px_w, px_h) = size(PCB_UNITLESS, SvgImportProfile::Auto);
        assert!((px_w - 63.9826 * 25.4 / 96.0).abs() < 1e-9, "width was {}", px_w);
        assert!((px_h - 59.9948 * 25.4 / 96.0).abs() < 1e-9, "height was {}", px_h);

        let (mm_w, mm_h) = size(PCB_UNITLESS, SvgImportProfile::PcbMillimetres);
        assert!((mm_w - 63.9826).abs() < 1e-9, "width was {}", mm_w);
        assert!((mm_h - 59.9948).abs() < 1e-9, "height was {}", mm_h);
    }

    #[test]
    fn a_view_box_alone_scales_with_the_profile() {
        let (px_w, _) = size(VIEW_BOX_ONLY, SvgImportProfile::Auto);
        assert!((px_w - 210.0 * 25.4 / 96.0).abs() < 1e-9, "width was {}", px_w);

        let (mm_w, mm_h) = size(VIEW_BOX_ONLY, SvgImportProfile::PcbMillimetres);
        assert!((mm_w - 210.0).abs() < 1e-9, "width was {}", mm_w);
        assert!((mm_h - 297.0).abs() < 1e-9, "height was {}", mm_h);
    }

    #[test]
    fn percentages_fall_back_to_the_view_box() {
        let data = r#"<svg width="100%" height="100%" viewBox="0 0 40 30"></svg>"#;
        let (w, h) = size(data, SvgImportProfile::Auto);
        assert!((w - 40.0 * 25.4 / 96.0).abs() < 1e-9, "width was {}", w);
        assert!((h - 30.0 * 25.4 / 96.0).abs() < 1e-9, "height was {}", h);
    }

    #[test]
    fn an_unreadable_header_falls_back_to_the_renderer() {
        let data = r#"<svg><g/></svg>"#;
        let (w, h) = size(data, SvgImportProfile::Auto);
        assert!((w - 100.0 * 25.4 / 96.0).abs() < 1e-9, "width was {}", w);
        assert!((h - 100.0 * 25.4 / 96.0).abs() < 1e-9, "height was {}", h);
    }

    #[test]
    fn a_closing_angle_bracket_inside_a_value_does_not_end_the_tag() {
        let data = r#"<svg width="10mm" height="20mm" data-note="a > b"></svg>"#;
        let (w, h) = size(data, SvgImportProfile::Auto);
        assert!((w - 10.0).abs() < 1e-9, "width was {}", w);
        assert!((h - 20.0).abs() < 1e-9, "height was {}", h);
    }

    #[test]
    fn comma_separated_view_boxes_are_read() {
        let data = r#"<svg viewBox="0,0,63.9826,59.9948"></svg>"#;
        let header = parse_svg_header(data);
        assert_eq!(header.view_box, Some([0.0, 0.0, 63.9826, 59.9948]));
    }

    #[test]
    fn label_covers_both_profiles() {
        assert_eq!(SvgImportProfile::Auto.label(), "Auto");
        assert_eq!(SvgImportProfile::PcbMillimetres.label(), "SCG (PCB)");
    }
}
