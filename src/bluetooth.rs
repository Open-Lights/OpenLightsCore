use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use bluez_async::{BluetoothError, BluetoothSession, DeviceId, MacAddress};
#[cfg(unix)]
use tokio::time::sleep;

use crate::app::{Notification, Timer};

impl BluetoothDevices {
    pub fn new(bt_sender: Sender<Notification>) -> Self {
        Self {
            devices: Arc::new(Mutex::new(Vec::new())),
            bt_sender,
        }
    }

    /// Searches for Bluetooth devices to pair
    #[cfg(unix)]
    pub fn refresh_bluetooth(&mut self) {
        let devices_clone = Arc::clone(&self.devices);
        let bt_sender_clone = self.bt_sender.clone();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let devices = locate_devices().await;
            if let Ok(devices_safe) = devices {
                let mut locked = devices_clone.lock().unwrap();
                *locked = devices_safe;
            } else {
                let notification = Notification {
                    title: "Bluetooth Failure".to_string(),
                    message: "A Bluetooth device has failed to be located.\
            Please ensure your device supports Bluetooth and there are pairable devices in your area.\
            Then try refreshing the Bluetooth again.".to_string(),
                    timer: Timer::new(Duration::from_secs(15)),
                    id: fastrand::i32(0..i32::MAX),
                };
                bt_sender_clone.send(notification).unwrap();
            }
        });
    }

    /// Connects to a Bluetooth device
    #[cfg(unix)]
    pub fn connect_to_device(&mut self, device_id: &DeviceId) {
        let bt_sender_clone = self.bt_sender.clone();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            if connect_device(device_id).await.is_err() {
                let notification = Notification {
                    title: "Bluetooth Failure".to_string(),
                    message: "The Bluetooth device that you tried connecting to isn't responding. \
                    Try restarting the device and then reconnect."
                        .to_string(),
                    timer: Timer::new(Duration::from_secs(15)),
                    id: fastrand::i32(0..i32::MAX),
                };
                bt_sender_clone.send(notification).unwrap();
            };
        });
    }
}

/// Fetches a vector of Bluetooth devices waiting to be paired
#[cfg(unix)]
async fn locate_devices() -> Result<Vec<BluetoothDevice>, BluetoothError> {
    let (_, session) = BluetoothSession::new().await?;

    session.start_discovery().await?;
    sleep(Duration::from_secs(5)).await;
    session.stop_discovery().await?;

    let devices = session.get_devices().await?;

    let mut bluetooth_devices: Vec<BluetoothDevice> = Vec::new();

    for device_info in devices {
        let device = BluetoothDevice {
            name: device_info.name.unwrap_or("Unknown".to_string()),
            paired: device_info.paired,
            connected: device_info.connected,
            id: device_info.id,
            alias: device_info.alias.unwrap_or("None".to_string()),
            mac_address: device_info.mac_address,
        };

        bluetooth_devices.push(device);
    }

    Ok(bluetooth_devices)
}

/// Async connection to a Bluetooth devices
#[cfg(unix)]
async fn connect_device(device_id: &DeviceId) -> Result<(), BluetoothError> {
    let (_, session) = BluetoothSession::new().await?;
    session
        .connect_with_timeout(device_id, Duration::from_secs(10))
        .await
}

/// Bluetooth Device data
///
/// name: The name of the devices
/// paired: If the device is paired
/// connected: If the device is connected
/// id: The device Bluetooth id
/// alias: The device's nickname
/// mac_address: The MAC of the device
pub struct BluetoothDevice {
    pub(crate) name: String,
    pub(crate) paired: bool,
    pub connected: bool,
    #[cfg(unix)]
    pub id: DeviceId,
    pub alias: String,
    #[cfg(unix)]
    pub mac_address: MacAddress,
}

pub struct BluetoothDevices {
    pub devices: Arc<Mutex<Vec<BluetoothDevice>>>,
    bt_sender: Sender<Notification>,
}
