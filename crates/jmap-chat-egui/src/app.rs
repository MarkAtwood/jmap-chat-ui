use std::collections::HashMap;
use std::time::{Duration, Instant};

use jmap_chat::{Chat, ChatKind, ContactPresence, Message};

use crate::event::{AppEvent, ConnectionStatus};

/// A single chat-list entry, precomputed for the render loop.
pub struct ChatEntry {
    pub id: String,
    pub label: String,
}

/// A single displayed message: the raw data plus precomputed render fields.
///
/// Grouping the three fields into one struct makes it impossible to clear or
/// update them out of sync — the previous three-parallel-vec design required
/// every code path to remember to touch all three in the same operation.
pub struct MessageEntry {
    pub message: Message,
    /// Message body stripped of C0 control characters (except `\n` and `\t`).
    /// Computed once at load time by [`strip_control_chars`].
    pub body: String,
    /// `sent_at` formatted as a string. Cached to avoid per-frame allocation.
    pub timestamp: String,
}

pub struct AppState {
    /// Raw chat records from the server.
    ///
    /// Private to enforce the `chat_display` sync invariant: every mutation
    /// must be followed by `rebuild_chat_display()`. Access read-only via
    /// [`AppState::chats`]. Prefer `chat_display` in the render loop; use
    /// `chats()` only when you need fields absent from `ChatEntry` (e.g.
    /// `unread_count`, `kind`).
    chats: Vec<Chat>,
    /// Precomputed display entries, always in sync with `chats`.
    ///
    /// **Invariant**: must be rebuilt via `rebuild_chat_display()` every time
    /// `self.chats` is mutated. Never mutate this field directly from outside
    /// `apply_event` — changes to `chats` must go through apply_event so the
    /// cache stays consistent.
    pub chat_display: Vec<ChatEntry>,
    pub selected_chat: Option<String>,
    /// Messages for the selected chat, with precomputed render fields.
    /// Cleared on chat switch; populated via `MessagesLoaded` in `apply_event`.
    pub message_entries: Vec<MessageEntry>,
    pub compose_text: String,
    pub status: ConnectionStatus,
    pub error: Option<String>,
    pub error_since: Option<Instant>,
    pub api_url: String,
    pub account_id: String,
    /// Active typing indicators: `(chat_id, sender_id)` → time of last `typing: true` event.
    ///
    /// Entries are removed when `typing: false` arrives or when they decay
    /// (use [`active_typers_in_chat`](AppState::active_typers_in_chat) to read,
    /// which filters out entries older than 10 s).
    pub typing_indicators: HashMap<(String, String), Instant>,
    /// Latest known presence state per `contact_id`.
    pub presence: HashMap<String, ContactPresence>,
    /// Whether the server advertises the chat WebSocket capability.
    ///
    /// False until `SessionReady` is received (safe default: features are
    /// suppressed until the server confirms support).
    pub supports_ephemeral: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            chats: Vec::new(),
            chat_display: Vec::new(),
            selected_chat: None,
            message_entries: Vec::new(),
            compose_text: String::new(),
            status: ConnectionStatus::Connecting,
            error: None,
            error_since: None,
            api_url: String::new(),
            account_id: String::new(),
            typing_indicators: HashMap::new(),
            presence: HashMap::new(),
            supports_ephemeral: false,
        }
    }
}

impl AppState {
    /// Read-only access to the raw chat list.
    ///
    /// Mutation goes through `apply_event` only, which keeps `chat_display` in
    /// sync. Do not expose a mutable reference — callers that need to change
    /// chats must emit an `AppEvent`.
    pub fn chats(&self) -> &[Chat] {
        &self.chats
    }

    /// Apply a single event from the background task.
    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::SessionReady {
                api_url,
                account_id,
                supports_ephemeral,
            } => {
                self.api_url = api_url;
                self.account_id = account_id;
                self.supports_ephemeral = supports_ephemeral;
            }
            AppEvent::ChatsLoaded(chats) => {
                self.chats = chats;
                self.rebuild_chat_display();
            }
            AppEvent::ChatsDelta {
                created_or_updated,
                destroyed,
            } => {
                let destroyed_set: std::collections::HashSet<&str> =
                    destroyed.iter().map(String::as_str).collect();
                self.chats
                    .retain(|c| !destroyed_set.contains(c.id.as_str()));
                for chat in created_or_updated {
                    if let Some(pos) = self.chats.iter().position(|c| c.id == chat.id) {
                        self.chats[pos] = chat;
                    } else {
                        self.chats.push(chat);
                    }
                }
                self.rebuild_chat_display();
            }
            AppEvent::MessagesLoaded { chat_id, messages } => {
                if self.selected_chat.as_deref() == Some(&chat_id) {
                    self.message_entries = messages
                        .into_iter()
                        .map(|m| {
                            let body = strip_control_chars(&m.body);
                            let timestamp = m.sent_at.to_string();
                            MessageEntry {
                                message: m,
                                body,
                                timestamp,
                            }
                        })
                        .collect();
                }
            }
            AppEvent::StatusChanged(status) => {
                self.status = status;
            }
            AppEvent::Error(msg) => {
                self.error = Some(msg);
                self.error_since = Some(Instant::now());
            }
            AppEvent::TypingIndicator {
                chat_id,
                sender_id,
                typing,
            } => {
                if typing {
                    self.typing_indicators
                        .insert((chat_id, sender_id), Instant::now());
                } else {
                    self.typing_indicators.remove(&(chat_id, sender_id));
                }
            }
            AppEvent::PresenceUpdate {
                contact_id,
                presence,
            } => {
                self.presence.insert(contact_id, presence);
            }
            AppEvent::EphemeralUnavailable => {
                self.supports_ephemeral = false;
            }
        }
    }

    /// Return the sender IDs of contacts who are currently typing in `chat_id`.
    ///
    /// Filters out entries older than 10 s, implementing the decay rule from
    /// draft-atwood-jmap-chat-wss-00: if no `ChatTypingEvent` with `typing: true`
    /// arrives within 10 s, the indicator is suppressed.
    pub fn active_typers_in_chat<'a>(&'a self, chat_id: &str) -> Vec<&'a str> {
        let threshold = Duration::from_secs(10);
        self.typing_indicators
            .iter()
            .filter(|((cid, _sid), ts)| cid == chat_id && ts.elapsed() < threshold)
            .map(|((_, sid), _)| sid.as_str())
            .collect()
    }

    /// Clear transient error if it has been displayed for 5 seconds.
    ///
    /// Called each frame from the egui paint loop, which acts as a low-resolution
    /// clock. Errors are non-fatal and auto-dismissed; persistent failures appear
    /// as status changes (Reconnecting or Disconnected), not as permanent overlays.
    pub fn tick_error_timeout(&mut self) {
        if let Some(since) = self.error_since {
            if since.elapsed().as_secs() >= 5 {
                self.error = None;
                self.error_since = None;
            }
        }
    }

    /// Rebuild `chat_display` from the current `chats` list.
    ///
    /// Must be called every time `self.chats` is mutated. The cache is the
    /// single source of truth for what the render loop displays; stale entries
    /// will show wrong labels or missing chats until the next rebuild.
    fn rebuild_chat_display(&mut self) {
        self.chat_display = self
            .chats
            .iter()
            .map(|chat| {
                let display_name = match chat.kind {
                    ChatKind::Direct => chat.contact_id.as_deref().unwrap_or("Direct").to_string(),
                    _ => chat.name.as_deref().unwrap_or("(unnamed)").to_string(),
                };
                let label = if chat.unread_count > 0 {
                    format!("{} ({})", display_name, chat.unread_count)
                } else {
                    display_name
                };
                ChatEntry {
                    id: chat.id.to_string(),
                    label,
                }
            })
            .collect();
    }
}

/// Strip C0 control characters from a string, preserving `\n` (newline) and
/// `\t` (tab).
///
/// Used to sanitise message bodies before display: ANSI escape sequences
/// (`\x1b[...m`), BEL (`\x07`), and similar control characters can corrupt
/// egui text rendering. Newlines and tabs are preserved because they carry
/// visible meaning in chat messages (paragraphs, code formatting, columns).
/// Bodies are stripped once at load time (see `AppState::apply_event`) rather
/// than per-frame.
pub(crate) fn strip_control_chars(s: &str) -> String {
    s.chars()
        .filter(|&c| !c.is_control() || c == '\n' || c == '\t')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oracle: plain text passes through unchanged.
    #[test]
    fn strip_control_chars_plain_text() {
        assert_eq!(strip_control_chars("Hello, World!"), "Hello, World!");
    }

    /// Oracle: newlines are preserved (explicit in the whitelist).
    #[test]
    fn strip_control_chars_preserves_newlines() {
        assert_eq!(strip_control_chars("line1\nline2"), "line1\nline2");
    }

    /// Oracle: tabs are preserved (used in code formatting and aligned columns).
    #[test]
    fn strip_control_chars_preserves_tabs() {
        assert_eq!(strip_control_chars("col1\tcol2"), "col1\tcol2");
    }

    /// Oracle: NUL (0x00) is a C0 control — removed.
    #[test]
    fn strip_control_chars_removes_nul() {
        assert_eq!(strip_control_chars("a\x00b"), "ab");
    }

    /// Oracle: BEL (0x07) is a C0 control — removed.
    #[test]
    fn strip_control_chars_removes_bel() {
        assert_eq!(strip_control_chars("a\x07b"), "ab");
    }

    /// Oracle: ESC (0x1b) is a C0 control — removed (strips ANSI escape preamble).
    #[test]
    fn strip_control_chars_removes_esc() {
        assert_eq!(strip_control_chars("a\x1bb"), "ab");
    }

    /// Oracle: non-ASCII Unicode (emoji, accented letters) passes through unchanged.
    #[test]
    fn strip_control_chars_passes_unicode() {
        assert_eq!(
            strip_control_chars("caf\u{00e9} \u{1f600}"),
            "caf\u{00e9} \u{1f600}"
        );
    }

    /// Oracle: a string containing only non-whitelisted control characters becomes empty.
    #[test]
    fn strip_control_chars_all_control_becomes_empty() {
        assert_eq!(strip_control_chars("\x00\x01\x1f\x7f"), "");
    }
}
