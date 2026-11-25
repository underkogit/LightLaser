use serde::{Deserialize, Serialize};

pub type GroupId = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerGroup {
    #[serde(default)]
    pub id: GroupId,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub collapsed: bool,

    #[serde(default = "default_group_flag")]
    pub visible: bool,

    #[serde(default = "default_group_flag")]
    pub enabled: bool,
}

impl LayerGroup {
    pub fn new(id: GroupId, name: String) -> Self {
        Self {
            id,
            name,
            collapsed: false,
            visible: true,
            enabled: true,
        }
    }

    pub fn is_empty_name(&self) -> bool {
        self.name.trim().is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropTarget {
    GroupStart(GroupId),
    GroupEnd(GroupId),
    BeforeLayer(u64),
    AfterLayer(u64),
}

fn default_group_flag() -> bool {
    true
}
