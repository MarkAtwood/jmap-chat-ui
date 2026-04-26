use std::time::Instant;

use jmap_chat::types::{Chat, Message};

use crate::event::{AppEvent, ConnectionStatus};

pub struct AppState {
    pub chats: Vec<Chat>,
    pub selected_chat: Option<String>,
    pub messages: Vec<Message>,
    pub compose_text: String,
    pub status: ConnectionStatus,
    pub error: Option<String>,
    pub error_since: Option<Instant>,
    pub api_url: String,
    pub account_id: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            chats: Vec::new(),
            selected_chat: None,
            messages: Vec::new(),
            compose_text: String::new(),
            status: ConnectionStatus::Connecting,
            error: None,
            error_since: None,
            api_url: String::new(),
            account_id: String::new(),
        }
    }
}

impl AppState {
    /// Apply a single event from the background task.
    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::SessionReady {
                api_url,
                account_id,
            } => {
                self.api_url = api_url;
                self.account_id = account_id;
            }
            AppEvent::ChatsLoaded(chats) => {
                self.chats = chats;
            }
            AppEvent::MessagesLoaded { chat_id, messages } => {
                if self.selected_chat.as_deref() == Some(&chat_id) {
                    self.messages = messages;
                }
            }
            AppEvent::StatusChanged(status) => {
                self.status = status;
            }
            AppEvent::Error(msg) => {
                self.error = Some(msg);
                self.error_since = Some(Instant::now());
            }
        }
    }

    /// Clear transient error if it has been displayed for 5 seconds.
    pub fn tick_error_timeout(&mut self) {
        if let Some(since) = self.error_since {
            if since.elapsed().as_secs() >= 5 {
                self.error = None;
                self.error_since = None;
            }
        }
    }
}
