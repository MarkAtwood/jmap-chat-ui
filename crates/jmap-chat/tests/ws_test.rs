// Integration tests for JmapChatClient::connect_ws, WsSession::next_frame,
// WsSession::send_stream_enable, and WsSession::send_stream_disable.
//
// Receive tests: spawn a real in-process WebSocket server using tokio-tungstenite's
// accept_async, send known JSON payloads, assert the client parses them correctly.
//
// Send tests: client sends a frame; mock server reads it and returns it via a
// channel so the test can assert the exact JSON shape.
//
// Oracles: hand-written JSON derived directly from draft-atwood-jmap-chat-wss-00
// and RFC 8887. None of these payloads are produced by the code under test.

use futures::{SinkExt as _, StreamExt as _};
use jmap_chat::{ChatStreamEnable, JmapChatClient, NoneAuth, WsFrame};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// Spawn a one-shot WebSocket server on a random port.
///
/// The server accepts a single connection, runs `server_fn` with the stream,
/// and closes. Returns the `ws://` URL to connect to.
async fn spawn_ws_server<F, Fut>(server_fn: F) -> String
where
    F: FnOnce(tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind must succeed");
    let addr = listener.local_addr().expect("local_addr must succeed");

    tokio::spawn(async move {
        let (tcp, _peer) = listener.accept().await.expect("accept must succeed");
        let ws = accept_async(tcp).await.expect("ws handshake must succeed");
        server_fn(ws).await;
    });

    format!("ws://127.0.0.1:{}/", addr.port())
}

/// Spawn a server that reads the first text frame sent by the client and
/// returns it via a oneshot channel.
async fn spawn_capture_server() -> (String, tokio::sync::oneshot::Receiver<String>) {
    let (tx, rx) = tokio::sync::oneshot::channel();

    let url = spawn_ws_server(|mut ws| async move {
        while let Some(msg) = ws.next().await {
            if let Ok(Message::Text(text)) = msg {
                let _ = tx.send(text.to_string());
                break;
            }
        }
    })
    .await;

    (url, rx)
}

// ---------------------------------------------------------------------------
// Test 1 — connect succeeds and clean close yields None
// ---------------------------------------------------------------------------

/// Oracle: RFC 8887 §4 — after server closes the WebSocket, next_frame returns None.
#[tokio::test]
async fn connect_and_clean_close() {
    let url = spawn_ws_server(|mut ws| async move {
        // Server closes immediately without sending any frames.
        let _ = ws.close(None).await;
    })
    .await;

    let client = JmapChatClient::new(NoneAuth, &format!("http://127.0.0.1"))
        .expect("client construction must not fail");

    let mut session = client
        .connect_ws(&url)
        .await
        .expect("connect_ws must succeed");

    let result = session.next_frame().await;
    // Server closed cleanly → stream ends → None
    assert!(
        result.is_none(),
        "clean close must yield None, got {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 2 — StateChange frame
// ---------------------------------------------------------------------------

/// Oracle: RFC 8887 §5.3 — StateChange @type; hand-written JSON derived from
/// spec example. Client must parse it as WsFrame::StateChange.
#[tokio::test]
async fn receives_state_change_frame() {
    let payload = r#"{"@type":"StateChange","changed":{"account1":{"Chat":"s2","Message":"s7"}}}"#;

    let url = spawn_ws_server({
        let payload = payload.to_owned();
        |mut ws| async move {
            ws.send(Message::Text(payload.into())).await.unwrap();
        }
    })
    .await;

    let client = JmapChatClient::new(NoneAuth, "http://127.0.0.1")
        .expect("client construction must not fail");
    let mut session = client.connect_ws(&url).await.expect("connect_ws");

    let frame = session
        .next_frame()
        .await
        .expect("stream must not end")
        .expect("frame must not be an error");

    assert!(
        matches!(frame, WsFrame::StateChange(_)),
        "expected StateChange, got {frame:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 3 — ChatTypingEvent frame
// ---------------------------------------------------------------------------

/// Oracle: draft-atwood-jmap-chat-wss-00 §ChatTypingEvent wire format.
/// Hand-written JSON; not produced by the code under test.
#[tokio::test]
async fn receives_chat_typing_event() {
    let payload =
        r#"{"@type":"ChatTypingEvent","chatId":"chat-abc","senderId":"user-xyz","typing":true}"#;

    let url = spawn_ws_server({
        let payload = payload.to_owned();
        |mut ws| async move {
            ws.send(Message::Text(payload.into())).await.unwrap();
        }
    })
    .await;

    let client = JmapChatClient::new(NoneAuth, "http://127.0.0.1").unwrap();
    let mut session = client.connect_ws(&url).await.expect("connect_ws");

    let frame = session
        .next_frame()
        .await
        .expect("stream must not end")
        .expect("frame must parse");

    match frame {
        WsFrame::ChatTyping(evt) => {
            assert_eq!(evt.chat_id.as_str(), "chat-abc");
            assert_eq!(evt.sender_id.as_str(), "user-xyz");
            assert!(evt.typing);
        }
        other => panic!("expected ChatTyping, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 4 — ChatPresenceEvent frame
// ---------------------------------------------------------------------------

/// Oracle: draft-atwood-jmap-chat-wss-00 §ChatPresenceEvent wire format.
/// Hand-written JSON; not produced by the code under test.
#[tokio::test]
async fn receives_chat_presence_event() {
    let payload = r#"{"@type":"ChatPresenceEvent","contactId":"user-abc","presence":"online","statusText":"Working remotely"}"#;

    let url = spawn_ws_server({
        let payload = payload.to_owned();
        |mut ws| async move {
            ws.send(Message::Text(payload.into())).await.unwrap();
        }
    })
    .await;

    let client = JmapChatClient::new(NoneAuth, "http://127.0.0.1").unwrap();
    let mut session = client.connect_ws(&url).await.expect("connect_ws");

    let frame = session
        .next_frame()
        .await
        .expect("stream must not end")
        .expect("frame must parse");

    match frame {
        WsFrame::ChatPresence(evt) => {
            assert_eq!(evt.contact_id.as_str(), "user-abc");
            assert_eq!(evt.presence, jmap_chat::ContactPresence::Online);
            assert_eq!(evt.status_text, Some(Some("Working remotely".to_string())));
        }
        other => panic!("expected ChatPresence, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 5 — Unknown @type is forwarded as WsFrame::Unknown
// ---------------------------------------------------------------------------

/// Oracle: spec forward-compat rule — unrecognized @type values MUST be
/// silently ignored (returned as Unknown, not an error).
#[tokio::test]
async fn receives_unknown_frame_type() {
    let payload = r#"{"@type":"FutureServerPush","data":{"x":1}}"#;

    let url = spawn_ws_server({
        let payload = payload.to_owned();
        |mut ws| async move {
            ws.send(Message::Text(payload.into())).await.unwrap();
        }
    })
    .await;

    let client = JmapChatClient::new(NoneAuth, "http://127.0.0.1").unwrap();
    let mut session = client.connect_ws(&url).await.expect("connect_ws");

    let frame = session
        .next_frame()
        .await
        .expect("stream must not end")
        .expect("frame must not be an error");

    match frame {
        WsFrame::Unknown { type_name } => assert_eq!(type_name, "FutureServerPush"),
        other => panic!("expected Unknown, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 6 — multiple frames in sequence
// ---------------------------------------------------------------------------

/// Oracle: WsSession must deliver all frames in order.
/// Server sends two frames; client must receive both.
#[tokio::test]
async fn receives_multiple_frames_in_order() {
    let typing = r#"{"@type":"ChatTypingEvent","chatId":"c1","senderId":"u1","typing":true}"#;
    let presence = r#"{"@type":"ChatPresenceEvent","contactId":"u1","presence":"busy"}"#;

    let url = spawn_ws_server({
        let typing = typing.to_owned();
        let presence = presence.to_owned();
        |mut ws| async move {
            ws.send(Message::Text(typing.into())).await.unwrap();
            ws.send(Message::Text(presence.into())).await.unwrap();
        }
    })
    .await;

    let client = JmapChatClient::new(NoneAuth, "http://127.0.0.1").unwrap();
    let mut session = client.connect_ws(&url).await.expect("connect_ws");

    let frame1 = session.next_frame().await.unwrap().unwrap();
    assert!(matches!(frame1, WsFrame::ChatTyping(_)));

    let frame2 = session.next_frame().await.unwrap().unwrap();
    assert!(matches!(frame2, WsFrame::ChatPresence(_)));
}

// ---------------------------------------------------------------------------
// Test 7 — send_stream_enable: correct JSON shape sent to server
// ---------------------------------------------------------------------------

/// Oracle: draft-atwood-jmap-chat-wss-00 §ChatStreamEnable — the JSON frame
/// sent by the client MUST have @type="ChatStreamEnable", dataTypes array,
/// and optional chatIds. Hand-verified expected JSON; not produced by the
/// code under test.
#[tokio::test]
async fn send_stream_enable_correct_json_shape() {
    let (url, rx) = spawn_capture_server().await;

    let client = JmapChatClient::new(NoneAuth, "http://127.0.0.1").unwrap();
    let mut session = client.connect_ws(&url).await.expect("connect_ws");

    let enable = ChatStreamEnable::new(
        vec![
            jmap_chat::ChatStreamDataType::Typing,
            jmap_chat::ChatStreamDataType::Presence,
        ],
        Some(vec![jmap_chat::Id::from_trusted("chat-1")]),
        None,
    );
    session
        .send_stream_enable(&enable)
        .await
        .expect("send must succeed");

    let received = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .expect("server must receive within 2s")
        .expect("channel must not be dropped");

    let val: serde_json::Value = serde_json::from_str(&received).expect("must be valid JSON");
    assert_eq!(val["@type"], "ChatStreamEnable");
    assert_eq!(val["dataTypes"], serde_json::json!(["typing", "presence"]));
    assert_eq!(val["chatIds"], serde_json::json!(["chat-1"]));
    assert!(
        val.get("contactIds").is_none(),
        "absent contactIds must be omitted"
    );
}

// ---------------------------------------------------------------------------
// Test 8 — send_stream_disable: correct JSON shape sent to server
// ---------------------------------------------------------------------------

/// Oracle: draft-atwood-jmap-chat-wss-00 §ChatStreamDisable — the JSON frame
/// sent by the client MUST have @type="ChatStreamDisable" and nothing else.
#[tokio::test]
async fn send_stream_disable_correct_json_shape() {
    let (url, rx) = spawn_capture_server().await;

    let client = JmapChatClient::new(NoneAuth, "http://127.0.0.1").unwrap();
    let mut session = client.connect_ws(&url).await.expect("connect_ws");

    session
        .send_stream_disable()
        .await
        .expect("send must succeed");

    let received = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .expect("server must receive within 2s")
        .expect("channel must not be dropped");

    let val: serde_json::Value = serde_json::from_str(&received).expect("must be valid JSON");
    assert_eq!(val["@type"], "ChatStreamDisable");
    // Only @type must be present — no other fields.
    let obj = val.as_object().unwrap();
    assert_eq!(
        obj.len(),
        1,
        "ChatStreamDisable must have exactly one field"
    );
}
