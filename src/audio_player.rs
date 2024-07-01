use std::cmp::PartialEq;
use std::{fs, thread};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use lofty::file::TaggedFileExt;
use lofty::prelude::*;
use lofty::probe::Probe;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use shuffle::irs::Irs;
use shuffle::shuffler::Shuffler;
use walkdir::WalkDir;

use crate::constants::{AudioThreadActions, PLAYLIST_DIRECTORY};

#[derive(Clone, Default)]
pub struct Song {
    pub name: String,
    pub artist: String,
    pub path: PathBuf,
    pub duration: f64,
}


impl Song {
    fn new(path: &str, artist: String, duration: f64) -> Self {
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
    pub playlist_vec: Vec<String>,
    pub song_vec: Vec<Song>,
    pub playing: Arc<AtomicBool>,
    song_loaded: bool,
    song_duration: f64,
    song_index: usize,
    pub looping: Arc<AtomicBool>,
    pub millisecond_position: Arc<AtomicU64>,
    pub progress: f32,
    pub(crate) sink: Sink,
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
}

unsafe impl Sync for AudioPlayer {}
unsafe impl Send for AudioPlayer {}

impl Default for AudioPlayer {
    fn default() -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        Self {
            playlist_vec: locate_playlists(&**PLAYLIST_DIRECTORY),
            song_vec: Vec::new(),
            playing: Arc::new(AtomicBool::new(false)),
            song_loaded: false,
            song_duration: 0.0,
            song_index: 0,
            looping: Arc::new(AtomicBool::new(false)),
            millisecond_position: Arc::new(AtomicU64::new(0)),
            progress: 0.0,
            sink: Sink::try_new(&stream_handle).unwrap(),
            _stream,
            stream_handle,
        }
    }
}

impl PartialEq for Song {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self::default()
    }

    fn prepare_song(&mut self) {
        let song = self.get_current_song();
        self.sink.clear();
        self.song_duration = song.duration;
        let file = File::open(song.path).unwrap();
        let source = Decoder::new(BufReader::new(file)).unwrap();
        self.sink.append(source);
        self.song_loaded = true;
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
        let mut rng = StdRng::from_entropy();
        let mut irs = Irs::default();
        irs.shuffle(&mut self.song_vec, &mut rng).expect("Failed to Shuffle");
        self.song_index = 0;
        self.song_loaded = false;
        self.play();
    }

    pub fn set_volume(&mut self, new_volume: f32) {
        self.sink.set_volume(new_volume);
    }

    pub fn get_current_song(&mut self) -> Song {
        // TODO Maybe remove Clone here?
        self.song_vec.get(self.song_index).unwrap().clone()
    }

    pub fn song_override(&mut self, song: &Song) {
        self.pause();
        let index = self.get_song_index(song);
        self.song_index = index;
        self.song_loaded = false;
        self.play();
    }

    pub fn next_song(&mut self) {
        self.pause();
        self.song_index += 1;

        if self.song_index >= self.song_vec.len() {
            self.song_index = 0;
        }

        self.song_loaded = false;
        self.play();
    }

    pub fn set_position(&mut self, time: Duration) {
        self.pause();
        self.sink.try_seek(time).unwrap();
        self.play();
        // TODO Set ms time
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

pub fn start_worker_thread(audio_player: Arc<Mutex<AudioPlayer>>, receiver: Receiver<AudioThreadActions>) {
    thread::spawn(move || {
        loop {
            // Check for messages
            if let Ok(action) = receiver.try_recv() {
                let mut player_guard = audio_player.lock().unwrap();
                match action {
                    AudioThreadActions::Play => {
                        player_guard.play();
                    }
                    AudioThreadActions::Pause => {
                        player_guard.pause();
                    }
                    AudioThreadActions::KillThread => {
                        // TODO
                    }
                    AudioThreadActions::Skip => {
                        player_guard.next_song();
                    }
                    AudioThreadActions::Loop => {
                        player_guard.toggle_looping();
                    }
                    AudioThreadActions::Volume => {
                        // TODO
                    }
                    AudioThreadActions::Rewind => {
                        player_guard.set_position(Duration::ZERO);
                    }
                    AudioThreadActions::Shuffle => {
                        player_guard.shuffle();
                    }
                }
            }

            // Update song position and progress if playing
            {
                let mut player_guard = audio_player.lock().unwrap();
                if player_guard.playing.load(Ordering::Relaxed) {
                    let pos = player_guard.sink.get_pos().as_millis();
                    player_guard.millisecond_position.store(pos as u64, Ordering::Relaxed);
                    let seconds = pos / 1000;
                    player_guard.progress = (seconds as f64 / player_guard.song_duration) as f32;

                    // Check for song finished
                    if player_guard.progress == 1.0 {
                        if player_guard.looping.load(Ordering::Relaxed) {
                            player_guard.prepare_song();
                        } else {
                            player_guard.next_song();
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

pub fn locate_playlists(path: &str) -> Vec<String> {
    let mut folder_names = Vec::new();

    for entry in fs::read_dir(path).unwrap() {
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
        let data = gather_metadata(song_path);
        let song = Song::new(song_path, data.1, data.0);
        songs.push(song);
    }
    songs
}