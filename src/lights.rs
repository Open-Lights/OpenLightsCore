use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use rppal::gpio::Gpio;

use serde::{Deserialize, Serialize};
use crate::audio_player::Song;

pub fn start_light_thread(current_song: Song) {
    let light_data = gather_light_data(current_song.path.to_string_lossy().to_string());

    if !light_data.is_empty() {
        let gpio = Gpio::new()?;
        thread::spawn(move || {
            loop {
                for mut channel_data in light_data {
                    let current_song_time = 10000; // TODO Actually acquire the current song time!
                    let target_time = channel_data.data.get(channel_data.index).unwrap();
                    if target_time.timestamp <= current_song_time {
                        for channel in channel_data.channels {
                            interface_gpio(channel, &gpio, &target_time.light_type);
                        }
                        channel_data.index += 1;
                    }
                }

                thread::sleep(Duration::from_millis(10));
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
    let parsed_data: Data = serde_json::from_reader(reader)?;

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