use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use rust_i18n::t;

use crate::comm::{GrblEvent, GrblStatus, Streamer, StreamerState};
use crate::core::layer::Project;
use crate::logger::LogEntry;
use crate::ui::{
    app_config::AppConfig,
    history::History,
    panels::canvas::{duplicate_selected, copy_selection, paste_selection, CanvasState},
    panels::right_panel::RightPanelState,
    selection::Selection,
    tools_state::ToolsState,
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

        self.handle_shortcuts(ui.ctx());

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
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = ctx.input(|i| i.modifiers.command);
        let shift = ctx.input(|i| i.modifiers.shift);

        if cmd && ctx.input(|i| i.key_pressed(egui::Key::Z)) {
            if shift {
                log::info!("Shortcut: Redo (Ctrl+Shift+Z)");
                self.redo();
            } else {
                log::info!("Shortcut: Undo (Ctrl+Z)");
                self.undo();
            }
        }

        if cmd && ctx.input(|i| i.key_pressed(egui::Key::Y)) {
            log::info!("Shortcut: Redo (Ctrl+Y)");
            self.redo();
        }

        if cmd && ctx.input(|i| i.key_pressed(egui::Key::S)) {
            if shift {
                log::info!("Shortcut: Save As");
                self.save_project_as_interactive();
            } else {
                log::info!("Shortcut: Save");
                self.save_project_interactive();
            }
        }

        if cmd && ctx.input(|i| i.key_pressed(egui::Key::O)) {
            log::info!("Shortcut: Open");
            self.open_project_interactive();
        }

        if cmd && ctx.input(|i| i.key_pressed(egui::Key::N)) {
            log::info!("Shortcut: New Project");
            self.new_project();
        }

        if cmd && ctx.input(|i| i.key_pressed(egui::Key::E)) {
            log::info!("Shortcut: Export G-code");
            self.export_dialog_open = true;
        }

        if cmd && ctx.input(|i| i.key_pressed(egui::Key::D)) {
            log::info!("Shortcut: Duplicate");
            duplicate_selected(self);
        }

        if cmd && ctx.input(|i| i.key_pressed(egui::Key::C)) {
            log::info!("Shortcut: Copy");
            copy_selection(self);
        }

        if cmd && ctx.input(|i| i.key_pressed(egui::Key::V)) {
            log::info!("Shortcut: Paste");
            paste_selection(self);
        }
    }
}