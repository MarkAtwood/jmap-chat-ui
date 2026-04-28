// WebSocket transport client for JMAP Chat (draft-atwood-jmap-chat-wss-00)
//
// Provides `JmapChatClient::connect_ws` which establishes a WebSocket
// connection and returns a `WsSession` for sending and receiving frames.
//
// URL source: `Session::websocket_capability()?.url` (from
// capabilities["urn:ietf:params:jmap:websocket"]).
// Chat ephemeral events (typing, presence) require the server to also
// advertise `urn:ietf:params:jmap:chat:websocket`.

use futures::SinkExt as _;
use futures::StreamExt as _;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;

/// Maximum WebSocket message size (1 MiB), consistent with the SSE frame limit.
/// Prevents a misbehaving or hostile server from forcing the client to buffer
/// large messages over the ephemeral-event connection.
const MAX_WS_MESSAGE_BYTES: usize = 1 << 20; // 1 MiB

/// A parsed frame received from the JMAP Chat WebSocket.
///
/// Marked `#[non_exhaustive]` because the spec may define additional
/// `@type` values in future revisions.
#[non_exhaustive]
#[derive(Debug)]
pub enum WsFrame {
    /// RFC 8887 StateChange — one or more object types have changed state;
    /// client must re-fetch the affected data types.
    StateChange(serde_json::Value),
    /// RFC 8887 Response — reply to a JMAP request sent on this connection.
    Response(crate::jmap::JmapResponse),
    /// Ephemeral typing indicator (draft-atwood-jmap-chat-wss-00).
    /// Server sends these only after `ChatStreamEnable` has been sent.
    ChatTyping(crate::types::ChatTypingEvent),
    /// Ephemeral presence update (draft-atwood-jmap-chat-wss-00).
    /// Server sends these only after `ChatStreamEnable` has been sent.
    ChatPresence(crate::types::ChatPresenceEvent),
    /// Unrecognized `@type` — silently ignored per forward-compatibility rules
    /// (spec §forward-compat: clients SHOULD ignore unknown message types).
    Unknown { type_name: String },
}

type Inner =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// An established JMAP Chat WebSocket session.
///
/// Call [`next_frame`](WsSession::next_frame) in a loop to receive events.
/// Use the crate-private `send_json` to transmit frames (called by typed
/// methods in the `ws` module, e.g. `send_stream_enable`).
///
/// The caller is responsible for reconnecting after the stream ends or returns
/// a transport error. Use exponential backoff.
pub struct WsSession {
    sink: futures::stream::SplitSink<Inner, Message>,
    stream: futures::stream::SplitStream<Inner>,
}

impl WsSession {
    /// Receive the next parsed frame from the server.
    ///
    /// Returns `None` when the server has cleanly closed the connection.
    /// Returns `Some(Err(...))` on parse failure or transport error. After a
    /// transport error the connection is broken; do not call `next_frame` again.
    pub async fn next_frame(&mut self) -> Option<Result<WsFrame, crate::error::ClientError>> {
        loop {
            match self.stream.next().await? {
                Ok(Message::Text(text)) => return Some(parse_ws_frame(&text)),
                Ok(Message::Close(_)) => return None,
                Ok(_) => continue, // Ping / Pong / Binary: silently skip
                Err(e) => return Some(Err(crate::error::ClientError::WebSocket(e))),
            }
        }
    }

    /// Send a JSON value as a WebSocket text frame.
    ///
    /// Used by typed send methods (`send_stream_enable`, etc.) in this module.
    pub(crate) async fn send_json(
        &mut self,
        value: &serde_json::Value,
    ) -> Result<(), crate::error::ClientError> {
        let text = serde_json::to_string(value)?;
        self.sink
            .send(Message::Text(text.into()))
            .await
            .map_err(crate::error::ClientError::WebSocket)
    }

    /// Subscribe to ephemeral events (typing indicators and/or presence updates).
    ///
    /// Sends a `ChatStreamEnable` frame to the server. A subsequent call
    /// replaces the prior subscription entirely; re-send after every reconnect
    /// because subscriptions are session-scoped (not persisted by the server).
    ///
    /// Spec: draft-atwood-jmap-chat-wss-00
    pub async fn send_stream_enable(
        &mut self,
        enable: &crate::types::ChatStreamEnable,
    ) -> Result<(), crate::error::ClientError> {
        let val = serde_json::to_value(enable)?;
        self.send_json(&val).await
    }

    /// Stop all ephemeral event delivery.
    ///
    /// Sends a `ChatStreamDisable` frame. The server MUST stop delivery
    /// silently even if no subscription is active.
    ///
    /// Spec: draft-atwood-jmap-chat-wss-00
    pub async fn send_stream_disable(&mut self) -> Result<(), crate::error::ClientError> {
        let val = serde_json::to_value(crate::types::ChatStreamDisable::default())?;
        self.send_json(&val).await
    }
}

/// Parse a raw WebSocket text frame into a `WsFrame`.
fn parse_ws_frame(text: &str) -> Result<WsFrame, crate::error::ClientError> {
    let val: serde_json::Value =
        serde_json::from_str(text).map_err(|e| crate::error::ClientError::Parse(e.to_string()))?;

    // Use a sentinel string for the absent case so Unknown carries a meaningful
    // type_name regardless of whether @type was missing or just unrecognized.
    let type_name = val
        .get("@type")
        .and_then(|v| v.as_str())
        .unwrap_or("<no @type>")
        .to_string();

    match type_name.as_str() {
        "StateChange" => Ok(WsFrame::StateChange(val)),
        // A malformed Response frame is skipped (returned as Unknown) rather than
        // treated as a transport error. A single bad server frame must not kill the
        // entire ephemeral-event connection; only actual WebSocket transport errors
        // (tungstenite::Error) warrant a reconnect.
        "Response" => match serde_json::from_value::<crate::jmap::JmapResponse>(val) {
            Ok(r) => Ok(WsFrame::Response(r)),
            Err(_) => Ok(WsFrame::Unknown {
                type_name: "Response".to_string(),
            }),
        },
        "ChatTypingEvent" => serde_json::from_value::<crate::types::ChatTypingEvent>(val)
            .map(WsFrame::ChatTyping)
            .map_err(|e| crate::error::ClientError::Parse(e.to_string())),
        "ChatPresenceEvent" => serde_json::from_value::<crate::types::ChatPresenceEvent>(val)
            .map(WsFrame::ChatPresence)
            .map_err(|e| crate::error::ClientError::Parse(e.to_string())),
        _ => Ok(WsFrame::Unknown { type_name }),
    }
}

impl crate::client::JmapChatClient {
    /// Open a JMAP Chat WebSocket connection.
    ///
    /// `ws_url` must come from `Session::websocket_capability()?.url` (a
    /// `wss://` endpoint in production; `ws://` is accepted in tests).
    /// The server MUST also advertise `urn:ietf:params:jmap:chat:websocket`
    /// in `Session.capabilities` for ephemeral push events to be delivered;
    /// check `Session::supports_chat_websocket()` before calling.
    ///
    /// Returns `ClientError::InvalidArgument` if the URL scheme is not
    /// `ws://` or `wss://`, preventing accidental use with untrusted URLs.
    ///
    /// Authentication headers from the `AuthProvider` are injected into the
    /// WebSocket upgrade request before the handshake.
    ///
    /// The returned [`WsSession`] provides [`WsSession::next_frame`] for
    /// receiving events. The caller is responsible for reconnecting after
    /// disconnect with exponential backoff.
    pub async fn connect_ws(&self, ws_url: &str) -> Result<WsSession, crate::error::ClientError> {
        // Validate scheme to prevent SSRF via a compromised or MITM'd session.
        let lc = ws_url.to_ascii_lowercase();
        if !lc.starts_with("ws://") && !lc.starts_with("wss://") {
            return Err(crate::error::ClientError::InvalidArgument(format!(
                "WebSocket URL must start with ws:// or wss://, got: {ws_url:?}"
            )));
        }

        let mut request = ws_url
            .into_client_request()
            .map_err(crate::error::ClientError::WebSocket)?;

        if let Some((name, value)) = self.auth.auth_header() {
            let hdr_name: reqwest::header::HeaderName = name
                .parse()
                .expect("pre-computed auth header name is always valid");
            let hdr_value: reqwest::header::HeaderValue = value
                .parse()
                .expect("pre-computed auth header value is always valid");
            request.headers_mut().insert(hdr_name, hdr_value);
        }

        // WebSocketConfig is #[non_exhaustive] in tungstenite; use Default + field assignment.
        let mut config = WebSocketConfig::default();
        config.max_message_size = Some(MAX_WS_MESSAGE_BYTES);
        config.max_frame_size = Some(MAX_WS_MESSAGE_BYTES);

        let (ws_stream, _response) =
            tokio_tungstenite::connect_async_with_config(request, Some(config), false)
                .await
                .map_err(crate::error::ClientError::WebSocket)?;

        let (sink, stream) = ws_stream.split();
        Ok(WsSession { sink, stream })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oracle: parse_ws_frame dispatches on @type field.
    #[test]
    fn parse_state_change() {
        let json = r#"{"@type":"StateChange","changed":{"account1":{"Chat":"s2"}}}"#;
        let frame = parse_ws_frame(json).expect("must parse");
        assert!(matches!(frame, WsFrame::StateChange(_)));
    }

    /// Oracle: parse_ws_frame returns Unknown for unrecognized @type.
    #[test]
    fn parse_unknown_type() {
        let json = r#"{"@type":"FutureEvent","foo":"bar"}"#;
        let frame = parse_ws_frame(json).expect("must parse");
        match frame {
            WsFrame::Unknown { type_name } => assert_eq!(type_name, "FutureEvent"),
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    /// Oracle: parse_ws_frame returns Unknown for missing @type.
    #[test]
    fn parse_missing_type_field() {
        let json = r#"{"foo":"bar"}"#;
        let frame = parse_ws_frame(json).expect("must parse");
        assert!(matches!(frame, WsFrame::Unknown { .. }));
    }

    /// Oracle: parse_ws_frame returns Err(Parse) for invalid JSON.
    #[test]
    fn parse_invalid_json_returns_parse_error() {
        let err = parse_ws_frame("not json").expect_err("must fail");
        assert!(matches!(err, crate::error::ClientError::Parse(_)));
    }

    /// Oracle: parse_ws_frame returns ChatTyping for ChatTypingEvent.
    #[test]
    fn parse_chat_typing_event() {
        let json =
            r#"{"@type":"ChatTypingEvent","chatId":"chat-1","senderId":"user-2","typing":true}"#;
        let frame = parse_ws_frame(json).expect("must parse");
        match frame {
            WsFrame::ChatTyping(evt) => {
                assert_eq!(evt.chat_id.as_str(), "chat-1");
                assert_eq!(evt.sender_id.as_str(), "user-2");
                assert!(evt.typing);
            }
            other => panic!("expected ChatTyping, got {other:?}"),
        }
    }

    /// Oracle: connect_ws must reject http:// and https:// URLs with InvalidArgument.
    ///
    /// This is the documented SSRF prevention guard: a compromised or MITM'd session
    /// could send an http:// URL; we must not follow it as a WebSocket URL.
    /// The scheme check runs before any network I/O.
    #[tokio::test]
    async fn connect_ws_rejects_non_ws_schemes() {
        let client = crate::client::JmapChatClient::new(
            crate::auth::DefaultTransport,
            crate::auth::NoneAuth,
            "https://example.com",
        )
        .expect("client construction must succeed");

        for bad_url in &["http://host/", "https://host/", "ftp://host/"] {
            let result = client.connect_ws(bad_url).await.map(|_| ());
            match result {
                Err(crate::error::ClientError::InvalidArgument(_)) => {}
                other => panic!("expected InvalidArgument for {bad_url:?}, got {other:?}"),
            }
        }
    }

    /// Oracle: parse_ws_frame returns ChatPresence for ChatPresenceEvent.
    #[test]
    fn parse_chat_presence_event() {
        let json = r#"{"@type":"ChatPresenceEvent","contactId":"user-3","presence":"away"}"#;
        let frame = parse_ws_frame(json).expect("must parse");
        match frame {
            WsFrame::ChatPresence(evt) => {
                assert_eq!(evt.contact_id.as_str(), "user-3");
                assert_eq!(evt.presence, crate::types::ContactPresence::Away,);
            }
            other => panic!("expected ChatPresence, got {other:?}"),
        }
    }
}
