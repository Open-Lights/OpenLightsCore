#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use open_lights_core::startup;

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    startup::initialize_files();
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_close_button(false)
            .with_maximize_button(false)
            .with_minimize_button(false)
            .with_resizable(false)
            .with_title_shown(false)
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon.ico")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    eframe::run_native(
        "Open Lights",
        native_options,
        Box::new(move |cc| {
            cc.egui_ctx.send_viewport_cmd(egui::viewport::ViewportCommand::Fullscreen(true));
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(open_lights_core::OpenLightsCore::new(cc)))
        }),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    startup::initialize_files();
    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "the_canvas_id", // hardcode it
                web_options,
                Box::new(|cc| {
                    egui_extras::install_image_loaders(&cc.egui_ctx);
                    Ok(Box::new(open_lights_core::OpenLightsCore::new(cc)))
                }),
            )
            .await
            .expect("failed to start eframe");
    });
}
