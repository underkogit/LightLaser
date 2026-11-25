use egui::{Align, Layout, Sense};
use rust_i18n::t;

use super::LayerAction;
use crate::core::layer::GroupId;
use crate::ui::LaserApp;
use crate::ui::panels::right_panel::{GroupRename, LayerRowDrag};

const ROW_HEIGHT: f32 = 22.0;
const COUNT_WIDTH: f32 = 34.0;

pub(super) struct LayerRow {
    pub(super) id: u64,
    pub(super) name: String,
    pub(super) visible: bool,
    pub(super) enabled: bool,
    pub(super) selected: bool,
}

pub(super) struct GroupRow {
    pub(super) id: GroupId,
    pub(super) name: String,
    pub(super) collapsed: bool,
    pub(super) visible: bool,
    pub(super) enabled: bool,
    pub(super) selected: bool,
    pub(super) members: Vec<LayerRow>,
}

#[derive(Clone, Copy)]
pub(super) enum RowKind {
    Group(GroupId),
    Layer { id: u64 },
}

#[derive(Clone, Copy)]
pub(super) struct RowEntry {
    pub(super) rect: egui::Rect,
    pub(super) kind: RowKind,
}

impl LaserApp {
    pub(super) fn group_header(
        &self,
        ui: &mut egui::Ui,
        group: &GroupRow,
        shift: bool,
        dragging: bool,
        rename: &mut Option<GroupRename>,
        actions: &mut Vec<LayerAction>,
    ) -> RowEntry {
        let dim = !group.visible;
        let fill = if group.selected {
            ui.style().visuals.selection.bg_fill
        } else {
            ui.style().visuals.faint_bg_color
        };
        let frame = egui::Frame::group(ui.style())
            .fill(fill)
            .inner_margin(egui::Margin::symmetric(6, 4));

        let editing = rename.as_ref().is_some_and(|state| state.group == group.id);
        let mut commit: Option<String> = None;
        let mut cancel = false;

        let response = frame
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let caret = if group.collapsed {
                        egui_phosphor::regular::CARET_RIGHT
                    } else {
                        egui_phosphor::regular::CARET_DOWN
                    };
                    if ui
                        .add(egui::Button::new(caret).frame(false).small())
                        .on_hover_text(t!("ui.tooltip.layer_group_collapse"))
                        .clicked()
                    {
                        actions.push(LayerAction::ToggleCollapsed(group.id));
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(egui_phosphor::regular::TRASH).small())
                            .on_hover_text(t!("ui.tooltip.layer_group_delete"))
                            .clicked()
                        {
                            actions.push(LayerAction::RemoveGroup(group.id));
                        }

                        let mut enabled = group.enabled;
                        if ui
                            .checkbox(&mut enabled, "")
                            .on_hover_text(t!("ui.tooltip.layer_group_enabled"))
                            .changed()
                        {
                            actions.push(LayerAction::SetGroupEnabled(group.id, enabled));
                        }

                        let mut visible = group.visible;
                        if ui
                            .checkbox(&mut visible, "")
                            .on_hover_text(t!("ui.tooltip.layer_group_visible"))
                            .changed()
                        {
                            actions.push(LayerAction::SetGroupVisible(group.id, visible));
                        }

                        let (area, area_response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), ROW_HEIGHT),
                            Sense::click_and_drag(),
                        );
                        paint_count(
                            ui.painter(),
                            area,
                            group.members.len(),
                            ui.style().visuals.weak_text_color(),
                        );

                        if editing {
                            let state = rename.as_mut().expect("editing group");
                            let width = text_rect(area, true).width();
                            let edit = ui.put(
                                text_rect(area, true),
                                egui::TextEdit::singleline(&mut state.buffer).desired_width(width),
                            );
                            if !state.focus_requested {
                                edit.request_focus();
                                state.focus_requested = true;
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                cancel = true;
                            } else if edit.lost_focus() {
                                commit = Some(state.buffer.clone());
                            }
                        } else {
                            let color = if dim {
                                ui.style().visuals.weak_text_color()
                            } else {
                                ui.style().visuals.strong_text_color()
                            };
                            row_text(ui, text_rect(area, true), &group.name, color);

                            if !dragging {
                                if area_response.clicked() {
                                    actions.push(if shift {
                                        LayerAction::ToggleGroup(group.id)
                                    } else {
                                        LayerAction::SelectGroup(group.id)
                                    });
                                }
                                if area_response.double_clicked() {
                                    actions.push(LayerAction::SelectGroup(group.id));
                                    *rename = Some(GroupRename {
                                        group: group.id,
                                        buffer: group.name.clone(),
                                        focus_requested: false,
                                    });
                                }
                            }

                            area_response.context_menu(|ui| {
                                if ui.button(t!("ui.menu.group_select")).clicked() {
                                    actions.push(LayerAction::SelectGroup(group.id));
                                    ui.close();
                                }
                                let all_visible = !group.members.is_empty()
                                    && group.members.iter().all(|member| member.visible);
                                let toggle_label = if all_visible {
                                    t!("ui.menu.group_hide_layers")
                                } else {
                                    t!("ui.menu.group_show_layers")
                                };
                                if ui.button(toggle_label).clicked() {
                                    actions.push(LayerAction::SetGroupLayersVisible(
                                        group.id,
                                        !all_visible,
                                    ));
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button(t!("ui.menu.group_delete")).clicked() {
                                    actions.push(LayerAction::RemoveGroup(group.id));
                                    ui.close();
                                }
                            });
                            area_response.on_hover_text(t!("ui.tip.group_rename").to_string());
                        }
                    });
                });
            })
            .response;

        if cancel {
            *rename = None;
        } else if let Some(name) = commit {
            *rename = None;
            actions.push(LayerAction::RenameGroup(group.id, name));
        }

        RowEntry {
            rect: response.rect,
            kind: RowKind::Group(group.id),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn layer_row(
        &self,
        ui: &mut egui::Ui,
        layer: &LayerRow,
        group: &GroupRow,
        shift: bool,
        dragging: bool,
        drag: &mut Option<LayerRowDrag>,
        actions: &mut Vec<LayerAction>,
    ) -> RowEntry {
        let dim = !group.visible || !layer.visible;
        let frame = egui::Frame::group(ui.style())
            .fill(if layer.selected {
                ui.style().visuals.selection.bg_fill
            } else {
                ui.style().visuals.faint_bg_color
            })
            .inner_margin(egui::Margin::symmetric(6, 3));

        let response = frame
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(egui_phosphor::regular::TRASH).small())
                            .on_hover_text(t!("ui.tooltip.layer_delete"))
                            .clicked()
                        {
                            actions.push(LayerAction::RemoveLayer(layer.id));
                        }

                        let mut enabled = layer.enabled;
                        if ui
                            .checkbox(&mut enabled, "")
                            .on_hover_text(t!("ui.tooltip.layer_enabled"))
                            .changed()
                        {
                            actions.push(LayerAction::SetLayerEnabled(layer.id, enabled));
                        }

                        let mut visible = layer.visible;
                        if ui
                            .checkbox(&mut visible, "")
                            .on_hover_text(t!("ui.tooltip.layer_visible"))
                            .changed()
                        {
                            actions.push(LayerAction::SetLayerVisible(layer.id, visible));
                        }

                        let (area, area_response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), ROW_HEIGHT),
                            Sense::click_and_drag(),
                        );
                        row_text(
                            ui,
                            text_rect(area, false),
                            &layer.name,
                            if dim {
                                ui.style().visuals.weak_text_color()
                            } else {
                                ui.style().visuals.text_color()
                            },
                        );

                        if !dragging && area_response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                        }

                        if !dragging
                            && area_response.hovered()
                            && ui.input(|i| i.pointer.primary_pressed())
                        {
                            let ids = self.drag_ids(layer.id);
                            *drag = Some(LayerRowDrag {
                                label: drag_label(&layer.name, ids.len()),
                                ids,
                                origin: ui.input(|i| i.pointer.interact_pos()).unwrap_or_default(),
                                active: false,
                            });
                        }

                        if !dragging && area_response.clicked() {
                            actions.push(if shift {
                                LayerAction::ToggleLayer(layer.id)
                            } else {
                                LayerAction::SelectLayer(layer.id)
                            });
                        }

                        area_response.on_hover_text(layer.name.clone());
                    });
                });
            })
            .response;

        RowEntry {
            rect: response.rect,
            kind: RowKind::Layer { id: layer.id },
        }
    }
}

pub(super) fn paint_group_rail(ui: &egui::Ui, rows: &[RowEntry]) {
    let (Some(first), Some(last)) = (rows.first(), rows.last()) else {
        return;
    };
    let color = ui.style().visuals.widgets.noninteractive.bg_stroke.color;
    let x = first.rect.left() - 7.0;
    ui.painter().vline(
        x,
        egui::Rangef::new(first.rect.top(), last.rect.bottom()),
        egui::Stroke::new(1.0, color),
    );
}

fn row_text(ui: &egui::Ui, area: egui::Rect, text: &str, color: egui::Color32) {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap.max_width = area.width();
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        egui::pos2(area.left(), area.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
}

fn text_rect(area: egui::Rect, with_count: bool) -> egui::Rect {
    let left = area.left() + 2.0;
    let right = if with_count {
        area.right() - COUNT_WIDTH
    } else {
        area.right()
    };
    egui::Rect::from_min_max(
        egui::pos2(left, area.top()),
        egui::pos2(right.max(left + 24.0), area.bottom()),
    )
}

fn paint_count(painter: &egui::Painter, area: egui::Rect, count: usize, color: egui::Color32) {
    painter.text(
        egui::pos2(area.right() - 2.0, area.center().y),
        egui::Align2::RIGHT_CENTER,
        format!("({count})"),
        egui::FontId::proportional(11.0),
        color,
    );
}

fn drag_label(name: &str, count: usize) -> String {
    if count > 1 {
        format!("{name} +{}", count - 1)
    } else {
        name.to_string()
    }
}
