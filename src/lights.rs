use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
//use rppal::gpio::Gpio;

use serde::{Deserialize, Serialize};
use crate::audio_player::Song;

pub fn start_light_thread(current_song: Song, millisecond_position: Arc<AtomicU64>, toggle: Arc<AtomicBool>, active: Arc<AtomicBool>, reset: Arc<AtomicBool>) {

    let mut light_data = gather_light_data(current_song.path.to_string_lossy().to_string());

    if !light_data.is_empty() {
        while toggle.load(Ordering::Relaxed) { // Ensure there aren't duplicate threads
            thread::sleep(Duration::from_millis(5));
        }
        //let gpio = Gpio::new()?;
        active.store(true, Ordering::Relaxed);
        thread::spawn(move || {
            loop {
                if toggle.load(Ordering::Relaxed) {
                    active.store(false, Ordering::Relaxed);
                    toggle.store(false, Ordering::Relaxed);
                    break;
                }
                if reset.load(Ordering::Relaxed) {
                    reset.store(false, Ordering::Relaxed);
                    for channel_data in &mut light_data {
                        channel_data.index = 0;
                    }
                }
                for channel_data in &mut light_data {
                    if let Some(target_time) = channel_data.data.get(channel_data.index) {
                        if target_time.timestamp <= millisecond_position.load(Ordering::Relaxed) as i32 {
                            for channel in &channel_data.channels {
                                println!("Correct: {}; Actual: {}", target_time.timestamp, millisecond_position.load(Ordering::Relaxed));
                                //interface_gpio(channel, &gpio, &target_time.light_type);
                            }
                            channel_data.index += 1;
                        }
                    }
                }

                thread::sleep(Duration::from_millis(5));
            }
        });
    }
}

enum LightType {
    On,
    Off,
}
#[derive(Serialize, Deserialize, Debug)]
struct Data {
    #[serde(flatten)]
    fields: HashMap<String, HashMap<String, i8>>,
}
struct LightData {
    timestamp: i32,
    light_type: LightType,
}

struct ChannelData {
    channels: Vec<i8>,
    data: Vec<LightData>,
    index: usize,
}

fn gather_light_data(song_path: String) -> Vec<ChannelData> {
    let path_string = song_path.replace("wav", "json");
    let path = PathBuf::from(path_string);

    let file = match File::open(&path) {
        Ok(file) => file,
        Err(_) => return Vec::new(),
    };
    let reader = BufReader::new(file);
    let parsed_data: Data = serde_json::from_reader(reader).unwrap();

    let mut data_vec: Vec<ChannelData> = Vec::new();

    for (channel, light_data) in parsed_data.fields {

        let mut light_data_vec: Vec<LightData> = Vec::new();
        for (timestamp_str, light_type_pre) in light_data {
            let timestamp = timestamp_str.parse::<i32>().unwrap();
            let light_type = match light_type_pre {
                0 => LightType::Off,
                1 => LightType::On,
                _ => LightType::Off,
            };
            light_data_vec.push(LightData {timestamp, light_type})
        }
        light_data_vec.sort_by_key(|ld| ld.timestamp);
        data_vec.push(ChannelData {channels: parse_channels(channel), data: light_data_vec, index: 0})
    }
    data_vec
}

fn parse_channels(channels_str: String) -> Vec<i8> {
    channels_str
        .split(',')
        .filter_map(|s| s.trim().parse::<i8>().ok())
        .collect()
}

/*
fn interface_gpio(channel: i8, gpio: &Gpio, light_type: &LightType) {
    let mut pin = gpio.get(channel as u8)?.into_output();
    match light_type {
        LightType::On => {
            pin.set_high();
        }
        LightType::Off => {
            pin.set_low();
        }
    }
}

 */