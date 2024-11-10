use std::env;

use once_cell::sync::Lazy;

/// The current version of OpenLightsCore
pub const VERSION: &str = "1.0.0";

/// The directory where playlists are stored
pub static PLAYLIST_DIRECTORY: Lazy<String> = Lazy::new(|| {
    let mut path = env::current_dir().expect("Failed to get current directory");
    path.push("open_lights/playlists/");
    path.to_str()
        .expect("Failed to convert path to string")
        .to_string()
});

/// Every action that the audio thread can invoke
///
/// KillThread: Stops the audio thread
/// Pause: Pauses the current audio
/// Play: Plays the current audio
/// Loops: Continues repeating the current audio when it completes
/// Volume: Adjusts the global volume of the program
/// Skip: Skips to the next audio in the playlist
/// Rewind: Goes back to the beginning of the audio
/// Shuffle: Randomizes the playlist and starts playing the next audio
/// SongOverride: Plays the audio selected by the user
/// RequestSongVec: Asks the audio thread to provide an audio list
/// LoadFromPlaylist: Loads all audio in a playlist
/// Reset: Resets all data in the audio thread
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
