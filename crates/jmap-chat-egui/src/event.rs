use jmap_chat::types::{Chat, ContactPresence, Message};

/// Events sent from the background client task to the egui UI.
///
/// ## ID representation
/// All server-assigned IDs in this enum are `String`, not `jmap_chat::jmap::Id`.
/// The egui event layer is a String boundary: the client task converts `Id` values
/// to `String` at event construction so that `AppState` (which uses `String` for
/// all ID-keyed collections) can operate without `jmap_chat` type dependencies on
/// its internal data structures.
#[derive(Debug)]
#[non_exhaustive]
pub enum AppEvent {
    /// JMAP session bootstrapped successfully.
    SessionReady { api_url: String, account_id: String },
    /// Full chat list loaded or refreshed (initial load or after `has_more_changes`).
    ChatsLoaded(Vec<Chat>),
    /// Incremental update from Chat/changes: chats to add/replace and IDs to remove.
    ///
    /// Apply by: removing all IDs in `destroyed` from the local list, then
    /// inserting or replacing each chat in `created_or_updated` (match on `id`).
    ChatsDelta {
        created_or_updated: Vec<Chat>,
        destroyed: Vec<String>,
    },
    /// Messages loaded for the specified chat.
    MessagesLoaded {
        chat_id: String,
        messages: Vec<Message>,
    },
    /// Connection status changed.
    StatusChanged(ConnectionStatus),
    /// Transient error message to display to the user.
    Error(String),
    /// Ephemeral typing indicator from the WebSocket stream (draft-atwood-jmap-chat-wss-00).
    ///
    /// `typing: true` = sender started typing; `false` = stopped.
    /// Indicators decay automatically after 10 s of no update (the server may not
    /// always send `typing: false` if the user disconnects while typing).
    TypingIndicator {
        chat_id: String,
        sender_id: String,
        typing: bool,
    },
    /// Ephemeral presence update from the WebSocket stream.
    PresenceUpdate {
        contact_id: String,
        presence: ContactPresence,
    },
}

/// Commands sent from the egui UI to the background client task.
#[derive(Debug)]
#[non_exhaustive]
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
    /// Background task exited permanently (auth failure, no JMAP Chat account).
    Disconnected,
}

impl ConnectionStatus {
    /// Return the status label as a `&'static str`, avoiding a heap allocation
    /// on every render frame (60 fps × 1 alloc = 60 allocs/sec with `to_string()`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connecting => "Connecting\u{2026}",
            Self::Connected => "Connected",
            Self::Reconnecting => "Reconnecting\u{2026}",
            Self::Disconnected => "Disconnected",
        }
    }
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
