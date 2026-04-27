use jmap_chat::ContactPresence;
use jmap_chat_egui::app::AppState;
use jmap_chat_egui::event::{AppEvent, ConnectionStatus};

/// Oracle: spec §4.9 — minimal valid Chat JSON derived from spec field definitions,
/// hand-written independently of the code under test.
fn minimal_chat_json(id: &str) -> String {
    format!(
        r#"{{
            "id": "{id}",
            "kind": "direct",
            "contactId": "user:bob@example.com",
            "createdAt": "2024-01-01T00:00:00Z",
            "unreadCount": 0,
            "pinnedMessageIds": [],
            "muted": false
        }}"#
    )
}

/// Oracle: spec §4.10 — minimal valid Message JSON derived from spec field
/// definitions, hand-written independently of the code under test.
fn minimal_message_json(id: &str, chat_id: &str) -> String {
    format!(
        r#"{{
            "id": "{id}",
            "senderMsgId": "{id}",
            "chatId": "{chat_id}",
            "senderId": "self",
            "body": "hello",
            "bodyType": "text/plain",
            "attachments": [],
            "mentions": [],
            "actions": [],
            "reactions": {{}},
            "replyCount": 0,
            "sentAt": "2024-01-01T00:00:01Z",
            "receivedAt": "2024-01-01T00:00:02Z",
            "deliveryState": "delivered"
        }}"#
    )
}

#[test]
fn apply_chats_loaded() {
    let json = minimal_chat_json("chat-001");
    let chat: jmap_chat::Chat = serde_json::from_str(&json).expect("oracle JSON must parse");

    let mut state = AppState::default();
    state.apply_event(AppEvent::ChatsLoaded(vec![chat.clone()]));

    assert_eq!(state.chats().len(), 1);
    assert_eq!(state.chats()[0], chat);
}

#[test]
fn apply_status_changed() {
    let mut state = AppState::default();
    state.apply_event(AppEvent::StatusChanged(ConnectionStatus::Connected));

    assert_eq!(state.status, ConnectionStatus::Connected);
}

#[test]
fn apply_error() {
    let mut state = AppState::default();
    state.apply_event(AppEvent::Error("oops".to_string()));

    assert_eq!(state.error.as_deref(), Some("oops"));
    assert!(state.error_since.is_some());
}

#[test]
fn apply_messages_for_selected_chat() {
    let chat_id = "c1";
    let json = minimal_message_json("msg-001", chat_id);
    let msg: jmap_chat::Message = serde_json::from_str(&json).expect("oracle JSON must parse");

    let mut state = AppState::default();
    state.selected_chat = Some(chat_id.to_string());
    state.apply_event(AppEvent::MessagesLoaded {
        chat_id: chat_id.to_string(),
        messages: vec![msg.clone()],
    });

    assert_eq!(state.message_entries.len(), 1);
    assert_eq!(state.message_entries[0].message, msg);
    // Verify the derived render fields are populated.
    assert_eq!(state.message_entries[0].body, "hello");
    assert!(!state.message_entries[0].timestamp.is_empty());
}

#[test]
fn apply_messages_for_wrong_chat() {
    let json = minimal_message_json("msg-002", "c2");
    let msg: jmap_chat::Message = serde_json::from_str(&json).expect("oracle JSON must parse");

    let mut state = AppState::default();
    state.selected_chat = Some("c1".to_string());
    state.apply_event(AppEvent::MessagesLoaded {
        chat_id: "c2".to_string(),
        messages: vec![msg],
    });

    assert!(
        state.message_entries.is_empty(),
        "messages for a different chat must be ignored"
    );
}

#[test]
fn tick_clears_no_error_without_panic() {
    let mut state = AppState::default();
    // error is None — must not panic
    state.tick_error_timeout();
    assert!(state.error.is_none());
    assert!(state.error_since.is_none());
}

// ---------------------------------------------------------------------------
// TypingIndicator tests
// ---------------------------------------------------------------------------

/// Oracle: draft-atwood-jmap-chat-wss-00 — typing=true inserts into the map;
/// active_typers_in_chat must include the sender.
#[test]
fn apply_typing_indicator_true_adds_typer() {
    let mut state = AppState::default();
    state.apply_event(AppEvent::TypingIndicator {
        chat_id: "chat-1".to_string(),
        sender_id: "user-a".to_string(),
        typing: true,
    });

    let typers = state.active_typers_in_chat("chat-1");
    assert_eq!(typers, vec!["user-a"]);
}

/// Oracle: typing=false removes from the map; active_typers_in_chat must be empty.
#[test]
fn apply_typing_indicator_false_removes_typer() {
    let mut state = AppState::default();
    state.apply_event(AppEvent::TypingIndicator {
        chat_id: "chat-1".to_string(),
        sender_id: "user-a".to_string(),
        typing: true,
    });
    state.apply_event(AppEvent::TypingIndicator {
        chat_id: "chat-1".to_string(),
        sender_id: "user-a".to_string(),
        typing: false,
    });

    let typers = state.active_typers_in_chat("chat-1");
    assert!(typers.is_empty(), "typing=false must remove the indicator");
}

/// Oracle: typing indicators in one chat must not appear in another.
#[test]
fn typing_indicators_are_chat_scoped() {
    let mut state = AppState::default();
    state.apply_event(AppEvent::TypingIndicator {
        chat_id: "chat-1".to_string(),
        sender_id: "user-a".to_string(),
        typing: true,
    });

    assert!(
        state.active_typers_in_chat("chat-2").is_empty(),
        "typing indicator in chat-1 must not appear in chat-2"
    );
}

// ---------------------------------------------------------------------------
// PresenceUpdate tests
// ---------------------------------------------------------------------------

/// Oracle: draft-atwood-jmap-chat-wss-00 — PresenceUpdate stores the latest
/// presence for the contact_id.
#[test]
fn apply_presence_update_stores_presence() {
    let mut state = AppState::default();
    state.apply_event(AppEvent::PresenceUpdate {
        contact_id: "user-b".to_string(),
        presence: ContactPresence::Busy,
    });

    assert_eq!(
        state.presence.get("user-b"),
        Some(&ContactPresence::Busy),
        "presence must be stored for contact_id"
    );
}

/// Oracle: subsequent PresenceUpdate replaces the previous value.
#[test]
fn apply_presence_update_replaces_previous() {
    let mut state = AppState::default();
    state.apply_event(AppEvent::PresenceUpdate {
        contact_id: "user-b".to_string(),
        presence: ContactPresence::Online,
    });
    state.apply_event(AppEvent::PresenceUpdate {
        contact_id: "user-b".to_string(),
        presence: ContactPresence::Away,
    });

    assert_eq!(
        state.presence.get("user-b"),
        Some(&ContactPresence::Away),
        "second presence update must replace the first"
    );
}
