//! Operator dashboard — does not replace browser audit tests.
//! Runs the HTTP server on a background thread and shows live logs + chat.

use eframe::egui;
use localhost::{Config, EventSender, ServerEngine, ServerEvent};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let conf = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("configs/default.conf"));

    let config = Config::load(&conf).unwrap_or_else(|e| {
        eprintln!("config error: {e}");
        std::process::exit(1);
    });

    let (tx, rx) = mpsc::channel();
    let cfg = config.clone();
    thread::Builder::new()
        .name("localhost-server".into())
        .spawn(move || {
            let _ = ServerEngine::run_with_events(cfg, Some(tx));
        })
        .expect("spawn server thread");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_title("Localhost — operator"),
        ..Default::default()
    };

    eframe::run_native(
        "localhost-gui",
        native_options,
        Box::new(move |_cc| Ok(Box::new(Dashboard::new(rx)))),
    )
}

struct Dashboard {
    events: Receiver<ServerEvent>,
    log: Vec<String>,
    chat: Vec<String>,
}

impl Dashboard {
    fn new(events: Receiver<ServerEvent>) -> Self {
        Self {
            events,
            log: vec!["GUI started — server thread running.".into()],
            chat: Vec::new(),
        }
    }

    fn drain_events(&mut self) {
        while let Ok(ev) = self.events.try_recv() {
            match ev {
                ServerEvent::Listener { addr } => {
                    self.log.push(format!("listen {addr}"));
                }
                ServerEvent::Request {
                    method,
                    path,
                    status,
                } => {
                    self.log.push(format!("{method} {path} → {status}"));
                    if self.log.len() > 500 {
                        self.log.drain(0..100);
                    }
                }
                ServerEvent::Chat { user, text } => {
                    let line = format!("{user}: {text}");
                    self.chat.push(line.clone());
                    self.log.push(format!("[chat] {line}"));
                    if self.chat.len() > 200 {
                        self.chat.drain(0..50);
                    }
                }
            }
        }
    }
}

impl eframe::App for Dashboard {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        ctx.request_repaint_after(std::time::Duration::from_millis(200));

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Localhost operator panel");
            ui.label("Visitors use the browser (http://127.0.0.1:8080/chat.html). This window is for you.");
            ui.separator();

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.heading("HTTP log");
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .show(ui, |ui| {
                            for line in &self.log {
                                ui.monospace(line);
                            }
                        });
                });
                ui.vertical(|ui| {
                    ui.heading("Chat mirror");
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .show(ui, |ui| {
                            for line in &self.chat {
                                ui.label(line);
                            }
                        });
                });
            });

            ui.separator();
            ui.hyperlink_to("Open chat in browser", "http://127.0.0.1:8080/chat.html");
            ui.hyperlink_to("Open home", "http://127.0.0.1:8080/");
        });
    }
}
