use jmap_chat::types::{Chat, Message};

/// Events sent from the background client task to the egui UI.
#[derive(Debug)]
pub enum AppEvent {
    /// JMAP session bootstrapped successfully.
    SessionReady { api_url: String, account_id: String },
    /// Chat list loaded or refreshed.
    ChatsLoaded(Vec<Chat>),
    /// Messages loaded for the specified chat.
    MessagesLoaded {
        chat_id: String,
        messages: Vec<Message>,
    },
    /// Connection status changed.
    StatusChanged(ConnectionStatus),
    /// Transient error message to display to the user.
    Error(String),
}

/// Commands sent from the egui UI to the background client task.
#[derive(Debug)]
pub enum AppCommand {
    /// User selected a chat — load its messages.
    SelectChat(String),
    /// User sent a message in the given chat.
    SendMessage { chat_id: String, body: String },
    /// User scrolled to a message — update read position.
    MarkRead { chat_id: String, message_id: String },
}

/// Connection lifecycle state.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
    Disconnected,
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connecting => write!(f, "Connecting\u{2026}"),
            Self::Connected => write!(f, "Connected"),
            Self::Reconnecting => write!(f, "Reconnecting\u{2026}"),
            Self::Error(e) => write!(f, "Error: {e}"),
            Self::Disconnected => write!(f, "Disconnected"),
        }
    }
}
