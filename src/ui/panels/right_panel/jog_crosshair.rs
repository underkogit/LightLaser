//! The jog crosshair shown on the right of the Machine Control panel.

use rust_i18n::t;

use crate::comm::StreamerCommand;
use crate::ui::LaserApp;

/// Edge length of one jog button, and therefore of one cell of the crosshair grid.
const BUTTON: f32 = 48.0;

/// Width the crosshair needs, so the Machine Control panel can reserve room for it.
pub(crate) const CROSSHAIR_WIDTH: f32 = BUTTON * 3.0 + 8.0;

/// A blank cell, so the crosshair keeps its 3x3 shape around the empty centre.
fn gap(ui: &mut egui::Ui) {
    let _ = ui.allocate_exact_size(egui::vec2(BUTTON, BUTTON), egui::Sense::hover());
}

impl LaserApp {
    /// The four jog buttons on a gapless 3x3 grid, with the centre cell left empty:
    ///
    /// ```text
    ///        [ Y+ ]
    /// [ X- ]        [ X+ ]
    ///        [ Y- ]
    /// ```
    ///
    /// Because the grid pitch equals the button size, Y+ sits on the horizontal centre line
    /// and X+ is exactly one button below and one button to the right of it.
    ///
    /// The caller is expected to supply a centred, non-justified layout.
    pub(crate) fn ui_jog_crosshair(&mut self, ui: &mut egui::Ui) {
        let connected = self.grbl_status.is_some();
        let step = self.project.machine.jog_step;
        let feed = self.project.machine.jog_feed;

        let tip = t!("ui.tip.jog").to_string();
        let cell = egui::vec2(BUTTON, BUTTON);

        // No gap between cells: the distance between two neighbouring buttons has to be
        // exactly one button for the cross to come out square.
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

        // Row 0: empty, Y+, empty.
        ui.horizontal(|ui| {
            gap(ui);

            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(egui_phosphor::regular::ARROW_UP).min_size(cell),
                )
                .on_hover_text(tip.clone())
                .clicked()
            {
                self.streamer.send(StreamerCommand::Jog {
                    dx: 0.0,
                    dy: step,
                    feed,
                });
            }

            gap(ui);
        });

        // Row 1: X-, empty centre, X+.
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(egui_phosphor::regular::ARROW_LEFT).min_size(cell),
                )
                .on_hover_text(tip.clone())
                .clicked()
            {
                self.streamer.send(StreamerCommand::Jog {
                    dx: -step,
                    dy: 0.0,
                    feed,
                });
            }

            gap(ui);

            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(egui_phosphor::regular::ARROW_RIGHT).min_size(cell),
                )
                .on_hover_text(tip.clone())
                .clicked()
            {
                self.streamer.send(StreamerCommand::Jog {
                    dx: step,
                    dy: 0.0,
                    feed,
                });
            }
        });

        // Row 2: empty, Y-, empty.
        ui.horizontal(|ui| {
            gap(ui);

            if ui
                .add_enabled(
                    connected,
                    egui::Button::new(egui_phosphor::regular::ARROW_DOWN).min_size(cell),
                )
                .on_hover_text(tip)
                .clicked()
            {
                self.streamer.send(StreamerCommand::Jog {
                    dx: 0.0,
                    dy: -step,
                    feed,
                });
            }

            gap(ui);
        });
    }
}
