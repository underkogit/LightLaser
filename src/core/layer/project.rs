use rust_i18n::t;
use serde::{Deserialize, Serialize};

use crate::core::config::MachineConfig;
use crate::core::layer::{DropTarget, GroupId, Layer, LayerGroup, LayerSource};

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

fn copy_suffix() -> String {
    let translated = t!("ui.layer_group.copy_suffix").to_string();
    if translated.starts_with("ui.") {
        "copy".to_string()
    } else {
        translated
    }
}

fn strip_copy_marker(name: &str, suffix: &str) -> Option<String> {
    let (head, tail) = name.rsplit_once(&format!(" {suffix}"))?;
    let head = head.trim_end();
    if head.is_empty() {
        return None;
    }
    let tail = tail.trim();
    if tail.is_empty() || tail.chars().all(|c| c.is_ascii_digit()) {
        Some(head.to_string())
    } else {
        None
    }
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

    pub fn group_order(&self) -> Vec<GroupId> {
        self.groups.iter().map(|group| group.id).collect()
    }

    pub fn group_members(&self, id: GroupId) -> Vec<usize> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, l)| l.group == id)
            .map(|(i, _)| i)
            .collect()
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
        self.group(id).is_none_or(|group| group.visible)
    }

    pub fn group_enabled(&self, id: GroupId) -> bool {
        self.group(id).is_none_or(|group| group.enabled)
    }

    pub fn layer_visible(&self, layer: &Layer) -> bool {
        layer.is_visible && self.group_visible(layer.group)
    }

    pub fn layer_enabled(&self, layer: &Layer) -> bool {
        layer.is_enabled && self.group_enabled(layer.group)
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

    fn auto_layer_name(&self) -> String {
        let translated = t!("ui.label.layer").to_string();
        let base = if translated.starts_with("ui.") {
            "Layer".to_string()
        } else {
            translated
        };
        for n in 1..=9999 {
            let name = format!("{base} {n}");
            if !self.layers.iter().any(|l| l.name == name) {
                return name;
            }
        }
        format!("{base} {}", self.layers.len() + 1)
    }

    pub fn add_group(&mut self, name: String) -> GroupId {
        let id = self.next_id();
        self.groups.push(LayerGroup::new(id, name));
        id
    }

    pub fn create_group(&mut self) -> GroupId {
        let name = self.auto_group_name();
        self.add_group(name)
    }

    fn unique_group_name(&self, base: &str) -> String {
        let base = base.trim();
        let suffix = copy_suffix();
        let root = strip_copy_marker(base, &suffix).unwrap_or_else(|| base.to_string());
        let root = if root.is_empty() {
            self.auto_group_name()
        } else {
            root
        };
        let first = format!("{root} {suffix}");
        if !self.groups.iter().any(|group| group.name == first) {
            return first;
        }
        for n in 2..=9999 {
            let name = format!("{first} {n}");
            if !self.groups.iter().any(|group| group.name == name) {
                return name;
            }
        }
        first
    }

    pub fn duplicate_group(&mut self, id: GroupId) -> Option<GroupId> {
        let source = self.group(id)?.clone();
        let members: Vec<Layer> = self
            .group_members(id)
            .into_iter()
            .filter_map(|index| self.layers.get(index).cloned())
            .collect();
        let position = self.groups.iter().position(|group| group.id == id)?;

        let copy_id = self.next_id();
        let mut group = LayerGroup::new(copy_id, self.unique_group_name(&source.name));
        group.collapsed = source.collapsed;
        group.visible = source.visible;
        group.enabled = source.enabled;
        self.groups.insert(position + 1, group);

        for mut layer in members {
            layer.id = 0;
            layer.group = copy_id;
            self.add_layer_to_group(layer, copy_id);
        }

        Some(copy_id)
    }

    fn ensure_group(&mut self, requested: GroupId) -> GroupId {
        if self.group(requested).is_some() {
            return requested;
        }
        self.create_group()
    }

    pub fn add_layer(&mut self, mut layer: Layer) -> usize {
        if layer.id == 0 {
            layer.id = self.next_id();
        }
        let group = self.ensure_group(layer.group);
        layer.group = group;
        self.insert_layer(layer, group)
    }

    pub fn add_layer_to_group(&mut self, mut layer: Layer, group: GroupId) -> usize {
        if layer.id == 0 {
            layer.id = self.next_id();
        }
        let group = self.ensure_group(group);
        layer.group = group;
        self.insert_layer(layer, group)
    }

    pub fn create_layer(&mut self, group: Option<GroupId>) -> usize {
        let group = group.unwrap_or_else(|| self.create_group());
        let name = self.auto_layer_name();
        self.add_layer_to_group(Layer::new(name, LayerSource::Vector), group)
    }

    fn insert_layer(&mut self, layer: Layer, group: GroupId) -> usize {
        let index = self.group_insert_index(group);
        self.layers.insert(index, layer);
        index
    }

    fn group_insert_index(&self, group: GroupId) -> usize {
        if let Some((_, end)) = self.group_block(group) {
            return end;
        }
        let order = self.group_order();
        let Some(position) = order.iter().position(|id| *id == group) else {
            return self.layers.len();
        };
        order[position + 1..]
            .iter()
            .find_map(|id| self.group_block(*id))
            .map(|(start, _)| start)
            .unwrap_or(self.layers.len())
    }

    pub fn remove_layer(&mut self, index: usize) {
        if index >= self.layers.len() {
            return;
        }
        self.layers.remove(index);
    }

    pub fn remove_group(&mut self, id: GroupId) {
        self.layers.retain(|l| l.group != id);
        self.groups.retain(|g| g.id != id);
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

    pub fn toggle_group_collapsed(&mut self, id: GroupId) {
        if let Some(group) = self.group_mut(id) {
            group.collapsed = !group.collapsed;
        }
    }

    pub fn set_group_visible(&mut self, id: GroupId, visible: bool) {
        if let Some(group) = self.group_mut(id) {
            group.visible = visible;
        }
    }

    pub fn set_group_enabled(&mut self, id: GroupId, enabled: bool) {
        if let Some(group) = self.group_mut(id) {
            group.enabled = enabled;
        }
    }

    pub fn set_group_members_visible(&mut self, id: GroupId, visible: bool) {
        for layer in self.layers.iter_mut().filter(|l| l.group == id) {
            layer.is_visible = visible;
        }
    }

    pub fn layers_bounds(&self, ids: &[u64]) -> Option<(f64, f64, f64, f64)> {
        let mut out: Option<(f64, f64, f64, f64)> = None;

        for id in ids {
            let Some((a, b, c, d)) = self
                .layers
                .iter()
                .find(|layer| layer.id == *id)
                .and_then(|layer| layer.bounds())
            else {
                continue;
            };

            out = Some(match out {
                None => (a, b, c, d),
                Some((x0, y0, x1, y1)) => (x0.min(a), y0.min(b), x1.max(c), y1.max(d)),
            });
        }

        out
    }

    pub fn mirror_layers(&mut self, ids: &[u64], mirror_x: bool) -> bool {
        let Some((min_x, min_y, max_x, max_y)) = self.layers_bounds(ids) else {
            return false;
        };

        let center = [(min_x + max_x) / 2.0, (min_y + max_y) / 2.0];

        for id in ids {
            if let Some(index) = self.layer_index(*id) {
                self.layers[index].mirror(mirror_x, center);
            }
        }

        true
    }

    pub fn move_layers_to(&mut self, ids: &[u64], target: DropTarget) -> bool {
        let Some((target_group, absolute)) = self.resolve_drop(target) else {
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

        let shifted = absolute - moving.iter().filter(|&&index| index < absolute).count();

        let mut extracted: Vec<Layer> = Vec::with_capacity(moving.len());
        for index in moving.iter().rev() {
            extracted.push(self.layers.remove(*index));
        }
        extracted.reverse();

        let start = shifted.min(self.layers.len());
        for (offset, mut layer) in extracted.into_iter().enumerate() {
            layer.group = target_group;
            self.layers.insert(start + offset, layer);
        }

        self.compact_groups();

        let after: Vec<(u64, GroupId)> = self.layers.iter().map(|l| (l.id, l.group)).collect();
        before != after
    }

    fn resolve_drop(&self, target: DropTarget) -> Option<(GroupId, usize)> {
        match target {
            DropTarget::GroupStart(group) => {
                let index = self
                    .group_block(group)
                    .map(|(start, _)| start)
                    .unwrap_or_else(|| self.group_insert_index(group));
                Some((group, index))
            }
            DropTarget::GroupEnd(group) => {
                let index = self
                    .group_block(group)
                    .map(|(_, end)| end)
                    .unwrap_or_else(|| self.group_insert_index(group));
                Some((group, index))
            }
            DropTarget::BeforeLayer(id) => {
                let index = self.layer_index(id)?;
                Some((self.layers[index].group, index))
            }
            DropTarget::AfterLayer(id) => {
                let index = self.layer_index(id)?;
                Some((self.layers[index].group, index + 1))
            }
        }
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
        let orphans: Vec<usize> = self
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| self.group(layer.group).is_none())
            .map(|(index, _)| index)
            .collect();

        if !orphans.is_empty() {
            let name = self.auto_group_name();
            let id = self.add_group(name);
            for index in orphans {
                self.layers[index].group = id;
            }
        }

        let unnamed: Vec<GroupId> = self
            .groups
            .iter()
            .filter(|group| group.is_empty_name())
            .map(|group| group.id)
            .collect();
        for id in unnamed {
            let name = self.auto_group_name();
            if let Some(group) = self.group_mut(id) {
                group.name = name;
            }
        }
    }

    fn compact_groups(&mut self) {
        let mut order: Vec<GroupId> = self.group_order();
        for layer in &self.layers {
            if !order.contains(&layer.group) {
                order.push(layer.group);
            }
        }

        let mut buckets: Vec<Vec<Layer>> = order.iter().map(|_| Vec::new()).collect();
        for layer in self.layers.drain(..) {
            if let Some(index) = order.iter().position(|id| *id == layer.group) {
                buckets[index].push(layer);
            }
        }

        self.layers = buckets.into_iter().flatten().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layer::LayerSource;

    fn layer(name: &str) -> Layer {
        Layer::new(name.to_string(), LayerSource::Vector)
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

    fn blocks_follow_group_order(project: &Project) -> bool {
        let order = project.group_order();
        let mut last = 0_usize;
        for layer in &project.layers {
            let Some(position) = order.iter().position(|id| *id == layer.group) else {
                return false;
            };
            if position < last {
                return false;
            }
            last = position;
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
    fn removing_last_member_keeps_empty_group() {
        let mut project = Project::new();
        project.add_layer(layer("a"));
        project.add_layer(layer("b"));

        project.remove_layer(0);

        assert_eq!(project.groups.len(), 2);
        assert_eq!(project.layers.len(), 1);
        assert_eq!(project.group_members(project.groups[0].id), Vec::<usize>::new());
    }

    #[test]
    fn empty_group_receives_layers_at_its_own_place() {
        let mut project = Project::new();
        project.add_layer(layer("a"));
        let middle = project.create_group();
        project.add_layer(layer("b"));

        let added = project.add_layer_to_group(layer("c"), middle);

        assert_eq!(names(&project), vec!["a", "c", "b"]);
        assert_eq!(project.layers[added].group, middle);
        assert!(blocks_follow_group_order(&project));
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
    fn dragging_out_of_group_leaves_it_empty_but_alive() {
        let mut project = Project::new();
        let first = project.add_layer(layer("a"));
        let group = project.layers[first].group;
        project.add_layer_to_group(layer("b"), group);
        project.add_layer(layer("c"));

        let target = project.layers[2].group;
        let moving: Vec<u64> = project.group_members(group).iter().map(|i| project.layers[*i].id).collect();
        project.move_layers_to(&moving, DropTarget::GroupEnd(target));

        assert!(project.group(group).is_some());
        assert!(project.group_block(group).is_none());
        assert_eq!(project.group_members(target).len(), 3);
        assert!(blocks_follow_group_order(&project));
    }

    #[test]
    fn group_flags_do_not_touch_members() {
        let mut project = Project::new();
        let a = project.add_layer(layer("a"));
        let group = project.layers[a].group;
        project.add_layer_to_group(layer("b"), group);

        project.set_group_visible(group, false);
        project.set_group_enabled(group, false);

        assert!(!project.group_visible(group));
        assert!(!project.group_enabled(group));
        assert!(project.layers.iter().all(|l| l.is_visible && l.is_enabled));
        assert!(project.layers.iter().all(|l| !project.layer_visible(l)));
        assert!(project.layers.iter().all(|l| !project.layer_enabled(l)));

        project.set_group_visible(group, true);
        project.set_group_enabled(group, true);

        assert!(project.layers.iter().all(|l| project.layer_visible(l)));
        assert!(project.layers.iter().all(|l| project.layer_enabled(l)));
    }

    #[test]
    fn member_flags_do_not_touch_group() {
        let mut project = Project::new();
        let a = project.add_layer(layer("a"));
        let group = project.layers[a].group;
        project.add_layer_to_group(layer("b"), group);

        project.set_group_members_visible(group, false);

        assert!(project.group_visible(group));
        assert!(project.group_enabled(group));
        assert!(project.layers.iter().all(|l| !l.is_visible));
        assert!(project.layers.iter().all(|l| l.is_enabled));

        project.set_group_members_visible(group, true);
        assert!(project.layers.iter().all(|l| l.is_visible));
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
    fn normalize_keeps_empty_groups() {
        let mut project = Project::new();
        project.add_layer(layer("a"));
        let empty = project.create_group();

        project.normalize();

        assert!(project.group(empty).is_some());
        assert!(project.group_block(empty).is_none());
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
    fn duplicate_group_copies_its_layers() {
        let mut project = Project::new();
        let a = project.add_layer(layer("a"));
        let source = project.layers[a].group;
        project.add_layer_to_group(layer("b"), source);
        project.add_layer(layer("c"));
        project.rename_group(source, "Engrave".to_string());

        let copy = project.duplicate_group(source).unwrap();

        assert_eq!(project.groups.len(), 3);
        assert_eq!(project.group_members(source).len(), 2);
        assert_eq!(project.group_members(copy).len(), 2);
        assert_eq!(project.group(copy).unwrap().name, "Engrave copy");
        assert_eq!(names(&project), vec!["a", "b", "a", "b", "c"]);
        assert_eq!(project.group_order()[1], copy);
        assert!(blocks_are_contiguous(&project));
        assert!(blocks_follow_group_order(&project));

        let source_ids: Vec<u64> = project
            .group_members(source)
            .iter()
            .map(|i| project.layers[*i].id)
            .collect();
        let copy_ids: Vec<u64> = project
            .group_members(copy)
            .iter()
            .map(|i| project.layers[*i].id)
            .collect();
        assert!(source_ids.iter().all(|id| !copy_ids.contains(id)));
    }

    #[test]
    fn duplicate_group_copies_flags_and_keeps_members_independent() {
        let mut project = Project::new();
        let a = project.add_layer(layer("a"));
        let source = project.layers[a].group;
        project.set_group_visible(source, false);
        project.set_group_enabled(source, false);

        let copy = project.duplicate_group(source).unwrap();
        project.set_group_members_visible(copy, true);
        project.duplicate_group(source).unwrap();

        let first = project.group(copy).unwrap();
        assert!(!first.visible);
        assert!(!first.enabled);
        assert!(!project.group(source).unwrap().visible);
        assert_eq!(project.groups.len(), 3);
        assert!(project.layers.iter().all(|l| l.is_visible));
        assert!(blocks_follow_group_order(&project));
    }

    #[test]
    fn duplicated_group_gets_unique_name() {
        let mut project = Project::new();
        let a = project.add_layer(layer("a"));
        let source = project.layers[a].group;
        project.rename_group(source, "Engrave".to_string());

        let one = project.duplicate_group(source).unwrap();
        let two = project.duplicate_group(source).unwrap();
        let three = project.duplicate_group(one).unwrap();

        assert_eq!(project.group(one).unwrap().name, "Engrave copy");
        assert_eq!(project.group(two).unwrap().name, "Engrave copy 2");
        assert_eq!(project.group(three).unwrap().name, "Engrave copy 3");
    }

    #[test]
    fn duplicating_missing_group_reports_nothing() {
        let mut project = Project::new();
        project.add_layer(layer("a"));

        assert!(project.duplicate_group(9999).is_none());
        assert_eq!(project.groups.len(), 1);
    }

    #[test]
    fn groups_survive_serialization() {
        let mut project = Project::new();
        project.add_layer(layer("a"));
        project.add_layer(layer("b"));
        project.rename_group(project.groups[0].id, "Engrave".to_string());
        project.set_group_visible(project.groups[1].id, false);

        let text = toml::to_string(&project).unwrap();
        let mut restored: Project = toml::from_str(&text).unwrap();
        restored.normalize();

        assert_eq!(restored.groups.len(), 2);
        assert_eq!(restored.layers.len(), 2);
        assert_eq!(restored.layers[0].group, restored.groups[0].id);
        assert_eq!(restored.layers[1].group, restored.groups[1].id);
        assert!(!restored.groups[1].visible);
        assert!(restored.groups[0].visible);
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

    fn boxed(name: &str, bounds: [f64; 4]) -> Layer {
        let mut layer = Layer::new(name.to_string(), LayerSource::Image {
            path: String::new(),
        });
        layer.set_bounds(bounds[0], bounds[1], bounds[2], bounds[3]);
        layer
    }

    fn mirrored(project: &Project) -> Vec<bool> {
        project
            .layers
            .iter()
            .map(|l| l.mirror_x || l.mirror_y)
            .collect()
    }

    #[test]
    fn mirror_moves_boxes_around_the_shared_center() {
        let mut project = Project::new();
        let left = project.add_layer(boxed("left", [0.0, 0.0, 10.0, 10.0]));
        let right = project.add_layer(boxed("right", [20.0, 0.0, 30.0, 10.0]));
        let ids = vec![project.layers[left].id, project.layers[right].id];

        assert!(project.mirror_layers(&ids, true));

        assert_eq!(project.layers[left].bounds, Some([20.0, 0.0, 30.0, 10.0]));
        assert_eq!(project.layers[right].bounds, Some([0.0, 0.0, 10.0, 10.0]));
        assert!(project.layers.iter().all(|l| l.mirror_x));
        assert!(project.layers.iter().all(|l| !l.mirror_y));
    }

    #[test]
    fn mirroring_twice_restores_bounds_and_flags() {
        let mut project = Project::new();
        project.add_layer(boxed("a", [0.0, 0.0, 10.0, 4.0]));
        project.add_layer(boxed("b", [12.0, 6.0, 20.0, 9.0]));
        let ids: Vec<u64> = project.layers.iter().map(|l| l.id).collect();

        project.mirror_layers(&ids, true);
        project.mirror_layers(&ids, true);

        assert_eq!(project.layers[0].bounds, Some([0.0, 0.0, 10.0, 4.0]));
        assert_eq!(project.layers[1].bounds, Some([12.0, 6.0, 20.0, 9.0]));
        assert!(mirrored(&project).iter().all(|m| !m));
    }

    #[test]
    fn mirroring_a_vector_layer_flips_its_nodes() {
        let mut project = Project::new();
        let mut layer = Layer::new("vector".to_string(), LayerSource::Vector);
        let mut shape = crate::core::layer::VectorShape::default();
        shape
            .nodes
            .push(crate::core::vector::PathNode::corner(0.0, 0.0));
        shape
            .nodes
            .push(crate::core::vector::PathNode::corner(1.0, 0.0));
        shape
            .nodes
            .push(crate::core::vector::PathNode::corner(4.0, 0.0));
        shape.stroke.width_mm = 0.0;
        layer.shapes.push(shape);
        layer.recompute_bounds();
        let index = project.add_layer(layer);
        let id = project.layers[index].id;

        assert!(project.mirror_layers(&[id], true));

        let nodes: Vec<f64> = project.layers[index].shapes[0]
            .nodes
            .iter()
            .map(|node| node.point.x)
            .collect();

        assert_eq!(nodes, vec![0.0, 3.0, 4.0]);
        assert_eq!(project.layers[index].bounds, Some([0.0, 0.0, 4.0, 0.0]));
    }

    #[test]
    fn mirroring_without_bounds_reports_nothing() {
        let mut project = Project::new();
        let index = project.add_layer(layer("empty"));
        let id = project.layers[index].id;

        assert!(!project.mirror_layers(&[id], true));
        assert!(!project.layers[index].mirror_x);
    }
}
