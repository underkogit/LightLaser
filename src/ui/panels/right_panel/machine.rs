use rust_i18n::t;

use crate::comm::{StreamerCommand, StreamerState};
use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn ui_machine_controls(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);

        ui.label(egui::RichText::new("Machine Control").strong());
        ui.add_space(4.0);

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
                .on_hover_text(t!("ui.button.home"))
                .clicked()
            {
                self.streamer.send(StreamerCommand::Home);
            }

            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(format!("{}", egui_phosphor::regular::LOCK_OPEN)),
                )
                .on_hover_text(t!("ui.button.unlock"))
                .clicked()
            {
                self.streamer.send(StreamerCommand::Unlock);
            }
        });

        ui.add_space(4.0);

        ui.label("Jog");
        ui.horizontal(|ui| {
            let step = self.project.machine.jog_step;
            let feed = self.project.machine.jog_feed;

            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(format!("{}", egui_phosphor::regular::ARROW_LEFT)),
                )
                .clicked()
            {
                self.streamer.send(StreamerCommand::Jog {
                    dx: -step,
                    dy: 0.0,
                    feed,
                });
            }

            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(format!("{}", egui_phosphor::regular::ARROW_UP)),
                )
                .clicked()
            {
                self.streamer.send(StreamerCommand::Jog {
                    dx: 0.0,
                    dy: step,
                    feed,
                });
            }

            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(format!("{}", egui_phosphor::regular::ARROW_DOWN)),
                )
                .clicked()
            {
                self.streamer.send(StreamerCommand::Jog {
                    dx: 0.0,
                    dy: -step,
                    feed,
                });
            }

            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(format!("{}", egui_phosphor::regular::ARROW_RIGHT)),
                )
                .clicked()
            {
                self.streamer.send(StreamerCommand::Jog {
                    dx: step,
                    dy: 0.0,
                    feed,
                });
            }
        });

        ui.label("Step");
        ui.horizontal(|ui| {
            ui.add_sized(
                [80.0, ui.spacing().interact_size.y],
                egui::DragValue::new(&mut self.project.machine.jog_step)
                    .speed(0.1)
                    .range(0.1..=100.0)
                    .suffix(" mm"),
            )
                .on_hover_text(t!("ui.tooltip.jog_step"));
        });

        ui.label("Feed rate");
        ui.horizontal(|ui| {
            ui.add_sized(
                [100.0, ui.spacing().interact_size.y],
                egui::DragValue::new(&mut self.project.machine.jog_feed)
                    .speed(10.0)
                    .range(1.0..=20000.0)
                    .suffix(" mm/min"),
            )
                .on_hover_text("Jog feed rate");
        });

        ui.add_space(4.0);
    }
}