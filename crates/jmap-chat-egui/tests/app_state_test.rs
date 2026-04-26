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
    let chat: jmap_chat::types::Chat = serde_json::from_str(&json).expect("oracle JSON must parse");

    let mut state = AppState::default();
    state.apply_event(AppEvent::ChatsLoaded(vec![chat.clone()]));

    assert_eq!(state.chats.len(), 1);
    assert_eq!(state.chats[0], chat);
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
    let msg: jmap_chat::types::Message =
        serde_json::from_str(&json).expect("oracle JSON must parse");

    let mut state = AppState::default();
    state.selected_chat = Some(chat_id.to_string());
    state.apply_event(AppEvent::MessagesLoaded {
        chat_id: chat_id.to_string(),
        messages: vec![msg.clone()],
    });

    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0], msg);
}

#[test]
fn apply_messages_for_wrong_chat() {
    let json = minimal_message_json("msg-002", "c2");
    let msg: jmap_chat::types::Message =
        serde_json::from_str(&json).expect("oracle JSON must parse");

    let mut state = AppState::default();
    state.selected_chat = Some("c1".to_string());
    state.apply_event(AppEvent::MessagesLoaded {
        chat_id: "c2".to_string(),
        messages: vec![msg],
    });

    assert!(
        state.messages.is_empty(),
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
