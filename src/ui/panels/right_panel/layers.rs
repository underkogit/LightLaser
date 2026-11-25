use egui::{Align, Layout, Sense};
use rust_i18n::t;

use super::GroupRename;
use crate::core::layer::{DropTarget, GroupId};
use crate::ui::LaserApp;
use crate::ui::selection::Selection;

#[derive(Clone, Copy)]
struct DragLayer {
    layer_id: u64,
}

struct LayerRow {
    id: u64,
    name: String,
    color: [u8; 3],
    visible: bool,
    enabled: bool,
    selected: bool,
}

struct GroupRow {
    id: GroupId,
    name: String,
    color: [u8; 3],
    collapsed: bool,
    visible: bool,
    enabled: bool,
    selected: bool,
    members: Vec<LayerRow>,
}

enum LayerAction {
    SelectLayer(u64),
    ToggleLayer(u64),
    SelectGroup(GroupId),
    ToggleGroup(GroupId),
    ToggleCollapsed(GroupId),
    RenameGroup(GroupId, String),
    SetGroupVisible(GroupId, bool),
    SetGroupEnabled(GroupId, bool),
    RemoveGroup(GroupId),
    RemoveLayer(u64),
    SetLayerVisible(u64, bool),
    SetLayerEnabled(u64, bool),
    MoveUp(u64),
    MoveDown(u64),
    Drop { ids: Vec<u64>, target: DropTarget },
}

impl LaserApp {
    pub(crate) fn ui_layer_list(&mut self, ui: &mut egui::Ui) {
        let tree = self.layer_tree();
        let mut rename = self.right_panel_state.group_rename.take();
        let mut actions: Vec<LayerAction> = Vec::new();
        let shift = ui.input(|i| i.modifiers.shift);

        if tree.is_empty() {
            ui.label(
                egui::RichText::new(t!("ui.label.no_layers").to_string())
                    .italics()
                    .weak(),
            );
        }

        for group in &tree {
            self.group_header(ui, group, shift, &mut rename, &mut actions);
            if group.collapsed {
                ui.add_space(4.0);
                continue;
            }
            for layer in &group.members {
                self.layer_row(ui, layer, shift, &mut actions);
                ui.add_space(4.0);
            }
        }

        self.right_panel_state.group_rename = rename;
        self.apply_layer_actions(actions);
    }

    fn layer_tree(&self) -> Vec<GroupRow> {
        let selection = &self.ui_state.selection;
        self.project
            .group_order()
            .into_iter()
            .filter_map(|id| {
                let group = self.project.group(id)?;
                let members: Vec<LayerRow> = self
                    .project
                    .group_members(id)
                    .into_iter()
                    .filter_map(|index| {
                        let layer = self.project.layers.get(index)?;
                        Some(LayerRow {
                            id: layer.id,
                            name: layer.name.clone(),
                            color: layer.color,
                            visible: layer.is_visible,
                            enabled: layer.is_enabled,
                            selected: selection.is_layer_selected(index),
                        })
                    })
                    .collect();
                let selected = !members.is_empty() && members.iter().all(|m| m.selected);
                Some(GroupRow {
                    id,
                    name: group.name.clone(),
                    color: group.color,
                    collapsed: group.collapsed,
                    visible: self.project.group_visible(id),
                    enabled: self.project.group_enabled(id),
                    selected,
                    members,
                })
            })
            .collect()
    }

    fn group_header(
        &self,
        ui: &mut egui::Ui,
        group: &GroupRow,
        shift: bool,
        rename: &mut Option<GroupRename>,
        actions: &mut Vec<LayerAction>,
    ) {
        let fill = if group.selected {
            ui.style().visuals.selection.bg_fill
        } else {
            ui.style().visuals.faint_bg_color
        };
        let frame = egui::Frame::group(ui.style())
            .fill(fill)
            .inner_margin(egui::Margin::symmetric(6, 4));

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

                    swatch(ui, group.color);

                    let editing = rename
                        .as_ref()
                        .is_some_and(|state| state.group == group.id);
                    if editing {
                        let state = rename.as_mut().expect("checked above");
                        let edit = ui.add(
                            egui::TextEdit::singleline(&mut state.buffer).desired_width(140.0),
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
                        let label = ui.add(
                            egui::Label::new(egui::RichText::new(&group.name).strong())
                                .sense(Sense::click())
                                .truncate(),
                        );
                        if label.clicked() {
                            actions.push(if shift {
                                LayerAction::ToggleGroup(group.id)
                            } else {
                                LayerAction::SelectGroup(group.id)
                            });
                        }
                        if label.double_clicked() {
                            actions.push(LayerAction::SelectGroup(group.id));
                            *rename = Some(GroupRename {
                                group: group.id,
                                buffer: group.name.clone(),
                                focus_requested: false,
                            });
                        }
                        label.on_hover_text(t!("ui.tip.group_rename").to_string());
                    }

                    ui.label(
                        egui::RichText::new(format!("({})", group.members.len()))
                            .weak()
                            .small(),
                    );

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

        if response.dnd_hover_payload::<DragLayer>().is_some() {
            ui.painter().rect_stroke(
                response.rect,
                4.0,
                egui::Stroke::new(1.5, ui.visuals().selection.stroke.color),
                egui::StrokeKind::Inside,
            );
        }
        if let Some(payload) = response.dnd_release_payload::<DragLayer>() {
            actions.push(LayerAction::Drop {
                ids: self.drag_ids(payload.layer_id),
                target: DropTarget::GroupEnd(group.id),
            });
        }
    }

    fn layer_row(
        &self,
        ui: &mut egui::Ui,
        layer: &LayerRow,
        shift: bool,
        actions: &mut Vec<LayerAction>,
    ) {
        let frame = egui::Frame::group(ui.style())
            .fill(if layer.selected {
                ui.style().visuals.selection.bg_fill
            } else {
                ui.style().visuals.faint_bg_color
            })
            .inner_margin(egui::Margin::symmetric(6, 4));

        let response = frame
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    swatch(ui, layer.color);

                    let name = ui.add(
                        egui::Label::new(egui::RichText::new(&layer.name))
                            .sense(Sense::click_and_drag())
                            .truncate(),
                    );
                    if name.drag_started() {
                        name.dnd_set_drag_payload(DragLayer {
                            layer_id: layer.id,
                        });
                    }
                    if name.clicked() {
                        actions.push(if shift {
                            LayerAction::ToggleLayer(layer.id)
                        } else {
                            LayerAction::SelectLayer(layer.id)
                        });
                    }
                    name.on_hover_text(layer.name.clone());

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(egui_phosphor::regular::TRASH).small())
                            .clicked()
                        {
                            actions.push(LayerAction::RemoveLayer(layer.id));
                        }
                        if ui
                            .add(
                                egui::Button::new(egui_phosphor::regular::ARROW_DOWN).small(),
                            )
                            .clicked()
                        {
                            actions.push(LayerAction::MoveDown(layer.id));
                        }
                        if ui
                            .add(egui::Button::new(egui_phosphor::regular::ARROW_UP).small())
                            .clicked()
                        {
                            actions.push(LayerAction::MoveUp(layer.id));
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
                    });
                });
            })
            .response;

        let drop_before = response.dnd_hover_payload::<DragLayer>().map(|_| {
            let pointer = ui
                .input(|i| i.pointer.latest_pos())
                .unwrap_or_else(|| response.rect.center());
            pointer.y < response.rect.center().y
        });

        if let Some(before) = drop_before {
            let y = if before {
                response.rect.top()
            } else {
                response.rect.bottom()
            };
            ui.painter().hline(
                response.rect.x_range(),
                y,
                egui::Stroke::new(2.0, ui.visuals().selection.stroke.color),
            );
        }

        if let Some(payload) = response.dnd_release_payload::<DragLayer>() {
            let target = if drop_before.unwrap_or(true) {
                DropTarget::BeforeLayer(layer.id)
            } else {
                DropTarget::AfterLayer(layer.id)
            };
            actions.push(LayerAction::Drop {
                ids: self.drag_ids(payload.layer_id),
                target,
            });
        }
    }

    fn drag_ids(&self, dragged: u64) -> Vec<u64> {
        let selected = self.selected_layer_ids();
        if selected.contains(&dragged) {
            selected
        } else {
            vec![dragged]
        }
    }

    fn selected_layer_ids(&self) -> Vec<u64> {
        let mut indices = self.ui_state.selection.layers.clone();
        if let Some(index) = self.ui_state.selection.layer {
            if !indices.contains(&index) {
                indices.push(index);
            }
        }
        indices.sort_unstable();
        indices
            .into_iter()
            .filter_map(|index| self.project.layers.get(index))
            .map(|layer| layer.id)
            .collect()
    }

    fn select_layer_ids(&mut self, ids: &[u64]) {
        let mut indices: Vec<usize> = ids
            .iter()
            .filter_map(|id| self.project.layer_index(*id))
            .collect();
        indices.sort_unstable();
        indices.dedup();

        self.ui_state.selection = Selection {
            layers: indices,
            layer: None,
            shape: None,
            node: None,
            handle: None,
        };
        self.ui_state.selection.layer = self.ui_state.selection.layers.last().copied();
    }

    fn toggle_layer_ids(&mut self, ids: &[u64]) {
        let mut current = self.selected_layer_ids();
        let every = !ids.is_empty() && ids.iter().all(|id| current.contains(id));
        if every {
            current.retain(|id| !ids.contains(id));
        } else {
            for id in ids {
                if !current.contains(id) {
                    current.push(*id);
                }
            }
        }
        self.select_layer_ids(&current);
    }

    fn group_layer_ids(&self, group: GroupId) -> Vec<u64> {
        self.project
            .group_members(group)
            .into_iter()
            .filter_map(|index| self.project.layers.get(index))
            .map(|layer| layer.id)
            .collect()
    }

    fn apply_layer_actions(&mut self, actions: Vec<LayerAction>) {
        for action in actions {
            match action {
                LayerAction::SelectLayer(id) => self.select_layer_ids(&[id]),
                LayerAction::ToggleLayer(id) => self.toggle_layer_ids(&[id]),
                LayerAction::SelectGroup(group) => {
                    let ids = self.group_layer_ids(group);
                    self.select_layer_ids(&ids);
                }
                LayerAction::ToggleGroup(group) => {
                    let ids = self.group_layer_ids(group);
                    self.toggle_layer_ids(&ids);
                }
                LayerAction::ToggleCollapsed(group) => {
                    self.project.toggle_group_collapsed(group);
                }
                LayerAction::RenameGroup(group, name) => {
                    self.push_history();
                    self.project.rename_group(group, name);
                }
                LayerAction::SetGroupVisible(group, visible) => {
                    self.push_history();
                    self.project.set_group_visible(group, visible);
                }
                LayerAction::SetGroupEnabled(group, enabled) => {
                    self.push_history();
                    self.project.set_group_enabled(group, enabled);
                }
                LayerAction::RemoveGroup(group) => {
                    let removed = self.group_layer_ids(group);
                    let mut kept = self.selected_layer_ids();
                    kept.retain(|id| !removed.contains(id));
                    self.push_history();
                    self.project.remove_group(group);
                    self.select_layer_ids(&kept);
                }
                LayerAction::RemoveLayer(id) => {
                    let mut kept = self.selected_layer_ids();
                    kept.retain(|kept| *kept != id);
                    let Some(index) = self.project.layer_index(id) else {
                        continue;
                    };
                    self.push_history();
                    self.project.remove_layer(index);
                    self.select_layer_ids(&kept);
                }
                LayerAction::SetLayerVisible(id, visible) => {
                    if let Some(index) = self.project.layer_index(id) {
                        self.project.layers[index].is_visible = visible;
                    }
                }
                LayerAction::SetLayerEnabled(id, enabled) => {
                    if let Some(index) = self.project.layer_index(id) {
                        self.project.layers[index].is_enabled = enabled;
                    }
                }
                LayerAction::MoveUp(id) => {
                    let Some(index) = self.project.layer_index(id) else {
                        continue;
                    };
                    self.push_history();
                    self.project.move_layer_up(index);
                    self.select_layer_ids(&[id]);
                }
                LayerAction::MoveDown(id) => {
                    let Some(index) = self.project.layer_index(id) else {
                        continue;
                    };
                    self.push_history();
                    self.project.move_layer_down(index);
                    self.select_layer_ids(&[id]);
                }
                LayerAction::Drop { ids, target } => {
                    let snapshot = self.project.clone();
                    if self.project.move_layers_to(&ids, target) {
                        self.history.push_owned(snapshot);
                        self.select_layer_ids(&ids);
                    }
                }
            }
        }
    }
}

fn swatch(ui: &mut egui::Ui, color: [u8; 3]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), Sense::hover());
    ui.painter().rect_filled(
        rect,
        2.0,
        egui::Color32::from_rgb(color[0], color[1], color[2]),
    );
}
