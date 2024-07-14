use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bluer::{Adapter, AdapterEvent, Address, DiscoveryFilter, DiscoveryTransport};
use futures::{pin_mut, StreamExt};

async fn query_device(adapter: &Adapter, addr: Address) -> bluer::Result<()> {
    let device = adapter.device(addr)?;
    println!("    Address type:       {}", device.address_type().await?);
    println!("    Name:               {:?}", device.name().await?);
    println!("    Icon:               {:?}", device.icon().await?);
    println!("    Class:              {:?}", device.class().await?);
    println!("    UUIDs:              {:?}", device.uuids().await?.unwrap_or_default());
    println!("    Paired:             {:?}", device.is_paired().await?);
    println!("    Connected:          {:?}", device.is_connected().await?);
    println!("    Trusted:            {:?}", device.is_trusted().await?);
    println!("    Modalias:           {:?}", device.modalias().await?);
    println!("    RSSI:               {:?}", device.rssi().await?);
    println!("    TX power:           {:?}", device.tx_power().await?);
    println!("    Manufacturer data:  {:?}", device.manufacturer_data().await?);
    println!("    Service data:       {:?}", device.service_data().await?);
    Ok(())
}

async fn query_all_device_properties(adapter: &Adapter, addr: Address) -> bluer::Result<()> {
    let device = adapter.device(addr)?;
    let props = device.all_properties().await?;
    for prop in props {
        println!("    {:?}", &prop);
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn main() -> bluer::Result<()> {

    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    println!("Discovering devices using Bluetooth adapter {}\n", adapter.name());
    adapter.set_powered(true).await?;

    let filter = DiscoveryFilter {
        transport: DiscoveryTransport::BrEdr,
        ..Default::default()
    };
    adapter.set_discovery_filter(filter).await?;
    println!("Using discovery filter:\n{:#?}\n\n", adapter.discovery_filter().await);

    let device_events = adapter.discover_devices().await?;
    pin_mut!(device_events);

    loop {
        tokio::select! {
            Some(device_event) = device_events.next() => {
                match device_event {
                    AdapterEvent::DeviceAdded(addr) => {

                        println!("Device added: {addr}");
                        let res = query_device(&adapter, addr).await;
                        if let Err(err) = res {
                            println!("    Error: {}", &err);
                        }
                    }
                    AdapterEvent::DeviceRemoved(addr) => {
                        println!("Device removed: {addr}");
                    }
                    _ => (),
                }
                println!();
            }
            else => break
        }
    }

    Ok(())
}

pub struct BluetoothDevice {
    name: String,
    paired: bool,
    connected: bool,
}

pub struct BluetoothDevices {
    devices: Arc<Mutex<HashMap<Address, BluetoothDevice>>>,
}