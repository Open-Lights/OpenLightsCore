use std::fs;
use crate::constants;

pub fn initialize_files() {
    fs::create_dir_all(constants::PLAYLIST_DIRECTORY)?;
}

pub fn initialize_songs(playlist: String) {
    
}

pub fn initialize_audio() {

}

pub fn initialize_gpio() {

}