use egui::{CursorIcon, Response, Vec2};

use crate::core::layer::{Layer, LayerSource, VectorShape};
use crate::core::tools::{
    ToolCommit, ToolContext, ToolEvent, ToolKind, ToolModifiers, ToolResponse,
};
use crate::core::vector::{CurvePoint, PathNode, VectorPath};
use crate::ui::{
    CanvasTransform, LaserApp, colors,
    selection::{self, HandleKind, Selection},
};

use super::state::DragMode;

pub fn handle_interaction(
    app: &mut LaserApp,
    ui: &mut egui::Ui,
    response: &Response,
    transform: &CanvasTransform,
) {
    let Some(pos) = response.hover_pos() else {
        app.status_bar.set_mouse_world(f64::NAN, f64::NAN);
        return;
    };
    let (wx, wy) = transform.to_world(pos);
    let world = CurvePoint::new(wx, wy);

    app.status_bar.set_mouse_world(wx, wy);
    app.canvas.hovered_layer = hit_test_layers(&app.project.layers, world);

    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll != 0.0 {
        let zoom_factor = 1.0 + scroll * 0.001;
        let cursor_rel = pos - transform.rect.center();
        app.zoom_on_position(zoom_factor, cursor_rel);
    }

    let modifiers = ui.input(|i| ToolModifiers {
        shift: i.modifiers.shift,
        ctrl: i.modifiers.ctrl,
        alt: i.modifiers.alt,
    });

    if app.tools.current != ToolKind::Select {
        handle_tool_input(app, ui, response, transform, &modifiers);
        ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
    } else {
        handle_select_input(app, ui, response, transform, &world, &modifiers);
    }
}

fn handle_select_input(
    app: &mut LaserApp,
    ui: &mut egui::Ui,
    response: &Response,
    transform: &CanvasTransform,
    world: &CurvePoint,
    modifiers: &ToolModifiers,
) {
    let world = *world;
    let tol_mm = (8.0 / transform.scale.max(0.0001) as f64).max(0.2);

    if ui.input(|i| i.pointer.middle_down()) {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        if response.dragged_by(egui::PointerButton::Middle) {
            app.canvas.offset += response.drag_delta();
        }
        return;
    }

    update_cursor(app, ui, response, world, tol_mm);

    if ui.input(|i| i.pointer.primary_pressed()) {
        handle_primary_down(app, world, tol_mm, modifiers);
    }

    if ui.input(|i| i.pointer.primary_released()) {
        if let DragMode::RubberBand {
            start_world,
            current_world,
        } = app.canvas.drag
        {
            apply_rubber_band_selection(app, start_world, current_world, modifiers);
        }
        app.canvas.drag = DragMode::None;
    }

    if let DragMode::RubberBand { current_world, .. } = &mut app.canvas.drag {
        *current_world = world;
    }

    if response.dragged_by(egui::PointerButton::Primary) {
        handle_drag(app, response, transform, world);
    }

    if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
        delete_selected(app);
    }
}

fn update_cursor(
    app: &LaserApp,
    ui: &mut egui::Ui,
    response: &Response,
    world: CurvePoint,
    tol_mm: f64,
) {
    if response.dragged_by(egui::PointerButton::Primary) || ui.input(|i| i.pointer.primary_down()) {
        match app.canvas.drag {
            DragMode::MoveLayer { .. } | DragMode::Pan => {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing)
            }
            DragMode::MoveNode { .. } => ui.ctx().set_cursor_icon(CursorIcon::Move),
            DragMode::MoveHandle { .. } => ui.ctx().set_cursor_icon(CursorIcon::PointingHand),
            DragMode::RubberBand { .. } => ui.ctx().set_cursor_icon(CursorIcon::Crosshair),
            DragMode::None => {}
        }
        return;
    }

    let mut hover_icon = CursorIcon::Default;

    if let Some(li) = app.ui_state.selection.layer {
        if let Some(layer) = app.project.layers.get(li) {
            if let Some(si) = app.ui_state.selection.shape {
                if let Some(shape) = layer.shapes.get(si) {
                    if let Some(ni) = app.ui_state.selection.node {
                        if let Some(node) = shape.nodes.get(ni) {
                            let t2 = tol_mm * tol_mm;
                            if node.has_handles() {
                                if selection::dist2(node.handle_out, world) <= t2
                                    || selection::dist2(node.handle_in, world) <= t2
                                {
                                    hover_icon = CursorIcon::PointingHand;
                                }
                            }
                        }
                    }

                    if hover_icon == CursorIcon::Default {
                        let mut path = VectorPath::new();
                        path.nodes = shape.nodes.clone();
                        for node in &path.nodes {
                            if selection::dist2(node.point, world) <= tol_mm * tol_mm {
                                hover_icon = CursorIcon::Move;
                                break;
                            }
                        }
                    }

                    if hover_icon == CursorIcon::Default {
                        let mut path = VectorPath::new();
                        path.nodes = shape.nodes.clone();
                        path.stroke = shape.stroke.clone();
                        path.closed = shape.closed;
                        if selection::hit_path(&path, world, tol_mm) {
                            hover_icon = CursorIcon::Move;
                        }
                    }
                }
            }

            if hover_icon == CursorIcon::Default && layer.is_vector() {
                'shapes: for shape in &layer.shapes {
                    let mut path = VectorPath::new();
                    path.nodes = shape.nodes.clone();
                    for node in &path.nodes {
                        if selection::dist2(node.point, world) <= tol_mm * tol_mm {
                            hover_icon = CursorIcon::Move;
                            break 'shapes;
                        }
                    }
                }
            }
        }
    }

    if hover_icon == CursorIcon::Default && hit_test_layers(&app.project.layers, world).is_some() {
        hover_icon = CursorIcon::Move;
    }

    ui.ctx().set_cursor_icon(hover_icon);
}

fn handle_primary_down(
    app: &mut LaserApp,
    world: CurvePoint,
    tol_mm: f64,
    modifiers: &ToolModifiers,
) {
    let mut picked = false;
    let mut changed = false;

    if let Some(li) = app.ui_state.selection.layer {
        if let Some(layer) = app.project.layers.get(li) {
            if let Some(si) = app.ui_state.selection.shape {
                if let Some(shape) = layer.shapes.get(si) {
                    if let Some(ni) = app.ui_state.selection.node {
                        if let Some(node) = shape.nodes.get(ni) {
                            let t2 = tol_mm * tol_mm;
                            if node.has_handles() {
                                if selection::dist2(node.handle_out, world) <= t2 {
                                    app.canvas.drag = DragMode::MoveHandle {
                                        layer: li,
                                        shape: si,
                                        node: ni,
                                        handle: HandleKind::Out,
                                    };
                                    picked = true;
                                } else if selection::dist2(node.handle_in, world) <= t2 {
                                    app.canvas.drag = DragMode::MoveHandle {
                                        layer: li,
                                        shape: si,
                                        node: ni,
                                        handle: HandleKind::In,
                                    };
                                    picked = true;
                                }
                            }
                        }
                    }

                    if !picked {
                        let mut path = VectorPath::new();
                        path.nodes = shape.nodes.clone();
                        for (ni, node) in path.nodes.iter().enumerate() {
                            if selection::dist2(node.point, world) <= tol_mm * tol_mm {
                                app.ui_state.selection.node = Some(ni);
                                app.ui_state.selection.handle = None;
                                app.canvas.drag = DragMode::MoveNode {
                                    layer: li,
                                    shape: si,
                                    node: ni,
                                };
                                picked = true;
                                changed = true;
                                break;
                            }
                        }
                    }
                }
            }

            if !picked && layer.is_vector() {
                'outer: for (si, shape) in layer.shapes.iter().enumerate() {
                    let mut path = VectorPath::new();
                    path.nodes = shape.nodes.clone();
                    for (ni, node) in path.nodes.iter().enumerate() {
                        if selection::dist2(node.point, world) <= tol_mm * tol_mm {
                            app.ui_state.selection.shape = Some(si);
                            app.ui_state.selection.node = Some(ni);
                            app.ui_state.selection.handle = None;
                            app.canvas.drag = DragMode::MoveNode {
                                layer: li,
                                shape: si,
                                node: ni,
                            };
                            picked = true;
                            changed = true;
                            break 'outer;
                        }
                    }
                }
            }

            if !picked && layer.is_vector() {
                for (si, shape) in layer.shapes.iter().enumerate() {
                    let mut path = VectorPath::new();
                    path.nodes = shape.nodes.clone();
                    path.stroke = shape.stroke.clone();
                    path.closed = shape.closed;
                    if selection::hit_path(&path, world, tol_mm) {
                        app.ui_state.selection.shape = Some(si);
                        app.ui_state.selection.node = None;
                        app.ui_state.selection.handle = None;
                        let start_offset = layer.offset;
                        app.canvas.drag = DragMode::MoveLayer {
                            start_world: world,
                            start_offset,
                        };
                        picked = true;
                        changed = true;
                        break;
                    }
                }
            }
        }
    }

    if !picked {
        if let Some(li) = hit_test_layers(&app.project.layers, world) {
            if modifiers.shift {
                app.ui_state.selection.toggle_layer(li);
            } else {
                if !app.ui_state.selection.is_layer_selected(li) {
                    app.ui_state.selection.set_single_layer(li);
                }
            }
            let start_offset = app.project.layers[li].offset;
            app.canvas.drag = DragMode::MoveLayer {
                start_world: world,
                start_offset,
            };
            changed = true;
        } else {
            if !modifiers.shift {
                app.ui_state.selection.clear();
            }
            app.canvas.drag = DragMode::RubberBand {
                start_world: world,
                current_world: world,
            };
        }
    }

    if changed {
        app.push_history();
    }
}

fn apply_rubber_band_selection(
    app: &mut LaserApp,
    start_world: CurvePoint,
    current_world: CurvePoint,
    modifiers: &ToolModifiers,
) {
    let min_x = start_world.x.min(current_world.x);
    let max_x = start_world.x.max(current_world.x);
    let min_y = start_world.y.min(current_world.y);
    let max_y = start_world.y.max(current_world.y);

    let mut found_indices = Vec::new();
    for (i, layer) in app.project.layers.iter().enumerate() {
        if !layer.is_visible {
            continue;
        }
        if let Some((lmin_x, lmin_y, lmax_x, lmax_y)) = layer.bounds() {
            if lmin_x >= min_x && lmax_x <= max_x && lmin_y >= min_y && lmax_y <= max_y {
                found_indices.push(i);
            }
        }
    }

    if modifiers.shift {
        for idx in found_indices {
            if !app.ui_state.selection.layers.contains(&idx) {
                app.ui_state.selection.layers.push(idx);
            }
        }
        app.ui_state.selection.layer = app.ui_state.selection.layers.last().copied();
    } else {
        app.ui_state.selection.layers = found_indices;
        app.ui_state.selection.layer = app.ui_state.selection.layers.last().copied();
    }
    app.ui_state.selection.shape = None;
    app.ui_state.selection.node = None;
    app.ui_state.selection.handle = None;
}

fn handle_drag(
    app: &mut LaserApp,
    response: &Response,
    transform: &CanvasTransform,
    world: CurvePoint,
) {
    let delta = response.drag_delta();
    let raw_dx = delta.x as f64 / transform.scale as f64;
    let raw_dy = -(delta.y as f64) / transform.scale as f64;

    match app.canvas.drag {
        DragMode::Pan => {
            app.canvas.offset += delta;
        }
        DragMode::MoveLayer {
            start_world,
            start_offset,
        } => {
            let mut target_x = start_offset[0] + (world.x - start_world.x);
            let mut target_y = start_offset[1] + (world.y - start_world.y);
            if app.tools.snap_enabled && app.tools.snap_mm > 0.0 {
                let step = app.tools.snap_mm;
                target_x = (target_x / step).round() * step;
                target_y = (target_y / step).round() * step;
            }
            let move_ids = if app.ui_state.selection.layers.is_empty() {
                vec![app.ui_state.selection.layer.unwrap()]
            } else {
                app.ui_state.selection.layers.clone()
            };

            let ref_idx = app.ui_state.selection.layer.unwrap_or(0);
            let ref_offset = app.project.layers[ref_idx].offset;

            let dx = target_x - ref_offset[0];
            let dy = target_y - ref_offset[1];

            for &id in &move_ids {
                if let Some(layer) = app.project.layers.get_mut(id) {
                    layer.translate(dx, dy);
                }
            }
        }
        DragMode::MoveNode { layer, shape, node } => {
            let mut tx = world.x;
            let mut ty = world.y;
            if app.tools.snap_enabled && app.tools.snap_mm > 0.0 {
                let step = app.tools.snap_mm;
                tx = (tx / step).round() * step;
                ty = (ty / step).round() * step;
            }
            if let Some(l) = app.project.layers.get_mut(layer) {
                if let Some(s) = l.shapes.get_mut(shape) {
                    if let Some(n) = s.nodes.get_mut(node) {
                        let dx = tx - n.point.x;
                        let dy = ty - n.point.y;
                        if dx != 0.0 || dy != 0.0 {
                            n.translate(dx, dy);
                        }
                    }
                    l.recompute_bounds();
                }
            }
        }
        DragMode::MoveHandle {
            layer,
            shape,
            node,
            handle,
        } => {
            let mut tx = world.x;
            let mut ty = world.y;
            if app.tools.snap_enabled && app.tools.snap_mm > 0.0 {
                let step = app.tools.snap_mm;
                tx = (tx / step).round() * step;
                ty = (ty / step).round() * step;
            }
            if let Some(l) = app.project.layers.get_mut(layer) {
                if let Some(s) = l.shapes.get_mut(shape) {
                    if let Some(n) = s.nodes.get_mut(node) {
                        match handle {
                            HandleKind::Out => n.set_handle_out(tx, ty),
                            HandleKind::In => {
                                let hx = n.point.x - tx;
                                let hy = n.point.y - ty;
                                n.handle_out.x = n.point.x + hx;
                                n.handle_out.y = n.point.y + hy;
                                n.handle_in.x = tx;
                                n.handle_in.y = ty;
                            }
                        }
                    }
                    l.recompute_bounds();
                }
            }
        }
        DragMode::RubberBand { .. } => {
            let _ = (raw_dx, raw_dy);
        }
        DragMode::None => {}
    }
}

fn delete_selected(app: &mut LaserApp) {
    let sel = app.ui_state.selection.clone();
    if let (Some(li), Some(si)) = (sel.layer, sel.shape) {
        app.push_history();
        if let Some(layer) = app.project.layers.get_mut(li) {
            if si < layer.shapes.len() {
                layer.shapes.remove(si);
                layer.recompute_bounds();
            }
        }
        app.ui_state.selection.shape = None;
        app.ui_state.selection.node = None;
        app.ui_state.selection.handle = None;
    } else if !sel.layers.is_empty() {
        app.push_history();
        let mut to_remove = sel.layers.clone();
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in to_remove {
            app.project.remove_layer(idx);
        }
        app.ui_state.selection.clear();
    }
}

fn handle_tool_input(
    app: &mut LaserApp,
    ui: &mut egui::Ui,
    response: &Response,
    transform: &CanvasTransform,
    modifiers: &ToolModifiers,
) {
    let Some(pos) = response.hover_pos() else {
        return;
    };
    if !transform.rect.contains(pos) {
        return;
    }

    if ui.input(|i| i.pointer.middle_down()) {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        if response.dragged_by(egui::PointerButton::Middle) {
            app.canvas.offset += response.drag_delta();
        }
        return;
    }

    let (wx, wy) = transform.to_world(pos);
    let raw = CurvePoint::new(wx, wy);
    let snapped = app.tools.snap(raw);

    let ctx = ToolContext {
        world: raw,
        snapped,
        modifiers: *modifiers,
        tolerance_mm: 8.0 / transform.scale.max(0.0001) as f64,
    };

    let mut events: Vec<ToolEvent> = Vec::new();

    if ui.input(|i| i.pointer.primary_pressed()) {
        events.push(ToolEvent::PrimaryDown);
    }
    if ui.input(|i| i.pointer.primary_released()) {
        events.push(ToolEvent::PrimaryUp);
    }
    if ui.input(|i| i.pointer.secondary_pressed()) {
        events.push(ToolEvent::SecondaryDown);
    }
    if ui.input(|i| i.pointer.secondary_released()) {
        events.push(ToolEvent::SecondaryUp);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        events.push(ToolEvent::Escape);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        events.push(ToolEvent::Enter);
    }

    let double_click = ui.input(|i| {
        i.pointer
            .button_double_clicked(egui::PointerButton::Primary)
    });
    if double_click {
        events.push(ToolEvent::Finish);
    }

    if events.is_empty() {
        if ui.input(|i| i.pointer.delta() != Vec2::ZERO) {
            events.push(ToolEvent::Move);
        }
    } else {
        events.push(ToolEvent::Move);
    }

    for ev in events {
        let resp = app.tools.dispatch(ev, &ctx);
        if let ToolResponse::Commit(commits) = resp {
            apply_commits(app, commits);
        }
    }
}

fn apply_commits(app: &mut LaserApp, commits: Vec<ToolCommit>) {
    let mut shapes: Vec<VectorShape> = Vec::new();
    for c in commits {
        if let ToolCommit::Path(p) = c {
            if p.nodes.len() < 2 {
                continue;
            }
            shapes.push(VectorShape {
                nodes: p.nodes,
                stroke: p.stroke,
                closed: p.closed,
            });
        }
    }

    if shapes.is_empty() {
        return;
    }

    app.push_history();

    let insert_at = app
        .ui_state
        .selection
        .layer
        .filter(|&i| i < app.project.layers.len() && app.project.layers[i].is_vector());

    if let Some(idx) = insert_at {
        for s in shapes {
            app.project.layers[idx].shapes.push(s);
        }
        app.project.layers[idx].recompute_bounds();
        let si = app.project.layers[idx].shapes.len() - 1;
        app.ui_state.selection.shape = Some(si);
        app.ui_state.selection.node = None;
        app.ui_state.selection.handle = None;
        return;
    }

    let color = colors::LAYER_DEFAULT;
    let mut layer = Layer::new("Vector".to_string(), color, LayerSource::Vector);
    for s in shapes {
        layer.shapes.push(s);
    }
    layer.recompute_bounds();

    app.project.add_layer(layer);
    let new_idx = app.project.layers.len() - 1;
    let new_si = app.project.layers[new_idx].shapes.len().saturating_sub(1);
    app.ui_state.selection.set_single_layer(new_idx);
    app.ui_state.selection.shape = Some(new_si);
    app.ui_state.selection.node = None;
    app.ui_state.selection.handle = None;
}

fn hit_test_layers(layers: &[Layer], p: CurvePoint) -> Option<usize> {
    for (i, layer) in layers.iter().enumerate().rev() {
        if !layer.is_visible {
            continue;
        }
        if let Some((min_x, min_y, max_x, max_y)) = layer.bounds() {
            if p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y {
                return Some(i);
            }
        }
    }
    None
}