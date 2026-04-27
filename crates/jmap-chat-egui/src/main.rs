use eframe::egui;
use jmap_chat_egui::{
    app::AppState,
    client_task,
    event::{AppCommand, AppEvent, ConnectionStatus},
};

fn main() -> eframe::Result<()> {
    let config = <jmap_chat_egui::config::Config as clap::Parser>::parse();

    let auth = config.auth_provider().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    let client = jmap_chat::JmapChatClient::new(auth, &config.server_url).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    // Unbounded channel from background task to UI: events are never dropped
    // due to backpressure (no channel-full condition; receiver is always draining).
    let (task_tx, ui_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    // Bounded channel from UI to background task: 256 cap; UI send() is in the
    // sync egui update() and blocking briefly is acceptable here.
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
    rx: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
}

impl App {
    pub fn new(
        state: AppState,
        tx: std::sync::mpsc::SyncSender<AppCommand>,
        rx: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    ) -> Self {
        Self { state, tx, rx }
    }
}

impl App {
    fn show_chat_list(&mut self, ui: &mut egui::Ui) {
        ui.heading("Chats");
        ui.separator();
        // Collect the clicked ID outside the scroll loop to avoid holding a borrow
        // on chat_display while we mutate selected_chat and handle the send result.
        let mut clicked_id: Option<String> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for entry in &self.state.chat_display {
                let is_selected = self.state.selected_chat.as_deref() == Some(entry.id.as_str());
                if ui.selectable_label(is_selected, &entry.label).clicked() {
                    clicked_id = Some(entry.id.clone());
                }
            }
        });
        if let Some(id) = clicked_id {
            if self.tx.send(AppCommand::SelectChat(id.clone())).is_err() {
                // Background task is gone — show Disconnected rather than silently
                // swallowing the user action.
                self.state
                    .apply_event(AppEvent::StatusChanged(ConnectionStatus::Disconnected));
            } else {
                self.state.selected_chat = Some(id);
                self.state.message_entries.clear();
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- 1. DRAIN EVENTS (cap at 100 per frame to bound render latency) ---
        let mut count = 0;
        while count < 100 {
            match self.rx.try_recv() {
                Ok(ev) => {
                    self.state.apply_event(ev);
                    count += 1;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    // Background task dropped its sender — died without sending a
                    // StatusChanged(Disconnected) event (e.g. unexpected panic). Synthesize
                    // the status so the UI does not stay frozen on "Connecting…".
                    self.state
                        .apply_event(AppEvent::StatusChanged(ConnectionStatus::Disconnected));
                    break;
                }
            }
        }
        self.state.tick_error_timeout();

        // --- 2. STATUS BAR (TopBottomPanel::top) ---
        egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.state.status.as_str());
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
                    let te_resp = ui.add_sized(
                        [ui.available_width() - 70.0, ui.available_height()],
                        text_edit,
                    );
                    // Enter sends only when the compose TextEdit has focus;
                    // guards against accidental sends while navigating the chat list.
                    let enter_pressed = te_resp.has_focus()
                        && ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                    let send_clicked = ui
                        .add_sized([60.0, ui.available_height()], egui::Button::new("Send"))
                        .clicked();
                    if enter_pressed || send_clicked {
                        if let Some(chat_id) = self.state.selected_chat.clone() {
                            let body = self.state.compose_text.trim().to_string();
                            if !body.is_empty() {
                                if self
                                    .tx
                                    .send(AppCommand::SendMessage { chat_id, body })
                                    .is_err()
                                {
                                    // Background task is gone — show Disconnected.
                                    self.state.apply_event(AppEvent::StatusChanged(
                                        ConnectionStatus::Disconnected,
                                    ));
                                } else {
                                    self.state.compose_text.clear();
                                }
                            }
                        }
                    }
                });
            });

        // --- 4. CHAT LIST (SidePanel::left) ---
        egui::SidePanel::left("chat_list")
            .default_width(220.0)
            .show(ctx, |ui| {
                self.show_chat_list(ui);
            });

        // --- 5. MESSAGE PANEL (CentralPanel — must be last) ---
        egui::CentralPanel::default().show(ctx, |ui| {
            // Error banner always visible — errors from bootstrap (auth failure,
            // no chats loaded) fire before any chat is selected, so they must
            // render in the no-selection branch too.
            if let Some(err) = &self.state.error {
                ui.label(egui::RichText::new(err.as_str()).color(egui::Color32::RED));
                ui.separator();
            }

            if self.state.selected_chat.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Select a chat to start messaging");
                });
            } else {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        // message_entries are precomputed in apply_event;
                        // no per-frame stripping or formatting.
                        for entry in &self.state.message_entries {
                            ui.horizontal(|ui| {
                                ui.strong(entry.message.sender_id.to_string());
                                ui.label(" \u{00b7} ");
                                ui.label(&entry.timestamp);
                            });

                            ui.label(&entry.body);

                            if entry.message.edited_at.is_some() {
                                ui.small("[edited]");
                            }

                            if entry.message.deleted_at.is_some() {
                                ui.label(egui::RichText::new("[deleted]").italics());
                            }

                            ui.add_space(4.0);
                        }
                    });
            }
        });
    }
}
