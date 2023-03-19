#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

pub mod client;
pub mod state;

use client::{start_tokio_thread, EguiMessage, TokioMessage};
use eframe::{
    egui::{self, CentralPanel, RichText, SidePanel, TopBottomPanel},
    epaint::{ahash::HashMap, mutex::Mutex},
};
use network_tables::v4::MessageData;
use state::UiState;
use tokio::sync::mpsc;

fn main() -> Result<(), eframe::Error> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt()
        .with_env_filter("info,network_tables=debug")
        .init();

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(640.0, 480.0)),
        ..Default::default()
    };

    eframe::run_native(
        "NT GUI",
        options,
        Box::new(|_cc| {
            let (message_sender, egui_receiver) = start_tokio_thread();

            message_sender
                .try_send(TokioMessage::Start("127.0.0.1:5810".parse().unwrap()))
                .unwrap();

            Box::new(MyApp::new(message_sender, egui_receiver))
        }),
    )
}

struct MyApp {
    message_sender: mpsc::Sender<client::TokioMessage>,
    egui_receiver: mpsc::Receiver<EguiMessage>,
    ui_state: UiState,
    topics: Mutex<HashMap<String, MessageData>>,
}

impl MyApp {
    fn new(
        message_sender: mpsc::Sender<client::TokioMessage>,
        egui_receiver: mpsc::Receiver<EguiMessage>,
    ) -> Self {
        Self {
            message_sender,
            egui_receiver,
            ui_state: UiState::default(),
            topics: Mutex::new(HashMap::default()),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let client_message = self.egui_receiver.try_recv();

        if client_message
            .as_ref()
            .err()
            .map(|err| *err == mpsc::error::TryRecvError::Disconnected)
            .unwrap_or_default()
        {
            // If other side closed, indicates a panic ocurred
            panic!("Panic ocurred in the client task.");
        };

        let client_message = client_message.ok();

        if let Some(EguiMessage::Message(message)) = client_message {
            // New topic data received

            // TODO: possibly remove clone of key???
            self.topics
                .lock()
                .insert(message.topic_name.clone(), message);
        };

        if self.ui_state.side_menu_opened() {
            SidePanel::left("side_menu")
                .resizable(true)
                .show(ctx, |ui| {
                    ui.horizontal_top(|ui| {
                        if ui.button(RichText::new("X").strong()).clicked() {
                            self.ui_state.toggle_side_menu();
                        }
                        ui.heading("NT GUI");
                    });

                    ui.vertical(|ui| {
                        ui.menu_button("Settings", |ui| {
                            ui.heading("Settings");
                        });
                    });
                });
        } else {
            TopBottomPanel::top("header").show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    if ui.button("-").clicked() {
                        self.ui_state.toggle_side_menu();
                    }

                    ui.heading("NT GUI");
                });
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            CentralPanel::default().show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (topic_name, topic_data) in &*self.topics.lock() {
                        ui.label(topic_data.data.to_string());
                    }
                });
            });
        });
    }
}
