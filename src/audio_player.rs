use std::cmp::PartialEq;
use std::fs;
use std::fs::File;
use std::io::{BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;

use lofty::file::TaggedFileExt;
use lofty::prelude::*;
use lofty::probe::Probe;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rodio::{Decoder, OutputStream, Sink};
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
    pub playing: bool,
    song_loaded: bool,
    song_index: usize,
    pub looping: bool,
    millisecond_position: f64,
    sink: Sink,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        Self {
            playlist_vec: locate_playlists(PLAYLIST_DIRECTORY),
            song_vec: Vec::new(),
            playing: false,
            song_loaded: false,
            song_index: 0,
            looping: false,
            millisecond_position: 0.,
            sink: Sink::try_new(&stream_handle).unwrap(),
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
        Default::default()
    }

    fn prepare_song(&mut self) {
        let song = self.get_current_song();
        let file = File::open(song.path).unwrap();
        let source = Decoder::new(BufReader::new(file)).unwrap();
        self.sink.append(source);
        self.song_loaded = true;
    }

    pub fn play(&mut self) {
        if self.song_loaded {
            self.sink.play();
            self.playing = true;
        } else {
            self.prepare_song();
            self.play();
        }
    }

    pub fn pause(&mut self) {
        self.sink.pause();
        self.playing = false;
    }

    fn load_songs(mut self, songs: Vec<Song>) {
        self.song_vec = songs;
    }

    pub fn get_song_index(&mut self, song: &Song) -> usize {
        self.song_vec.iter().position(|x| x == song).unwrap()
    }

    pub fn get_current_position_seconds(&mut self) -> i32 {
        (self.millisecond_position / 1000.) as i32
    }

    pub fn shuffle(&mut self) {
        let mut rng = StdRng::from_entropy();
        let mut irs = Irs::default();
        irs.shuffle(&mut self.song_vec, &mut rng).expect("Failed to Shuffle");
        self.song_index = 0;
        self.song_loaded = false;
        self.play();
    }

    fn set_volume(&mut self, new_volume: f32) {
        self.sink.set_volume(new_volume);
    }

    fn get_volume(&mut self) -> f32 {
        self.sink.volume()
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
}

pub fn load_songs_from_playlist(playlist: &String) {
    let mut songs: Vec<Song> = Vec::new();
    let path = format!("/open_lights/playlists/{}/", playlist);

    for file in WalkDir::new(path) {
        let song_file = file.unwrap();
        let song_path = song_file.path().to_str().expect("Invalid UTF-8 sequence");
        let data = gather_metadata(song_path);
        let song = Song::new(song_path, data.1, data.0);
        songs.push(song);
    }
}

fn gather_metadata(path: &str) -> (f64, String) {
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