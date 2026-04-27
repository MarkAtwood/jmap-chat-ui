// Integration tests for typed JMAP Chat method wrappers (Step 8).
//
// Each test mounts a wiremock POST handler returning a hand-written fixture,
// calls the typed method, and asserts key fields of the typed response.
//
// Fixtures live in tests/fixtures/methods/ and are hand-written from the
// RFC 8620 §5 response shapes — they are NOT derived from the code under test.

use jmap_chat::client::JmapChatClient;
use jmap_chat::error::ClientError;
use jmap_chat::methods::{
    ChatQueryInput, GetResponse, MessageCreateInput, MessageQueryInput, PresenceStatusSetInput,
    SpaceBanCreateInput, SpaceInviteCreateInput,
};
use jmap_chat::types::OwnerPresence;
use wiremock::matchers::{body_json, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_session(api_url: &str) -> jmap_chat::jmap::Session {
    // "account1" is the chat primary account id used across all test fixtures.
    // accounts map is intentionally empty — tests only need chat_account_id(),
    // not the full AccountInfo. chat_capability() returns Ok(None) on this session.
    serde_json::from_value(serde_json::json!({
        "capabilities": {},
        "accounts": {},
        "primaryAccounts": {
            "urn:ietf:params:jmap:chat": "account1"
        },
        "username": "test",
        "apiUrl": api_url,
        "downloadUrl": "",
        "uploadUrl": "",
        "eventSourceUrl": "",
        "state": ""
    }))
    .unwrap()
}

fn fixture(name: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/methods")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {name}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {name} is not valid JSON: {e}"))
}

// ---------------------------------------------------------------------------
// Test 1: chat_get — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.1 — Chat/get response shape: accountId, state, list, notFound.
/// Fixture hand-written from §5.1 /get response definition.
#[tokio::test]
async fn chat_get_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("chat_get_response.json")))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_get(&test_session(&api_url), None, None)
        .await
        .expect("chat_get must succeed");

    // Oracle: RFC 8620 §5.1 — response has accountId and state
    assert_eq!(result.account_id, "account1");
    assert_eq!(result.state, "state-chat-001");
    // One chat returned in the list
    assert_eq!(result.list.len(), 1);
    assert_eq!(result.list[0].id, "01HV5Z6QKWJ7N3P8R2X4YTMD3G");
    assert_eq!(result.list[0].unread_count, 3);
    // notFound is present but empty
    assert_eq!(result.not_found.as_deref(), Some([].as_slice()));
}

// ---------------------------------------------------------------------------
// Test 2: message_create — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 — Message/set response shape: newState, created map
/// keyed by client_id. Fixture hand-written from §5.3 /set response definition.
#[tokio::test]
async fn message_create_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("message_create_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let sent_at = jmap_chat::jmap::UTCDate::from_trusted("2024-01-02T12:00:00Z");
    let result = client
        .message_create(
            &test_session(&api_url),
            &MessageCreateInput {
                client_id: "client-ulid-001",
                chat_id: "01HV5Z6QKWJ7N3P8R2X4YTMD3G",
                body: "Hello, world!",
                body_type: "text/plain",
                sent_at: &sent_at,
                reply_to: None,
            },
        )
        .await
        .expect("message_create must succeed");

    // Oracle: RFC 8620 §5.3 — newState is present
    assert_eq!(result.new_state, "state-msg-001");
    assert_eq!(result.old_state.as_deref(), Some("state-msg-000"));

    // Oracle: RFC 8620 §5.3 — created map is keyed by client_id
    let created = result.created.expect("created map must be present");
    assert!(
        created.contains_key("client-ulid-001"),
        "created map must contain the client_id key"
    );
    let server_obj = &created["client-ulid-001"];
    assert_eq!(
        server_obj["id"].as_str(),
        Some("01HV5Z6QKWJ7N3P8R2X4YTMD42")
    );
}

// ---------------------------------------------------------------------------
// Test 3: read_position_set — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 — ReadPosition/set response shape: newState, updated map.
/// Fixture hand-written from §5.3 /set response definition.
#[tokio::test]
async fn read_position_set_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("read_position_set_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .read_position_set(
            &test_session(&api_url),
            "01HV5Z6QKWJ7N3P8R2X4RPOS01",
            "01HV5Z6QKWJ7N3P8R2X4YTMD42",
        )
        .await
        .expect("read_position_set must succeed");

    // Oracle: RFC 8620 §5.3 — newState is present
    assert_eq!(result.new_state, "state-rp-001");

    // Oracle: RFC 8620 §5.3 — updated map contains the read_position_id
    let updated = result.updated.expect("updated map must be present");
    assert!(
        updated.contains_key("01HV5Z6QKWJ7N3P8R2X4RPOS01"),
        "updated map must contain the read_position_id"
    );
}

// ---------------------------------------------------------------------------
// Test 4: extract_response with method error
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §3.6.1 — when the server returns an "error" invocation,
/// the method wrapper must surface ClientError::MethodError with the type and
/// description from the error arguments.
///
/// This exercises extract_response via chat_get so that the full method
/// dispatch path is covered.
#[tokio::test]
async fn chat_get_method_error_returns_client_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("method_error_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let err = client
        .chat_get(&test_session(&api_url), None, None)
        .await
        .expect_err("error invocation must return Err");

    // Oracle: RFC 8620 §3.6.1 — error type and description are passed through
    assert!(
        matches!(
            &err,
            ClientError::MethodError { error_type, description }
                if error_type == "unknownMethod"
                && description == "The server does not support Chat/get"
        ),
        "expected MethodError{{unknownMethod, ...}}, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 5: message_query — invalid filter guard
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat spec — servers MUST return `unsupportedFilter` when
/// neither `chatId` is provided nor `hasMention: true`. The client must
/// pre-validate and return `ClientError::InvalidArgument` for:
///   (a) both chat_id and has_mention are None
///   (b) chat_id is None and has_mention=Some(false) — false is not an anchor
///
/// No mock server is needed: the guard fires before any network call.
#[tokio::test]
async fn message_query_rejects_invalid_filter() {
    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, "http://127.0.0.1:1")
        .expect("client construction must succeed");

    // (a) both None
    let err_none = client
        .message_query(
            &test_session("http://127.0.0.1:1/api"),
            &MessageQueryInput::default(),
        )
        .await
        .expect_err("no filter must be rejected");
    assert!(
        matches!(&err_none, ClientError::InvalidArgument(msg) if msg.contains("chat_id or has_mention=true")),
        "expected InvalidArgument, got {err_none:?}"
    );

    // (b) has_mention=Some(false) — not a valid anchor
    let err_false = client
        .message_query(
            &test_session("http://127.0.0.1:1/api"),
            &MessageQueryInput {
                has_mention: Some(false),
                ..Default::default()
            },
        )
        .await
        .expect_err("has_mention=false without chat_id must be rejected");
    assert!(
        matches!(&err_false, ClientError::InvalidArgument(msg) if msg.contains("chat_id or has_mention=true")),
        "expected InvalidArgument, got {err_false:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 6: message_get — empty ids guard
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat spec — fetching all messages is impractical; the client
/// must pre-validate that `ids` is non-empty and return
/// `ClientError::InvalidArgument` before making any network call.
///
/// No mock server is needed: the guard fires before any network call.
#[tokio::test]
async fn message_get_rejects_empty_ids() {
    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, "http://127.0.0.1:1")
        .expect("client construction must succeed");

    let err = client
        .message_get(&test_session("http://127.0.0.1:1/api"), &[], None)
        .await
        .expect_err("empty ids must be rejected");

    assert!(
        matches!(&err, ClientError::InvalidArgument(msg) if msg.contains("ids")),
        "expected InvalidArgument mentioning 'ids', got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 7: GetResponse<T> type alias works for empty list
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.1 — a /get response with an empty list and no notFound
/// deserializes cleanly. Verifies the generic GetResponse<T> works for a type
/// we do not fully construct (PresenceStatus).
///
/// We reuse the message_create fixture's JmapResponse shape to build a minimal
/// Chat/get response inline without a fixture file.
#[test]
fn get_response_empty_list_deserializes() {
    // Oracle: RFC 8620 §5.1 — hand-written minimal /get response
    let raw = serde_json::json!({
        "accountId": "acct1",
        "state": "s0",
        "list": [],
        "notFound": null
    });

    let result: GetResponse<serde_json::Value> =
        serde_json::from_value(raw).expect("GetResponse<Value> must deserialize");

    assert_eq!(result.account_id, "acct1");
    assert_eq!(result.state, "s0");
    assert!(result.list.is_empty());
    assert!(result.not_found.is_none());
}

// ---------------------------------------------------------------------------
// Test 8: chat_query — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.5 — Chat/query response shape: queryState, ids, position.
/// Fixture hand-written from §5.5 /query response definition.
///
/// Body matcher: verifies accountId, filter null, and limit sent as integer (not null).
#[tokio::test]
async fn chat_query_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Chat/query", {
                "accountId": "account1",
                "filter": null,
                "limit": 50
            }, "r1"]]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("chat_query_response.json")))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_query(
            &test_session(&api_url),
            &ChatQueryInput {
                limit: Some(50),
                ..Default::default()
            },
        )
        .await
        .expect("chat_query must succeed");

    // Oracle: RFC 8620 §5.5 — ids is non-empty, queryState is present
    assert!(
        !result.ids.is_empty(),
        "ids must contain at least one entry"
    );
    assert!(
        !result.query_state.is_empty(),
        "query_state must not be empty"
    );
}

// ---------------------------------------------------------------------------
// Test 9: chat_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.2 — Chat/changes response shape: oldState, newState, hasMoreChanges.
/// Fixture hand-written from §5.2 /changes response definition.
#[tokio::test]
async fn chat_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("chat_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_changes(&test_session(&api_url), "state-chat-000", None)
        .await
        .expect("chat_changes must succeed");

    // Oracle: RFC 8620 §5.2 — oldState echoes the sinceState argument
    assert_eq!(result.old_state, "state-chat-000");
    assert!(!result.has_more_changes);
}

// ---------------------------------------------------------------------------
// Test 10: message_get — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.1 — Message/get response shape: list with one Message.
/// Fixture hand-written from §5.1 /get response definition.
#[tokio::test]
async fn message_get_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("message_get_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .message_get(
            &test_session(&api_url),
            &["01HV5Z6QKWJ7N3P8R2X4YTMD42"],
            None,
        )
        .await
        .expect("message_get must succeed");

    // Oracle: RFC 8620 §5.1 — list has exactly one entry with the requested id
    assert_eq!(result.list.len(), 1);
    assert_eq!(result.list[0].id, "01HV5Z6QKWJ7N3P8R2X4YTMD42");
}

// ---------------------------------------------------------------------------
// Test 11: message_query — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.5 — Message/query response shape: queryState, ids, position.
/// Fixture hand-written from §5.5 /query response definition.
///
/// Body matcher: verifies chatId filter, sort direction (isAscending: false),
/// and that position/limit are absent (not null) when not provided.
#[tokio::test]
async fn message_query_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Message/query", {
                "accountId": "account1",
                "filter": {"chatId": "01HV5Z6QKWJ7N3P8R2X4YTMD3G"},
                "sort": [{"property": "sentAt", "isAscending": false}]
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("message_query_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .message_query(
            &test_session(&api_url),
            &MessageQueryInput {
                chat_id: Some("01HV5Z6QKWJ7N3P8R2X4YTMD3G"),
                ..Default::default()
            },
        )
        .await
        .expect("message_query must succeed");

    // Oracle: RFC 8620 §5.5 — ids is non-empty, queryState is present
    assert!(!result.ids.is_empty(), "ids must have length > 0");
    assert!(
        !result.query_state.is_empty(),
        "query_state must not be empty"
    );
}

// ---------------------------------------------------------------------------
// Test 12: message_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.2 — Message/changes response shape: oldState, hasMoreChanges.
/// Fixture hand-written from §5.2 /changes response definition.
#[tokio::test]
async fn message_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("message_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .message_changes(&test_session(&api_url), "state-msg-000", None)
        .await
        .expect("message_changes must succeed");

    // Oracle: RFC 8620 §5.2 — oldState echoes the sinceState argument
    assert_eq!(result.old_state, "state-msg-000");
    assert!(!result.has_more_changes);
}

// ---------------------------------------------------------------------------
// Test 13: chat_contact_get — happy path
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §5 — ChatContact/get response shape: list with one ChatContact.
/// Fixture hand-written from §5.1 /get response definition.
///
/// Body matcher: verifies accountId and that ids/properties are null when not provided.
#[tokio::test]
async fn chat_contact_get_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["ChatContact/get", {
                "accountId": "account1",
                "ids": null,
                "properties": null
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("chat_contact_get_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_contact_get(&test_session(&api_url), None, None)
        .await
        .expect("chat_contact_get must succeed");

    // Oracle: JMAP Chat §5 — list has at least one entry
    assert!(
        result.list.len() >= 1,
        "list must have at least one contact"
    );
}

// ---------------------------------------------------------------------------
// Test 14: read_position_get — happy path
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §5 — ReadPosition/get response shape: list with one ReadPosition.
/// Fixture hand-written from §5.1 /get response definition.
///
/// Body matcher: verifies accountId and ids:null (the spec-correct way to
/// fetch all ReadPositions).
#[tokio::test]
async fn read_position_get_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["ReadPosition/get", {
                "accountId": "account1",
                "ids": null
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("read_position_get_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .read_position_get(&test_session(&api_url), None)
        .await
        .expect("read_position_get must succeed");

    // Oracle: JMAP Chat §5 — list has at least one entry with a non-empty chatId
    assert!(result.list.len() >= 1, "list must have at least one entry");
    assert!(
        !result.list[0].chat_id.as_str().is_empty(),
        "chat_id must not be empty"
    );
}

// ---------------------------------------------------------------------------
// Test 15: presence_status_get — happy path
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §5 — PresenceStatus/get response shape: list with one PresenceStatus.
/// Fixture hand-written from §5.1 /get response definition.
#[tokio::test]
async fn presence_status_get_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("presence_status_get_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .presence_status_get(&test_session(&api_url))
        .await
        .expect("presence_status_get must succeed");

    // Oracle: JMAP Chat §5 — list has at least one entry (singleton per account)
    assert!(result.list.len() >= 1, "list must have at least one entry");
}

// ---------------------------------------------------------------------------
// Test 16: read_position_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.2 — ReadPosition/changes response shape: oldState, newState,
/// hasMoreChanges, updated list. Fixture hand-written from §5.2 /changes response definition.
///
/// Body matcher: verifies sinceState and that maxChanges:null is sent (null is
/// a valid UnsignedInt|null per RFC 8620 §5.2 — server treats it as no limit).
#[tokio::test]
async fn read_position_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["ReadPosition/changes", {
                "accountId": "account1",
                "sinceState": "rp-state-001",
                "maxChanges": null
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture("read_position_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .read_position_changes(&test_session(&api_url), "rp-state-001", None)
        .await
        .expect("read_position_changes must succeed");

    // Oracle: RFC 8620 §5.2 — oldState echoes the sinceState argument
    assert_eq!(result.old_state, "rp-state-001");
    assert!(!result.has_more_changes);
}

// ---------------------------------------------------------------------------
// Test 17: presence_status_set — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 — PresenceStatus/set response shape: newState, updated map.
/// Fixture hand-written from §5.3 /set response definition.
#[tokio::test]
async fn presence_status_set_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("presence_status_set_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .presence_status_set(
            &test_session(&api_url),
            &PresenceStatusSetInput {
                id: "01HV5Z6QKWJ7N3P8R2X4YTMD99",
                presence: Some(OwnerPresence::Away),
                status_text: None,
                status_emoji: None,
                expires_at: None,
                receipt_sharing: None,
            },
        )
        .await
        .expect("presence_status_set must succeed");

    // Oracle: RFC 8620 §5.3 — newState is present
    assert_eq!(result.new_state, "ps-state-002");
}

// ---------------------------------------------------------------------------
// Test 18: presence_status_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.2 — PresenceStatus/changes response shape: oldState, newState,
/// hasMoreChanges, updated list. Fixture hand-written from §5.2 /changes response definition.
#[tokio::test]
async fn presence_status_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture("presence_status_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .presence_status_changes(&test_session(&api_url), "ps-state-001", None)
        .await
        .expect("presence_status_changes must succeed");

    // Oracle: RFC 8620 §5.2 — oldState echoes the sinceState argument
    assert_eq!(result.old_state, "ps-state-001");
    assert_eq!(result.updated.len(), 1);
}

// ---------------------------------------------------------------------------
// Test 19: custom_emoji_get — happy path
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §4.16 — CustomEmoji/get response shape: list with one CustomEmoji.
/// Fixture hand-written from §5.1 /get response definition.
#[tokio::test]
async fn custom_emoji_get_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("custom_emoji_get_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .custom_emoji_get(&test_session(&api_url), None, None)
        .await
        .expect("custom_emoji_get must succeed");

    // Oracle: §5.1 — list has exactly one entry with the expected name
    assert_eq!(result.list.len(), 1);
    assert_eq!(result.list[0].name, "catjam");
}

// ---------------------------------------------------------------------------
// Test 20: custom_emoji_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.2 — CustomEmoji/changes response shape: oldState, hasMoreChanges.
/// Fixture hand-written from §5.2 /changes response definition.
#[tokio::test]
async fn custom_emoji_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("custom_emoji_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .custom_emoji_changes(&test_session(&api_url), "emoji-state-000", None)
        .await
        .expect("custom_emoji_changes must succeed");

    // Oracle: RFC 8620 §5.2 — oldState echoes the sinceState argument
    assert_eq!(result.old_state, "emoji-state-000");
    assert_eq!(result.new_state, "emoji-state-001");
    assert!(!result.has_more_changes);
}

// ---------------------------------------------------------------------------
// Test 21: custom_emoji_set — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 — CustomEmoji/set response shape: newState, created map.
/// Fixture hand-written from §5.3 /set response definition.
#[tokio::test]
async fn custom_emoji_set_returns_typed_response() {
    use jmap_chat::methods::CustomEmojiCreateInput;

    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("custom_emoji_set_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let input = CustomEmojiCreateInput {
        client_id: "emoji-new-1",
        name: "partyparrot",
        blob_id: "01HV5Z6QKWJ7N3P8R2X4YTMDDD",
        space_id: None,
    };
    let result = client
        .custom_emoji_set(&test_session(&api_url), &input, &[])
        .await
        .expect("custom_emoji_set must succeed");

    // Oracle: RFC 8620 §5.3 — created map is keyed by client_id
    assert!(result.created.is_some(), "created map must be present");
}

// ---------------------------------------------------------------------------
// Test 22: custom_emoji_query — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.5 — CustomEmoji/query response shape: queryState, ids.
/// Fixture hand-written from §5.5 /query response definition.
///
/// Body matcher: verifies that `filter` is absent (not null) when no
/// filter_space_id is provided — RFC 8620 §5.5 does not require an explicit
/// null filter and some servers may reject unknown null fields.
#[tokio::test]
async fn custom_emoji_query_returns_typed_response() {
    use jmap_chat::methods::CustomEmojiQueryInput;

    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["CustomEmoji/query", {
                "accountId": "account1"
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("custom_emoji_query_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .custom_emoji_query(&test_session(&api_url), &CustomEmojiQueryInput::default())
        .await
        .expect("custom_emoji_query must succeed");

    // Oracle: RFC 8620 §5.5 — ids is non-empty, queryState is present
    assert!(
        !result.ids.is_empty(),
        "ids must contain at least one entry"
    );
    assert!(
        !result.query_state.is_empty(),
        "query_state must not be empty"
    );
}

// ---------------------------------------------------------------------------
// Test 23: custom_emoji_query_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.6 — CustomEmoji/queryChanges response shape: added items.
/// Fixture hand-written from §5.6 /queryChanges response definition.
#[tokio::test]
async fn custom_emoji_query_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture("custom_emoji_query_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .custom_emoji_query_changes(&test_session(&api_url), "emoji-qs-000", None)
        .await
        .expect("custom_emoji_query_changes must succeed");

    // Oracle: RFC 8620 §5.6 — added contains the newly visible emoji
    assert_eq!(result.added.len(), 1);
    assert_eq!(result.added[0].id, "01HV5Z6QKWJ7N3P8R2X4YTMDAA");
    assert_eq!(result.added[0].index, 0);
}

// ---------------------------------------------------------------------------
// Test 24: space_ban_get — happy path
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §4.18 — SpaceBan/get response shape: list with one SpaceBan.
/// Fixture hand-written from §5.1 /get response definition.
#[tokio::test]
async fn space_ban_get_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("space_ban_get_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_ban_get(&test_session(&api_url), None, None)
        .await
        .expect("space_ban_get must succeed");

    // Oracle: §5.1 — list has exactly one entry with the expected spaceId
    assert_eq!(result.list.len(), 1);
    assert_eq!(
        result.list[0].space_id.as_str(),
        "01HV5Z6QKWJ7N3P8R2X4YTMD10"
    );
}

// ---------------------------------------------------------------------------
// Test 25: space_ban_set — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 — SpaceBan/set response shape: newState, created map.
/// Fixture hand-written from §5.3 /set response definition.
#[tokio::test]
async fn space_ban_set_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("space_ban_set_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let input = SpaceBanCreateInput {
        client_id: "ban-new-1",
        space_id: "01HV5Z6QKWJ7N3P8R2X4YTMD10",
        user_id: "01HV5Z6QKWJ7N3P8R2X4YTMD03",
        reason: None,
        expires_at: None,
    };
    let result = client
        .space_ban_set(&test_session(&api_url), &input, &[])
        .await
        .expect("space_ban_set must succeed");

    // Oracle: RFC 8620 §5.3 — created map is present
    assert!(result.created.is_some(), "created map must be present");
}

// ---------------------------------------------------------------------------
// Test 29: space_ban_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.2 — SpaceBan/changes response shape: oldState, created list.
/// Fixture hand-written from §5.2 /changes response definition.
/// JMAP Chat §4.18 — SpaceBan/changes is a standard /changes method.
#[tokio::test]
async fn space_ban_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("space_ban_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_ban_changes(&test_session(&api_url), "ban-state-000", None)
        .await
        .expect("space_ban_changes must succeed");

    // Oracle: RFC 8620 §5.2 — oldState echoes the sinceState argument, created has one entry
    assert_eq!(result.old_state, "ban-state-000");
    assert_eq!(result.created.len(), 1);
    assert_eq!(result.created[0], "01HV5Z6QKWJ7N3P8R2X4YTMDBB");
}

// ---------------------------------------------------------------------------
// Test 26: space_invite_get — happy path
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §4.17 — SpaceInvite/get response shape: list with one SpaceInvite.
/// Fixture hand-written from §5.1 /get response definition.
#[tokio::test]
async fn space_invite_get_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("space_invite_get_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_invite_get(&test_session(&api_url), None, None)
        .await
        .expect("space_invite_get must succeed");

    // Oracle: §5.1 — list has exactly one entry with the expected invite code
    assert_eq!(result.list.len(), 1);
    assert_eq!(result.list[0].code, "ABC123XYZ");
}

// ---------------------------------------------------------------------------
// Test 27: space_invite_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.2 — SpaceInvite/changes response shape: oldState, created list.
/// Fixture hand-written from §5.2 /changes response definition.
#[tokio::test]
async fn space_invite_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("space_invite_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_invite_changes(&test_session(&api_url), "invite-state-000", None)
        .await
        .expect("space_invite_changes must succeed");

    // Oracle: RFC 8620 §5.2 — oldState echoes the sinceState argument, created has one entry
    assert_eq!(result.old_state, "invite-state-000");
    assert_eq!(result.created.len(), 1);
}

// ---------------------------------------------------------------------------
// Test 28: space_invite_set — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 — SpaceInvite/set response shape: newState, created map.
/// Fixture hand-written from §5.3 /set response definition.
#[tokio::test]
async fn space_invite_set_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("space_invite_set_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let input = SpaceInviteCreateInput {
        client_id: "invite-new-1",
        space_id: "01HV5Z6QKWJ7N3P8R2X4YTMD10",
        default_channel_id: None,
        expires_at: None,
        max_uses: Some(10),
    };
    let result = client
        .space_invite_set(&test_session(&api_url), Some(&input), &[])
        .await
        .expect("space_invite_set must succeed");

    // Oracle: RFC 8620 §5.3 — created map is present
    assert!(result.created.is_some(), "created map must be present");
}
