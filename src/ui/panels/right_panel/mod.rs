mod jog_crosshair;
mod layers;
mod machine;
mod properties;

use egui::Panel;
use rust_i18n::t;

use crate::ui::LaserApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightPanelTab {
    #[default]
    Layers,
    Properties,
}

pub struct RightPanelState {
    pub active_tab: RightPanelTab,

    /// Layer whose position is being dragged or typed in right now, so one continuous edit
    /// adds a single entry to the undo history instead of one per frame.
    pub position_edit_layer: Option<usize>,
}

impl Default for RightPanelState {
    fn default() -> Self {
        Self {
            active_tab: RightPanelTab::Layers,
            position_edit_layer: None,
        }
    }
}

impl LaserApp {
    pub(crate) fn ui_right_panel(&mut self, ui: &mut egui::Ui) {
        Panel::bottom("machine_control_bottom")
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
            ui.selectable_value(&mut active_tab, RightPanelTab::Properties, "Properties");
        });
        ui.separator();

        self.right_panel_state.active_tab = active_tab;

        // A position edit cannot continue once the panel stops showing that layer, so the next
        // edit starts a fresh undo step instead of extending the previous one.
        let position_field_shown = active_tab == RightPanelTab::Properties
            && self
                .ui_state
                .selection
                .layer
                .is_some_and(|i| i < self.project.layers.len());
        if !position_field_shown {
            self.right_panel_state.position_edit_layer = None;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);
            match self.right_panel_state.active_tab {
                RightPanelTab::Layers => {
                    self.ui_layer_list(ui);
                }
                RightPanelTab::Properties => {
                    let selected = self.ui_state.selection.layer;
                    if let Some(idx) = selected {
                        if idx < self.project.layers.len() {
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
}
