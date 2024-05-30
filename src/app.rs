use egui::{FontFamily, FontId, RichText, TextStyle};
use egui::scroll_area::ScrollBarVisibility;

use crate::constants;

/// We derive Deserialize/Serialize, so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    playlist: String,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            playlist: String::from(""),
        }
    }
}

#[inline]
fn heading2() -> TextStyle {
    TextStyle::Name("Heading2".into())
}

#[inline]
fn heading3() -> TextStyle {
    TextStyle::Name("ContextHeading".into())
}

fn configure_text_styles(ctx: &egui::Context) {
    use FontFamily::Proportional;
    use TextStyle::*;

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (Heading, FontId::new(100.0, Proportional)),
        (heading2(), FontId::new(30.0, Proportional)),
        (heading3(), FontId::new(20.0, Proportional)),
        (Body, FontId::new(18.0, Proportional)),
        (Monospace, FontId::new(14.0, Proportional)),
        (Button, FontId::new(14.0, Proportional)),
        (Small, FontId::new(10.0, Proportional)),
    ]
        .into();
    ctx.set_style(style);
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.
        configure_text_styles(&cc.egui_ctx);
        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl eframe::App for TemplateApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            top_menu(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                heading(ui);
                select_playlist_frame(ui);

                egui::ScrollArea::vertical()
                    .auto_shrink([true, true])
                    .max_height(200.)
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        for option in TEST_STRING_OPTIONS {
                            if ui.add(egui::SelectableLabel::new(
                                self.playlist == option,
                                option,
                            )).clicked() {
                                self.playlist = option.into();
                            };
                            ui.add_space(10.);
                        }
                    });

                ui.add_space(30.);
                if ui.add_sized([210., 80.], egui::Button::new("Confirm")).clicked() {

                };
            });

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                version(ui);
            });
        });
    }

    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}

fn version(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Version ");
        ui.label(constants::VERSION);
    });
}

fn heading(ui: &mut egui::Ui) {
    ui.add(
        egui::Image::new(egui::include_image!("../assets/open_lights.png"))
            .max_width(200.0)
            .rounding(10.0),
    );
}

const TEST_STRING_OPTIONS: [&str; 8] = [
    "Option 1", "Option 2", "Option 3", "Option 4", "Option 5", "Option 6", "Option 7", "Option 8",
];

fn select_playlist_frame(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("Select a Playlist").text_style(heading2()).strong());
    });
}

fn top_menu(ui: &mut egui::Ui) {
    egui::menu::bar(ui, |ui| {
        egui::widgets::global_dark_light_mode_buttons(ui);

        if ui.button("Playlists").clicked() {

        }
    });
}
