#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub mod constants;
pub mod startup;
pub mod audio_player;
pub use app::TemplateApp;
