// JMAP core wire types — RFC 8620 §1.2, §1.4, §2, §3.2, §3.3, §3.4

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// An opaque server-assigned identifier string (RFC 8620 §1.2).
/// Guaranteed non-empty. Serializes/deserializes transparently as a JSON string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct Id(String);

impl<'de> serde::Deserialize<'de> for Id {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s.is_empty() {
            return Err(serde::de::Error::custom("Id may not be empty"));
        }
        Ok(Id(s))
    }
}

impl Id {
    /// Create an Id from a string, returning Err if the string is empty.
    pub fn new(s: impl Into<String>) -> Result<Self, crate::error::ClientError> {
        let s = s.into();
        if s.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "Id may not be empty".into(),
            ));
        }
        Ok(Self(s))
    }

    /// Create an `Id` from a string, bypassing the non-empty validation.
    ///
    /// **Only use for server-assigned identifiers and test fixtures.**
    /// Do not pass user-controlled input — call [`Id::new`] instead.
    /// Panics in debug builds if `s` is empty.
    pub fn from_trusted(s: impl Into<String>) -> Self {
        let s = s.into();
        debug_assert!(!s.is_empty(), "from_trusted called with empty string");
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Id {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for Id {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for Id {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<Id> for &str {
    fn eq(&self, other: &Id) -> bool {
        *self == other.0
    }
}

/// An RFC 3339 UTC timestamp string (JMAP UTCDate, RFC 8620 §1.4).
/// Guaranteed non-empty. Serializes/deserializes transparently as a JSON string.
///
/// Note: `UTCDate::new` validates non-empty but not RFC 3339 format. Call
/// [`UTCDate::parse`] when datetime arithmetic is needed; use [`UTCDate::as_str`]
/// or `Display` for logging or display.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct UTCDate(String);

impl<'de> serde::Deserialize<'de> for UTCDate {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s.is_empty() {
            return Err(serde::de::Error::custom("UTCDate may not be empty"));
        }
        Ok(UTCDate(s))
    }
}

impl UTCDate {
    /// Create a UTCDate from a string, returning Err if the string is empty.
    pub fn new(s: impl Into<String>) -> Result<Self, crate::error::ClientError> {
        let s = s.into();
        if s.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "UTCDate may not be empty".into(),
            ));
        }
        Ok(Self(s))
    }

    /// Create a `UTCDate` from a string, bypassing the non-empty validation.
    ///
    /// **Only use for server-assigned identifiers and test fixtures.**
    /// Do not pass user-controlled input — call [`UTCDate::new`] instead.
    /// Panics in debug builds if `s` is empty.
    pub fn from_trusted(s: impl Into<String>) -> Self {
        let s = s.into();
        debug_assert!(!s.is_empty(), "from_trusted called with empty string");
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parse the stored RFC 3339 string as a [`chrono::DateTime<chrono::Utc>`].
    ///
    /// Returns `ClientError::Parse` if the stored string is not valid RFC 3339.
    pub fn parse(&self) -> Result<chrono::DateTime<chrono::Utc>, crate::error::ClientError> {
        chrono::DateTime::parse_from_rfc3339(&self.0)
            .map(|dt| dt.to_utc())
            .map_err(|e| crate::error::ClientError::Parse(e.to_string()))
    }
}

impl std::fmt::Display for UTCDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<str> for UTCDate {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for UTCDate {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for UTCDate {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for UTCDate {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<UTCDate> for &str {
    fn eq(&self, other: &UTCDate) -> bool {
        *self == other.0
    }
}

/// A single method call or response: `[methodName, arguments, callId]` (RFC 8620 §3.2).
pub type Invocation = (String, serde_json::Value, String);

/// JMAP API request (RFC 8620 §3.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JmapRequest {
    /// Capability URIs the client is using in this request.
    pub using: Vec<String>,
    /// Ordered list of method calls to execute.
    #[serde(rename = "methodCalls")]
    pub method_calls: Vec<Invocation>,
}

/// JMAP API response (RFC 8620 §3.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JmapResponse {
    /// Ordered list of method responses.
    #[serde(rename = "methodResponses")]
    pub method_responses: Vec<Invocation>,
    /// Server session state at the time the response was generated.
    #[serde(rename = "sessionState")]
    pub session_state: String,
    /// Map of client-supplied creation ids to server-assigned ids, if any.
    #[serde(rename = "createdIds", skip_serializing_if = "Option::is_none")]
    pub created_ids: Option<HashMap<String, String>>,
}

// ---------------------------------------------------------------------------
// Session (RFC 8620 §2 + JMAP Chat §3)
// ---------------------------------------------------------------------------

/// JMAP Session object returned by `GET /.well-known/jmap` (RFC 8620 §2).
///
/// JMAP Chat §3 extension fields (`ownerUserId`, `ownerLogin`, `ownerEndpoints`)
/// are included as optional fields.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// Map of capability URI → capability object (RFC 8620 §2).
    pub capabilities: HashMap<String, serde_json::Value>,
    /// Map of accountId → AccountInfo (RFC 8620 §2).
    pub accounts: HashMap<String, AccountInfo>,
    /// Map of capability URI → primary accountId (RFC 8620 §2).
    pub primary_accounts: HashMap<String, String>,
    /// Human-readable username for this session (RFC 8620 §2).
    pub username: String,
    /// URL for JMAP API POST requests (RFC 8620 §2).
    pub api_url: String,
    /// URL template for blob downloads (RFC 8620 §2).
    pub download_url: String,
    /// URL for blob uploads (RFC 8620 §2).
    pub upload_url: String,
    /// URL for the SSE push stream (RFC 8620 §2).
    pub event_source_url: String,
    /// Opaque session state token (RFC 8620 §2).
    pub state: String,

    /// The mailbox owner's ChatContact.id (JMAP Chat §3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_user_id: Option<String>,
    /// Human-readable login name for the owner (JMAP Chat §3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_login: Option<String>,
    /// Owner's out-of-band capability endpoints (JMAP Chat §3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_endpoints: Option<Vec<crate::types::Endpoint>>,
}

impl Session {
    /// Returns the primary accountId for the JMAP Chat capability, if present.
    pub fn chat_account_id(&self) -> Option<&str> {
        self.primary_accounts
            .get("urn:ietf:params:jmap:chat")
            .map(String::as_str)
    }

    /// Returns the parsed `ChatCapability` for the given account.
    ///
    /// - `Ok(None)` — the account exists but has no chat capability key.
    /// - `Ok(Some(...))` — the capability is present and valid.
    /// - `Err(ClientError::Parse(...))` — the key is present but malformed.
    pub fn chat_capability(
        &self,
        account_id: &str,
    ) -> Result<Option<ChatCapability>, crate::error::ClientError> {
        let account = match self.accounts.get(account_id) {
            Some(a) => a,
            None => return Ok(None),
        };
        let raw = match account
            .account_capabilities
            .get("urn:ietf:params:jmap:chat")
        {
            Some(r) => r,
            None => return Ok(None),
        };
        serde_json::from_value::<ChatCapability>(raw.clone())
            .map(Some)
            .map_err(|e| {
                crate::error::ClientError::Parse(format!("malformed chat capability: {e}"))
            })
    }

    /// Returns the parsed `WebSocketCapability` for the JMAP WebSocket transport, if present.
    ///
    /// Reads from `capabilities["urn:ietf:params:jmap:websocket"]` (RFC 8887).
    /// This capability provides the `url` for WebSocket connections.
    ///
    /// - `Ok(None)` — server does not advertise JMAP WebSocket support.
    /// - `Ok(Some(...))` — WebSocket is supported; use `result.url` to connect.
    /// - `Err(ClientError::Parse(...))` — capability present but malformed.
    pub fn websocket_capability(
        &self,
    ) -> Result<Option<WebSocketCapability>, crate::error::ClientError> {
        let raw = match self.capabilities.get("urn:ietf:params:jmap:websocket") {
            Some(r) => r,
            None => return Ok(None),
        };
        serde_json::from_value::<WebSocketCapability>(raw.clone())
            .map(Some)
            .map_err(|e| {
                crate::error::ClientError::Parse(format!("malformed websocket capability: {e}"))
            })
    }

    /// Returns whether the server supports JMAP Chat WebSocket ephemeral events.
    ///
    /// Checks for presence of `capabilities["urn:ietf:params:jmap:chat:websocket"]`.
    /// Use [`Session::websocket_capability`] to get the actual WebSocket URL.
    pub fn supports_chat_websocket(&self) -> bool {
        self.capabilities
            .contains_key("urn:ietf:params:jmap:chat:websocket")
    }

    /// Returns the parsed `ChatPushCapability` for the given account, if present.
    ///
    /// Reads from `accounts[account_id].accountCapabilities["urn:ietf:params:jmap:chat:push"]`.
    ///
    /// - `Ok(None)` — account exists but has no chat push capability.
    /// - `Ok(Some(...))` — chat push is supported for this account.
    /// - `Err(ClientError::Parse(...))` — capability present but malformed.
    pub fn chat_push_capability(
        &self,
        account_id: &str,
    ) -> Result<Option<ChatPushCapability>, crate::error::ClientError> {
        let account = match self.accounts.get(account_id) {
            Some(a) => a,
            None => return Ok(None),
        };
        let raw = match account
            .account_capabilities
            .get("urn:ietf:params:jmap:chat:push")
        {
            Some(r) => r,
            None => return Ok(None),
        };
        serde_json::from_value::<ChatPushCapability>(raw.clone())
            .map(Some)
            .map_err(|e| {
                crate::error::ClientError::Parse(format!("malformed chat push capability: {e}"))
            })
    }

    /// Returns the VAPID public key advertised by the server, if present.
    ///
    /// The VAPID key lives at `capabilities["urn:ietf:params:jmap:webpush-vapid"]["vapidPublicKey"]`.
    /// It is a base64url-encoded P-256 public key to pass to the platform push service
    /// when registering a `PushSubscription` endpoint.
    ///
    /// Returns `None` if the capability is absent or if `vapidPublicKey` is missing/not a string.
    pub fn vapid_public_key(&self) -> Option<&str> {
        self.capabilities
            .get("urn:ietf:params:jmap:webpush-vapid")?
            .get("vapidPublicKey")?
            .as_str()
    }
}

/// Per-account metadata in a JMAP Session (RFC 8620 §2).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    /// Human-readable account name.
    pub name: String,
    /// Whether this is the user's primary/personal account.
    pub is_personal: bool,
    /// Whether this account is read-only.
    pub is_read_only: bool,
    /// Map of capability URI → capability object for this account.
    pub account_capabilities: HashMap<String, serde_json::Value>,
}

/// Chat-capability fields from `accounts[id].accountCapabilities["urn:ietf:params:jmap:chat"]`.
///
/// Spec: draft-atwood-jmap-chat-00 §3
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCapability {
    /// Maximum UTF-8 byte length of a Message body.
    pub max_body_bytes: u64,
    /// Maximum single attachment blob size in bytes.
    pub max_attachment_bytes: u64,
    /// Maximum number of attachments per message.
    pub max_attachments_per_message: u64,
    /// Maximum number of members in a group Chat.
    pub max_group_members: u64,
    /// Maximum number of members in a Space.
    pub max_space_members: u64,
    /// Maximum number of roles per Space.
    pub max_roles_per_space: u64,
    /// Maximum number of channels per Space.
    pub max_channels_per_space: u64,
    /// Maximum number of categories per Space.
    pub max_categories_per_space: u64,
    /// MIME types accepted in `bodyType`; always includes `"text/plain"`.
    pub supported_body_types: Vec<String>,
    /// Whether the server supports the optional thread model.
    pub supports_threads: bool,
}

/// Capability fields from `capabilities["urn:ietf:params:jmap:websocket"]` (RFC 8887).
///
/// The WebSocket URL for JMAP Chat ephemeral push (typing, presence) comes from this
/// standard JMAP WebSocket capability, NOT from the chat-specific WebSocket capability.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSocketCapability {
    /// The WebSocket endpoint URL (`wss://`).
    pub url: String,
    /// Whether the server supports push over this WebSocket connection.
    #[serde(default)]
    pub supports_push: bool,
}

/// Capability object for `"urn:ietf:params:jmap:chat:websocket"`.
///
/// Per draft-atwood-jmap-chat-wss-00, this capability value is an empty JSON object `{}`.
/// Its presence signals support for `ChatStreamEnable`, `ChatStreamDisable`,
/// `ChatTypingEvent`, and `ChatPresenceEvent` over the WebSocket from `WebSocketCapability.url`.
#[derive(Debug, Clone, Deserialize)]
pub struct ChatWebSocketCapability {}

/// Account-level capability for `"urn:ietf:params:jmap:chat:push"`.
///
/// Spec: draft-atwood-jmap-chat-push-00
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPushCapability {
    /// Maximum byte length of a `bodySnippet` in `ChatMessagePush`. Truncation on UTF-8 boundary.
    pub max_snippet_bytes: u64,
    /// Supported Web Push urgency values. MUST include at least `"normal"` and `"high"`.
    pub supported_urgency_values: Vec<String>,
    /// Maximum number of `ChatMessageEntry` objects per push payload. Absent = no bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_messages_per_push: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(name: &str) -> serde_json::Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/jmap")
            .join(name);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read fixture {name}: {e}"));
        serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("fixture {name} is not valid JSON: {e}"))
    }

    // Oracle: RFC 8620 §3.3 — hand-written fixture derived from spec structure
    #[test]
    fn deserialize_request_from_fixture() {
        let val = fixture("request_chat_get.json");
        let req: JmapRequest = serde_json::from_value(val).expect("deserialize JmapRequest");

        assert_eq!(req.using[0], "urn:ietf:params:jmap:core");
        assert_eq!(req.method_calls[0].0, "Chat/get");
        assert_eq!(req.method_calls[0].2, "r1");
    }

    // Oracle: RFC 8620 §3.3 — serialize matches hand-written fixture exactly
    #[test]
    fn serialize_request_matches_fixture() {
        let req = JmapRequest {
            using: vec![
                "urn:ietf:params:jmap:core".to_string(),
                "urn:ietf:params:jmap:chat".to_string(),
            ],
            method_calls: vec![
                (
                    "Chat/get".to_string(),
                    json!({"accountId": "account1", "ids": null}),
                    "r1".to_string(),
                ),
                (
                    "Message/get".to_string(),
                    json!({"accountId": "account1", "ids": ["msg1", "msg2"]}),
                    "r2".to_string(),
                ),
            ],
        };

        let serialized = serde_json::to_value(&req).expect("serialize JmapRequest");
        let expected = fixture("request_chat_get.json");
        assert_eq!(serialized, expected);
    }

    // Oracle: RFC 8620 §3.4 — hand-written fixture derived from spec structure
    #[test]
    fn deserialize_response_from_fixture() {
        let val = fixture("response_chat_get.json");
        let resp: JmapResponse = serde_json::from_value(val).expect("deserialize JmapResponse");

        assert_eq!(resp.session_state, "session-xyz789");
        assert_eq!(resp.method_responses[0].0, "Chat/get");
        assert!(resp.created_ids.is_none());
    }

    // Oracle: RFC 8620 §3.2 — Invocation is a 3-element JSON array
    #[test]
    fn invocation_serializes_as_array() {
        let inv: Invocation = ("Foo/get".to_string(), json!({}), "c1".to_string());
        let val = serde_json::to_value(&inv).expect("serialize Invocation");
        assert_eq!(val, json!(["Foo/get", {}, "c1"]));
    }

    // Oracle: RFC 8620 §3.4 — createdIds MUST be absent when not present
    #[test]
    fn response_created_ids_absent_when_none() {
        let resp = JmapResponse {
            method_responses: vec![],
            session_state: "s1".to_string(),
            created_ids: None,
        };
        let val = serde_json::to_value(&resp).expect("serialize JmapResponse");
        assert!(!val.as_object().unwrap().contains_key("createdIds"));
    }

    // Oracle: RFC 8620 §3.4 — createdIds MUST be present when populated
    #[test]
    fn response_created_ids_present_when_some() {
        let mut ids = HashMap::new();
        ids.insert("client-id-1".to_string(), "server-id-abc".to_string());
        let resp = JmapResponse {
            method_responses: vec![],
            session_state: "s1".to_string(),
            created_ids: Some(ids),
        };
        let val = serde_json::to_value(&resp).expect("serialize JmapResponse");
        let obj = val.as_object().unwrap();
        assert!(obj.contains_key("createdIds"));
        assert_eq!(obj["createdIds"]["client-id-1"], "server-id-abc");
    }

    // Oracle: RFC 8620 §2 — hand-written fixture matches spec Session structure
    #[test]
    fn session_deserializes_from_fixture() {
        let val = fixture("session.json");
        let session: Session =
            serde_json::from_value(val).expect("session.json must deserialize as Session");

        assert_eq!(session.username, "alice@example.com");
        assert_eq!(session.api_url, "https://jmap.example.com/api");
        assert_eq!(
            session.event_source_url,
            "https://jmap.example.com/eventsource/"
        );
        assert_eq!(session.state, "session-abc123");
        assert!(session.accounts.contains_key("account1"));
        assert!(session
            .capabilities
            .contains_key("urn:ietf:params:jmap:core"));
        assert!(session
            .capabilities
            .contains_key("urn:ietf:params:jmap:chat"));
        // JMAP Chat extension fields are absent in this fixture
        assert!(session.owner_user_id.is_none());
        assert!(session.owner_login.is_none());
        assert!(session.owner_endpoints.is_none());
    }

    // Oracle: RFC 8620 §2 — chat_account_id() extracts the primary account
    // from the fixture's primaryAccounts["urn:ietf:params:jmap:chat"] field.
    #[test]
    fn session_chat_account_id_returns_primary_account() {
        let val = fixture("session.json");
        let session: Session = serde_json::from_value(val).expect("session.json must deserialize");

        assert_eq!(session.chat_account_id(), Some("account1"));
    }

    // Oracle: draft-atwood-jmap-chat-00 §3 — chat_capability() parses the
    // account-level chat capability fields from the fixture.
    #[test]
    fn session_chat_capability_parses_account_capability() {
        let val = fixture("session.json");
        let session: Session = serde_json::from_value(val).expect("session.json must deserialize");

        let cap = session
            .chat_capability("account1")
            .expect("chat_capability must not return Err")
            .expect("account1 must have chat capability");

        assert_eq!(cap.max_body_bytes, 65536);
        assert_eq!(cap.max_attachment_bytes, 10485760);
        assert_eq!(cap.max_attachments_per_message, 10);
        assert_eq!(cap.max_group_members, 100);
        assert_eq!(cap.max_space_members, 500);
        assert_eq!(cap.max_roles_per_space, 50);
        assert_eq!(cap.max_channels_per_space, 200);
        assert_eq!(cap.max_categories_per_space, 25);
        assert_eq!(
            cap.supported_body_types,
            vec!["text/plain", "text/markdown"]
        );
        assert!(cap.supports_threads);
    }

    // Oracle: draft-atwood-jmap-chat-00 §3 — chat_capability() returns Ok(None)
    // when the account exists but lacks the chat capability key.
    #[test]
    fn session_chat_capability_absent_key_returns_ok_none() {
        let val = fixture("session.json");
        let mut session: Session =
            serde_json::from_value(val).expect("session.json must deserialize");

        session
            .accounts
            .get_mut("account1")
            .unwrap()
            .account_capabilities
            .remove("urn:ietf:params:jmap:chat");

        let result = session.chat_capability("account1");
        assert!(
            matches!(result, Ok(None)),
            "expected Ok(None), got {result:?}"
        );
    }

    // Oracle: session_malformed_chat_cap.json — hand-written fixture with
    // maxBodyBytes set to a string instead of a u64, derived from the spec
    // field type (draft-atwood-jmap-chat-00 §3); NOT produced by the code
    // under test.
    #[test]
    fn session_chat_capability_malformed_returns_err() {
        let val = fixture("session_malformed_chat_cap.json");
        let session: Session =
            serde_json::from_value(val).expect("fixture must deserialize as Session");

        let result = session.chat_capability("account1");
        match result {
            Err(crate::error::ClientError::Parse(msg)) => {
                assert!(
                    msg.contains("malformed chat capability"),
                    "error message should mention 'malformed chat capability', got: {msg}"
                );
            }
            other => panic!("expected Err(ClientError::Parse(...)), got {other:?}"),
        }
    }

    // Oracle: RFC 8620 §2 — chat_account_id() returns None when the capability
    // URI is absent from primaryAccounts.
    #[test]
    fn session_chat_account_id_absent_returns_none() {
        let val = fixture("session.json");
        let mut session: Session =
            serde_json::from_value(val).expect("session.json must deserialize");

        session.primary_accounts.remove("urn:ietf:params:jmap:chat");
        assert!(session.chat_account_id().is_none());
    }

    // Oracle: RFC 8887 + WSS spec — websocket_capability() parses url and supportsPush.
    #[test]
    fn session_websocket_capability_parses_correctly() {
        let val = fixture("session_with_ws_and_push.json");
        let session: Session = serde_json::from_value(val).expect("must deserialize");

        let ws = session
            .websocket_capability()
            .expect("must not error")
            .expect("websocket capability must be present");

        assert_eq!(ws.url, "wss://jmap.example.com/ws");
        assert!(ws.supports_push);
    }

    // Oracle: WSS spec — supports_chat_websocket() true when capability key present.
    #[test]
    fn session_supports_chat_websocket_when_capability_present() {
        let val = fixture("session_with_ws_and_push.json");
        let session: Session = serde_json::from_value(val).expect("must deserialize");
        assert!(session.supports_chat_websocket());
    }

    // Oracle: WSS spec — supports_chat_websocket() false when capability key absent.
    #[test]
    fn session_supports_chat_websocket_false_when_absent() {
        let val = fixture("session.json");
        let session: Session = serde_json::from_value(val).expect("must deserialize");
        assert!(!session.supports_chat_websocket());
    }

    // Oracle: push spec — chat_push_capability() parses maxSnippetBytes, urgency values, maxMessagesPerPush.
    #[test]
    fn session_chat_push_capability_parses_correctly() {
        let val = fixture("session_with_ws_and_push.json");
        let session: Session = serde_json::from_value(val).expect("must deserialize");

        let push = session
            .chat_push_capability("account1")
            .expect("must not error")
            .expect("push capability must be present");

        assert_eq!(push.max_snippet_bytes, 256);
        assert_eq!(push.supported_urgency_values, vec!["normal", "high"]);
        assert_eq!(push.max_messages_per_push, Some(10));
    }

    // Oracle: VAPID spec — vapid_public_key() returns the key string.
    #[test]
    fn session_vapid_public_key_returns_key() {
        let val = fixture("session_with_ws_and_push.json");
        let session: Session = serde_json::from_value(val).expect("must deserialize");
        let key = session
            .vapid_public_key()
            .expect("vapid key must be present");
        assert_eq!(
            key,
            "BNNOfS9lCWcSqcNFxf8GaDJb0JnrIq4z7VDchBNJYEFXP3kUEzixdOMU6VFZX2pGmREFzQ=="
        );
    }

    // Oracle: vapid_public_key() returns None when capability absent.
    #[test]
    fn session_vapid_public_key_absent_returns_none() {
        let val = fixture("session.json");
        let session: Session = serde_json::from_value(val).expect("must deserialize");
        assert!(session.vapid_public_key().is_none());
    }

    // Oracle: websocket_capability() returns Ok(None) when key absent.
    #[test]
    fn session_websocket_capability_absent_returns_ok_none() {
        let val = fixture("session.json");
        let session: Session = serde_json::from_value(val).expect("must deserialize");
        let result = session.websocket_capability();
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn utc_date_parse_valid() {
        // Oracle: RFC 3339 string "2024-01-02T12:00:00Z" → year=2024, month=1, day=2
        let d = UTCDate::from_trusted("2024-01-02T12:00:00Z");
        let dt = d.parse().expect("valid RFC 3339 must parse");
        use chrono::Datelike;
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 2);
    }

    #[test]
    fn utc_date_parse_invalid() {
        let d = UTCDate::from_trusted("not-a-date");
        assert!(d.parse().is_err());
    }
}
