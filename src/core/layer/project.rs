use rust_i18n::t;
use serde::{Deserialize, Serialize};

use crate::core::config::MachineConfig;
use crate::core::layer::{DropTarget, GroupId, Layer, LayerGroup};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Project {
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub layers: Vec<Layer>,

    #[serde(default)]
    pub groups: Vec<LayerGroup>,

    #[serde(default)]
    pub machine: MachineConfig,
}

pub const PROJECT_VERSION: u32 = 1;

fn default_version() -> u32 {
    PROJECT_VERSION
}

impl Project {
    pub fn new() -> Self {
        Self {
            version: PROJECT_VERSION,
            name: String::new(),
            layers: Vec::new(),
            groups: Vec::new(),
            machine: MachineConfig::default(),
        }
    }

    fn next_id(&self) -> u64 {
        let layers = self.layers.iter().map(|l| l.id).max().unwrap_or(0);
        let groups = self.groups.iter().map(|g| g.id).max().unwrap_or(0);
        layers.max(groups) + 1
    }

    pub fn layer_index(&self, id: u64) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub fn group(&self, id: GroupId) -> Option<&LayerGroup> {
        self.groups.iter().find(|g| g.id == id)
    }

    pub fn group_mut(&mut self, id: GroupId) -> Option<&mut LayerGroup> {
        self.groups.iter_mut().find(|g| g.id == id)
    }

    pub fn group_members(&self, id: GroupId) -> Vec<usize> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, l)| l.group == id)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn group_order(&self) -> Vec<GroupId> {
        let mut order: Vec<GroupId> = Vec::new();
        for layer in &self.layers {
            if self.group(layer.group).is_some() && !order.contains(&layer.group) {
                order.push(layer.group);
            }
        }
        order
    }

    pub fn group_block(&self, id: GroupId) -> Option<(usize, usize)> {
        let start = self.layers.iter().position(|l| l.group == id)?;
        let end = self.layers[start..]
            .iter()
            .position(|l| l.group != id)
            .map(|pos| start + pos)
            .unwrap_or(self.layers.len());
        Some((start, end))
    }

    pub fn group_visible(&self, id: GroupId) -> bool {
        self.layers
            .iter()
            .filter(|l| l.group == id)
            .all(|l| l.is_visible)
    }

    pub fn group_enabled(&self, id: GroupId) -> bool {
        self.layers
            .iter()
            .filter(|l| l.group == id)
            .all(|l| l.is_enabled)
    }

    fn auto_group_name(&self) -> String {
        let translated = t!("ui.layer_group.name").to_string();
        let base = if translated.starts_with("ui.") {
            "Group".to_string()
        } else {
            translated
        };
        for n in 1..=9999 {
            let name = format!("{base} {n}");
            if !self.groups.iter().any(|g| g.name == name) {
                return name;
            }
        }
        format!("{base} {}", self.groups.len() + 1)
    }

    pub fn add_group(&mut self, name: String, color: [u8; 3]) -> GroupId {
        let id = self.next_id();
        self.groups.push(LayerGroup::new(id, name, color));
        id
    }

    fn ensure_group(&mut self, requested: GroupId, color: [u8; 3]) -> GroupId {
        if self.group(requested).is_some() {
            return requested;
        }
        let name = self.auto_group_name();
        self.add_group(name, color)
    }

    pub fn add_layer(&mut self, mut layer: Layer) -> usize {
        if layer.id == 0 {
            layer.id = self.next_id();
        }
        let group = self.ensure_group(layer.group, layer.color);
        layer.group = group;
        self.insert_layer(layer, group)
    }

    pub fn add_layer_to_group(&mut self, mut layer: Layer, group: GroupId) -> usize {
        if layer.id == 0 {
            layer.id = self.next_id();
        }
        let group = self.ensure_group(group, layer.color);
        layer.group = group;
        self.insert_layer(layer, group)
    }

    fn insert_layer(&mut self, layer: Layer, group: GroupId) -> usize {
        let index = self
            .group_block(group)
            .map(|(_, end)| end)
            .unwrap_or(self.layers.len());
        self.layers.insert(index, layer);
        index
    }

    pub fn remove_layer(&mut self, index: usize) {
        if index >= self.layers.len() {
            return;
        }
        let group = self.layers.remove(index).group;
        if !self.layers.iter().any(|l| l.group == group) {
            self.groups.retain(|g| g.id != group);
        }
    }

    pub fn remove_group(&mut self, id: GroupId) {
        self.layers.retain(|l| l.group != id);
        self.groups.retain(|g| g.id != id);
    }

    pub fn move_layer_up(&mut self, index: usize) {
        let Some(layer) = self.layers.get(index) else {
            return;
        };
        let Some((start, _)) = self.group_block(layer.group) else {
            return;
        };
        if index > start {
            self.layers.swap(index, index - 1);
        }
    }

    pub fn move_layer_down(&mut self, index: usize) {
        let Some(layer) = self.layers.get(index) else {
            return;
        };
        let Some((_, end)) = self.group_block(layer.group) else {
            return;
        };
        if index + 1 < end {
            self.layers.swap(index, index + 1);
        }
    }

    pub fn rename_group(&mut self, id: GroupId, name: String) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        if let Some(group) = self.group_mut(id) {
            group.name = name;
        }
    }

    pub fn set_group_collapsed(&mut self, id: GroupId, collapsed: bool) {
        if let Some(group) = self.group_mut(id) {
            group.collapsed = collapsed;
        }
    }

    pub fn toggle_group_collapsed(&mut self, id: GroupId) {
        if let Some(group) = self.group_mut(id) {
            group.collapsed = !group.collapsed;
        }
    }

    pub fn set_group_visible(&mut self, id: GroupId, visible: bool) {
        for layer in self.layers.iter_mut().filter(|l| l.group == id) {
            layer.is_visible = visible;
        }
    }

    pub fn set_group_enabled(&mut self, id: GroupId, enabled: bool) {
        for layer in self.layers.iter_mut().filter(|l| l.group == id) {
            layer.is_enabled = enabled;
        }
    }

    pub fn move_layers_to(&mut self, ids: &[u64], target: DropTarget) -> bool {
        let resolved = match target {
            DropTarget::GroupStart(group) => {
                self.group_block(group).map(|(start, _)| (group, 0, start))
            }
            DropTarget::GroupEnd(group) => self
                .group_block(group)
                .map(|(start, end)| (group, end - start, start)),
            DropTarget::BeforeLayer(id) => self
                .layer_index(id)
                .and_then(|index| self.anchor(index)),
            DropTarget::AfterLayer(id) => self
                .layer_index(id)
                .and_then(|index| self.anchor(index))
                .map(|(group, offset, start)| (group, offset + 1, start)),
        };
        let Some((target_group, offset, block_start)) = resolved else {
            return false;
        };

        let mut moving: Vec<usize> = Vec::new();
        for id in ids {
            if let Some(index) = self.layer_index(*id) {
                if !moving.contains(&index) {
                    moving.push(index);
                }
            }
        }
        moving.sort_unstable();
        if moving.is_empty() {
            return false;
        }

        let before: Vec<(u64, GroupId)> = self.layers.iter().map(|l| (l.id, l.group)).collect();

        let absolute = block_start + offset;
        let shifted = absolute - moving.iter().filter(|&&index| index < absolute).count();

        let mut extracted: Vec<Layer> = Vec::with_capacity(moving.len());
        for index in moving.iter().rev() {
            extracted.push(self.layers.remove(*index));
        }
        extracted.reverse();

        let mut at = shifted.min(self.layers.len());
        for mut layer in extracted {
            layer.group = target_group;
            self.layers.insert(at, layer);
            at += 1;
        }

        self.compact_groups();

        let after: Vec<(u64, GroupId)> = self.layers.iter().map(|l| (l.id, l.group)).collect();
        before != after
    }

    fn anchor(&self, index: usize) -> Option<(GroupId, usize, usize)> {
        let group = self.layers.get(index)?.group;
        let (start, _) = self.group_block(group)?;
        Some((group, index - start, start))
    }

    pub fn normalize(&mut self) {
        self.normalize_ids();
        self.normalize_groups();
        self.compact_groups();
    }

    fn normalize_ids(&mut self) {
        let mut next = self.next_id();
        let mut seen: Vec<u64> = Vec::new();
        for layer in &mut self.layers {
            if layer.id == 0 || seen.contains(&layer.id) {
                layer.id = next;
                next += 1;
            }
            seen.push(layer.id);
        }

        let mut next = self.next_id();
        let mut seen: Vec<GroupId> = Vec::new();
        for group in &mut self.groups {
            if group.id == 0 || seen.contains(&group.id) {
                group.id = next;
                next += 1;
            }
            seen.push(group.id);
        }
    }

    fn normalize_groups(&mut self) {
        let used: Vec<GroupId> = self.layers.iter().map(|l| l.group).collect();
        self.groups.retain(|group| used.contains(&group.id));

        let orphan: Option<usize> = self
            .layers
            .iter()
            .position(|l| self.group(l.group).is_none());
        let Some(first) = orphan else {
            return;
        };
        let color = self.layers[first].color;
        let name = self.auto_group_name();
        let id = self.add_group(name, color);
        let known: Vec<GroupId> = self.groups.iter().map(|g| g.id).collect();
        for layer in self.layers.iter_mut() {
            if !known.contains(&layer.group) {
                layer.group = id;
            }
        }
    }

    fn compact_groups(&mut self) {
        let mut order: Vec<GroupId> = Vec::new();
        for layer in &self.layers {
            if !order.contains(&layer.group) {
                order.push(layer.group);
            }
        }

        let mut buckets: Vec<(GroupId, Vec<Layer>)> =
            order.into_iter().map(|id| (id, Vec::new())).collect();
        for layer in self.layers.drain(..) {
            if let Some(bucket) = buckets.iter_mut().find(|(id, _)| *id == layer.group) {
                bucket.1.push(layer);
            }
        }

        self.layers = buckets.into_iter().flat_map(|(_, layers)| layers).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layer::LayerSource;

    fn layer(name: &str) -> Layer {
        Layer::new(name.to_string(), [10, 20, 30], LayerSource::Vector)
    }

    fn names(project: &Project) -> Vec<String> {
        project.layers.iter().map(|l| l.name.clone()).collect()
    }

    fn blocks_are_contiguous(project: &Project) -> bool {
        let mut seen: Vec<GroupId> = Vec::new();
        let mut current: Option<GroupId> = None;
        for layer in &project.layers {
            if current == Some(layer.group) {
                continue;
            }
            if seen.contains(&layer.group) {
                return false;
            }
            if let Some(previous) = current {
                seen.push(previous);
            }
            current = Some(layer.group);
        }
        true
    }

    #[test]
    fn first_layer_creates_group() {
        let mut project = Project::new();
        project.add_layer(layer("a"));
        project.add_layer(layer("b"));

        assert_eq!(project.groups.len(), 2);
        assert_ne!(project.layers[0].group, project.layers[1].group);
        assert!(project.layers.iter().all(|l| l.id != 0));
        assert!(blocks_are_contiguous(&project));
    }

    #[test]
    fn layers_of_one_group_stay_together() {
        let mut project = Project::new();
        let first = project.add_layer(layer("a"));
        let group = project.layers[first].group;
        project.add_layer_to_group(layer("b"), group);
        project.add_layer(layer("c"));

        assert_eq!(names(&project), vec!["a", "b", "c"]);
        assert_eq!(project.groups.len(), 2);
        assert!(blocks_are_contiguous(&project));
    }

    #[test]
    fn removing_last_member_drops_group() {
        let mut project = Project::new();
        project.add_layer(layer("a"));
        project.add_layer(layer("b"));

        project.remove_layer(0);

        assert_eq!(project.groups.len(), 1);
        assert_eq!(project.layers.len(), 1);
    }

    #[test]
    fn moving_layer_to_other_group_end() {
        let mut project = Project::new();
        project.add_layer(layer("a"));
        project.add_layer(layer("b"));
        project.add_layer(layer("c"));

        let target = project.layers[0].group;
        let moved = project.layers[2].id;
        let changed = project.move_layers_to(&[moved], DropTarget::GroupEnd(target));

        assert!(changed);
        assert_eq!(names(&project), vec!["a", "c", "b"]);
        assert_eq!(project.layers[1].group, target);
        assert!(blocks_are_contiguous(&project));
    }

    #[test]
    fn moving_layer_before_another_keeps_group_of_target() {
        let mut project = Project::new();
        project.add_layer(layer("a"));
        project.add_layer(layer("b"));
        project.add_layer(layer("c"));

        let target_group = project.layers[2].group;
        let anchor = project.layers[2].id;
        let moved = project.layers[0].id;
        let changed = project.move_layers_to(&[moved], DropTarget::BeforeLayer(anchor));

        assert!(changed);
        assert_eq!(names(&project), vec!["b", "a", "c"]);
        assert_eq!(project.layers[1].group, target_group);
    }

    #[test]
    fn drop_in_place_reports_no_change() {
        let mut project = Project::new();
        project.add_layer(layer("a"));
        project.add_layer(layer("b"));

        let id = project.layers[0].id;
        assert!(!project.move_layers_to(&[id], DropTarget::BeforeLayer(id)));
    }

    #[test]
    fn multi_drag_keeps_relative_order() {
        let mut project = Project::new();
        for name in ["a", "b", "c", "d"] {
            project.add_layer(layer(name));
        }

        let ids: Vec<u64> = vec![project.layers[0].id, project.layers[2].id];
        let target = project.layers[3].group;
        project.move_layers_to(&ids, DropTarget::GroupEnd(target));

        assert_eq!(names(&project), vec!["b", "d", "a", "c"]);
        assert!(blocks_are_contiguous(&project));
    }

    #[test]
    fn move_within_group_stops_at_boundary() {
        let mut project = Project::new();
        let first = project.add_layer(layer("a"));
        let group = project.layers[first].group;
        project.add_layer_to_group(layer("b"), group);
        project.add_layer(layer("c"));

        project.move_layer_up(0);
        assert_eq!(names(&project), vec!["a", "b", "c"]);

        project.move_layer_down(1);
        assert_eq!(names(&project), vec!["a", "b", "c"]);

        project.move_layer_down(0);
        assert_eq!(names(&project), vec!["b", "a", "c"]);
    }

    #[test]
    fn normalize_assigns_orphan_layers_to_one_group() {
        let mut project = Project::new();
        project.layers.push(layer("a"));
        project.layers.push(layer("b"));

        project.normalize();

        assert_eq!(project.groups.len(), 1);
        assert!(project.layers.iter().all(|l| l.group == project.groups[0].id));
        assert!(project.layers.iter().all(|l| l.id != 0));
    }

    #[test]
    fn normalize_restores_contiguity() {
        let mut project = Project::new();
        let a = project.add_layer(layer("a"));
        let first = project.layers[a].group;
        project.add_layer_to_group(layer("b"), first);
        let c = project.add_layer(layer("c"));
        let other = project.layers[c].group;

        project.layers.swap(1, 2);
        assert!(!blocks_are_contiguous(&project));

        project.normalize();

        assert!(blocks_are_contiguous(&project));
        assert_eq!(project.groups.len(), 2);
        assert!(project.groups.iter().any(|g| g.id == first));
        assert!(project.groups.iter().any(|g| g.id == other));
    }

    #[test]
    fn group_toggle_visible_and_enabled_reaches_members() {
        let mut project = Project::new();
        let a = project.add_layer(layer("a"));
        let group = project.layers[a].group;
        project.add_layer_to_group(layer("b"), group);

        project.set_group_visible(group, false);
        project.set_group_enabled(group, false);

        assert!(!project.group_visible(group));
        assert!(!project.group_enabled(group));
        assert!(project.layers.iter().all(|l| !l.is_visible && !l.is_enabled));
    }

    #[test]
    fn groups_survive_serialization() {
        let mut project = Project::new();
        project.add_layer(layer("a"));
        project.add_layer(layer("b"));
        project.rename_group(project.groups[0].id, "Engrave".to_string());

        let text = toml::to_string(&project).unwrap();
        let mut restored: Project = toml::from_str(&text).unwrap();
        restored.normalize();

        assert_eq!(restored.groups.len(), 2);
        assert_eq!(restored.layers.len(), 2);
        assert_eq!(restored.layers[0].group, restored.groups[0].id);
        assert_eq!(restored.layers[1].group, restored.groups[1].id);
    }

    #[test]
    fn legacy_project_without_groups_is_upgraded() {
        let mut project = Project::new();
        project.layers.push(layer("a"));
        project.layers.push(layer("b"));
        let text = toml::to_string(&project).unwrap();

        let mut restored: Project = toml::from_str(&text).unwrap();
        restored.normalize();

        assert_eq!(restored.groups.len(), 1);
        assert!(restored.layers.iter().all(|l| l.group == restored.groups[0].id));
    }
}
