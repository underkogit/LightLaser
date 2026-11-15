use crate::core::tools::ToolKind;
use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn ui_tool_panel(&mut self, ui: &mut egui::Ui) {
        const BUTTON_SIZE: f32 = 36.0;
        const GAP: f32 = 8.0;

        ui.vertical_centered(|ui| {
            ui.add_space(GAP);

            for kind in [
                ToolKind::Select,
                ToolKind::Line,
                ToolKind::Curve,
                ToolKind::Rect,
                ToolKind::Circle,
            ] {
                let selected = self.tools.current == kind;
                let btn = egui::Button::new(egui::RichText::new(kind.icon()).size(18.0))
                    .selected(selected)
                    .min_size(egui::vec2(BUTTON_SIZE, BUTTON_SIZE));

                if ui.add(btn).on_hover_text(kind.tooltip()).clicked() {
                    self.tools.set(kind);
                }

                ui.add_space(GAP);
            }
        });
    }
}
