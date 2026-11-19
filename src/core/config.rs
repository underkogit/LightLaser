use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineConfig {
    #[serde(default)]
    pub port: String,

    #[serde(default = "default_baudrate")]
    pub baudrate: u32,

    #[serde(default = "default_work_area")]
    pub work_area_mm: (f64, f64),

    #[serde(default = "default_jog_feed")]
    pub jog_feed: f64,

    #[serde(default = "default_jog_step")]
    pub jog_step: f64,

    /// How many times the whole job is run.
    #[serde(default = "default_passes")]
    pub passes: u32,
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            port: String::new(),
            baudrate: default_baudrate(),
            work_area_mm: default_work_area(),
            jog_feed: default_jog_feed(),
            jog_step: default_jog_step(),
            passes: default_passes(),
        }
    }
}

fn default_baudrate() -> u32 {
    115200
}

fn default_work_area() -> (f64, f64) {
    (300.0, 300.0)
}

fn default_jog_feed() -> f64 {
    1000.0
}

fn default_jog_step() -> f64 {
    1.0
}

fn default_passes() -> u32 {
    1
}
