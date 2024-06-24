use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use egui::{Align, CentralPanel, Context, FontFamily, FontId, Layout, ProgressBar, RichText, ScrollArea, TextStyle, Ui, Vec2};
use egui::scroll_area::ScrollBarVisibility;

use crate::audio_player::AudioPlayer;
use crate::constants;
use crate::constants::PLAYLIST_DIRECTORY;

#[derive(PartialEq, Default)]
enum Screen {
    #[default]
    Playlist,
    Jukebox,
    FileManager,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct OpenLightsCore {
    #[serde(skip)]
    playlist: String,
    #[serde(skip)]
    current_screen: Screen,
    #[serde(skip)]
    audio_player: AudioPlayer,
    #[serde(skip)]
    progress: f32,
    #[serde(skip)]
    volume: f32,
}

impl Default for OpenLightsCore {
    fn default() -> Self {
        Self {
            playlist: String::from(""),
            current_screen: Screen::default(),
            audio_player: AudioPlayer::new(),
            progress: 0.0,
            volume: 0.5,
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

fn configure_text_styles(ctx: &Context) {
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

impl OpenLightsCore {
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

    fn show_playlist_screen(&mut self, ctx: &Context) {

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.top_menu(ui);
        });

        CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.add(
                    egui::Image::new(egui::include_image!("../assets/open_lights.png"))
                        .max_width(200.0)
                        .rounding(10.0),
                );

                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Select a Playlist").text_style(heading2()).strong());
                });

                ScrollArea::vertical()
                    .auto_shrink([true, true])
                    .max_height(200.)
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        for option in &self.audio_player.playlist_vec {
                            if ui.add(egui::SelectableLabel::new(
                                &self.playlist == option,
                                option,
                            )).clicked() {
                                self.playlist = String::from(option);
                            };
                            ui.add_space(10.);
                        }
                    });

                ui.add_space(30.);
                if ui.add_sized([210., 80.], egui::Button::new("Confirm")).clicked() && self.playlist != *"" {
                    self.audio_player.load_songs_from_playlist(&self.playlist);
                    self.current_screen = Screen::Jukebox;
                };
            });

            ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("Version ");
                    ui.label(constants::VERSION);
                });
            });
        });
    }

    fn top_menu(&mut self, ui: &mut Ui) {
        egui::menu::bar(ui, |ui| {
            egui::widgets::global_dark_light_mode_buttons(ui);

            if ui.button("Playlists").clicked() {
                self.current_screen = Screen::Playlist
            }

            if ui.button("Song Manager").clicked() {
                self.current_screen = Screen::FileManager
            }
        });
    }

    fn show_jukebox_screen(&mut self, ctx: &Context) {
        // Menu Bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.top_menu(ui);
        });
        // Center
        CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                // Playlist
                ui.label(RichText::new("  Playlist  ").text_style(heading2()).strong().underline());

                ScrollArea::vertical()
                    .auto_shrink([true, true])
                    .max_height(200.)
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        let current_song = self.audio_player.get_current_song();
                        for song in &self.audio_player.song_vec.clone() {
                            if ui.add(egui::SelectableLabel::new(
                                &current_song == song,
                                format!("{} by {}", song.name, song.artist),
                            )).clicked() {
                                self.audio_player.song_override(song);
                            };
                            ui.add_space(10.);
                        }
                    });
            });
        });

        egui::TopBottomPanel::bottom("bottom_taskbar").show(ctx, |ui| {
            ui.set_height(150.0);

            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.separator();

                // Song Title
                ui.label("Song by Artist");

                // Loading Bar
                Self::centered_song_progress_display(self, ui);

                // Buttons
                Self::centered_buttons(self, ui);

                // Slider
                Self::centered_volume_slider(self, ui);

            });
        });
    }

    fn show_file_manager_screen(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.top_menu(ui);
        });
        CentralPanel::default().show(ctx, |ui| {
            let mut explorer = FileExplorer::new(&**PLAYLIST_DIRECTORY);
            explorer.show(ui);
        });
    }

    fn centered_song_progress_display(&mut self, ui: &mut Ui) {
        let bar = ProgressBar::new(self.progress).animate(false);

        // Get the width of the text to center it
        let text = format!("{} / {}", Self::format_time(75), Self::format_time(120)); // Example times

        // Layout the progress bar
        ui.vertical_centered(|ui| {
            // Add the progress bar
            let response = ui.add(bar);

            // Calculate the position to center the text
            let rect = response.rect;
            let text_pos = egui::pos2(
                rect.center().x,
                rect.center().y,
            );

            // Paint the centered text
            ui.painter().text(
                text_pos,
                egui::Align2::CENTER_CENTER,
                text,
                FontId::proportional(12.),
                ui.style().visuals.text_color(),
            );
        });
    }

    pub fn set_progress(&mut self, seconds: i32) {
        self.progress = (seconds as f64 / self.audio_player.get_current_song().duration) as f32;
    }

    fn format_time(seconds: i32) -> String {
        let minutes = seconds / 60;
        let remaining_seconds = seconds % 60;
        format!("{:02}:{:02}", minutes, remaining_seconds)
    }

    fn centered_buttons(&mut self, ui: &mut Ui) {
        let button_count = 5;
        let button_size = Vec2::new(40.0, 40.0); // Width and height of each button
        let button_spacing = ui.spacing().item_spacing.x;
        let total_button_width = button_count as f32 * button_size.x + (button_count as f32 - 1.0) * button_spacing;

        // Add space to the left to center the buttons
        let available_width = ui.available_width();
        let left_padding = (available_width - total_button_width) / 2.0;

        ui.horizontal(|ui| {
            ui.add_space(left_padding);

            if ui.add_sized(button_size, egui::Button::new("⏭")).clicked() {
                self.audio_player.next_song();
            }

            if ui.add_sized(button_size, egui::Button::new("⏪")).clicked() {
                self.audio_player.set_position(Duration::from_secs(0));
            }

            if ui.add_sized(button_size, egui::Button::new(if self.audio_player.playing { "⏸" } else { "▶" })).clicked() {
                if self.audio_player.playing {
                    self.audio_player.pause();
                } else {
                    self.audio_player.play();
                }
            }

            if ui.add_sized(button_size, egui::Button::new("🔀")).clicked() {
                self.audio_player.shuffle();
            }

            if ui.add_sized(button_size, egui::SelectableLabel::new(self.audio_player.looping, "🔁")).clicked() {
                self.audio_player.toggle_looping();
            }
        });
    }

    fn centered_volume_slider(&mut self, ui: &mut Ui) {
        let slider_width = Vec2::new(200., 50.);
        let available_width = ui.available_width();
        let left_padding = (available_width - slider_width.x) / 2.0;

        ui.add_space(left_padding);

        if ui.add_sized(slider_width, egui::Slider::new(&mut self.volume, 0.0..=100.).suffix("%")).drag_stopped {
            self.audio_player.set_volume((self.volume) / 100.0);
        }
    }
}

impl eframe::App for OpenLightsCore {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        match self.current_screen {
            Screen::Playlist => self.show_playlist_screen(ctx),
            Screen::Jukebox => self.show_jukebox_screen(ctx),
            Screen::FileManager => self.show_file_manager_screen(ctx),
        }
    }

    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // TODO Determine if saving is important
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}

struct FileExplorer {
    current_path: PathBuf,
    entries: Vec<PathBuf>,
    error_message: Option<String>,
}

impl FileExplorer {
    fn new(start_path: &str) -> Self {
        let start_path = PathBuf::from(start_path);
        let entries = Self::read_directory(&start_path).unwrap_or_else(|_| vec![]);
        Self {
            current_path: start_path,
            entries,
            error_message: None,
        }
    }

    fn read_directory(path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut entries = vec![];
        for entry in fs::read_dir(path)? {
            let entry = entry?.path();
            entries.push(entry);
        }
        Ok(entries)
    }

    fn navigate_to(&mut self, path: &PathBuf) {
        match Self::read_directory(path) {
            Ok(entries) => {
                self.current_path = path.clone();
                self.entries = entries;
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Error: {}", e));
            }
        }
    }

    fn go_up(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            self.navigate_to(&parent.to_owned());
        }
    }

    fn delete_entry(&mut self, path: &Path) {
        if let Err(e) = fs::remove_file(path) {
            self.error_message = Some(format!("Error deleting file: {}", e));
        } else {
            self.navigate_to(&self.current_path.clone());
        }
    }

    fn show(&mut self, ui: &mut Ui) {
        ui.label(format!("Current Path: {:?}", self.current_path));

        if ui.button("Go Up").clicked() {
            self.go_up();
        }

        if ui.button("Add File").clicked() {
            // TODO Add a way to open file manager
        }

        ScrollArea::vertical().show(ui, |ui| {
            for entry in &self.entries.clone() {
                let file_name_str = entry.to_string_lossy();
                ui.horizontal(|ui| {
                    if entry.is_dir() {
                        if ui.button(format!("📁 {}", file_name_str)).clicked() {
                            self.navigate_to(entry);
                        }
                    } else {
                        ui.label(format!("📄 {}", file_name_str));
                        if ui.button("Delete").clicked() {
                            self.delete_entry(entry);
                        }
                    }
                });
            }
        });

        if let Some(ref error_message) = self.error_message {
            ui.label(error_message);
        }
    }

    fn handle_file_input(&mut self, file_name: &str) {
        let destination = self.current_path.join(file_name);
        if let Err(e) = fs::write(destination, "") { // Create an empty file
            self.error_message = Some(format!("Error creating file: {}", e));
        } else {
            self.navigate_to(&self.current_path.clone());
        }
    }
}
