use serde::{Deserialize, Serialize};

pub type GroupId = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerGroup {
    #[serde(default)]
    pub id: GroupId,

    #[serde(default)]
    pub name: String,

    #[serde(default = "default_group_color")]
    pub color: [u8; 3],

    #[serde(default)]
    pub collapsed: bool,
}

impl LayerGroup {
    pub fn new(id: GroupId, name: String, color: [u8; 3]) -> Self {
        Self {
            id,
            name,
            color,
            collapsed: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropTarget {
    GroupStart(GroupId),
    GroupEnd(GroupId),
    BeforeLayer(u64),
    AfterLayer(u64),
}

fn default_group_color() -> [u8; 3] {
    [128, 128, 128]
}
