use std::sync::mpsc;

use rust_i18n::t;

use crate::{
    comm::{GrblEvent, Streamer},
    dirs::get_config_dir,
    logger::MultiLogger,
    ui::{LaserApp, app_config::AppConfig},
};

use crate::i18n_backend::FileBackend;
use eframe;

mod comm;
mod core;
mod dirs;
mod i18n_backend;
mod logger;
mod ui;

const VERSION: &str = env!("CARGO_PKG_VERSION");

rust_i18n::i18n!("locales", fallback = "en", backend = FileBackend::new());

fn main() {
    let log_buffer = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new()));
    let logger = MultiLogger::new(log_buffer.clone());
    let max_level = logger.console.filter();

    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(max_level);

    log::info!("LightLaser v{} starting", VERSION);

    let config_folder = match get_config_dir() {
        Some(f) => f,
        None => {
            log::error!("{}", t!("main.error.get_config_folder"));
            return;
        }
    };

    if !config_folder.exists() {
        if let Err(e) = std::fs::create_dir_all(&config_folder) {
            log::error!(
                "{}",
                t!("main.error.create_config_folder", e = e.to_string())
            );
            return;
        }
    }

    let app_config = AppConfig::new(&config_folder);
    rust_i18n::set_locale(&app_config.app_locale);

    let (events_tx, events_rx) = mpsc::channel::<GrblEvent>();

    let streamer = Streamer::spawn(events_tx.clone());

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("LightLaser - v{VERSION}"))
            .with_inner_size([1200.0, 720.0])
            .with_resizable(true)
            .with_position([100.0, 100.0]),
        ..eframe::NativeOptions::default()
    };

    let _ = eframe::run_native(
        "LightLaser",
        native_options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);

            cc.egui_ctx.all_styles_mut(|style| {
                style.spacing.item_spacing = egui::vec2(8.0, 2.0);
                style.spacing.button_padding = egui::vec2(12.0, 2.0);
                style.spacing.interact_size = egui::vec2(44.0, 32.0);
                style.spacing.icon_width = 16.0;
                style.spacing.icon_width_inner = 10.0;
            });

            Ok(Box::new(LaserApp::new(
                cc,
                log_buffer,
                app_config,
                config_folder,
                streamer,
                events_rx,
                events_tx,
            )))
        }),
    );
}
