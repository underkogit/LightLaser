use rust_i18n::t;

use crate::comm::StreamerCommand;
use crate::core::gcode::GcodeCommand;
use crate::core::image::{engrave_layer, load_image, rasterize, RasterizeOptions};
use crate::core::layer::LayerSource;
use crate::core::vector_gcode::export_vector_layer;
use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn start_job(&mut self) {
        let lines = match self.build_job_gcode() {
            Ok(lines) => lines,
            Err(e) => {
                log::error!("{}", t!("ui.error.build_job", e = e));
                return;
            }
        };

        if lines.is_empty() {
            log::warn!("{}", t!("ui.warn.empty_job"));
            return;
        }

        self.streamer.send(StreamerCommand::Start { lines });
    }

    fn build_job_gcode(&self) -> Result<Vec<String>, String> {
        let mut lines: Vec<String> = Vec::new();

        lines.push("G21 ; mm".to_string());
        lines.push("G90 ; absolute".to_string());
        lines.push("G17 ; XY plane".to_string());

        for (i, layer) in self.project.layers.iter().enumerate() {
            if !layer.is_enabled {
                continue;
            }

            lines.push(format!("; --- layer #{}: {} ---", i + 1, layer.name));

            let layer_cmds: Vec<GcodeCommand> = match &layer.source {
                LayerSource::Image { path } => {
                    let loaded = load_image(std::path::Path::new(path))
                        .map_err(|e| format!("image '{}': {}", path, e))?;

                    let raster = rasterize(
                        &loaded.dynamic,
                        RasterizeOptions {
                            pixels_per_mm: layer.params.pixels_per_mm,
                            threshold: layer.params.threshold,
                            invert: layer.params.invert,
                        },
                        None,
                    );

                    engrave_layer(&raster, &layer.params, (0.0, 0.0))
                        .map_err(|e| format!("engrave '{}': {}", path, e))?
                }
                LayerSource::Svg { path } => {
                    log::warn!("SVG layer not implemented yet: {}", path);
                    Vec::new()
                }
                LayerSource::Gcode { path } => {
                    let content = std::fs::read_to_string(path)
                        .map_err(|e| format!("gcode '{}': {}", path, e))?;
                    content
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty() && !l.starts_with(';'))
                        .map(|raw| GcodeCommand {
                            raw,
                            line_number: None,
                            motion: None,
                            x: None,
                            y: None,
                            z: None,
                            i: None,
                            j: None,
                            feed: None,
                            spindle: None,
                            comment: None,
                        })
                        .collect()
                }
                LayerSource::Vector => {
                    export_vector_layer(layer, &layer.params, (0.0, 0.0))
                        .map_err(|e| format!("vector '{}': {}", layer.name, e))?
                }
            };

            for cmd in layer_cmds {
                lines.push(cmd.raw);
            }

            lines.push("M5".to_string());
        }

        lines.push("G0 X0 Y0 ; return home".to_string());
        lines.push("M5 ; laser off".to_string());

        Ok(lines)
    }

    pub(crate) fn export_job_gcode(&self) -> Result<String, String> {
        let lines = self.build_job_gcode()?;
        Ok(lines.join("\n"))
    }
}