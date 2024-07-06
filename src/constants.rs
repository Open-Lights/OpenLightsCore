use std::env;

use once_cell::sync::Lazy;

pub const VERSION: &str = "1.0.0";
pub static PLAYLIST_DIRECTORY: Lazy<String> = Lazy::new(|| {
    let mut path = env::current_dir().expect("Failed to get current directory");
    path.push("open_lights/playlists/");
    path.to_str().expect("Failed to convert path to string").to_string()
});

pub enum AudioThreadActions {
    KillThread,
    Pause,
    Play,
    Loop,
    Volume,
    Skip,
    Rewind,
    Shuffle,
    SongOverride,
    RequestSongVec,
    LoadFromPlaylist,
    Reset,
}
