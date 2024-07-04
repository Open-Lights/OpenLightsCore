use std::{fs, thread};
use std::cmp::PartialEq;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicI8, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use lofty::file::TaggedFileExt;
use lofty::prelude::*;
use lofty::probe::Probe;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use walkdir::WalkDir;

use crate::constants::{AudioThreadActions, PLAYLIST_DIRECTORY};
use crate::lights::start_light_thread;

#[derive(Clone, Default)]
pub struct Song {
    pub name: String,
    pub artist: String,
    pub path: PathBuf,
    pub duration: f32,
}


impl Song {
    fn new(path: &str, artist: String, duration: f32) -> Self {
        let path_ref = Path::new(&path);
        let path: PathBuf = path_ref.to_path_buf();
        let name: String = path.file_stem().unwrap().to_string_lossy().into_owned().replace('_', " ");
        Self {
            name,
            artist,
            path,
            duration,
        }
    }
}

pub struct AudioPlayer {
    pub song_vec: Vec<Song>,
    pub playing: Arc<AtomicBool>,
    song_loaded: bool,
    pub song_duration: Arc<AtomicU32>,
    pub song_index: Arc<AtomicUsize>,
    pub looping: Arc<AtomicBool>,
    pub millisecond_position: Arc<AtomicU64>,
    pub progress: Arc<AtomicU32>,
    volume: Arc<AtomicI8>,
    clicked_index: Arc<AtomicUsize>,
    pub(crate) sink: Sink,
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    light_thread_active: Arc<AtomicBool>,
    light_thread_toggle: Arc<AtomicBool>,
    light_thread_reset: Arc<AtomicBool>,
}

unsafe impl Sync for AudioPlayer {}
unsafe impl Send for AudioPlayer {}

impl PartialEq for Song {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl AudioPlayer {
    pub fn new(volume: Arc<AtomicI8>, clicked_index: Arc<AtomicUsize>) -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        Self {
            song_vec: Vec::new(),
            playing: Arc::new(AtomicBool::new(false)),
            song_loaded: false,
            song_duration: Arc::new(AtomicU32::new(0)),
            song_index: Arc::new(AtomicUsize::new(0)),
            looping: Arc::new(AtomicBool::new(false)),
            millisecond_position: Arc::new(AtomicU64::new(0)),
            progress: Arc::new(AtomicU32::new(0)),
            volume,
            clicked_index,
            sink: Sink::try_new(&stream_handle).unwrap(),
            _stream,
            stream_handle,
            light_thread_active: Arc::new(AtomicBool::new(false)),
            light_thread_toggle: Arc::new(AtomicBool::new(false)),
            light_thread_reset: Arc::new(AtomicBool::new(false)),
        }
    }

    fn prepare_song(&mut self) {
        let song = self.get_current_song();
        self.sink.clear();
        self.millisecond_position.store(0, Ordering::Relaxed);
        set_atomic_float(&self.song_duration, song.duration);
        let file = File::open(&song.path).unwrap();
        let source = Decoder::new(BufReader::new(file)).unwrap();
        self.sink.append(source);
        self.song_loaded = true;
        self.kill_light_thread();
        start_light_thread(song, Arc::clone(&self.millisecond_position), Arc::clone(&self.light_thread_toggle), Arc::clone(&self.light_thread_active), Arc::clone(&self.light_thread_reset));
    }

    fn kill_light_thread(&mut self) {
        if self.light_thread_active.load(Ordering::Relaxed) {
            self.light_thread_toggle.store(true, Ordering::Relaxed);
        }
    }

    pub fn play(&mut self) {
        if self.song_loaded {
            self.sink.play();
            self.playing.store(true, Ordering::Relaxed);
        } else {
            self.prepare_song();
            self.play();
        }
    }

    pub fn pause(&mut self) {
        self.sink.pause();
        self.playing.store(false, Ordering::Relaxed);
    }

    pub fn get_song_index(&mut self, song: &Song) -> usize {
        self.song_vec.iter().position(|x| x == song).unwrap()
    }

    pub fn shuffle(&mut self) {
        fastrand::shuffle(&mut self.song_vec);
        self.song_index.store(0, Ordering::Relaxed);
        self.song_loaded = false;
        self.play();
    }

    pub fn set_volume(&mut self, new_volume: f32) {
        self.sink.set_volume(new_volume);
    }

    pub fn get_current_song(&mut self) -> Song {
        // TODO Maybe remove Clone here?
        self.song_vec.get(self.song_index.load(Ordering::Relaxed)).unwrap().clone()
    }

    pub fn song_override(&mut self, song: &Song) {
        self.pause();
        let index = self.get_song_index(song);
        self.song_index.store(index, Ordering::Relaxed);
        self.song_loaded = false;
        self.play();
    }

    pub fn next_song(&mut self) {
        self.pause();
        let new_index = self.song_index.load(Ordering::Relaxed) + 1;
        self.song_index.store(new_index, Ordering::Relaxed);

        if new_index >= self.song_vec.len() {
            self.song_index.store(0, Ordering::Relaxed);
        }

        self.song_loaded = false;
        self.play();
    }

    pub fn set_position(&mut self, time: Duration) {
        self.pause();
        self.sink.try_seek(time).unwrap();
        self.play();
        self.millisecond_position.store(time.as_millis() as u64, Ordering::Relaxed);
        if Duration::is_zero(&time) {
            self.light_thread_reset.store(true, Ordering::Relaxed);
        } else {
            // TODO Add code for lights to find closest point to the time
        }
    }

    pub fn toggle_looping(&mut self) {
        self.looping.store(!self.looping.load(Ordering::Relaxed), Ordering::Relaxed);
    }

    pub fn load_songs_from_playlist(&mut self, playlist: &String) {
        let path = format!("{}{}/", &**PLAYLIST_DIRECTORY, &playlist);
        self.song_vec = gather_songs_from_path(&path);
        println!("Loaded songs from playlist: {}", &playlist);
    }
}

pub fn get_atomic_float(float: &Arc<AtomicU32>) -> f32 {
    let value_as_u32 = float.load(Ordering::Relaxed);
    value_as_u32 as f32 / 100.0
}

pub fn set_atomic_float(float: &Arc<AtomicU32>, value: f32) {
    let value_as_u32 = (value * 100.0) as u32;
    float.store(value_as_u32, Ordering::Relaxed);
}

pub fn start_worker_thread(audio_player: Arc<Mutex<AudioPlayer>>, receiver: Receiver<AudioThreadActions>, song_vec_sender: Sender<Vec<Song>>) {
    thread::spawn(move || {
        loop {
            // Check for messages
            if let Ok(action) = receiver.try_recv() {
                let mut audio_player_safe = audio_player.lock().unwrap();
                match action {
                    AudioThreadActions::Play => {
                        audio_player_safe.play();
                    }
                    AudioThreadActions::Pause => {
                        audio_player_safe.pause();
                    }
                    AudioThreadActions::KillThread => {
                        break;
                    }
                    AudioThreadActions::Skip => {
                        audio_player_safe.next_song();
                    }
                    AudioThreadActions::Loop => {
                        audio_player_safe.toggle_looping();
                    }
                    AudioThreadActions::Volume => {
                        let volume = audio_player_safe.volume.load(Ordering::Relaxed) as f32 / 100.0;
                        audio_player_safe.set_volume(volume);
                    }
                    AudioThreadActions::Rewind => {
                        audio_player_safe.set_position(Duration::ZERO);
                    }
                    AudioThreadActions::Shuffle => {
                        audio_player_safe.shuffle();
                    }
                    AudioThreadActions::SongOverride => {
                        let song = audio_player_safe.song_vec.get(audio_player_safe.clicked_index.load(Ordering::Relaxed)).unwrap().clone();
                        audio_player_safe.song_override(&song);
                    }
                    AudioThreadActions::RequestSongVec => {
                        song_vec_sender.send(audio_player_safe.song_vec.clone()).unwrap();
                    }
                    AudioThreadActions::LoadFromPlaylist => {
                        let playlist = playlist_from_index(&audio_player_safe.clicked_index);
                        audio_player_safe.load_songs_from_playlist(&playlist);
                        audio_player_safe.clicked_index.store(0, Ordering::Relaxed);
                    }
                }
            }

            // Update song position and progress if playing
            {
                let mut audio_player_safe = audio_player.lock().unwrap();
                if audio_player_safe.playing.load(Ordering::Relaxed) {
                    let pos = audio_player_safe.sink.get_pos().as_millis();
                    audio_player_safe.millisecond_position.store(pos as u64, Ordering::Relaxed);
                    let seconds = pos / 1000;
                    set_atomic_float(&audio_player_safe.progress, (seconds as f64 / get_atomic_float(&audio_player_safe.song_duration) as f64) as f32);

                    // Check for song finished
                    if get_atomic_float(&audio_player_safe.progress) >= 0.99 && audio_player_safe.sink.empty() {
                        if audio_player_safe.looping.load(Ordering::Relaxed) {
                            audio_player_safe.prepare_song();
                            audio_player_safe.play();
                        } else {
                            audio_player_safe.next_song();
                        }
                    }
                }
            }

            thread::sleep(Duration::from_millis(10));
        }
    });
}

fn gather_metadata(path: &str) -> (f64, String) {
    println!("{}", path);
    let wav_reader = hound::WavReader::open(path).unwrap();
    let spec = wav_reader.spec();
    let duration = wav_reader.duration();
    let duration_seconds = duration as f64 / spec.sample_rate as f64;

    let mut reader = BufReader::new(File::open(path).unwrap());
    let tagged_file = Probe::new(&mut reader).guess_file_type().unwrap().read().unwrap();
    if let Some(tag) = tagged_file.primary_tag() {
        let artist = tag.artist();
        let author = artist.as_deref().unwrap_or("Unknown");
        (duration_seconds, String::from(author))
    } else {
        (duration_seconds, String::from("Unknown"))
    }
}

fn playlist_from_index(index: &Arc<AtomicUsize>) -> String {
    let playlists = locate_playlists();
    let playlist: Option<String> = playlists.get(index.load(Ordering::Relaxed)).cloned();
    playlist.unwrap()
}

pub fn locate_playlists() -> Vec<String> {
    let mut folder_names = Vec::new();

    for entry in fs::read_dir(&**PLAYLIST_DIRECTORY).unwrap() {
        let directory = entry.unwrap();
        let path = directory.path();

        if path.is_dir() {
            if let Some(folder_name) = path.file_name().and_then(|name| name.to_str()) {
                folder_names.push(folder_name.to_string());
            }
        }
    }

    folder_names
}

pub fn gather_songs_from_path(path: &String) -> Vec<Song> {
    let mut songs: Vec<Song> = Vec::new();
    for file in WalkDir::new(path).min_depth(2).max_depth(3) {
        let song_file = file.unwrap();
        let song_path = song_file.path().to_str().expect("Invalid UTF-8 sequence");

        // Check if the file is a WAV file
        if song_path.ends_with(".wav") {
            let data = gather_metadata(song_path);
            let song = Song::new(song_path, data.1, data.0 as f32);
            songs.push(song);
        }
    }
    songs
}