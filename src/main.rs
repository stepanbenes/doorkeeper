use async_std::{
    io,
    sync::{channel},
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
use std::str::FromStr;

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
    DeviceNotification(String),
    InputCommand(String),
}

#[async_std::main]
async fn main() {

    let peripheral_address = btleplug::api::BDAddr::from_str("00:13:AA:00:BA:0E").unwrap();
    let characteristic_uuid: u16 = 0xFFE1;

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
    let event_sender_clone2 = event_sender.clone();

    // Add ourselves to the central event handler output now, so we don't
    // have to carry around the Central object. We'll be using this in
    // connect anyways.
    let on_event = move |event: CentralEvent| match event {
        CentralEvent::DeviceDiscovered(bd_addr) => {
            //println!("DeviceDiscovered: {:?}", bd_addr);
            let s = event_sender.clone();
            task::spawn(async move {
                s.send(Notification::DeviceDiscovered(bd_addr)).await;
            });
        }
        CentralEvent::DeviceConnected(bd_addr) => {
            //println!("DeviceConnected: {:?}", bd_addr);
            let s = event_sender.clone();
            task::spawn(async move {
                s.send(Notification::DeviceConnected(bd_addr)).await;
            });
        }
        CentralEvent::DeviceDisconnected(bd_addr) => {
            //println!("DeviceDisconnected: {:?}", bd_addr);
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
                    s.send(Notification::InputCommand(line)).await;
                }
            });
            thread::sleep(Duration::from_millis(200));
        }
    });

    central.on_event(Box::new(on_event));
  
    loop {
        let result = event_receiver.recv().await;
        //println!("Received: {:?}", result);
        if let Ok(notification) = result {
        match notification {
                Notification::DeviceDiscovered(device_address) => {
                    if device_address == peripheral_address {
                        if let Some(device) = central.peripheral(device_address) {
                            device.connect().expect("Can't connect to peripheral...");
                        }
                        else {
                            panic!("Device not discovered.");
                        }
                    }
                }
                Notification::DeviceConnected(device_address) => {
                    if let Some(peripheral) = central.peripheral(device_address) {
                        if peripheral.is_connected() {
                            let characteristics = peripheral.discover_characteristics().expect(&format!("Error while discovering characteristics of device '{}'", device_address));
                            if let Some(ch) = characteristics.iter().find(|c| c.uuid == btleplug::api::UUID::B16(characteristic_uuid)) {

                                peripheral.subscribe(ch).expect("Subscription to characteristic failed");
                                
                                let event_sender_clone3 = event_sender_clone2.clone(); // TODO: learn Rust and avoid this ugly clones
                                
                                peripheral.on_notification(Box::new(move |vn| {
                                    let s = event_sender_clone3.clone();
                                    let text = String::from_utf8(vn.value).expect("Notification message contains invalid UTF8 characters.");
                                    task::spawn(async move {
                                        s.send(Notification::DeviceNotification(text)).await;
                                    });
                                }));
                            }
                            else {
                                eprintln!("Characteristic not found!");
                            }
                        }
                    }
                }
                Notification::DeviceDisconnected(device_address) => {
                    // try to reconnect
                    if device_address == peripheral_address {
                        if let Some(device) = central.peripheral(device_address) {
                            device.connect().expect("Can't connect to peripheral...");
                        }
                        else {
                            panic!("Device not discovered.");
                        }
                    }
                }
                Notification::DeviceNotification(notification_text) => {
                    println!("{}", notification_text.trim());
                }
                Notification::InputCommand(command) => {
                    if let Some(peripheral) = central.peripheral(peripheral_address) {
                        if peripheral.is_connected() {
                            if let Some(ch) = peripheral.characteristics().iter().find(|c| c.uuid == btleplug::api::UUID::B16(characteristic_uuid)) {
                                //peripheral.command(ch, &[b'b', b'a', b'h', b'o', b'j', b'\n']).expect("Command failed!");
                                peripheral.command(ch, command.as_bytes()).expect("Command failed!");

                                // TODO: send command/request to peripheral
                                //let result = peripheral.request(last_char, &[b'1', b'\n']).expect("Request failed!");
                                //let result = peripheral.request(last_char, &[b'b', b'a', b'h', b'o', b'j', b'\n']).expect("Request failed!");
                                //println!("Result: {:?}", result);
                                //peripheral.command(last_char, &[b'b', b'a', b'h', b'o', b'j', b'\n']).expect("Command failed!");
                            }
                        }
                    }
                }
            }
        }
    }
}

// https://github.com/deviceplug/btleplug
// https://github.com/deviceplug/btleplug/blob/master/src/api/mod.rs
// https://book.async.rs/
// https://docs.rs/async-std/1.6.1/async_std/
