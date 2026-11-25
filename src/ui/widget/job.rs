use rust_i18n::t;

use crate::comm::StreamerCommand;
use crate::core::job::build_job;
use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn start_job(&mut self) {
        let (lines, reports) = match build_job(
            &self.project,
            self.project.machine.spindle_max,
            self.grbl_laser_mode,
        ) {
            Ok(job) => job,
            Err(e) => {
                log::error!("{}", t!("ui.error.build_job", e = e));
                return;
            }
        };

        if lines.is_empty() {
            log::warn!("{}", t!("ui.warn.empty_job"));
            return;
        }

        for report in &reports {
            if report.is_silent() {
                log::warn!(
                    "Layer #{} '{}' ({}) has S{:.0} but nothing to cut: the laser stays off",
                    report.index + 1,
                    report.name,
                    report.kind,
                    report.laser_power
                );
            } else if report.laser_power <= 0.0 {
                log::warn!(
                    "Layer #{} '{}' ({}) runs at 0%: set the power slider above zero",
                    report.index + 1,
                    report.name,
                    report.kind
                );
            } else {
                log::info!(
                    "Layer #{} '{}' ({}): {:.0}% = S{:.0}, F{:.0}, {} pass(es), {} moves, {:.1} mm, energy {:.2} %/1000mm",
                    report.index + 1,
                    report.name,
                    report.kind,
                    report.power_percent,
                    report.laser_power,
                    report.feed_rate,
                    report.passes,
                    report.moves,
                    report.cut_length,
                    report.power_percent / report.feed_rate.max(1.0) * 1000.0
                );
            }
        }

        log::info!(
            "Job: {} lines, full-scale S {:.0}, laser mode {}",
            lines.len(),
            self.project.machine.spindle_max,
            match self.grbl_laser_mode {
                Some(true) => "on",
                Some(false) => "off",
                None => "unknown",
            }
        );

        self.streamer.send(StreamerCommand::Start { lines });
    }

    pub(crate) fn export_job_gcode(&self) -> Result<String, String> {
        let (lines, _) = build_job(
            &self.project,
            self.project.machine.spindle_max,
            self.grbl_laser_mode,
        )?;
        Ok(lines.join("\n"))
    }
}
