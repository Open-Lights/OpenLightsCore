use std::cmp::PartialEq;
use std::{fs, thread};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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

use crate::constants::PLAYLIST_DIRECTORY;

#[derive(Clone, Default)]
pub struct Song {
    pub name: String,
    pub artist: String,
    path: PathBuf,
    pub duration: f64,
}


impl Song {
    fn new(path: &str, artist: String, duration: f64) -> Self {
        let path_ref = Path::new(&path);
        let path: PathBuf = path_ref.to_path_buf();
        let name: String = path.file_name().unwrap().to_string_lossy().into_owned();
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
    pub playing: Arc<Mutex<bool>>,
    song_loaded: bool,
    song_duration: Arc<Mutex<f64>>,
    song_index: usize,
    pub looping: Arc<Mutex<bool>>,
    pub millisecond_position: Arc<Mutex<u128>>,
    pub progress: Arc<Mutex<f32>>,
    volume: f32,
    pub(crate) sink: Arc<Mutex<Sink>>,
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    thread_alive: Arc<Mutex<bool>>,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        Self {
            playlist_vec: locate_playlists(&**PLAYLIST_DIRECTORY),
            song_vec: Vec::new(),
            playing: Arc::new(Mutex::new(false)),
            song_loaded: false,
            song_duration: Arc::new(Mutex::new(0.0)),
            song_index: 0,
            looping: Arc::new(Mutex::new(false)),
            millisecond_position: Arc::new(Mutex::new(0)),
            progress: Arc::new(Mutex::new(0.0)),
            volume: 100.,
            sink: Arc::new(Mutex::new(Sink::try_new(&stream_handle).unwrap())),
            _stream,
            stream_handle,
            thread_alive: Arc::new(Mutex::new(true)),
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
        let binding = &self.sink;
        let sink_guard = binding.lock().unwrap();
        sink_guard.clear();
        let mut duration_guard = self.song_duration.lock().unwrap();
        *duration_guard = song.duration;
        let file = File::open(song.path).unwrap();
        let source = Decoder::new(BufReader::new(file)).unwrap();
        sink_guard.append(source);
        self.song_loaded = true;
        println!("Loaded a new song");
    }

    pub fn play(&mut self) {
        if self.song_loaded {
            let binding = &self.sink;
            let sink_guard = binding.lock().unwrap();
            sink_guard.play();

            let binding = &self.playing;
            let mut playing_guard = binding.lock().unwrap();
            *playing_guard = true;
        } else {
            self.prepare_song();
            self.play();
        }
    }

    pub fn pause(&mut self) {
        let binding = &self.sink;
        let sink_guard = binding.lock().unwrap();
        sink_guard.pause();

        let binding = &self.playing;
        let mut playing_guard = binding.lock().unwrap();
        *playing_guard = false;
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
        self.volume = new_volume;
        let binding = &self.sink;
        let sink_guard = binding.lock().unwrap();
        sink_guard.set_volume(self.volume);
    }

    pub fn get_volume(&mut self) -> f32 {
        self.volume
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
        {
            let binding = &self.sink;
            let sink_guard = binding.lock().unwrap();
            sink_guard.try_seek(time).unwrap();
        }
        self.play();
        // TODO Set ms time
    }

    pub fn toggle_looping(&mut self) {
        let binding = &self.looping;
        let mut looping_guard = binding.lock().unwrap();
        *looping_guard = !*looping_guard;
    }

    pub fn is_looping(&mut self) -> bool {
        *self.looping.lock().unwrap()
    }

    pub fn is_playing(&mut self) -> bool {
        *self.playing.lock().unwrap()
    }

    pub fn get_ms_pos(&mut self) -> u128 {
        *self.millisecond_position.lock().unwrap()
    }

    pub fn get_progress(&mut self) -> f32 {
        *self.progress.lock().unwrap()
    }

    pub fn load_songs_from_playlist(&mut self, playlist: &String) {
        let mut songs: Vec<Song> = Vec::new();
        let path = format!("{}{}/", &**PLAYLIST_DIRECTORY, &playlist);

        for file in WalkDir::new(path).min_depth(2).max_depth(3) {
            let song_file = file.unwrap();
            let song_path = song_file.path().to_str().expect("Invalid UTF-8 sequence");
            let data = gather_metadata(song_path);
            let song = Song::new(song_path, data.1, data.0);
            songs.push(song);
        }

        self.song_vec = songs;
        println!("Loaded songs from playlist: {}", &playlist);
    }

    pub fn start_worker_thread(&mut self) {
        let playing = Arc::clone(&self.playing);
        let thread_alive = Arc::clone(&self.thread_alive);
        let sink = Arc::clone(&self.sink);
        let saved_pos = Arc::clone(&self.millisecond_position);
        let progress = Arc::clone(&self.progress);
        let duration = Arc::clone(&self.song_duration);
        let looping = Arc::clone(&self.looping);

        thread::spawn(move || {
            while *thread_alive.lock().unwrap() {
                if *playing.lock().unwrap() {
                    // Update song pos
                    let sink_guard = sink.lock().unwrap();
                    let pos = sink_guard.get_pos().as_millis();
                    let mut pos_guard = saved_pos.lock().unwrap();
                    *pos_guard = pos;
                    // Set progress
                    let seconds= pos / 1000;
                    let mut progress_guard = progress.lock().unwrap();
                    let duration_guard = duration.lock().unwrap();
                    *progress_guard = (seconds as f64 / *duration_guard) as f32;
                    // Check for song finished
                    if *progress_guard == 1.0 {
                        let looping_guard = looping.lock().unwrap();
                        if *looping_guard {
                            // TODO run prepare_song()
                        } else {
                            // TODO run next_song()
                        }
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
    }
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