use crate::core::layer::DropTarget;

use super::row::{RowEntry, RowKind};

pub(super) fn drop_target_at(rows: &[RowEntry], pos: egui::Pos2) -> Option<DropTarget> {
    let first = rows.first()?;
    if pos.x < first.rect.left() - 32.0 || pos.x > first.rect.right() + 32.0 {
        return None;
    }
    let row = rows.iter().min_by(|a, b| {
        gap_to(a, pos.y)
            .partial_cmp(&gap_to(b, pos.y))
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;
    let above = pos.y < row.rect.center().y;
    Some(match row.kind {
        RowKind::Group(group) => {
            if above {
                DropTarget::GroupStart(group)
            } else {
                DropTarget::GroupEnd(group)
            }
        }
        RowKind::Layer { id } => {
            if above {
                DropTarget::BeforeLayer(id)
            } else {
                DropTarget::AfterLayer(id)
            }
        }
    })
}

pub(super) fn row_of(rows: &[RowEntry], target: DropTarget) -> Option<(egui::Rect, bool)> {
    match target {
        DropTarget::GroupStart(group) => rows
            .iter()
            .find(|row| matches!(row.kind, RowKind::Group(id) if id == group))
            .map(|row| (row.rect, true)),
        DropTarget::GroupEnd(group) => rows
            .iter()
            .find(|row| matches!(row.kind, RowKind::Group(id) if id == group))
            .map(|row| (row.rect, false)),
        DropTarget::BeforeLayer(id) => rows
            .iter()
            .find(|row| matches!(row.kind, RowKind::Layer { id: layer } if layer == id))
            .map(|row| (row.rect, true)),
        DropTarget::AfterLayer(id) => rows
            .iter()
            .find(|row| matches!(row.kind, RowKind::Layer { id: layer } if layer == id))
            .map(|row| (row.rect, false)),
    }
}

pub(super) fn paint_drop_indicator(ui: &egui::Ui, rows: &[RowEntry], target: DropTarget) {
    let Some((rect, above)) = row_of(rows, target) else {
        return;
    };
    let color = ui.visuals().selection.stroke.color;
    let y = if above { rect.top() } else { rect.bottom() };
    let painter = ui.painter();
    painter.hline(rect.x_range(), y, egui::Stroke::new(2.0, color));
    painter.circle_filled(egui::pos2(rect.left() + 3.0, y), 3.0, color);
}

pub(super) fn paint_drag_ghost(ui: &egui::Ui, pos: egui::Pos2, text: &str) {
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("layer_drag_ghost"),
    ));
    let font = egui::TextStyle::Body.resolve(ui.style());
    let color = ui.style().visuals.strong_text_color();
    let galley = painter.layout_no_wrap(text.to_string(), font, color);
    let rect = egui::Rect::from_min_size(
        pos + egui::vec2(16.0, 8.0),
        galley.size() + egui::vec2(14.0, 8.0),
    );
    painter.rect(
        rect,
        4.0,
        ui.style().visuals.window_fill,
        ui.style().visuals.window_stroke,
        egui::StrokeKind::Outside,
    );
    painter.galley(rect.min + egui::vec2(7.0, 4.0), galley, color);
}

fn gap_to(row: &RowEntry, y: f32) -> f32 {
    if y < row.rect.top() {
        row.rect.top() - y
    } else if y > row.rect.bottom() {
        y - row.rect.bottom()
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(top: f32, kind: RowKind) -> RowEntry {
        RowEntry {
            rect: egui::Rect::from_min_size(egui::pos2(0.0, top), egui::vec2(200.0, 20.0)),
            kind,
        }
    }

    fn rows() -> Vec<RowEntry> {
        vec![
            entry(0.0, RowKind::Group(7)),
            entry(24.0, RowKind::Layer { id: 1 }),
            entry(48.0, RowKind::Layer { id: 2 }),
        ]
    }

    #[test]
    fn dropping_on_the_upper_half_targets_the_row_before() {
        let target = drop_target_at(&rows(), egui::pos2(100.0, 26.0));
        assert_eq!(target, Some(DropTarget::BeforeLayer(1)));
    }

    #[test]
    fn dropping_on_the_lower_half_targets_the_row_after() {
        let target = drop_target_at(&rows(), egui::pos2(100.0, 60.0));
        assert_eq!(target, Some(DropTarget::AfterLayer(2)));
    }

    #[test]
    fn dropping_on_a_group_header_targets_the_group_edge() {
        assert_eq!(
            drop_target_at(&rows(), egui::pos2(100.0, 2.0)),
            Some(DropTarget::GroupStart(7))
        );
        assert_eq!(
            drop_target_at(&rows(), egui::pos2(100.0, 18.0)),
            Some(DropTarget::GroupEnd(7))
        );
    }

    #[test]
    fn dropping_in_the_gap_picks_the_nearer_row() {
        assert_eq!(
            drop_target_at(&rows(), egui::pos2(100.0, 21.0)),
            Some(DropTarget::GroupEnd(7))
        );
        assert_eq!(
            drop_target_at(&rows(), egui::pos2(100.0, 23.0)),
            Some(DropTarget::BeforeLayer(1))
        );
    }

    #[test]
    fn dropping_outside_the_list_has_no_target() {
        assert_eq!(drop_target_at(&rows(), egui::pos2(600.0, 30.0)), None);
        assert_eq!(drop_target_at(&[], egui::pos2(100.0, 30.0)), None);
    }

    #[test]
    fn indicator_finds_the_row_it_belongs_to() {
        let rows = rows();
        assert_eq!(
            row_of(&rows, DropTarget::BeforeLayer(2)).map(|(rect, above)| (rect.top(), above)),
            Some((48.0, true))
        );
        assert_eq!(
            row_of(&rows, DropTarget::GroupEnd(7)).map(|(rect, above)| (rect.bottom(), above)),
            Some((20.0, false))
        );
    }
}
