use std::{
    error::Error,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    net::TcpStream,
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(PartialEq)]
enum Operations {
    Read,
    Insert,
    Delete,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum Request {
    Insert(String, Value),
    Delete(String),
    Read(String),
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
        ..Default::default()
    };
    let mut operation = Operations::Read;
    let mut address = String::from("127.0.0.1:3000");
    let mut key = String::new();
    let mut value = String::new();
    let mut reader = None;
    let mut writer = None;
    let mut error_message = String::new();
    let mut last_response = String::new();

    eframe::run_simple_native("Mini Redis Client", options, move |ctx, _frame| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.style_mut().spacing.item_spacing = (8.0, 8.0).into();
            ui.heading(egui::RichText::from("Mini Redis Client"));

            ui.horizontal(|ui| {
                ui.label("Connect to: ");
                ui.text_edit_singleline(&mut address);
                if ui.button("Connect").clicked() {
                    match TcpStream::connect(&address) {
                        Ok(c) => {
                            reader = Some(BufReader::new(c.try_clone().unwrap()));
                            writer = Some(BufWriter::new(c.try_clone().unwrap()));
                            error_message = "Connected".into();
                        }
                        Err(e) => {
                            error_message = format!("Error: {:?}", e);
                        }
                    }
                }

                ui.label(format!("{}", error_message));
            });

            ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing = (12.0, 12.0).into();
                ui.radio_value(&mut operation, Operations::Read, "Read");
                ui.radio_value(&mut operation, Operations::Insert, "Insert");
                ui.radio_value(&mut operation, Operations::Delete, "Delete");
            });
            ui.horizontal(|ui| {
                ui.label("Key: ");
                ui.text_edit_singleline(&mut key);
            });
            ui.vertical(|ui| {
                ui.label("Value");
                ui.text_edit_multiline(&mut value);
            });

            if ui.button("Send").clicked() {
                match writer.as_mut() {
                    Some(c) => {
                        let req = match operation {
                            Operations::Read => Request::Read(key.clone()),
                            Operations::Insert => {
                                Request::Insert(key.clone(), value.clone().into())
                            }
                            Operations::Delete => Request::Delete(key.clone()),
                        };
                        match serde_json::to_vec(&req) {
                            Ok(mut b) => {
                                b.extend("\n".as_bytes());
                                match c.write_all(&b) {
                                    Ok(_) => {
                                        let _ = c.flush();
                                        error_message = format!("Success");

                                        match reader.as_mut() {
                                            Some(r) => match r.read_line(&mut last_response) {
                                                Ok(_) => {}
                                                Err(e) => {
                                                    error_message = format!("Error: {}", e);
                                                }
                                            },
                                            None => error_message = "Error reading response".into(),
                                        }
                                    }
                                    Err(e) => {
                                        error_message = format!("Error: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                error_message = format!("Error: {}", e);
                            }
                        }
                    }
                    None => error_message = "Not connected.".into(),
                }
            };
            ui.horizontal(|ui| {
                ui.label(format!("Last response: {}", last_response));
            });
        });
    })
}
