mod drop;
mod row;

use egui::Sense;
use rust_i18n::t;

use super::GroupRename;
use crate::core::layer::{DropTarget, GroupId};
use crate::ui::LaserApp;
use crate::ui::selection::Selection;

use drop::{drop_target_at, paint_drag_ghost, paint_drop_indicator};
use row::{paint_group_rail, GroupRow, LayerRow, RowEntry};

const DRAG_THRESHOLD: f32 = 6.0;

pub(super) enum LayerAction {
    SelectLayer(u64),
    ToggleLayer(u64),
    SelectGroup(GroupId),
    ToggleGroup(GroupId),
    ToggleCollapsed(GroupId),
    RenameGroup(GroupId, String),
    SetGroupVisible(GroupId, bool),
    SetGroupEnabled(GroupId, bool),
    SetGroupLayersVisible(GroupId, bool),
    DuplicateGroup(GroupId),
    RemoveGroup(GroupId),
    RemoveLayer(u64),
    ReplaceLayer(u64),
    SetLayerVisible(u64, bool),
    SetLayerEnabled(u64, bool),
    CreateGroup,
    CreateLayer,
    ClearSelection,
    Drop { ids: Vec<u64>, target: DropTarget },
}

impl LaserApp {
    pub(crate) fn ui_layer_list(&mut self, ui: &mut egui::Ui) {
        let tree = self.layer_tree();
        let mut rename = self.right_panel_state.group_rename.take();
        let mut drag = self.right_panel_state.layer_drag.take();
        let mut actions: Vec<LayerAction> = Vec::new();
        let shift = ui.input(|i| i.modifiers.shift);
        let dragging = drag.as_ref().is_some_and(|state| state.active);
        let mut rows: Vec<RowEntry> = Vec::new();

        ui.horizontal(|ui| {
            ui.menu_button(
                format!(
                    "{} {} {}",
                    egui_phosphor::regular::PLUS,
                    t!("ui.menu.create"),
                    egui_phosphor::regular::CARET_DOWN
                ),
                |ui| {
                    if ui.button(t!("ui.menu.create_group")).clicked() {
                        actions.push(LayerAction::CreateGroup);
                        ui.close();
                    }
                    if ui.button(t!("ui.menu.create_layer")).clicked() {
                        actions.push(LayerAction::CreateLayer);
                        ui.close();
                    }
                },
            );
        });
        ui.add_space(4.0);

        if tree.is_empty() {
            ui.label(
                egui::RichText::new(t!("ui.label.no_layers").to_string())
                    .italics()
                    .weak(),
            );
            ui.add_space(4.0);
        }

        for group in &tree {
            rows.push(self.group_header(ui, group, shift, dragging, &mut rename, &mut actions));

            if group.collapsed {
                ui.add_space(6.0);
                continue;
            }

            let first_member = rows.len();
            ui.indent(("layer_group_body", group.id), |ui| {
                for layer in &group.members {
                    rows.push(self.layer_row(
                        ui,
                        layer,
                        group,
                        shift,
                        dragging,
                        &mut drag,
                        &mut actions,
                    ));
                    ui.add_space(3.0);
                }
            });
            paint_group_rail(ui, &rows[first_member..]);
            ui.add_space(6.0);
        }

        let leftover = (ui.clip_rect().bottom() - ui.next_widget_position().y - 2.0).max(24.0);
        let empty = ui.allocate_response(egui::vec2(ui.available_width(), leftover), Sense::click());
        if empty.clicked() {
            actions.push(LayerAction::ClearSelection);
        }
        empty.context_menu(|ui| {
            if ui.button(t!("ui.menu.group_new")).clicked() {
                actions.push(LayerAction::CreateGroup);
                ui.close();
            }
        });

        let pointer = ui.input(|i| i.pointer.interact_pos().or_else(|| i.pointer.latest_pos()));
        let target = match (&drag, pointer) {
            (Some(state), Some(pos)) if state.active => drop_target_at(&rows, pos),
            _ => None,
        };

        if let Some(target) = target {
            paint_drop_indicator(ui, &rows, target);
        }

        if let Some(state) = drag.as_ref().filter(|state| state.active) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(pos) = pointer {
                paint_drag_ghost(ui, pos, &state.label);
            }
        }

        if let Some(state) = drag.as_mut() {
            match pointer {
                Some(pos) if ui.input(|i| i.pointer.primary_down()) => {
                    if !state.active && pos.distance(state.origin) >= DRAG_THRESHOLD {
                        state.active = true;
                    }
                }
                _ => {
                    if let Some(target) = target {
                        actions.push(LayerAction::Drop {
                            ids: state.ids.clone(),
                            target,
                        });
                    }
                    drag = None;
                }
            }
        }

        self.right_panel_state.group_rename = rename;
        self.right_panel_state.layer_drag = drag;
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
                            source: layer.source.clone(),
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
                    collapsed: group.collapsed,
                    visible: group.visible,
                    enabled: group.enabled,
                    selected,
                    members,
                })
            })
            .collect()
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
                LayerAction::SetGroupLayersVisible(group, visible) => {
                    self.push_history();
                    self.project.set_group_members_visible(group, visible);
                }
                LayerAction::DuplicateGroup(group) => {
                    self.push_history();
                    let step = self.duplicate_offset();
                    if let Some(copy) = self.project.duplicate_group(group) {
                        for index in self.project.group_members(copy) {
                            if let Some(layer) = self.project.layers.get_mut(index) {
                                layer.translate(step, step);
                            }
                        }
                        let ids = self.group_layer_ids(copy);
                        self.select_layer_ids(&ids);
                    }
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
                LayerAction::ReplaceLayer(id) => {
                    self.replace_layer_interactive(id);
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
                LayerAction::CreateGroup => {
                    let selected = self.selected_layer_ids();
                    self.push_history();
                    let group = self.project.create_group();
                    if !selected.is_empty() {
                        self.project.move_layers_to(&selected, DropTarget::GroupEnd(group));
                    }
                    let buffer = self
                        .project
                        .group(group)
                        .map(|group| group.name.clone())
                        .unwrap_or_default();
                    self.right_panel_state.group_rename = Some(GroupRename {
                        group,
                        buffer,
                        focus_requested: false,
                    });
                }
                LayerAction::CreateLayer => {
                    let group = self
                        .selected_layer_ids()
                        .first()
                        .and_then(|id| self.project.layer_index(*id))
                        .map(|index| self.project.layers[index].group)
                        .or_else(|| self.project.group_order().last().copied());
                    self.push_history();
                    let index = self.project.create_layer(group);
                    let id = self.project.layers[index].id;
                    self.select_layer_ids(&[id]);
                }
                LayerAction::ClearSelection => {
                    self.ui_state.selection.clear();
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
