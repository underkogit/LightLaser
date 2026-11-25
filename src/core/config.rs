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

    #[serde(default = "default_spindle_max")]
    pub spindle_max: f64,

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
            spindle_max: default_spindle_max(),
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

fn default_spindle_max() -> f64 {
    1000.0
}

fn default_passes() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_saved_before_the_grbl_scale_use_1000() {
        let machine: MachineConfig = toml::from_str(
            "port = \"COM14\"\nbaudrate = 115200\nwork_area_mm = [380.0, 370.0]\njog_feed = 1000.0\njog_step = 1.0\npasses = 1\n",
        )
        .unwrap();

        assert_eq!(machine.spindle_max, 1000.0);
    }
}
