//! Gerber (RS-274X) import.
//!
//! A Gerber layer is a set of *areas*, not a set of lines: a trace is an aperture-wide
//! stroke, a pad is a filled shape, and a region is a filled polygon. Drawing each of those
//! as its own outline produces a wireframe in which every overlapping pad and trace adds
//! visible edges.
//!
//! So the document is first turned into filled contours, those contours are merged with a
//! boolean union, and only the resulting outline of the copper is handed on as line
//! segments. That keeps the layer compatible with the existing segment based rendering,
//! caching, preview and export paths while showing only the real edges of the copper.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use clipper2::{EndType, FillRule, JoinType, Milli};
use evalexpr::{
    Context, ContextWithMutableVariables, HashMapContext, Value, eval_float_with_context,
};
use gerber_parser::GerberDoc;
use gerber_parser::gerber_types::{
    Aperture, Command, CoordinateOffset, Coordinates, DCode, ExtendedCode, FunctionCode, GCode,
    InterpolationMode, MacroContent, MacroDecimal, MacroInteger, MCode, Operation, Unit,
};

/// clipper2 stores coordinates as integers, so a scaler has to be picked. `Milli` keeps a
/// micrometre of precision, which fits the rest of the app working in millimetres.
///
/// Fully qualified because `Path` is already taken by `std::path::Path` in this module.
type ClipPaths = clipper2::Paths<Milli>;
type ClipPath = clipper2::Path<Milli>;

/// A line segment in millimetres: `(x0, y0, x1, y1)`.
pub type Segment = (f64, f64, f64, f64);

/// Straight pieces used to approximate a full circle or a full arc sweep.
const ARC_STEPS: usize = 64;

/// Half width used when a stroke has no usable aperture, so it still comes out as a line.
const FALLBACK_HALF_WIDTH: f64 = 0.05;

/// Tolerance passed to clipper2's path simplification, in millimetres.
const SIMPLIFY_EPSILON: f64 = 0.01;

/// Reads a Gerber file and returns the outline of its copper as line segments in
/// millimetres.
pub fn parse_gerber_segments(path: &Path) -> Result<Vec<Segment>, String> {
    let walk = read_document(path)?;

    // Turn every primitive into a filled area, then merge them all. Overlapping strokes and
    // pads stop contributing their own edges and only the outer boundary survives.
    let mut paths = ClipPaths::new(Vec::new());
    for primitive in &walk.primitives {
        paths.append(primitive_to_paths(primitive));
    }

    let merged = union_paths(paths);

    // A union that fails or collapses must not cost the layer its geometry.
    let segments = if merged.is_empty() {
        log::warn!("gerber: union produced nothing, keeping the unmerged outlines");
        let mut unmerged = ClipPaths::new(Vec::new());
        for primitive in &walk.primitives {
            unmerged.append(primitive_to_paths(primitive));
        }
        paths_to_segments(&unmerged)
    } else {
        paths_to_segments(&merged)
    };

    log::info!(
        "gerber: {} primitives -> {} contours -> {} segments",
        walk.primitives.len(),
        merged.len(),
        segments.len()
    );

    if segments.is_empty() {
        return Err("no geometry found in gerber".to_string());
    }

    Ok(segments)
}

/// Parses a file into primitives, scaled to millimetres.
fn read_document(path: &Path) -> Result<Walk, String> {
    let file = File::open(path).map_err(|e| format!("open {}: {}", path.display(), e))?;
    let doc = gerber_parser::parse(BufReader::new(file))
        .map_err(|(_, e)| format!("gerber: {}", e))?;

    let mut walk = Walk::default();
    for command in doc.commands() {
        walk.command(&doc, command);
    }
    walk.finish();

    if walk.primitives.is_empty() {
        return Err("no geometry found in gerber".to_string());
    }

    // Everything downstream works in millimetres, and so do the tolerances above.
    if walk.unit == Unit::Inches {
        for primitive in &mut walk.primitives {
            scale_primitive(primitive, 25.4);
        }
    }

    Ok(walk)
}

/// A Gerber primitive, kept in its own terms until it is turned into an area.
enum Primitive {
    Segment {
        start: (f64, f64),
        end: (f64, f64),
        width: f64,
    },
    Arc {
        start: (f64, f64),
        end: (f64, f64),
        center: (f64, f64),
        clockwise: bool,
        width: f64,
    },
    Circle {
        center: (f64, f64),
        diameter: f64,
    },
    Rectangle {
        center: (f64, f64),
        width: f64,
        height: f64,
        rotation: f64,
    },
    Polygon(Vec<(f64, f64)>),
    Complex(Vec<Primitive>),
}

/// Walks the commands of a document, keeping the modal state the format relies on.
struct Walk {
    position: (f64, f64),
    aperture: i32,
    interpolation: InterpolationMode,
    region_mode: bool,
    region_points: Vec<(f64, f64)>,
    macros: HashMap<String, Vec<MacroContent>>,
    unit: Unit,
    primitives: Vec<Primitive>,
}

impl Default for Walk {
    fn default() -> Self {
        Self {
            position: (0.0, 0.0),
            aperture: 0,
            interpolation: InterpolationMode::Linear,
            region_mode: false,
            region_points: Vec::new(),
            macros: HashMap::new(),
            unit: Unit::Millimeters,
            primitives: Vec::new(),
        }
    }
}

impl Walk {
    fn command(&mut self, doc: &GerberDoc, command: &Command) {
        match command {
            Command::FunctionCode(FunctionCode::DCode(dcode)) => self.dcode(doc, dcode),
            Command::FunctionCode(FunctionCode::GCode(gcode)) => self.gcode(gcode),
            Command::FunctionCode(FunctionCode::MCode(MCode::EndOfFile)) => {}
            Command::ExtendedCode(extended) => self.extended(extended),
        }
    }

    fn dcode(&mut self, doc: &GerberDoc, dcode: &DCode) {
        match dcode {
            DCode::SelectAperture(id) => self.aperture = *id,
            DCode::Operation(operation) => self.operation(doc, operation),
        }
    }

    fn operation(&mut self, doc: &GerberDoc, operation: &Operation) {
        match operation {
            Operation::Interpolate(coordinates, offset) => {
                let target = coordinates_to_position(coordinates, self.position);

                match self.interpolation {
                    InterpolationMode::Linear => {
                        if self.region_mode {
                            if self.region_points.is_empty() {
                                self.region_points.push(self.position);
                            }
                            self.region_points.push(target);
                        } else {
                            let width = self.aperture_width(doc);
                            self.primitives.push(Primitive::Segment {
                                start: self.position,
                                end: target,
                                width,
                            });
                        }
                    }

                    InterpolationMode::ClockwiseCircular
                    | InterpolationMode::CounterclockwiseCircular => {
                        if self.region_mode {
                            log::warn!(
                                "gerber: circular interpolation is not supported in region mode"
                            );
                        } else if let Some(offset) = offset {
                            let center = offset_to_position(offset, self.position);
                            let width = self.aperture_width(doc);
                            self.primitives.push(Primitive::Arc {
                                start: self.position,
                                end: target,
                                center,
                                clockwise: self.interpolation
                                    == InterpolationMode::ClockwiseCircular,
                                width,
                            });
                        } else {
                            log::warn!(
                                "gerber: circular interpolation without an offset is not supported"
                            );
                        }
                    }
                }

                self.position = target;
            }

            Operation::Move(coordinates) => {
                self.position = coordinates_to_position(coordinates, self.position);
                self.flush_region();
            }

            Operation::Flash(coordinates) => {
                if !self.region_mode {
                    let flash = coordinates_to_position(coordinates, self.position);
                    if let Some(aperture) = doc.apertures.get(&self.aperture) {
                        if let Some(primitive) =
                            aperture_to_primitive(aperture, flash, &self.macros)
                        {
                            self.primitives.push(primitive);
                        }
                    }
                    self.position = flash;
                }
            }
        }
    }

    fn gcode(&mut self, gcode: &GCode) {
        match gcode {
            GCode::InterpolationMode(mode) => self.interpolation = *mode,
            GCode::RegionMode(enabled) => {
                self.region_mode = *enabled;
                if !enabled {
                    self.flush_region();
                }
            }
            GCode::Unit(unit) => self.unit = *unit,
            GCode::QuadrantMode(_) => {}
            GCode::Comment(_) => {}
            GCode::CoordinateMode(_) => {}
            GCode::SelectAperture => {}
        }
    }

    fn extended(&mut self, extended: &ExtendedCode) {
        match extended {
            ExtendedCode::Unit(unit) => self.unit = *unit,
            ExtendedCode::ApertureMacro(mac) => {
                self.macros.insert(mac.name.clone(), mac.content.clone());
            }
            _ => {}
        }
    }

    /// Width of the currently selected aperture, which is what turns a stroke into an area.
    fn aperture_width(&self, doc: &GerberDoc) -> f64 {
        doc.apertures
            .get(&self.aperture)
            .map(aperture_stroke_width)
            .unwrap_or(0.0)
    }

    /// Closes an open region (`G36`/`G37`) into a polygon.
    fn flush_region(&mut self) {
        if self.region_points.len() > 2 {
            let points = std::mem::take(&mut self.region_points);
            self.primitives.push(Primitive::Polygon(points));
        }
        self.region_points.clear();
    }

    fn finish(&mut self) {
        self.flush_region();
    }
}

/// Coordinates are modal: an omitted axis keeps its previous value.
fn coordinates_to_position(coordinates: &Option<Coordinates>, current: (f64, f64)) -> (f64, f64) {
    let mut result = current;

    if let Some(coordinates) = coordinates {
        if let Some(x) = coordinates.x {
            result.0 = x.into();
        }
        if let Some(y) = coordinates.y {
            result.1 = y.into();
        }
    }

    result
}

/// Arc offsets are relative to the current point.
fn offset_to_position(offset: &CoordinateOffset, current: (f64, f64)) -> (f64, f64) {
    let mut result = current;

    if let Some(x) = offset.x {
        result.0 = x.into();
    }
    if let Some(y) = offset.y {
        result.1 = y.into();
    }

    result
}

fn aperture_stroke_width(aperture: &Aperture) -> f64 {
    match aperture {
        Aperture::Circle(circle) => circle.diameter,
        Aperture::Rectangle(rectangular) => rectangular.x.max(rectangular.y),
        Aperture::Obround(rectangular) => rectangular.x.max(rectangular.y),
        Aperture::Polygon(polygon) => polygon.diameter,
        Aperture::Macro(..) => 0.0,
    }
}

fn scale_primitive(primitive: &mut Primitive, factor: f64) {
    match primitive {
        Primitive::Segment {
            start,
            end,
            width,
        } => {
            start.0 *= factor;
            start.1 *= factor;
            end.0 *= factor;
            end.1 *= factor;
            *width *= factor;
        }

        Primitive::Arc {
            start,
            end,
            center,
            width,
            ..
        } => {
            start.0 *= factor;
            start.1 *= factor;
            end.0 *= factor;
            end.1 *= factor;
            center.0 *= factor;
            center.1 *= factor;
            *width *= factor;
        }

        Primitive::Circle { center, diameter } => {
            center.0 *= factor;
            center.1 *= factor;
            *diameter *= factor;
        }

        Primitive::Rectangle {
            center,
            width,
            height,
            ..
        } => {
            center.0 *= factor;
            center.1 *= factor;
            *width *= factor;
            *height *= factor;
        }

        Primitive::Polygon(points) => {
            for point in points {
                point.0 *= factor;
                point.1 *= factor;
            }
        }

        Primitive::Complex(parts) => {
            for part in parts {
                scale_primitive(part, factor);
            }
        }
    }
}

/// Converts one primitive into the filled contour(s) it covers.
fn primitive_to_paths(primitive: &Primitive) -> ClipPaths {
    match primitive {
        Primitive::Segment {
            start,
            end,
            width,
        } => stroke_to_paths(&[*start, *end], *width),

        Primitive::Arc {
            start,
            end,
            center,
            clockwise,
            width,
        } => {
            let points = tessellate_arc(*start, *end, *center, *clockwise);
            stroke_to_paths(&points, *width)
        }

        Primitive::Circle { center, diameter } => {
            let radius = diameter / 2.0;
            if radius <= 0.0 {
                return ClipPaths::new(Vec::new());
            }

            let mut points = Vec::with_capacity(ARC_STEPS);
            for i in 0..ARC_STEPS {
                let angle = i as f64 / ARC_STEPS as f64 * std::f64::consts::TAU;
                points.push((
                    center.0 + angle.cos() * radius,
                    center.1 + angle.sin() * radius,
                ));
            }

            points.into()
        }

        Primitive::Rectangle {
            center,
            width,
            height,
            rotation,
        } => {
            let (half_w, half_h) = (width / 2.0, height / 2.0);
            let (sin, cos) = rotation.to_radians().sin_cos();

            let corners: Vec<(f64, f64)> =
                [(-half_w, -half_h), (half_w, -half_h), (half_w, half_h), (-half_w, half_h)]
                    .iter()
                    .map(|(x, y)| {
                        (
                            center.0 + x * cos - y * sin,
                            center.1 + x * sin + y * cos,
                        )
                    })
                    .collect();

            corners.into()
        }

        Primitive::Polygon(points) => {
            if points.len() < 3 {
                return ClipPaths::new(Vec::new());
            }
            points.to_vec().into()
        }

        Primitive::Complex(parts) => {
            let mut paths = ClipPaths::new(Vec::new());
            for part in parts {
                paths.append(primitive_to_paths(part));
            }
            union_paths(paths)
        }
    }
}

/// Inflates a polyline by half the aperture width, which is what turns a stroke into the
/// area the laser actually has to clear.
fn stroke_to_paths(points: &[(f64, f64)], width: f64) -> ClipPaths {
    if points.len() < 2 {
        return ClipPaths::new(Vec::new());
    }

    let half = if width > 0.0 {
        width / 2.0
    } else {
        FALLBACK_HALF_WIDTH
    };

    let path: ClipPath = points.to_vec().into();

    path.inflate(half, JoinType::Round, EndType::Round, 2.0)
        .simplify(SIMPLIFY_EPSILON, false)
}

fn union_paths(paths: ClipPaths) -> ClipPaths {
    // NonZero rather than EvenOdd: overlapping copper has to merge, not cancel out.
    clipper2::union::<Milli>(
        orient_positive(paths),
        ClipPaths::new(Vec::new()),
        FillRule::NonZero,
    )
    .unwrap_or_else(|_| ClipPaths::new(Vec::new()))
}

/// Rewinds every contour so they all run the same way round.
///
/// The NonZero fill rule works on winding numbers, so a contour that runs opposite to the
/// one it overlaps cancels it out instead of merging with it. Gerber regions and macro
/// outlines inherit whatever direction the source file happened to use, and clipper2's own
/// offsetting is not guaranteed to match either, so the winding is normalised here rather
/// than trusted.
fn orient_positive(paths: ClipPaths) -> ClipPaths {
    let mut out: Vec<ClipPath> = Vec::with_capacity(paths.len());

    for path in paths.iter() {
        if path.len() < 3 {
            continue;
        }

        let points: Vec<(f64, f64)> = path.iter().map(|point| (point.x(), point.y())).collect();

        if path.signed_area() < 0.0 {
            out.push(points.into_iter().rev().collect::<Vec<_>>().into());
        } else {
            out.push(points.into());
        }
    }

    out.into()
}

/// Emits every contour of the merged areas as a closed run of line segments.
fn paths_to_segments(paths: &ClipPaths) -> Vec<Segment> {
    let mut out = Vec::new();

    for path in paths.iter() {
        let points: Vec<(f64, f64)> = path.iter().map(|point| (point.x(), point.y())).collect();
        if points.len() < 2 {
            continue;
        }

        for i in 0..points.len() {
            let a = points[i];
            let b = points[(i + 1) % points.len()];

            if (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12 {
                continue;
            }

            out.push((a.0, a.1, b.0, b.1));
        }
    }

    out
}

fn tessellate_arc(
    start: (f64, f64),
    end: (f64, f64),
    center: (f64, f64),
    clockwise: bool,
) -> Vec<(f64, f64)> {
    let radius = ((start.0 - center.0).powi(2) + (start.1 - center.1).powi(2)).sqrt();
    if radius <= 1e-12 {
        return vec![start, end];
    }

    let from = (start.1 - center.1).atan2(start.0 - center.0);
    let to = (end.1 - center.1).atan2(end.0 - center.0);

    let mut sweep = to - from;
    if clockwise {
        while sweep > 0.0 {
            sweep -= std::f64::consts::TAU;
        }
        while sweep < -std::f64::consts::TAU {
            sweep += std::f64::consts::TAU;
        }
    } else {
        while sweep < 0.0 {
            sweep += std::f64::consts::TAU;
        }
        while sweep > std::f64::consts::TAU {
            sweep -= std::f64::consts::TAU;
        }
    }

    let steps = ((sweep.abs() / std::f64::consts::TAU) * ARC_STEPS as f64)
        .ceil()
        .max(1.0) as usize;

    let mut points = Vec::with_capacity(steps + 1);
    points.push(start);

    for i in 1..steps {
        let angle = from + sweep * (i as f64 / steps as f64);
        points.push((
            center.0 + angle.cos() * radius,
            center.1 + angle.sin() * radius,
        ));
    }

    points.push(end);
    points
}

fn aperture_to_primitive(
    aperture: &Aperture,
    position: (f64, f64),
    macros: &HashMap<String, Vec<MacroContent>>,
) -> Option<Primitive> {
    match aperture {
        Aperture::Circle(circle) => Some(Primitive::Circle {
            center: position,
            diameter: circle.diameter,
        }),

        Aperture::Rectangle(rectangular) => Some(Primitive::Rectangle {
            center: position,
            width: rectangular.x,
            height: rectangular.y,
            rotation: 0.0,
        }),

        // An obround is exactly a stroke of the short side's width with round caps, so the
        // caps do not need to be added separately.
        Aperture::Obround(rectangular) => {
            let (width, height) = (rectangular.x, rectangular.y);

            if (width - height).abs() < 1e-9 {
                Some(Primitive::Circle {
                    center: position,
                    diameter: width,
                })
            } else if width > height {
                let offset = (width - height) / 2.0;
                Some(Primitive::Segment {
                    start: (position.0 - offset, position.1),
                    end: (position.0 + offset, position.1),
                    width: height,
                })
            } else {
                let offset = (height - width) / 2.0;
                Some(Primitive::Segment {
                    start: (position.0, position.1 - offset),
                    end: (position.0, position.1 + offset),
                    width,
                })
            }
        }

        Aperture::Polygon(polygon) => {
            let radius = polygon.diameter / 2.0;
            let vertices = polygon.vertices.max(3) as usize;
            let rotation = polygon.rotation.unwrap_or(0.0).to_radians();

            let mut points = Vec::with_capacity(vertices);
            for i in 0..vertices {
                let angle = rotation + i as f64 / vertices as f64 * std::f64::consts::TAU;
                points.push((
                    position.0 + angle.cos() * radius,
                    position.1 + angle.sin() * radius,
                ));
            }

            Some(Primitive::Polygon(points))
        }

        Aperture::Macro(name, data) => {
            let content = macros.get(name)?;
            let context = macro_context(data);

            let mut parts = Vec::new();
            for element in content {
                if let Some(part) = macro_element_to_primitive(element, &context, position) {
                    parts.push(part);
                }
            }

            Some(Primitive::Complex(parts))
        }
    }
}

/// Binds the aperture's parameters to `$1`, `$2`, ... so macro expressions can be evaluated.
fn macro_context(data: &Option<Vec<MacroDecimal>>) -> HashMapContext {
    let mut context = HashMapContext::new();

    if let Some(data) = data {
        for (index, element) in data.iter().enumerate() {
            if let MacroDecimal::Value(value) = element {
                let _ = context.set_value(format!("${}", index + 1), Value::Float(*value));
            }
        }
    }

    context
}

fn resolve_macro_decimal(value: &MacroDecimal, context: &HashMapContext) -> f64 {
    match value {
        MacroDecimal::Value(value) => *value,
        MacroDecimal::Variable(name) => context
            .get_value(&format!("${}", name))
            .and_then(|value| value.as_number().ok())
            .unwrap_or(f64::NAN),
        MacroDecimal::Expression(expression) => {
            eval_float_with_context(expression, context).unwrap_or(f64::NAN)
        }
    }
}

fn resolve_macro_integer(value: &MacroInteger, context: &HashMapContext) -> u32 {
    let resolved = match value {
        MacroInteger::Value(value) => *value as f64,
        MacroInteger::Variable(name) => context
            .get_value(&format!("${}", name))
            .and_then(|value| value.as_number().ok())
            .unwrap_or(f64::NAN),
        MacroInteger::Expression(expression) => {
            eval_float_with_context(expression, context).unwrap_or(f64::NAN)
        }
    };

    if resolved.is_finite() {
        resolved.round().max(3.0) as u32
    } else {
        3
    }
}

fn macro_pair(pair: &(MacroDecimal, MacroDecimal), context: &HashMapContext) -> (f64, f64) {
    (
        resolve_macro_decimal(&pair.0, context),
        resolve_macro_decimal(&pair.1, context),
    )
}

fn macro_element_to_primitive(
    element: &MacroContent,
    context: &HashMapContext,
    position: (f64, f64),
) -> Option<Primitive> {
    match element {
        MacroContent::Circle(circle) => {
            let center = macro_pair(&circle.center, context);
            Some(Primitive::Circle {
                center: (position.0 + center.0, position.1 + center.1),
                diameter: resolve_macro_decimal(&circle.diameter, context),
            })
        }

        MacroContent::VectorLine(line) => {
            let start = macro_pair(&line.start, context);
            let end = macro_pair(&line.end, context);
            let (dx, dy) = (end.0 - start.0, end.1 - start.1);
            let length = dx.hypot(dy);

            if length <= 0.0 {
                return None;
            }

            let half = resolve_macro_decimal(&line.width, context) / 2.0;
            let (nx, ny) = (-dy / length * half, dx / length * half);

            Some(Primitive::Polygon(vec![
                (position.0 + start.0 + nx, position.1 + start.1 + ny),
                (position.0 + start.0 - nx, position.1 + start.1 - ny),
                (position.0 + end.0 - nx, position.1 + end.1 - ny),
                (position.0 + end.0 + nx, position.1 + end.1 + ny),
            ]))
        }

        MacroContent::CenterLine(line) => {
            let center = macro_pair(&line.center, context);
            let size = macro_pair(&line.dimensions, context);

            Some(Primitive::Rectangle {
                center: (position.0 + center.0, position.1 + center.1),
                width: size.0,
                height: size.1,
                rotation: resolve_macro_decimal(&line.angle, context),
            })
        }

        MacroContent::Outline(outline) => {
            let points = outline
                .points
                .iter()
                .map(|point| {
                    let point = macro_pair(point, context);
                    (position.0 + point.0, position.1 + point.1)
                })
                .collect();

            Some(Primitive::Polygon(points))
        }

        MacroContent::Polygon(polygon) => {
            let vertices = resolve_macro_integer(&polygon.vertices, context).max(3) as usize;
            let radius = resolve_macro_decimal(&polygon.diameter, context) / 2.0;
            let center = macro_pair(&polygon.center, context);
            let rotation = resolve_macro_decimal(&polygon.angle, context).to_radians();

            let mut points = Vec::with_capacity(vertices);
            for i in 0..vertices {
                let angle = rotation + i as f64 / vertices as f64 * std::f64::consts::TAU;
                points.push((
                    position.0 + center.0 + angle.cos() * radius,
                    position.1 + center.1 + angle.sin() * radius,
                ));
            }

            Some(Primitive::Polygon(points))
        }

        // Moire and thermal primitives are decorative; skipping them keeps the rest of the
        // image intact instead of failing the whole import.
        MacroContent::Moire(_) | MacroContent::Thermal(_) => None,
        MacroContent::VariableDefinition(_) | MacroContent::Comment(_) => None,
    }
}
