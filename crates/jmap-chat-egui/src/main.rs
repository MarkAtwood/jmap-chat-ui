use eframe::egui;
use jmap_chat::types::ChatKind;
use jmap_chat_egui::{
    app::AppState,
    client_task,
    event::{AppCommand, AppEvent},
};

fn main() -> eframe::Result<()> {
    let config = <jmap_chat_egui::config::Config as clap::Parser>::parse();

    let auth = config.auth_provider().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    let client =
        jmap_chat::client::JmapChatClient::new(auth, &config.server_url).unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(1);
        });

    let (task_tx, ui_rx) = std::sync::mpsc::sync_channel::<AppEvent>(256);
    let (ui_tx, task_rx) = std::sync::mpsc::sync_channel::<AppCommand>(256);

    eframe::run_native(
        "JMAP Chat",
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .expect("tokio runtime")
                    .block_on(client_task::run(client, task_tx, task_rx, ctx));
            });
            Ok(Box::new(App::new(AppState::default(), ui_tx, ui_rx)))
        }),
    )
}

struct App {
    state: AppState,
    tx: std::sync::mpsc::SyncSender<AppCommand>,
    rx: std::sync::mpsc::Receiver<AppEvent>,
}

impl App {
    pub fn new(
        state: AppState,
        tx: std::sync::mpsc::SyncSender<AppCommand>,
        rx: std::sync::mpsc::Receiver<AppEvent>,
    ) -> Self {
        Self { state, tx, rx }
    }
}

/// Strip control characters from a string, preserving newlines.
fn strip_control_chars(s: &str) -> String {
    s.chars()
        .filter(|&c| !c.is_control() || c == '\n')
        .collect()
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- 1. DRAIN EVENTS (cap at 100 per frame) ---
        let mut count = 0;
        while count < 100 {
            match self.rx.try_recv() {
                Ok(ev) => {
                    self.state.apply_event(ev);
                    count += 1;
                }
                Err(_) => break,
            }
        }
        self.state.tick_error_timeout();

        // --- Check for Enter key to send (before panels so state is consistent) ---
        let enter_pressed = ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
        if enter_pressed {
            if let Some(chat_id) = &self.state.selected_chat.clone() {
                let body = self.state.compose_text.trim().to_string();
                if !body.is_empty() {
                    let _ = self.tx.send(AppCommand::SendMessage {
                        chat_id: chat_id.clone(),
                        body,
                    });
                    self.state.compose_text.clear();
                }
            }
        }

        // --- 2. STATUS BAR (TopBottomPanel::top) ---
        egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.state.status.to_string());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !self.state.account_id.is_empty() {
                        ui.label(&self.state.account_id);
                    }
                });
            });
        });

        // --- 3. COMPOSE BAR (TopBottomPanel::bottom) ---
        egui::TopBottomPanel::bottom("compose_bar")
            .exact_height(70.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let text_edit = egui::TextEdit::multiline(&mut self.state.compose_text)
                        .hint_text("Type a message\u{2026}")
                        .desired_rows(2);
                    ui.add_sized(
                        [ui.available_width() - 70.0, ui.available_height()],
                        text_edit,
                    );
                    let send_clicked = ui
                        .add_sized([60.0, ui.available_height()], egui::Button::new("Send"))
                        .clicked();
                    if send_clicked {
                        if let Some(chat_id) = &self.state.selected_chat.clone() {
                            let body = self.state.compose_text.trim().to_string();
                            if !body.is_empty() {
                                let _ = self.tx.send(AppCommand::SendMessage {
                                    chat_id: chat_id.clone(),
                                    body,
                                });
                                self.state.compose_text.clear();
                            }
                        }
                    }
                });
            });

        // --- 4. CHAT LIST (SidePanel::left) ---
        egui::SidePanel::left("chat_list")
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Chats");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Collect chat data to avoid borrow conflict during click handling
                    let chat_entries: Vec<(String, String, bool)> = self
                        .state
                        .chats
                        .iter()
                        .map(|chat| {
                            let display_name = match chat.kind {
                                ChatKind::Direct => {
                                    chat.contact_id.as_deref().unwrap_or("Direct").to_string()
                                }
                                _ => chat.name.as_deref().unwrap_or("(unnamed)").to_string(),
                            };
                            let label = if chat.unread_count > 0 {
                                format!("{} ({})", display_name, chat.unread_count)
                            } else {
                                display_name
                            };
                            let is_selected =
                                self.state.selected_chat.as_deref() == Some(chat.id.as_ref());
                            (chat.id.to_string(), label, is_selected)
                        })
                        .collect();

                    for (chat_id, label, is_selected) in chat_entries {
                        if ui.selectable_label(is_selected, &label).clicked() {
                            let _ = self.tx.send(AppCommand::SelectChat(chat_id.clone()));
                            self.state.selected_chat = Some(chat_id);
                            self.state.messages.clear();
                        }
                    }
                });
            });

        // --- 5. MESSAGE PANEL (CentralPanel — must be last) ---
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.state.selected_chat.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Select a chat to start messaging");
                });
            } else {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for msg in &self.state.messages {
                            // Sender line: bold sender_id · timestamp
                            ui.horizontal(|ui| {
                                ui.strong(&msg.sender_id);
                                ui.label(" \u{00b7} ");
                                ui.label(msg.sent_at.to_string());
                            });

                            // Body line (strip control chars)
                            let body = strip_control_chars(&msg.body);
                            ui.label(&body);

                            // Edited annotation
                            if msg.edited_at.is_some() {
                                ui.small("[edited]");
                            }

                            // Deleted annotation
                            if msg.deleted_at.is_some() {
                                ui.label(egui::RichText::new("[deleted]").italics());
                            }

                            ui.add_space(4.0);
                        }

                        // Error label at bottom of panel
                        if let Some(err) = &self.state.error {
                            ui.label(egui::RichText::new(err.as_str()).color(egui::Color32::RED));
                        }
                    });
            }
        });
    }
}
