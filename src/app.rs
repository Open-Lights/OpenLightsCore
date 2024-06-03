use std::fs;
use std::fs::DirEntry;
use std::path::{Path, PathBuf};
use std::time::Duration;

use egui::{CentralPanel, FontFamily, FontId, RichText, ScrollArea, TextStyle, Ui};
use egui::scroll_area::ScrollBarVisibility;

use crate::audio_player::{AudioPlayer, load_songs_from_playlist};
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
}

impl Default for OpenLightsCore {
    fn default() -> Self {
        Self {
            playlist: String::from(""),
            current_screen: Screen::Playlist,
            audio_player: AudioPlayer::new(),
            progress: 0.0,
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

    fn show_playlist_screen(&mut self, ctx: &egui::Context) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            self.top_menu(ui);
        });

        CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
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
                                &*option,
                            )).clicked() {
                                self.playlist = String::from(option);
                            };
                            ui.add_space(10.);
                        }
                    });

                ui.add_space(30.);
                if ui.add_sized([210., 80.], egui::Button::new("Confirm")).clicked() {
                    if self.playlist != String::from("") {
                        load_songs_from_playlist(&self.playlist);
                        self.current_screen = Screen::Jukebox;
                    }
                };
            });

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
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

    fn show_jukebox_screen(&mut self, ctx: &egui::Context) {
        // Menu Bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.top_menu(ui);
        });
        // Center
        CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                // Playlist
                ScrollArea::vertical()
                    .auto_shrink([true, true])
                    .max_height(200.)
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        let current_song = self.audio_player.get_current_song();
                        for song in &self.audio_player.song_vec {
                            if ui.add(egui::SelectableLabel::new(
                                &current_song == song,
                                format!("{} by {}", song.name, song.artist),
                            )).clicked() {
                                self.audio_player.song_override(song);
                            };
                            ui.add_space(10.);
                        }
                    });

                // Loading Bar
                let bar = egui::ProgressBar::new(self.progress)
                    .text(format!("{} / {}", Self::format_time(self.audio_player.get_current_position_seconds()), Self::format_time(self.audio_player.get_current_song().duration as i32)))
                    .animate(false);
                ui.add(bar);
                // Controls
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if ui.button(if self.audio_player.playing { "⏸️" } else { "▶️" }).clicked() {
                        if self.audio_player.playing {
                            self.audio_player.pause();
                        } else {
                            self.audio_player.play();
                        }
                    }

                    if ui.button("⏭️").clicked() {
                        self.audio_player.next_song();
                    }

                    if ui.button("🔀").clicked() {
                        self.audio_player.shuffle();
                    }

                    if ui.button("⏪").clicked() {
                        self.audio_player.set_position(Duration::from_secs(0));
                    }

                    if ui.selectable_label(self.audio_player.looping, "🔁").clicked() {
                        self.audio_player.looping = !self.audio_player.looping;
                    }
                })
            });
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

    fn show_file_manager_screen(&mut self, ctx: &egui::Context) {
        CentralPanel::default().show(ctx, |ui| {
            let mut explorer = FileExplorer::new(PLAYLIST_DIRECTORY);
            explorer.show(ui);
        });
    }
}

impl eframe::App for OpenLightsCore {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
    entries: Vec<DirEntry>,
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

    fn read_directory(path: &Path) -> Result<Vec<DirEntry>, std::io::Error> {
        let mut entries = vec![];
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            entries.push(entry);
        }
        Ok(entries)
    }

    fn navigate_to(&mut self, path: PathBuf) {
        match Self::read_directory(&path) {
            Ok(entries) => {
                self.current_path = path;
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
            self.navigate_to(parent.to_path_buf());
        }
    }

    fn delete_entry(&mut self, path: &Path) {
        if let Err(e) = fs::remove_file(path) {
            self.error_message = Some(format!("Error deleting file: {}", e));
        } else {
            self.navigate_to(self.current_path.clone());
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
            for entry in &self.entries {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                ui.horizontal(|ui| {
                    if entry.path().is_dir() {
                        if ui.button(format!("📁 {}", file_name_str)).clicked() {
                            self.navigate_to(entry.path());
                        }
                    } else {
                        ui.label(format!("📄 {}", file_name_str));
                        if ui.button("Delete").clicked() {
                            self.delete_entry(&entry.path());
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
        if let Err(e) = fs::write(&destination, "") { // Create an empty file
            self.error_message = Some(format!("Error creating file: {}", e));
        } else {
            self.navigate_to(self.current_path.clone());
        }
    }
}
