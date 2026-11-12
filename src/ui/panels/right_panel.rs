use egui::{Align, Layout};
use rust_i18n::t;

use crate::comm::{StreamerCommand, StreamerState};
use crate::core::layer::{LayerParams, LayerSource};
use crate::ui::LaserApp;
use crate::ui::panels::canvas::duplicate_selected;
use crate::ui::selection::Selection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightPanelTab {
    #[default]
    Layers,
    Connection,
    Properties,
}

pub struct RightPanelState {
    pub active_tab: RightPanelTab,
}

impl Default for RightPanelState {
    fn default() -> Self {
        Self {
            active_tab: RightPanelTab::Layers,
        }
    }
}

impl LaserApp {
    pub(crate) fn ui_right_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("machine_control_bottom")
            .resizable(false)
            .show_inside(ui, |ui| {
                self.ui_machine_controls(ui);
            });

        let mut active_tab = self.right_panel_state.active_tab;

        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut active_tab,
                RightPanelTab::Layers,
                t!("ui.label.layers").to_string(),
            );
            ui.selectable_value(
                &mut active_tab,
                RightPanelTab::Connection,
                t!("ui.label.connection.title").to_string(),
            );
            ui.selectable_value(&mut active_tab, RightPanelTab::Properties, "Properties");
        });
        ui.separator();

        self.right_panel_state.active_tab = active_tab;

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);
            match self.right_panel_state.active_tab {
                RightPanelTab::Layers => {
                    self.ui_layer_list(ui);
                }
                RightPanelTab::Connection => {
                    self.ui_connection_params(ui);
                }
                RightPanelTab::Properties => {
                    let selected = self.ui_state.selection.layer;
                    if let Some(idx) = selected {
                        if idx < self.project.layers.len() {
                            self.ui_selection_actions(ui);
                            ui.separator();
                            self.ui_layer_params(ui, idx);
                        } else {
                            self.ui_state.selection.clear();
                            ui.label(t!("ui.label.no_layer_selected").to_string());
                        }
                    } else {
                        ui.label(
                            egui::RichText::new(t!("ui.label.no_layer_selected").to_string())
                                .italics()
                                .weak(),
                        );
                    }
                }
            }
        });
    }

    fn ui_machine_controls(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Machine Control").strong());
        ui.add_space(4.0);

        let connected = self.grbl_status.is_some();
        let is_running = self.streamer_state == StreamerState::Running;
        let is_paused = self.streamer_state == StreamerState::Paused;
        let is_idle = self.streamer_state == StreamerState::Idle;

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
        ui.add_space(4.0);
    }

    fn ui_connection_params(&mut self, ui: &mut egui::Ui) {
        let ports: Vec<String> = serialport::available_ports()
            .map(|list| list.into_iter().map(|p| p.port_name).collect())
            .unwrap_or_default();

        ui.horizontal(|ui| {
            ui.label(t!("ui.label.connection.port").to_string());
            egui::ComboBox::from_id_salt("port_combo_panel")
                .selected_text(if self.project.machine.port.is_empty() {
                    "—".to_string()
                } else {
                    self.project.machine.port.clone()
                })
                .width(180.0)
                .show_ui(ui, |ui| {
                    for p in &ports {
                        ui.selectable_value(&mut self.project.machine.port, p.clone(), p);
                    }
                });
            if ui.button("⟳").clicked() {
                ui.ctx().request_repaint();
            }
        });

        ui.horizontal(|ui| {
            ui.label(t!("ui.label.connection.baudrate").to_string());
            egui::ComboBox::from_id_salt("baudrate_combo_panel")
                .selected_text(self.project.machine.baudrate.to_string())
                .width(180.0)
                .show_ui(ui, |ui| {
                    for b in [9600u32, 19200, 38400, 57600, 115200] {
                        ui.selectable_value(&mut self.project.machine.baudrate, b, b.to_string());
                    }
                });
        });

        ui.add_space(8.0);
        ui.checkbox(
            &mut self.project.machine.enable_laser_mode,
            t!("ui.label.connection.laser_mode").to_string(),
        );
        ui.checkbox(
            &mut self.project.machine.soft_limits,
            t!("ui.label.connection.soft_limits").to_string(),
        );
        ui.checkbox(
            &mut self.project.machine.homing_cycle,
            t!("ui.label.connection.homing_cycle").to_string(),
        );
    }

    fn ui_selection_actions(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .button(format!("{} Duplicate", egui_phosphor::regular::COPY))
                .on_hover_text("Ctrl+D")
                .clicked()
            {
                duplicate_selected(self);
            }
            if ui
                .button(format!("{} Delete", egui_phosphor::regular::TRASH))
                .on_hover_text("Del")
                .clicked()
            {
                self.delete_selection_public();
            }
        });
        ui.add_space(6.0);
    }

    fn delete_selection_public(&mut self) {
        let sel = self.ui_state.selection.clone();
        if let (Some(li), Some(si)) = (sel.layer, sel.shape) {
            self.push_history();
            if let Some(layer) = self.project.layers.get_mut(li) {
                if si < layer.shapes.len() {
                    layer.shapes.remove(si);
                    layer.recompute_bounds();
                }
            }
            self.ui_state.selection.shape = None;
            self.ui_state.selection.node = None;
            self.ui_state.selection.handle = None;
        } else if let Some(li) = sel.layer {
            self.push_history();
            self.project.remove_layer(li);
            self.ui_state.selection.clear();
        }
    }

    fn ui_layer_list(&mut self, ui: &mut egui::Ui) {
        let mut to_remove: Option<usize> = None;
        let mut to_move_up: Option<usize> = None;
        let mut to_move_down: Option<usize> = None;
        let mut new_selection: Option<usize> = self.ui_state.selection.layer;

        let selected = self.ui_state.selection.layer;

        for (i, layer) in self.project.layers.iter_mut().enumerate() {
            let is_selected = selected == Some(i);

            let frame = egui::Frame::group(ui.style())
                .fill(if is_selected {
                    ui.style().visuals.selection.bg_fill
                } else {
                    ui.style().visuals.faint_bg_color
                })
                .inner_margin(egui::Margin::symmetric(6, 4));

            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        2.0,
                        egui::Color32::from_rgb(layer.color[0], layer.color[1], layer.color[2]),
                    );

                    let resp = ui.add(
                        egui::Label::new(egui::RichText::new(&layer.name).strong())
                            .sense(egui::Sense::click()),
                    );
                    if resp.clicked() {
                        new_selection = Some(i);
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(format!(
                                "{}",
                                egui_phosphor::regular::TRASH
                            )))
                            .clicked()
                        {
                            to_remove = Some(i);
                        }

                        if ui
                            .add(
                                egui::Button::new(format!("{}", egui_phosphor::regular::ARROW_UP))
                                    .small(),
                            )
                            .clicked()
                        {
                            to_move_up = Some(i);
                        }
                        if ui
                            .add(
                                egui::Button::new(format!(
                                    "{}",
                                    egui_phosphor::regular::ARROW_DOWN
                                ))
                                .small(),
                            )
                            .clicked()
                        {
                            to_move_down = Some(i);
                        }

                        if ui
                            .checkbox(&mut layer.is_visible, "")
                            .on_hover_text(t!("ui.tooltip.layer_visible"))
                            .changed()
                        {}

                        if ui
                            .checkbox(&mut layer.is_enabled, "")
                            .on_hover_text(t!("ui.tooltip.layer_enabled"))
                            .changed()
                        {}
                    });
                });
            });
            ui.add_space(4.0);
        }

        if let Some(i) = to_remove {
            self.project.remove_layer(i);
            if self.ui_state.selection.layer == Some(i) {
                self.ui_state.selection.clear();
            } else if let Some(sel) = self.ui_state.selection.layer {
                if sel > i {
                    self.ui_state.selection.layer = Some(sel - 1);
                }
            }
        }
        if let Some(i) = to_move_up {
            self.project.move_layer_up(i);
            if new_selection == Some(i) {
                new_selection = Some(i - 1);
            }
        }
        if let Some(i) = to_move_down {
            self.project.move_layer_down(i);
            if new_selection == Some(i) {
                new_selection = Some(i + 1);
            }
        }

        if new_selection != self.ui_state.selection.layer {
            self.ui_state.selection = Selection {
                layer: new_selection,
                shape: None,
                node: None,
                handle: None,
            };
        }
    }

    fn ui_layer_params(&mut self, ui: &mut egui::Ui, idx: usize) {
        let (name, source_label, params, color) = {
            let l = &self.project.layers[idx];
            (
                l.name.clone(),
                l.source.kind_label(),
                l.params.clone(),
                l.color,
            )
        };

        let mut new_name = name;
        let mut new_params: LayerParams = params;
        let mut color32 = egui::Color32::from_rgb(color[0], color[1], color[2]);

        ui.add_space(8.0);
        ui.heading(format!("{} #{}", t!("ui.label.layer"), idx + 1));
        ui.label(format!("{}: {}", t!("ui.label.source"), source_label));
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(t!("ui.label.layer_name"));
            ui.text_edit_singleline(&mut new_name);
        });

        ui.horizontal(|ui| {
            ui.label("Color");
            if ui.color_edit_button_srgba(&mut color32).changed() {
                self.project.layers[idx].color = [color32.r(), color32.g(), color32.b()];
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(egui::RichText::new(t!("ui.label.params").to_string()).strong());
        ui.add_space(4.0);

        ui.add(
            egui::Slider::new(&mut new_params.power_percent, 0.0..=100.0)
                .text(format!("{} (%)", t!("ui.label.power")))
                .suffix("%"),
        );

        ui.add(
            egui::DragValue::new(&mut new_params.feed_rate)
                .speed(10.0)
                .range(1.0..=20000.0)
                .suffix(" mm/min"),
        )
        .on_hover_text(t!("ui.label.feed_rate"));

        ui.add(
            egui::DragValue::new(&mut new_params.passes)
                .speed(0.1)
                .range(1..=100),
        )
        .on_hover_text(t!("ui.label.passes"));

        ui.checkbox(&mut new_params.dynamic_power, t!("ui.label.dynamic_power"));

        let is_image = matches!(self.project.layers[idx].source, LayerSource::Image { .. });
        if is_image {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(egui::RichText::new(t!("ui.label.image_params").to_string()).strong());
            ui.add_space(4.0);

            ui.add(egui::Slider::new(&mut new_params.pixels_per_mm, 1.0..=50.0).text("px/mm"));

            ui.add(
                egui::Slider::new(&mut new_params.threshold, 0..=255)
                    .text(t!("ui.label.threshold").to_string()),
            );

            ui.checkbox(&mut new_params.dither, t!("ui.label.dither"));
            ui.checkbox(&mut new_params.invert, t!("ui.label.invert"));
        }

        let layer = &mut self.project.layers[idx];
        if layer.name != new_name {
            layer.name = new_name;
        }
        if layer.params.power_percent != new_params.power_percent
            || layer.params.feed_rate != new_params.feed_rate
            || layer.params.passes != new_params.passes
            || layer.params.dynamic_power != new_params.dynamic_power
            || layer.params.pixels_per_mm != new_params.pixels_per_mm
            || layer.params.threshold != new_params.threshold
            || layer.params.dither != new_params.dither
            || layer.params.invert != new_params.invert
        {
            layer.params = new_params;
        }

        if let Some(si) = self.ui_state.selection.shape {
            if si < self.project.layers[idx].shapes.len() {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Shape").strong());
                ui.add_space(4.0);

                let mut width = self.project.layers[idx].shapes[si].stroke.width_mm;
                if ui
                    .add(egui::Slider::new(&mut width, 0.05..=5.0).text("Line width (mm)"))
                    .changed()
                {
                    self.project.layers[idx].shapes[si].stroke.width_mm = width;
                }

                let mut closed = self.project.layers[idx].shapes[si].closed;
                if ui.checkbox(&mut closed, "Closed path").changed() {
                    self.project.layers[idx].shapes[si].closed = closed;
                }
            }
        }
    }
}
