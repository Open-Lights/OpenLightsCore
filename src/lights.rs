use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "x86_64"))]
use rppal::gpio::Gpio;
#[cfg(not(target_arch = "x86_64"))]
use rppal::gpio::OutputPin;

/// Creates a new thread for reading light data
/// The lighting thread is in charge of toggling lights when the light file specifies.
///
/// song_path: The current path of the audio being played (to get the light file)
/// millisecond_position: The current position in the audio
/// toggle: Whether the light thread should be executing
/// active: If the thread is current executing
/// reset: If the thread should reset its data
/// gpio_pins: A hashmap of all channels
pub fn start_light_thread(
    song_path: &Path,
    millisecond_position: Arc<AtomicU64>,
    toggle: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    reset: Arc<AtomicBool>,
    #[cfg(not(target_arch = "x86_64"))] gpio_pins: Arc<Mutex<HashMap<i32, OutputPin>>>,
) {
    let mut light_data = gather_light_data(song_path.to_string_lossy().to_string());

    if !light_data.is_empty() {
        while toggle.load(Ordering::Relaxed) {
            // Ensure there aren't duplicate threads
            thread::sleep(Duration::from_millis(5));
        }

        active.store(true, Ordering::Relaxed);
        thread::spawn(move || loop {
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
                #[cfg(not(target_arch = "x86_64"))]
                {
                    let mut pin1 = gpio_pins.lock().unwrap();
                    all_off(&mut *pin1);
                }
            }
            for channel_data in &mut light_data {
                if let Some(target_time) = channel_data.data.get(channel_data.index) {
                    if target_time.timestamp <= millisecond_position.load(Ordering::Relaxed) as i32
                    {
                        for _channel in &channel_data.channels {
                            #[cfg(target_arch = "x86_64")]
                            println!(
                                "Correct: {}; Actual: {}",
                                target_time.timestamp,
                                millisecond_position.load(Ordering::Relaxed)
                            );

                            #[cfg(not(target_arch = "x86_64"))]
                            let mut pin = gpio_pins.lock().unwrap();
                            #[cfg(not(target_arch = "x86_64"))]
                            interface_gpio(
                                pin.get_mut(&(*_channel as i32)).unwrap(),
                                &target_time.light_type,
                            );
                        }
                        channel_data.index += 1;
                    }
                }
            }

            thread::sleep(Duration::from_millis(5));
        });
    }
}

/// The status of the channel
///
/// On: Powered on
/// Off: Powered off
pub enum LightType {
    On,
    Off,
}

/// The data structure of a light file
/// Contains all the channels and their respective data
#[derive(Serialize, Deserialize, Debug)]
struct Data {
    #[serde(flatten)]
    fields: HashMap<String, HashMap<String, i8>>,
}

/// The data structure of a light file
///
/// Example:
///     "1000, 1" # 1000ms in, turn on
struct LightData {
    timestamp: i32,
    light_type: LightType,
}

/// The data structure of a channel
struct ChannelData {
    channels: Vec<i8>,
    data: Vec<LightData>,
    index: usize,
}

/// Gets the Light Data for an audio path as a vector
///
/// song_path: Path to the audio
fn gather_light_data(song_path: String) -> Vec<ChannelData> {
    let path_string = song_path.replace("wav", "json");
    let path = PathBuf::from(path_string);

    let file = match File::open(path) {
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
            light_data_vec.push(LightData {
                timestamp,
                light_type,
            })
        }
        light_data_vec.sort_by_key(|ld| ld.timestamp);
        data_vec.push(ChannelData {
            channels: parse_channels(channel),
            data: light_data_vec,
            index: 0,
        })
    }
    data_vec
}

/// Gets the channels from a string
///
/// channels_str: The string containing a list of channels
fn parse_channels(channels_str: String) -> Vec<i8> {
    channels_str
        .split(',')
        .filter_map(|s| s.trim().parse::<i8>().ok())
        .collect()
}

/// Sets the output for channels
///
/// pin: The GPIO pin to interface with
/// light_type: Whether to turn it on or off
#[cfg(not(target_arch = "x86_64"))]
pub fn interface_gpio(pin: &mut OutputPin, light_type: &LightType) {
    match light_type {
        LightType::On => {
            pin.set_high();
        }
        LightType::Off => {
            pin.set_low();
        }
    }
}

/// Gets a hashmap of GPIO pins
#[cfg(not(target_arch = "x86_64"))]
pub fn get_gpio_map() -> HashMap<i32, OutputPin> {
    let mut map = HashMap::new();
    for i in 0..16 {
        let gpio = Gpio::new().unwrap();
        let out = gpio.get(i).unwrap().into_output();
        map.insert(i as i32, out);
    }
    map
}

/// Turns off all GPIO pins
///
/// pins: hashmap of GPIO pins
#[cfg(not(target_arch = "x86_64"))]
pub fn all_off(pins: &mut HashMap<i32, OutputPin>) {
    for index in 0..pins.len() {
        interface_gpio(pins.get_mut(&(index as i32)).unwrap(), &LightType::Off);
    }
}
