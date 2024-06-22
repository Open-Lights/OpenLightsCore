use std::fs;
use std::path::Path;

use crate::constants::PLAYLIST_DIRECTORY;

pub fn initialize_files() {
    fs::create_dir_all(Path::new(&*PLAYLIST_DIRECTORY)).unwrap();
}

pub fn initialize_playlists() {
    
}

pub fn initialize_audio() {

}

pub fn initialize_gpio() {

}