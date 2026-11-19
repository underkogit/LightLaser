//! Font installation.
//!
//! egui ships a proportional and a monospace face; the icon font is added on top of those.
//! A font file chosen in the settings is inserted in front of both families so it wins for
//! ordinary text.

use std::sync::Arc;

/// Installs the icon font plus, optionally, a font file chosen by the user.
pub(crate) fn apply_fonts(ctx: &egui::Context, custom: Option<&str>) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    if let Some(path) = custom {
        match std::fs::read(path) {
            Ok(data) => {
                fonts.font_data.insert(
                    "user_font".to_owned(),
                    Arc::new(egui::FontData::from_owned(data)),
                );

                for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                    fonts
                        .families
                        .entry(family)
                        .or_default()
                        .insert(0, "user_font".to_owned());
                }
            }
            Err(e) => log::error!("font '{}': {}", path, e),
        }
    }

    ctx.set_fonts(fonts);
}
