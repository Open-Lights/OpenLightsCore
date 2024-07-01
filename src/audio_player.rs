use std::{fs, thread};
use std::cmp::PartialEq;
use std::fs::File;
use std::io::BufReader;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
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
    pub playlist_vec: Vec<String>,
    pub song_vec: Vec<Song>,
    pub playing: Arc<AtomicBool>,
    song_loaded: bool,
    pub song_duration: Arc<AtomicU32>,
    song_index: usize,
    pub looping: Arc<AtomicBool>,
    pub millisecond_position: Arc<AtomicU64>,
    pub progress: Arc<AtomicU32>,
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
            song_duration: Arc::new(AtomicU32::new(0)),
            song_index: 0,
            looping: Arc::new(AtomicBool::new(false)),
            millisecond_position: Arc::new(AtomicU64::new(0)),
            progress: Arc::new(AtomicU32::new(0)),
            sink: Sink::try_new(&stream_handle).unwrap(),
            _stream,
            stream_handle,
        }
    }
}

impl Deref for AudioPlayer {
    type Target = ();

    fn deref(&self) -> &Self::Target {
        todo!()
    }
}

impl DerefMut for AudioPlayer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        todo!()
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
        set_atomic_float(&self.song_duration, song.duration);
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

pub fn get_atomic_float(float: &Arc<AtomicU32>) -> f32 {
    let value_as_u32 = float.load(Ordering::Relaxed);
    value_as_u32 as f32 / 100.0
}

pub fn set_atomic_float(float: &Arc<AtomicU32>, value: f32) {
    let value_as_u32 = (value * 100.0) as u32;
    float.store(value_as_u32, Ordering::Relaxed);
}

pub fn start_worker_thread(audio_player: &Arc<AudioPlayer>, receiver: Receiver<AudioThreadActions>) {
    thread::spawn(move || {
        loop {
            // Check for messages
            if let Ok(action) = receiver.try_recv() {
                match action {
                    AudioThreadActions::Play => {
                        audio_player.play();
                    }
                    AudioThreadActions::Pause => {
                        audio_player.pause();
                    }
                    AudioThreadActions::KillThread => {
                        break;
                    }
                    AudioThreadActions::Skip => {
                        audio_player.next_song();
                    }
                    AudioThreadActions::Loop => {
                        audio_player.toggle_looping();
                    }
                    AudioThreadActions::Volume => {
                        // TODO
                    }
                    AudioThreadActions::Rewind => {
                        audio_player.set_position(Duration::ZERO);
                    }
                    AudioThreadActions::Shuffle => {
                        audio_player.shuffle();
                    }
                }
            }

            // Update song position and progress if playing
            {
                if audio_player.playing.load(Ordering::Relaxed) {
                    let pos = audio_player.sink.get_pos().as_millis();
                    audio_player.millisecond_position.store(pos as u64, Ordering::Relaxed);
                    let seconds = pos / 1000;
                    set_atomic_float(&audio_player.progress, (seconds as f64 / get_atomic_float(&audio_player.song_duration) as f64) as f32);

                    // Check for song finished
                    if get_atomic_float(&audio_player.progress) == 1.0 {
                        if audio_player.looping.load(Ordering::Relaxed) {
                            audio_player.prepare_song();
                        } else {
                            audio_player.next_song();
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
        let song = Song::new(song_path, data.1, data.0 as f32);
        songs.push(song);
    }
    songs
}