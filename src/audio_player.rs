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

#[derive(Clone)]
struct Song {
    name: String,
    artist: String,
    path: PathBuf,
    duration: f64,
}

impl Song {
    fn new(path: &str, artist: String, duration: f64) -> Self {
        let path_ref = Path::new(&path);
        let path: PathBuf = path_ref.to_path_buf();
        let name: String = path.file_name().unwrap().into();
        Self {
            name,
            artist,
            path,
            duration,
        }
    }
}

pub struct AudioPlayer {
    song_vec: Vec<Song>,
    playing: bool,
    song_loaded: bool,
    song_index: usize,
    sink: Sink,
}

impl AudioPlayer {
    fn new() -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        Self {
            song_vec: Vec::new(),
            playing: false,
            song_loaded: false,
            song_index: 0,
            sink: Sink::try_new(&stream_handle).unwrap(),
        }
    }

    fn prepare_song(&mut self) {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let song = self.get_current_song();
        let file = File::open(song.path).unwrap();
        let source = Decoder::new(BufReader::new(file)).unwrap();
        self.sink.append(source);
        self.song_loaded = true;
    }

    fn play(&mut self) {
        if self.song_loaded {
            self.sink.play();
            self.playing = true;
        } else {
            self.prepare_song();
            self.play();
        }
    }

    fn pause(&mut self) {
        self.sink.pause();
        self.playing = false;
    }

    fn load_songs(mut self, songs: Vec<Song>) {
        self.song_vec = songs;
    }

    fn shuffle(mut self) {
        let mut rng = StdRng::from_entropy();
        let mut irs = Irs::default();
        // TODO Figure out why this is upset
        irs.shuffle(&mut self.song_vec, &mut rng).expect("TODO: panic message");
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

    fn get_current_song(&mut self) -> Song {
        // TODO Maybe remove Clone here?
        self.song_vec.get(self.song_index).unwrap().clone()
    }

    fn next_song(&mut self) {
        self.pause();
        self.song_index += 1;

        if self.song_index >= self.song_vec.len() {
            self.song_index = 0;
        }

        self.song_loaded = false;
        self.play();
    }

    fn set_position(&mut self, time: Duration) {
        self.pause();
        self.sink.try_seek(time).unwrap();
        self.play();
    }
}

pub fn load_songs_from_playlist(playlist: String) {
    let mut songs: Vec<Song> = Vec::new();
    let path = format!("/open_lights/playlists/{}/", playlist);

    for file in WalkDir::new(&path) {
        let song_path = file.unwrap().path().to_str().expect("Invalid UTF-8 sequence");
        let data = gather_metadata(&song_path);
        let song = Song::new(&song_path, data.1, data.0);
        songs.push(song);
    }
}

fn gather_metadata(path: &str) -> (f64, String) {
    let wav_reader = hound::WavReader::open(path)?;
    let spec = wav_reader.spec();
    let duration = wav_reader.duration();
    let duration_seconds = duration as f64 / spec.sample_rate as f64;

    let mut reader = BufReader::new(File::open(path)?);
    let tagged_file = Probe::new(&mut reader).guess_file_type()?.read()?;
    if let Some(tag) = tagged_file.primary_tag() {
        if let Some(author) = tag.artist().as_deref().unwrap_or("Unknown") {
            (duration_seconds, String::from(author))
        } else {
            (duration_seconds, String::from("Unknown"))
        }
    } else {
        (duration_seconds, String::from("Unknown"))
    }
}