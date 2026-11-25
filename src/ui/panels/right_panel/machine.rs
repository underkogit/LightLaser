use rust_i18n::t;

use crate::comm::{StreamerCommand, StreamerState};
use crate::ui::LaserApp;

use super::jog_crosshair::CROSSHAIR_WIDTH;

impl LaserApp {
    pub(crate) fn ui_machine_controls(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);

        ui.label(egui::RichText::new("Machine Control").strong());
        ui.add_space(4.0);

        // Actions fill the panel; the jog crosshair is pinned to the right at a fixed width.
        //
        // `Ui::columns` is deliberately not used here. It builds its columns with
        // `Layout::top_down_justified`, which stretches every widget to the full column
        // width - buttons become full-width bars and the fields run into each other. The
        // explicit allocation below keeps both sides at their natural size.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(CROSSHAIR_WIDTH, 0.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    self.ui_jog_crosshair(ui);
                },
            );

            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                self.ui_machine_actions(ui);
            });
        });

        ui.add_space(4.0);
    }

    fn ui_machine_actions(&mut self, ui: &mut egui::Ui) {
        let connected = self.grbl_status.is_some();
        let is_running = self.streamer_state == StreamerState::Running;
        let is_paused = self.streamer_state == StreamerState::Paused;
        let is_idle = self.streamer_state == StreamerState::Idle;

        ui.label("Job");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    is_idle,
                    egui::Button::new(format!(
                        "{} {}",
                        egui_phosphor::regular::PLAY,
                        t!("ui.button.start")
                    )),
                )
                .on_hover_text(t!("ui.tip.job_start").to_string())
                .clicked()
            {
                self.start_job();
            }

            if is_running {
                if ui
                    .button(format!(
                        "{} {}",
                        egui_phosphor::regular::PAUSE,
                        t!("ui.button.pause")
                    ))
                    .on_hover_text(t!("ui.tip.job_pause").to_string())
                    .clicked()
                {
                    self.streamer.send(StreamerCommand::Pause);
                    self.streamer_state = StreamerState::Paused;
                }
            } else if is_paused {
                if ui
                    .button(format!(
                        "{} {}",
                        egui_phosphor::regular::PLAY,
                        t!("ui.button.resume")
                    ))
                    .on_hover_text(t!("ui.tip.job_resume").to_string())
                    .clicked()
                {
                    self.streamer.send(StreamerCommand::Resume);
                    self.streamer_state = StreamerState::Running;
                }
            }

            if ui
                .add_enabled(
                    is_running || is_paused,
                    egui::Button::new(format!(
                        "{} {}",
                        egui_phosphor::regular::STOP,
                        t!("ui.button.stop")
                    )),
                )
                .on_hover_text(t!("ui.tip.job_stop").to_string())
                .clicked()
            {
                self.streamer.send(StreamerCommand::Stop);
            }
        });

        if self.job_progress.1 > 0 {
            let (sent, total) = self.job_progress;
            let frac = sent as f32 / total as f32;
            ui.add(
                egui::ProgressBar::new(frac)
                    .desired_width(ui.available_width())
                    .show_percentage(),
            )
            .on_hover_text(format!("{} / {}", sent, total));
        }

        ui.add_space(4.0);

        ui.label("Machine");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(format!("{}", egui_phosphor::regular::HOUSE)),
                )
                .on_hover_text(t!("ui.tip.home").to_string())
                .clicked()
            {
                self.streamer.send(StreamerCommand::Home);
            }

            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(format!("{}", egui_phosphor::regular::LOCK_OPEN)),
                )
                .on_hover_text(t!("ui.tip.unlock").to_string())
                .clicked()
            {
                self.streamer.send(StreamerCommand::Unlock);
            }
        });

        ui.add_space(4.0);

        // Step and feed rate sit side by side to keep the panel short. The widths are kept
        // small so the pair still fits when the right panel is dragged to its minimum.
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label("Step");
                ui.add_sized(
                    [60.0, ui.spacing().interact_size.y],
                    egui::DragValue::new(&mut self.project.machine.jog_step)
                        .speed(0.1)
                        .range(0.1..=100.0)
                        .suffix(" mm"),
                )
                .on_hover_text(t!("ui.tip.jog_step").to_string());
            });

            ui.vertical(|ui| {
                ui.label("Feed rate");
                ui.add_sized(
                    [80.0, ui.spacing().interact_size.y],
                    egui::DragValue::new(&mut self.project.machine.jog_feed)
                        .speed(10.0)
                        .range(1.0..=20000.0)
                        .suffix(" mm/min"),
                )
                .on_hover_text(t!("ui.tip.jog_feed").to_string());
            });
        });

        ui.label("Passes");
        ui.add_sized(
            [60.0, ui.spacing().interact_size.y],
            egui::DragValue::new(&mut self.project.machine.passes)
                .speed(0.2)
                .range(1..=100),
        )
        .on_hover_text(t!("ui.tip.machine_passes").to_string());

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!(
                "{} {:.0} · {}",
                t!("ui.label.spindle_max"),
                self.project.machine.spindle_max,
                laser_mode_label(self.grbl_laser_mode)
            ))
            .weak(),
        )
        .on_hover_text(t!("ui.tip.machine_scale").to_string());
    }
}

fn laser_mode_label(mode: Option<bool>) -> String {
    match mode {
        Some(true) => t!("ui.machine.laser_on").to_string(),
        Some(false) => t!("ui.machine.laser_off").to_string(),
        None => t!("ui.machine.laser_untold").to_string(),
    }
}
