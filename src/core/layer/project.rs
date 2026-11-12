use serde::{Deserialize, Serialize};

use crate::core::config::MachineConfig;
use crate::core::layer::Layer;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Project {
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub layers: Vec<Layer>,

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
            machine: MachineConfig::default(),
        }
    }

    pub fn add_layer(&mut self, layer: Layer) {
        self.layers.push(layer);
    }

    pub fn remove_layer(&mut self, index: usize) {
        if index < self.layers.len() {
            self.layers.remove(index);
        }
    }

    pub fn move_layer_up(&mut self, index: usize) {
        if index > 0 && index < self.layers.len() {
            self.layers.swap(index, index - 1);
        }
    }

    pub fn move_layer_down(&mut self, index: usize) {
        if index + 1 < self.layers.len() {
            self.layers.swap(index, index + 1);
        }
    }
}