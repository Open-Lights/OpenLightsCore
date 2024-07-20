#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use open_lights_core::constants::PLAYLIST_DIRECTORY;
use std::fs;
use std::path::Path;

fn main() -> eframe::Result<()> {
    fs::create_dir_all(Path::new(&*PLAYLIST_DIRECTORY)).unwrap();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_close_button(false)
            .with_maximize_button(false)
            .with_minimize_button(false)
            .with_resizable(false)
            .with_title_shown(false)
            .with_fullscreen(true)
            .with_icon(
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon.ico")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    eframe::run_native(
        "Open Lights",
        native_options,
        Box::new(move |cc| {
            cc.egui_ctx
                .send_viewport_cmd(egui::viewport::ViewportCommand::Fullscreen(true));
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(open_lights_core::OpenLightsCore::new(cc)))
        }),
    )
}
