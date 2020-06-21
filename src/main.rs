use async_std::{
    io,
    prelude::{FutureExt, StreamExt},
    sync::{channel, Receiver},
    task,
};
use btleplug::api::{Central, CentralEvent, Peripheral};
#[cfg(target_os = "linux")]
use btleplug::bluez::{adapter::ConnectedAdapter, manager::Manager};
#[cfg(target_os = "macos")]
use btleplug::corebluetooth::{adapter::Adapter, manager::Manager};
#[cfg(target_os = "windows")]
use btleplug::winrtble::{adapter::Adapter, manager::Manager};
use std::thread;
use std::time::Duration;

// adapter retrieval works differently depending on your platform right now.
// API needs to be aligned.

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn get_central(manager: &Manager) -> Adapter {
    let adapters = manager.adapters().unwrap();
    if adapters.len() <= 0 {
        panic!("Bluetooth adapter(s) were NOT found, sorry...\n");
    }
    adapters.into_iter().nth(0).unwrap()
}

#[cfg(target_os = "linux")]
fn get_central(manager: &Manager) -> ConnectedAdapter {
    let adapters = manager.adapters().unwrap();
    if adapters.len() <= 0 {
        panic!("Bluetooth adapter(s) were NOT found, sorry...\n");
    }
    let mut adapter = adapters.into_iter().nth(0).unwrap();

    // reset the adapter -- clears out any errant state
    adapter = manager.down(&adapter).unwrap();
    adapter = manager.up(&adapter).unwrap();

    adapter.connect().expect("Error connecting to BLE Adapter....")
}

#[derive(Debug, Clone)]
enum Notification {
    DeviceDiscovered(btleplug::api::BDAddr),
    DeviceConnected(btleplug::api::BDAddr),
    DeviceDisconnected(btleplug::api::BDAddr),
    InputLine(String),
}

#[async_std::main]
async fn main() {
    let manager = Manager::new().unwrap();

    // get the first bluetooth adapter
    // connect to the adapter
    let central = get_central(&manager);

    // start scanning for devices
    central.start_scan().expect("Can't scan BLE adapter for connected devices...");
    // instead of waiting, you can use central.on_event to be notified of
    // new devices

    let (event_sender, event_receiver) = channel(256);
    
    let event_sender_clone = event_sender.clone();

    // Add ourselves to the central event handler output now, so we don't
    // have to carry around the Central object. We'll be using this in
    // connect anyways.
    let on_event = move |event: CentralEvent| match event {
        CentralEvent::DeviceDiscovered(bd_addr) => {
            println!("DeviceDiscovered: {:?}", bd_addr);
            let s = event_sender.clone();
            task::spawn(async move {
                s.send(Notification::DeviceDiscovered(bd_addr)).await;
            });
        }
        CentralEvent::DeviceConnected(bd_addr) => {
            println!("DeviceConnected: {:?}", bd_addr);
            let s = event_sender.clone();
            task::spawn(async move {
                s.send(Notification::DeviceConnected(bd_addr)).await;
            });
        }
        CentralEvent::DeviceDisconnected(bd_addr) => {
            println!("DeviceDisconnected: {:?}", bd_addr);
            let s = event_sender.clone();
            task::spawn(async move {
                s.send(Notification::DeviceDisconnected(bd_addr)).await;
            });
        }
        _ => {}
    };
    
    thread::spawn(move || {
        loop {
            let s = event_sender_clone.clone(); // must clone each iteration of the loop
            let stdin = io::stdin();
            let mut line = String::new();
            task::spawn(async move {
                if let Ok(_) = stdin.read_line(&mut line).await {
                    s.send(Notification::InputLine(String::from(line.trim()))).await;
                }
            });
            thread::sleep(Duration::from_millis(200));
        }
    });

    central.on_event(Box::new(on_event));
    
    loop {
        let result = event_receiver.recv().await;
        println!("Received: {:?}", result);
        if let Ok(notification) = result {
        match notification {
                Notification::DeviceDiscovered(device_address) => {
                    if device_address.to_string() == "00:13:AA:00:BA:0E" {
                        if let Some(device) = central.peripheral(device_address) {
                            device.connect().expect("Can't connect to peripheral...");
                        }
                    }
                }
                Notification::DeviceConnected(device_address) => {

                }
                Notification::DeviceDisconnected(device_address) => {}
                Notification::InputLine(line) => {

                }
            }
        }
        //task::sleep(Duration::from_millis(200)).await;
    }
}

// TODO: https://github.com/deviceplug/btleplug
// TODO: https://github.com/deviceplug/btleplug/blob/master/src/api/mod.rs
// TODO: https://book.async.rs/
// TODO: https://docs.rs/async-std/1.6.1/async_std/
