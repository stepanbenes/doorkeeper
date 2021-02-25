use async_std::{io, sync::channel, task};
use btleplug::api::{Central, CentralEvent, Peripheral};
#[cfg(target_os = "linux")]
use btleplug::bluez::{adapter::ConnectedAdapter, manager::Manager};
#[cfg(target_os = "macos")]
use btleplug::corebluetooth::{adapter::Adapter, manager::Manager};
#[cfg(target_os = "windows")]
use btleplug::winrtble::{adapter::Adapter, manager::Manager};
use std::str::FromStr;
use std::thread;
use std::time::Duration;

use std::fs::OpenOptions;
use std::io::prelude::*;

use chrono::Local;

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

    adapter
        .connect()
        .expect("Error connecting to BLE Adapter....")
}

#[derive(Debug, Clone)]
enum Notification {
    DeviceDiscovered(btleplug::api::BDAddr),
    DeviceConnected(btleplug::api::BDAddr),
    DeviceDisconnected(btleplug::api::BDAddr),
    DeviceNotification(String),
    InputCommand(String),
}

enum LogMessage {
    CommandInput(String),
    DeviceOutput(String),
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
    central
        .start_scan()
        .expect("Can't scan BLE adapter for connected devices...");
    // instead of waiting, you can use central.on_event to be notified of
    // new devices

    let (event_sender, event_receiver) = channel(256);
    let event_sender_clone = event_sender.clone();
    let event_sender_clone2 = event_sender.clone();

    let mut notification_buffer = String::new();

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

    // stdin checking
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

    // bluetooth event handling
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
                        } else {
                            panic!("Device not discovered.");
                        }
                    }
                }
                Notification::DeviceConnected(device_address) => {
                    if let Some(peripheral) = central.peripheral(device_address) {
                        if peripheral.is_connected() {
                            let characteristics =
                                peripheral.discover_characteristics().expect(&format!(
                                    "Error while discovering characteristics of device '{}'",
                                    device_address
                                ));
                            if let Some(ch) = characteristics
                                .iter()
                                .find(|c| c.uuid == btleplug::api::UUID::B16(characteristic_uuid))
                            {
                                peripheral
                                    .subscribe(ch)
                                    .expect("Subscription to characteristic failed");

                                let event_sender_clone3 = event_sender_clone2.clone(); // TODO: learn Rust and avoid this ugly clones
                                peripheral.on_notification(Box::new(move |vn| {
                                    let s = event_sender_clone3.clone();
                                    let text = String::from_utf8(vn.value).expect(
                                        "Notification message contains invalid UTF8 characters.",
                                    );
                                    task::spawn(async move {
                                        s.send(Notification::DeviceNotification(text)).await;
                                    });
                                }));
                            } else {
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
                        } else {
                            panic!("Device not discovered.");
                        }
                    }
                }
                Notification::DeviceNotification(notification_text) => {
                    process_device_notification(&notification_text, &mut notification_buffer);
                }
                Notification::InputCommand(command) => {
                    if !command.is_empty() {
                        log_message(LogMessage::CommandInput(command.to_owned()));
                        if let Some(peripheral) = central.peripheral(peripheral_address) {
                            if peripheral.is_connected() {
                                if let Some(ch) = peripheral.characteristics().iter().find(|c| {
                                    c.uuid == btleplug::api::UUID::B16(characteristic_uuid)
                                }) {
                                    //peripheral.command(ch, &[b'b', b'a', b'h', b'o', b'j', b'\n']).expect("Command failed!");
                                    peripheral
                                        .command(ch, command.as_bytes())
                                        .expect("Command failed!");

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
}

fn process_device_notification(notification: &str, notification_buffer: &mut String) {
    // TODO: buffer messages, look for new line character, do not print before new line is received
    for ch in notification.chars() {
        match ch {
            '\n' => {
                process_message(&notification_buffer);
                notification_buffer.clear();
            }
            '\r' => {
                // ignore
            }
            '\0' => {
                // ignore zero byte (received during initialization of arduino)
            }
            _ => {
                notification_buffer.push(ch);
            }
        };
    }
}

fn process_message(message: &str) {
    if message.is_empty() {
        return;
    }
    log_message(LogMessage::DeviceOutput(String::from(message)));

    let tokens: Vec<&str> = message.split(':').collect();
    if tokens.len() == 0 {
        return;
    }

    match tokens[0] {
        "hello" => {
            assert_eq!(tokens.len(), 1);
            report_message(message);
        }
        "bye" => {
            assert_eq!(tokens.len(), 1);
            report_message(message);
        }
        "uptime" => {
            // u32 (millis)
            assert_eq!(tokens.len(), 2);
            report_message(message);
        }
        "led" => {
            // "on" | "off"
            assert_eq!(tokens.len(), 2);
            report_message(message);
        }
        "button" => {
            // "down" | "up" | "press" | "hold"
            assert_eq!(tokens.len(), 2);
            match tokens[1] {
                "press" | "hold" => {
                    report_message(message);
                }
                "down" | "up" => {
                    // ignore
                }
                _ => {
                    panic!(format!("unexpected button modified: {}", tokens[1]));
                }
            }
        }
        "buzzer" => {
            // "on" | "off"
            assert_eq!(tokens.len(), 2);
            report_message(message);
        }
        "buzzer-duration" => {
            // u16 (millis)
            assert_eq!(tokens.len(), 2);
            report_message(message);
        }
        "volume-threshold" => {
            // u16 (max=5000, %)
            assert_eq!(tokens.len(), 2);
            report_message(message);
        }
        "noise" => {
            process_noise_message(&tokens);
        }
        "invalid-command" => {
            // byte : char
            assert_eq!(tokens.len(), 3);
            panic!(format!("{}", message));
        }
        _ => {
            // unknown message
            panic!(format!("unknown message: {}", message));
        }
    }
}

fn process_noise_message(tokens: &Vec<&str>) {
    // noise_duration : max_sound_level : average_peak_frequency : min_peak_frequency : max_peak_frequency
    // u32 : i32 (max=100, %) : i16 : i16 : i16
    assert_eq!(tokens.len(), 6);
    assert_eq!(tokens[0], "noise");
    let noise_duration: u32 = tokens[1].parse().expect(
        format!(
            "Could not parse noise_duration token as u32. '{}'",
            tokens[1]
        )
        .as_str(),
    );
    let max_sound_level: i32 = tokens[2].parse().expect(
        format!(
            "Could not parse max_sound_level token as i32. '{}'",
            tokens[2]
        )
        .as_str(),
    );
    let average_peak_frequency: i16 = tokens[3].parse().expect(
        format!(
            "Could not parse average_peak_frequency token as i16. '{}'",
            tokens[3]
        )
        .as_str(),
    );
    //let min_peak_frequency: i16 = tokens[4].parse().expect(format!("Could not parse min_peak_frequency token as i16. '{}'", tokens[4]).as_str());
    //let max_peak_frequency: i16 = tokens[5].parse().expect(format!("Could not parse max_peak_frequency token as i16. '{}'", tokens[5]).as_str());
    //let max_deviation = std::cmp::max(average_peak_frequency - min_peak_frequency, max_peak_frequency - average_peak_frequency);

    if noise_duration > 300
        && max_sound_level > 50
        && average_peak_frequency > 500
        && average_peak_frequency < 900
    {
        report_message(format!("Nekdo zvoni! ({} ms)", noise_duration).as_str());
    }
}

fn report_message(message: &str) {
    // print to standard output
    println!("{}", message); // do not use print!, it does not flush the stream
}

fn log_message(message: LogMessage) {
    let now = Local::now();
    let file_name = format!("/var/log/doorkeeper/{}.log", now.format("%Y-%m-%d"));
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&file_name)
        .expect(format!("Cannot open file '{}'.", file_name).as_str());
    match message {
        LogMessage::CommandInput(text) => {
            writeln!(file, "[{}] > {}", now.format("%H:%M:%S"), text.trim())
                .expect(format!("Could not write to file '{}'.", file_name).as_str());
        }
        LogMessage::DeviceOutput(text) => {
            writeln!(file, "[{}] < {}", now.format("%H:%M:%S"), text.trim())
                .expect(format!("Could not write to file '{}'.", file_name).as_str());
        }
    }
}

// https://github.com/deviceplug/btleplug
// https://github.com/deviceplug/btleplug/blob/master/src/api/mod.rs
// https://book.async.rs/
// https://docs.rs/async-std/1.6.1/async_std/
