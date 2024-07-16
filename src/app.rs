use std::cmp::PartialEq;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc, Mutex};
use std::sync::atomic::{AtomicI8, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::epaint::Color32;
use egui::{Align, CentralPanel, Context, FontFamily, FontId, Layout, ProgressBar, RichText, ScrollArea, TextStyle, Ui, Vec2};
use egui::scroll_area::ScrollBarVisibility;
use egui::TextStyle::Body;
use walkdir::WalkDir;

#[cfg(unix)]
use rppal::gpio::Gpio;

use crate::audio_player::{AudioPlayer, gather_songs_from_path, get_atomic_float, locate_playlists, Song, start_worker_thread};
use crate::bluetooth::{BluetoothDevice, BluetoothDevices};
use crate::constants;
use crate::constants::{AudioThreadActions, PLAYLIST_DIRECTORY};
#[cfg(unix)]
use crate::lights::{interface_gpio, LightType};

#[derive(PartialEq, Default)]
enum Screen {
    #[default]
    Playlist,
    Jukebox,
    FileManager,
    Audio,
    Debug,
}

pub struct OpenLightsCore {
    playlist_vec: Vec<String>,
    song_vec_cache: Option<Vec<Song>>,
    playlist: String,
    current_screen: Screen,
    file_explorer: FileExplorer,
    pub audio_player: Arc<Mutex<AudioPlayer>>,
    messenger: Sender<AudioThreadActions>,
    song_vec_receiver: Receiver<Vec<Song>>,
    volume: Arc<AtomicI8>,
    clicked_index: Arc<AtomicUsize>,
    selected_bt_device: i8,
    cached_selected_bt_device: Option<BluetoothDevice>,
    clicked_squares: HashSet<usize>,
    notifications: VecDeque<Notification>,
    bluetooth: BluetoothDevices,
    bt_receiver: Receiver<Notification>,
}

impl Default for OpenLightsCore {
    fn default() -> Self {
        let volume = Arc::new(AtomicI8::new(100));
        let clicked_index = Arc::new(AtomicUsize::new(0));
        let(tx_song_vec, rx_song_vec) = mpsc::channel();
        let audio_player = Arc::new(Mutex::new(AudioPlayer::new(Arc::clone(&volume), Arc::clone(&clicked_index))));

        let (tx_bt, rx_bt) = mpsc::channel();
        let bluetooth = BluetoothDevices::new(tx_bt);

        let (tx, rx) = mpsc::channel();
        start_worker_thread(Arc::clone(&audio_player), rx, tx_song_vec);

        Self {
            playlist_vec: locate_playlists(),
            song_vec_cache: None,
            playlist: String::from(""),
            current_screen: Screen::default(),
            file_explorer: FileExplorer::new(),
            audio_player,
            messenger: tx,
            song_vec_receiver: rx_song_vec,
            volume,
            clicked_index,
            selected_bt_device: -1,
            cached_selected_bt_device: None,
            clicked_squares: HashSet::new(),
            notifications: VecDeque::new(),
            bluetooth,
            bt_receiver: rx_bt,
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

#[inline]
fn notification_font() -> TextStyle {
    TextStyle::Name("Notification".into())
}

fn configure_text_styles(ctx: &Context) {
    use FontFamily::Proportional;
    use TextStyle::*;

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (Heading, FontId::new(100.0, Proportional)),
        (heading2(), FontId::new(30.0, Proportional)),
        (heading3(), FontId::new(20.0, Proportional)),
        (notification_font(), FontId::new(12.0, Proportional)),
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

        Default::default()
    }

    fn show_playlist_screen(&mut self, ctx: &Context) {

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            let _ = &self.top_menu(ui);
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
                        if !self.playlist_vec.is_empty() {
                            for (index, option) in self.playlist_vec.iter().enumerate() {
                                if ui.add(egui::SelectableLabel::new(
                                    self.clicked_index.load(Ordering::Relaxed) == index,
                                    option,
                                )).clicked() {
                                    self.playlist = option.clone();
                                    self.clicked_index.store(index, Ordering::Relaxed);
                                };
                                ui.add_space(10.);
                            }

                            ui.add_space(30.);
                            if ui.add_sized([210., 80.], egui::Button::new("Confirm")).clicked() && self.playlist != "" {
                                if self.quick_playlist_valid() {
                                    self.song_vec_cache = None;
                                    self.messenger.send(AudioThreadActions::LoadFromPlaylist).unwrap();
                                    self.current_screen = Screen::Jukebox;
                                } else {
                                    let notification = Notification {
                                        title: "Invalid Playlist".to_string(),
                                        message: format!("The playlist {} does not contain any songs. \
                                        Please add songs inside of a folder named the same as the song inside of the playlist folder. \
                                        Ex: /open_lights/playlists/{}/SONG_NAME/SONG_NAME.wav", self.playlist, self.playlist),
                                        timer: Timer::new(Duration::from_secs(30)),
                                        id: fastrand::i32(0..i32::MAX),
                                    };
                                    self.notifications.push_front(notification);
                                }
                            };
                        } else {
                            ui.add_space(30.);
                            ui.add(egui::Label::new(format!("Please add a playlist folder in {}", &**PLAYLIST_DIRECTORY)));
                            if ui.add_sized([210., 80.], egui::Button::new("Create Playlist")).clicked() {
                                let mut path = PathBuf::from(&&*PLAYLIST_DIRECTORY);
                                path.push("Playlist");
                                fs::create_dir(path).unwrap();
                                self.playlist_vec = locate_playlists();
                            }
                        }
                    });
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

    fn quick_playlist_valid(&mut self) -> bool {
        let path = format!("{}{}/", &**PLAYLIST_DIRECTORY, &self.playlist);
        for file in WalkDir::new(path).min_depth(2).max_depth(3) {
            let song_file = file.unwrap();
            let song_path = song_file.path().to_str().expect("Invalid UTF-8 sequence");

            // Check if the file is a WAV file
            if song_path.ends_with(".wav") {
                return true;
            }
        }
        false
    }

    fn top_menu(&mut self, ui: &mut Ui) {
        egui::menu::bar(ui, |ui| {
            egui::widgets::global_dark_light_mode_buttons(ui);

            if ui.button("Playlists").clicked() {
                self.messenger.send(AudioThreadActions::Reset).unwrap();
                self.current_screen = Screen::Playlist;
            }

            if ui.button("Song Manager").clicked() {
                self.messenger.send(AudioThreadActions::Reset).unwrap();
                self.current_screen = Screen::FileManager;
            }

            if ui.button("Bluetooth Manager").clicked() {
                self.messenger.send(AudioThreadActions::Reset).unwrap();
                self.current_screen = Screen::Audio;
            }

            if ui.button("Debug").clicked() {
                self.messenger.send(AudioThreadActions::Reset).unwrap();
                self.clicked_squares.clear();
                self.current_screen = Screen::Debug;
            }
        });
    }

    fn show_jukebox_screen(&mut self, ctx: &Context) {
        ctx.request_repaint_after(Duration::from_millis(500));

        // Menu Bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.top_menu(ui);
        });

        if self.song_vec_cache.is_none() {
            self.messenger.send(AudioThreadActions::RequestSongVec).unwrap();
            self.song_vec_cache = Some(self.song_vec_receiver.recv().unwrap_or_else(|_| Vec::new()));
        }

        let current_song = {
            let song_index = self.audio_player.lock().unwrap().song_index.clone();
            let song_index_value = song_index.load(Ordering::Relaxed);
            if let Some(ref song_vec) = self.song_vec_cache {
                song_vec.get(song_index_value).unwrap().clone()
            } else {
                return;
            }
        };


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
                        if let Some(ref song_vec) = self.song_vec_cache {
                            for (index, song) in song_vec.iter().enumerate() {
                                if ui.add(egui::SelectableLabel::new(
                                    &current_song == song,
                                    format!("{} by {}", song.name, song.artist),
                                )).clicked() {
                                    self.clicked_index.store(index, Ordering::Relaxed);
                                    self.messenger.send(AudioThreadActions::SongOverride).unwrap();
                                };
                                ui.add_space(10.);
                            }
                        }
                    });
            });
        });

        egui::TopBottomPanel::bottom("bottom_taskbar").show(ctx, |ui| {
            ui.set_height(150.0);

            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.separator();

                // Song Title
                ui.label(format!("{} by {}", current_song.name, current_song.artist));

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
            self.file_explorer.render(ui);
        });
    }

    fn centered_song_progress_display(&mut self, ui: &mut Ui) {

        let audio_player_safe = self.audio_player.lock().unwrap();
        let progress = &audio_player_safe.progress.clone();
        let ms_pos = &audio_player_safe.millisecond_position.clone();
        let song_duration = &audio_player_safe.song_duration.clone();
        drop(audio_player_safe);

        let bar = ProgressBar::new(get_atomic_float(progress)).animate(false);

        // Get the width of the text to center it
        let text = format!("{} / {}", Self::format_time(Self::milliseconds_to_seconds(ms_pos.load(Ordering::Relaxed))), Self::format_time(get_atomic_float(song_duration) as i32));

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

    fn format_time(seconds: i32) -> String {
        let minutes = seconds / 60;
        let remaining_seconds = seconds % 60;
        format!("{:02}:{:02}", minutes, remaining_seconds)
    }
    fn milliseconds_to_seconds(ms: u64) -> i32 {
        (ms / 1000) as i32
    }

    fn centered_buttons(&mut self, ui: &mut Ui) {
        let button_size = Vec2::new(40.0, 40.0); // Width and height of each button

        ui.horizontal(|ui| {
            center_objects(button_size, 5, ui);

            if ui.add_sized(button_size, egui::Button::new("⏭")).clicked() {
                self.messenger.send(AudioThreadActions::Skip).unwrap();
            }

            if ui.add_sized(button_size, egui::Button::new("⏪")).clicked() {
                self.messenger.send(AudioThreadActions::Rewind).unwrap();
            }

            let audio_player_safe = self.audio_player.lock().unwrap();
            let playing_clone = audio_player_safe.playing.clone();
            let looping = audio_player_safe.looping.clone();
            drop(audio_player_safe);
            let playing = playing_clone.load(Ordering::Relaxed);
            if ui.add_sized(button_size, egui::Button::new(if playing { "⏸" } else { "▶" })).clicked() {
                if playing  {
                    self.messenger.send(AudioThreadActions::Pause).unwrap();
                } else {
                    self.messenger.send(AudioThreadActions::Play).unwrap();
                }
            }

            if ui.add_sized(button_size, egui::Button::new("🔀")).clicked() {
                self.song_vec_cache = None;
                self.messenger.send(AudioThreadActions::Shuffle).unwrap();
            }

            if ui.add_sized(button_size, egui::SelectableLabel::new(looping.load(Ordering::Relaxed), "🔁")).clicked() {
                self.messenger.send(AudioThreadActions::Loop).unwrap();
            }
        });
    }

    fn centered_volume_slider(&mut self, ui: &mut Ui) {
        let slider_width = Vec2::new(200., 50.);
        let mut slider_percent = self.volume.load(Ordering::Relaxed);

        if ui.add_sized(slider_width, egui::Slider::new(&mut slider_percent, 0..=100).text("Volume").suffix("%")).drag_stopped {
            self.volume.store(slider_percent, Ordering::Relaxed);
            self.messenger.send(AudioThreadActions::Volume).unwrap();
        }
    }

    fn show_bt_settings_screen(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.top_menu(ui);
        });
        CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.label(RichText::new("  Select Bluetooth Audio Device  ").text_style(heading2()).strong().underline());
                ui.separator();

                ScrollArea::vertical()
                    .auto_shrink([true, true])
                    .max_height(200.)
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        let bt_lock = self.bluetooth.devices.lock().unwrap();
                        let bt_devices = bt_lock.iter().clone();

                        for (index, device) in bt_devices.enumerate() {
                            let color = if device.connected {
                                Color32::GREEN
                            } else {
                                ui.style().visuals.text_color()
                            };
                            if ui.add(egui::SelectableLabel::new(
                                self.selected_bt_device == index as i8,
                                RichText::new(&device.name).color(color),
                            )).clicked() {
                                self.selected_bt_device = index as i8;
                                self.cached_selected_bt_device = Some(BluetoothDevice {
                                    name: device.name.to_string(),
                                    paired: device.paired,
                                    connected: device.connected,
                                    #[cfg(unix)]
                                    id: device.id.clone(),
                                    alias: device.alias.to_string(),
                                    #[cfg(unix)]
                                    mac_address: device.mac_address,
                                });
                            };
                            ui.add_space(10.);
                        }
                    });

                ui.separator();

                if let Some(device) = &self.cached_selected_bt_device {
                    let data = format!("Device Name: {}\n
                    Device Alias: {}\n
                    Mac Address: {}\n
                    Paired: {}\n
                    Connected: {}", device.name, device.alias, device.mac_address.to_string(), device.paired, device.connected);
                    ui.label(RichText::new(data).text_style(notification_font()));
                }

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    let button_size = Vec2::new(100., 50.);
                    center_objects(button_size, 2, ui);

                    if ui.add_sized(button_size, egui::Button::new("Refresh")).clicked() {
                        #[cfg(unix)]
                        self.bluetooth.refresh_bluetooth();
                        self.selected_bt_device = -1;
                        self.cached_selected_bt_device = None;
                    }

                    if ui.add_sized(button_size, egui::Button::new("Connect")).clicked() {
                        if self.selected_bt_device != -1 {
                            if let Some(device) = &self.cached_selected_bt_device {
                                #[cfg(unix)]
                                self.bluetooth.connect_to_device(&device.id);
                            } else {
                                let notification = Notification {
                                    title: "Bluetooth Connection Failure".to_string(),
                                    message: "The device to be connected to is no longer present. \
                                This can happen if a device is selected, the list is refreshed, and then the connection button is pressed. \
                                Please select a the device again.".to_string(),
                                    timer: Timer::new(Duration::from_secs(30)),
                                    id: fastrand::i32(0..i32::MAX),
                                };
                                self.notifications.push_front(notification);
                            };
                        }
                    }
                })
            });
        });
    }

    fn show_debug_screen(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.top_menu(ui);
        });
        CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.label(RichText::new("  Interactable Lights Debug  ").text_style(heading2()).strong().underline());
                ui.add_space(50.0);

                let square_size = Vec2::new(100.0, 100.0); // Each square is 100x100 pixels
                let total_size = Vec2::new(400.0, 400.0); // Total area is 400x400 pixels

                ui.allocate_ui_with_layout(total_size, Layout::top_down(Align::Center), |ui| {
                    for row in 0..4 {
                        ui.horizontal(|ui| {

                            for col in 0..4 {
                                let index = row * 4 + col;
                                let visuals = ui.style().visuals.clone();
                                let mut color = visuals.widgets.inactive.bg_fill; // Default color
                                if self.clicked_squares.contains(&index) {
                                    color = Color32::GREEN; // Change to green if clicked
                                }
                                if ui.add_sized(square_size, egui::Button::new(index.to_string()).fill(color)).clicked() {
                                    #[cfg(unix)]
                                    let gpio = Gpio::new().unwrap();
                                    if self.clicked_squares.contains(&index) {
                                        self.clicked_squares.remove(&index);
                                        #[cfg(unix)]
                                        interface_gpio(index as i8, &gpio, &LightType::Off);
                                    } else {
                                        self.clicked_squares.insert(index);
                                        #[cfg(unix)]
                                        interface_gpio(index as i8, &gpio, &LightType::On);
                                    }
                                }
                            }
                        });
                    }
                });
            });
        });
    }
}

fn center_objects(object_size: Vec2, item_count: i8, mut ui: &mut Ui) {
    ui.add_space(get_center_offset(object_size, item_count, ui.available_width(), ui.spacing().item_spacing.x));
}

fn get_center_offset(object_size: Vec2, item_count: i8, available_width: f32, item_spacing: f32) -> f32 {
    let total_button_width = item_count as f32 * object_size.x + (item_count as f32 - 1.0) * item_spacing;
    (available_width - total_button_width) / 2.0
}

impl eframe::App for OpenLightsCore {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {

        show_notification(&ctx, &mut self.notifications);
        if let Ok(notification) = self.bt_receiver.try_recv() {
            self.notifications.push_front(notification);
        }

        match self.current_screen {
            Screen::Playlist => self.show_playlist_screen(ctx),
            Screen::Jukebox => self.show_jukebox_screen(ctx),
            Screen::FileManager => self.show_file_manager_screen(ctx),
            Screen::Audio => self.show_bt_settings_screen(ctx),
            Screen::Debug => self.show_debug_screen(ctx),
        }
    }
}

#[derive(PartialEq, Default)]
enum Selection {
    #[default]
    Playlist,
    Song,
}

struct FileExplorer {
    selection: Selection,
    playlists: Vec<PathBuf>,
    songs: Vec<Song>,
    selected_index: usize,
    show_edit_buttons: bool,
}

impl FileExplorer {
    fn new() -> Self {
        let playlists = Self::read_directory((&PLAYLIST_DIRECTORY).as_ref()).unwrap_or_else(|_| vec![]);
        Self {
            selection: Selection::Playlist,
            playlists,
            songs: Vec::new(),
            selected_index: 0,
            show_edit_buttons: false,
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

    fn render(&mut self, ui: &mut Ui) {
        CentralPanel::default().show(ui.ctx(), |ui| {
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                // Playlist
                ui.label(RichText::new("  Playlist  ").text_style(heading2()).strong().underline());

                ScrollArea::vertical()
                    .auto_shrink([true, true])
                    .max_height(200.)
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        match self.selection {
                            Selection::Playlist => {
                                self.render_playlists(ui);
                            }
                            Selection::Song => {
                                self.render_songs(ui);
                            }
                        }
                    });

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    if self.show_edit_buttons {
                        if ui.add_sized(Vec2::new(70.0, 20.0), egui::Button::new("Delete")).clicked() {
                            self.remove_current_selected();
                        }
                    }
                    if self.selection == Selection::Song {
                        if ui.add_sized(Vec2::new(90.0, 20.0), egui::Button::new("Playlists")).clicked() {
                            self.selected_index = 0;
                            self.show_edit_buttons = false;
                            self.selection = Selection::Playlist;
                        }
                    }
                });
            });
        });
    }

    fn render_playlists(&mut self, ui: &mut Ui) {
        for (index, path) in self.playlists.iter().enumerate() {
            let label = ui.add(egui::SelectableLabel::new(
                index == self.selected_index,
                format!("{}", path.file_stem().unwrap().to_string_lossy().into_owned().replace('_', " ")),
            ));

            if label.clicked() {
                self.show_edit_buttons = true;
                self.selected_index = index;
            }

            if label.double_clicked() {
                self.selection = Selection::Song;
                self.selected_index = 0;
                self.songs = gather_songs_from_path(&path.to_string_lossy().into_owned());
            }
            ui.add_space(10.);
        }
    }

    fn render_songs(&mut self, ui: &mut Ui) {
        for (index, song) in self.songs.iter().enumerate() {
            if ui.add(egui::SelectableLabel::new(
                index == self.selected_index,
                format!("{} by {}", song.name, song.artist),
            )).clicked() {
                self.show_edit_buttons = true;
                self.selected_index = index;
            };
            ui.add_space(10.);
        }
    }

    fn remove_current_selected(&mut self) {
        match self.selection {
            Selection::Playlist => {
                let path = self.playlists.get(self.selected_index).unwrap();
                fs::remove_dir_all(path).expect("Failed to delete playlist");
                self.playlists.remove(self.selected_index);
                self.selected_index = 0;
            }
            Selection::Song => {
                let song = self.songs.get(self.selected_index).unwrap();
                let path = song.path.parent().unwrap();
                fs::remove_dir_all(path).expect("Failed to delete song");
                self.songs.remove(self.selected_index);
                self.selected_index = 0;
            }
        }
    }
}

#[derive(Clone)]
pub struct Notification {
    pub title: String,
    pub message: String,
    pub timer: Timer,
    pub id: i32,
}

fn show_notification(ctx: &Context, notifications: &mut VecDeque<Notification>) {
    if !notifications.is_empty() {
        let screen_size = ctx.screen_rect();
        let notification_size = Vec2 {x: 300.0, y: 100.0};
        let mut notification_pos = screen_size.max - egui::vec2(notification_size.x + 15.0, notification_size.y + 15.0);
        let mut notifications_clone = notifications.clone();

        for (index, notification) in notifications_clone.iter_mut().enumerate() {

            if index > 2 {
                notifications.remove(index);
                continue;
            }

            egui::Window::new(format!("Notification{}", notification.id))
                .title_bar(false)
                .fixed_pos(notification_pos)
                .resizable(false)
                .collapsible(false)
                .movable(false)
                .show(ctx, |ui| {
                    ui.set_min_size(notification_size);

                    ui.add_sized(Vec2 {x: 300.0, y: 20.0}, egui::Label::new(RichText::new(&notification.title).text_style(Body).strong()));
                    ui.label(RichText::new(&notification.message).text_style(notification_font()).strong());

                    if ui.add_sized(Vec2 {x: 300.0, y: 10.0}, egui::Button::new(RichText::new("Close").text_style(notification_font()).strong())).clicked() {
                        notifications.remove(index);
                    }

                });

            notification_pos.y -= notification_size.y + 20.0;

            if notification.timer.update() {
                notifications.remove(index);
            }
        }
    }
}

#[derive(Clone)]
pub struct Timer {
    pub start_time: Instant,
    pub duration: Duration,
}

impl Timer {
    pub(crate) fn new(duration: Duration) -> Self {
        Self {
            start_time: Instant::now(),
            duration,
        }
    }

    fn update(&mut self) -> bool {
        let current_time = Instant::now();
        let elapsed_time = current_time.duration_since(self.start_time);
        elapsed_time >= self.duration
    }
}
