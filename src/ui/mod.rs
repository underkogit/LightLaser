use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use rust_i18n::t;

use crate::comm::{GrblEvent, GrblStatus, Streamer, StreamerState};
use crate::core::layer::Project;
use crate::logger::LogEntry;
use crate::ui::{
    app_config::AppConfig, history::History, panels::canvas::CanvasState,
    panels::right_panel::RightPanelState, selection::Selection, tools_state::ToolsState,
    widget::status_bar::StatusBar,
};

pub mod app_config;
mod canvas_transform;
mod colors;
mod history;
mod panels;
mod project_io;
mod selection;
mod tools_state;
mod widget;

pub(crate) use canvas_transform::CanvasTransform;

#[derive(Debug, Default)]
pub struct UiState {
    pub selection: Selection,
}

pub struct LaserApp {
    pub(crate) project: Project,
    pub(crate) current_project_path: Option<PathBuf>,

    pub(crate) canvas: CanvasState,
    pub(crate) ui_state: UiState,
    pub(crate) status_bar: StatusBar,
    pub(crate) tools: ToolsState,
    pub(crate) history: History,
    pub(crate) right_panel_state: RightPanelState,

    pub(crate) streamer: Streamer,
    pub(crate) grbl_status: Option<GrblStatus>,
    pub(crate) streamer_state: StreamerState,
    pub(crate) events_rx: Receiver<GrblEvent>,
    pub(crate) events_tx: Sender<GrblEvent>,
    pub(crate) job_progress: (usize, usize),

    pub(crate) log_buffer: Arc<Mutex<VecDeque<LogEntry>>>,
    pub(crate) app_config: AppConfig,
    pub(crate) config_folder: PathBuf,

    pub(crate) connection_dialog_open: bool,
    pub(crate) export_dialog_open: bool,

    pub(crate) clipboard: Option<crate::core::layer::Layer>,
}

impl LaserApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        log_buffer: Arc<Mutex<VecDeque<LogEntry>>>,
        app_config: AppConfig,
        config_folder: PathBuf,
        streamer: Streamer,
        events_rx: Receiver<GrblEvent>,
        events_tx: Sender<GrblEvent>,
    ) -> Self {
        Self {
            project: Project::new(),
            current_project_path: None,
            canvas: CanvasState::default(),
            ui_state: UiState::default(),
            status_bar: StatusBar::default(),
            tools: ToolsState::default(),
            history: History::default(),
            right_panel_state: RightPanelState::default(),
            streamer,
            grbl_status: None,
            streamer_state: StreamerState::Idle,
            events_rx,
            events_tx,
            job_progress: (0, 0),
            log_buffer,
            app_config,
            config_folder,
            connection_dialog_open: false,
            export_dialog_open: false,
            clipboard: None,
        }
    }

    pub(crate) fn push_history(&mut self) {
        self.history.push(&self.project);
    }

    pub(crate) fn undo(&mut self) {
        if let Some(prev) = self.history.undo(&self.project) {
            self.project = prev;
            self.ui_state.selection.clear();
            log::info!("Undo");
        } else {
            log::info!("Nothing to undo");
        }
    }

    pub(crate) fn redo(&mut self) {
        if let Some(next) = self.history.redo(&self.project) {
            self.project = next;
            self.ui_state.selection.clear();
            log::info!("Redo");
        } else {
            log::info!("Nothing to redo");
        }
    }

    pub(crate) fn drain_grbl_events(&mut self) {
        while let Ok(ev) = self.events_rx.try_recv() {
            match ev {
                GrblEvent::Connected => {
                    log::info!("{}", t!("comm.info.connected"));
                }
                GrblEvent::Disconnected => {
                    log::info!("{}", t!("comm.info.disconnected"));
                    self.grbl_status = None;
                }
                GrblEvent::Status(st) => {
                    self.grbl_status = Some(st);
                }
                GrblEvent::Ok => {}
                GrblEvent::Error(e) => {
                    log::error!("GRBL: {}", e);
                }
                GrblEvent::Alarm(a) => {
                    log::error!("GRBL ALARM: {}", a);
                }
                GrblEvent::JobStarted { total_lines } => {
                    log::info!("Job started: {} lines", total_lines);
                    self.streamer_state = StreamerState::Running;
                    self.job_progress = (0, total_lines);
                }
                GrblEvent::JobProgress { sent, total } => {
                    self.job_progress = (sent, total);
                }
                GrblEvent::JobFinished { success } => {
                    log::info!("Job finished: success={}", success);
                    self.streamer_state = StreamerState::Idle;
                }
            }
        }
    }
}

impl eframe::App for LaserApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_grbl_events();

        let screen_rect = ui.ctx().viewport_rect();
        self.app_config.window_width = screen_rect.width();
        self.app_config.window_height = screen_rect.height();

        let mut copy = false;
        let mut paste = false;
        let mut duplicate = false;
        let mut save = false;
        let mut save_as = false;
        let mut dropped_paths: Vec<PathBuf> = Vec::new();

        ui.ctx().input(|i| {
            for event in &i.events {
                if let egui::Event::Copy = event {
                    copy = true;
                }
                if let egui::Event::Paste(_) = event {
                    paste = true;
                }
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    if modifiers.command {
                        match key {
                            egui::Key::D => duplicate = true,
                            egui::Key::S => {
                                if modifiers.shift {
                                    save_as = true;
                                } else {
                                    save = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            for f in &i.raw.dropped_files {
                dropped_paths.push(f.path().to_path_buf());
            }
        });

        if copy {
            log::info!("Global trigger: Copy");
            self.copy_selection();
        }
        if paste {
            log::info!("Global trigger: Paste");
            self.paste_selection();
        }
        if duplicate {
            log::info!("Global trigger: Duplicate");
            self.duplicate_selection();
        }
        if save {
            log::info!("Global trigger: Save");
            self.save_project_interactive();
        }
        if save_as {
            log::info!("Global trigger: Save As");
            self.save_project_as_interactive();
        }

        for path in dropped_paths {
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            match ext.as_str() {
                "png" | "jpg" | "jpeg" | "bmp" => {
                    log::info!("Dropped image: {}", path.display());
                    self.add_image_from_path(&path);
                    self.center_view();
                }
                "svg" => {
                    log::info!("Dropped svg: {}", path.display());
                    self.add_svg_from_path(&path);
                    self.center_view();
                }
                "nc" | "gcode" | "gbr" | "txt" => {
                    log::info!("Dropped gcode: {}", path.display());
                    self.add_gcode_from_path(&path);
                    self.center_view();
                }
                "toml" => {
                    log::info!("Dropped project: {}", path.display());
                    let _ = self.load_project_from(&path);
                    self.current_project_path = Some(path);
                    self.center_view();
                }
                _ => {
                    log::warn!("Dropped unsupported file: {}", path.display());
                }
            }
        }

        self.ui_top_bar(ui);
        self.ui_bottom_bar(ui);

        egui::Panel::left("left_tools")
            .resizable(false)
            .default_size(52.0)
            .min_size(52.0)
            .max_size(52.0)
            .show(ui, |ui| {
                self.ui_tool_panel(ui);
            });

        egui::Panel::right("right_panel")
            .resizable(true)
            .default_size(400.0)
            .min_size(300.0)
            .max_size(600.0)
            .show(ui, |ui| {
                self.ui_right_panel(ui);
            });

        egui::CentralPanel::default().show(ui, |ui| {
            self.ui_canvas(ui);
        });

        self.ui_connection_dialog(ui.ctx());
        self.ui_export_dialog(ui.ctx());
    }

    fn on_exit(&mut self) {
        self.app_config.save();
    }
}

impl LaserApp {
    fn copy_selection(&mut self) {
        let sel = self.ui_state.selection.clone();
        let Some(li) = sel.layer else {
            log::warn!("COPY FAILED: No layer selected");
            return;
        };
        let Some(layer) = self.project.layers.get(li) else {
            log::warn!("COPY FAILED: Layer {} not found", li);
            return;
        };

        if let Some(si) = sel.shape {
            if let Some(shape) = layer.shapes.get(si) {
                let mut layer_clone = layer.clone();
                layer_clone.shapes = vec![shape.clone()];
                layer_clone.recompute_bounds();
                self.clipboard = Some(layer_clone);
                log::info!("COPY SUCCESS: Shape {} from layer {} copied", si, li);
            } else {
                log::warn!("COPY FAILED: Shape {} not found in layer {}", si, li);
            }
        } else {
            self.clipboard = Some(layer.clone());
            log::info!("COPY SUCCESS: Layer {} copied", li);
        }
    }

    fn paste_selection(&mut self) {
        let clipboard = self.clipboard.clone();

        if let Some(mut layer_src) = clipboard {
            log::info!(
                "PASTE ATTEMPT: Found layer in memory with {} shapes",
                layer_src.shapes.len()
            );
            self.push_history();
            let offset = self.duplicate_offset();
            for shape in &mut layer_src.shapes {
                for n in &mut shape.nodes {
                    n.translate(offset, offset);
                }
            }
            layer_src.recompute_bounds();
            layer_src.name = format!("{} copy", layer_src.name);
            self.project.add_layer(layer_src);
            let new_idx = self.project.layers.len() - 1;
            self.ui_state.selection = Selection {
                layer: Some(new_idx),
                shape: None,
                node: None,
                handle: None,
            };
            log::info!("PASTE SUCCESS: Layer pasted as index {}", new_idx);
        } else {
            log::warn!("PASTE FAILED: Memory clipboard is empty");
        }
    }

    fn duplicate_selection(&mut self) {
        let sel = self.ui_state.selection.clone();
        let Some(li) = sel.layer else {
            log::warn!("DUPLICATE FAILED: No layer selected");
            return;
        };
        let Some(layer_src) = self.project.layers.get(li) else {
            log::warn!("DUPLICATE FAILED: Layer {} not found", li);
            return;
        };

        if let Some(si) = sel.shape {
            let Some(shape_src) = layer_src.shapes.get(si).cloned() else {
                log::warn!("DUPLICATE FAILED: Shape {} not found", si);
                return;
            };
            self.push_history();
            let offset = self.duplicate_offset();
            let mut new_shape = shape_src;
            for n in &mut new_shape.nodes {
                n.translate(offset, offset);
            }
            if let Some(layer_dst) = self.project.layers.get_mut(li) {
                layer_dst.shapes.push(new_shape);
                layer_dst.recompute_bounds();
                let new_si = layer_dst.shapes.len() - 1;
                self.ui_state.selection = Selection {
                    layer: Some(li),
                    shape: Some(new_si),
                    node: None,
                    handle: None,
                };
                log::info!("DUPLICATE SUCCESS: Shape {} duplicated", si);
            }
            return;
        }

        let mut new_layer = layer_src.clone();
        new_layer.name = format!("{} copy", new_layer.name);
        self.push_history();
        let offset = self.duplicate_offset();
        for shape in &mut new_layer.shapes {
            for n in &mut shape.nodes {
                n.translate(offset, offset);
            }
        }
        new_layer.recompute_bounds();
        self.project.add_layer(new_layer);
        let new_idx = self.project.layers.len() - 1;
        self.ui_state.selection = Selection {
            layer: Some(new_idx),
            shape: None,
            node: None,
            handle: None,
        };
        log::info!("DUPLICATE SUCCESS: Layer {} duplicated", li);
    }

    fn duplicate_offset(&self) -> f64 {
        let step = if self.tools.snap_enabled && self.tools.snap_mm > 0.0 {
            self.tools.snap_mm
        } else {
            1.0
        };
        step.max(0.5)
    }
}
