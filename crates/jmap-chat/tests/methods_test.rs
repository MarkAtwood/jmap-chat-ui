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
    AddMemberInput, ChatContactQueryInput, ChatContactSetInput, ChatCreateChannelInput,
    ChatCreateDirectInput, ChatCreateGroupInput, ChatQueryInput, ChatUpdateInput, GetResponse,
    MessageCreateInput, MessageQueryInput, MessageUpdateInput, PresenceStatusSetInput,
    PushSubscriptionCreateInput, ReactionChange, SpaceBanCreateInput, SpaceCreateInput,
    SpaceInviteCreateInput, SpaceJoinInput, SpaceQueryInput, SpaceUpdateInput,
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
/// Body matcher: verifies sinceState is sent and maxChanges key is absent
/// when None (RFC 8620 §5.2: omit to let the server choose the limit).
#[tokio::test]
async fn read_position_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["ReadPosition/changes", {
                "accountId": "account1",
                "sinceState": "rp-state-001"
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

// ---------------------------------------------------------------------------
// Test 30: message_set_update (body edit) — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 — Message/set update response shape: newState, updated map.
/// Fixture hand-written from §5.3 /set response definition.
///
/// Body matcher: verifies that the update patch contains only the body field,
/// not null fields for absent options, confirming conditional patch building.
#[tokio::test]
async fn message_set_update_body_edit_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Message/set", {
                "accountId": "account1",
                "update": {
                    "01HV5Z6QKWJ7N3P8R2X4YTMD42": {
                        "body": "Hello, edited!"
                    }
                }
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("message_set_update_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .message_set_update(
            &test_session(&api_url),
            &MessageUpdateInput {
                id: "01HV5Z6QKWJ7N3P8R2X4YTMD42",
                body: Some("Hello, edited!"),
                body_type: None,
                reaction_changes: &[],
                read_at: None,
                deleted_at: None,
                deleted_for_all: None,
            },
        )
        .await
        .expect("message_set_update must succeed");

    // Oracle: RFC 8620 §5.3 — newState is present, updated map contains the message ID
    assert_eq!(result.new_state, "state-msg-002");
    let updated = result.updated.expect("updated map must be present");
    assert!(
        updated.contains_key("01HV5Z6QKWJ7N3P8R2X4YTMD42"),
        "updated map must contain the message id"
    );
}

// ---------------------------------------------------------------------------
// Test 31: message_set_update (add reaction) — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 / JMAP Chat §4.5 — reaction add uses JSON Pointer
/// patch key `reactions/<senderReactionId>` with value `{emoji, sentAt}`.
/// Fixture hand-written from §5.3 /set response definition.
///
/// Body matcher: verifies the reaction patch key format and emoji value.
#[tokio::test]
async fn message_set_update_add_reaction_sends_correct_patch() {
    let server = MockServer::start().await;
    let sent_at = jmap_chat::jmap::UTCDate::from_trusted("2024-01-02T12:00:00Z");

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Message/set", {
                "accountId": "account1",
                "update": {
                    "01HV5Z6QKWJ7N3P8R2X4YTMD42": {
                        "reactions/01HVREACTIONULID01": {
                            "emoji": "\u{1F44D}",
                            "sentAt": "2024-01-02T12:00:00Z"
                        }
                    }
                }
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("message_set_update_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let reaction = ReactionChange::Add {
        sender_reaction_id: "01HVREACTIONULID01",
        emoji: "\u{1F44D}",
        sent_at: &sent_at,
    };
    let result = client
        .message_set_update(
            &test_session(&api_url),
            &MessageUpdateInput {
                id: "01HV5Z6QKWJ7N3P8R2X4YTMD42",
                body: None,
                body_type: None,
                reaction_changes: &[reaction],
                read_at: None,
                deleted_at: None,
                deleted_for_all: None,
            },
        )
        .await
        .expect("message_set_update with reaction must succeed");

    // Oracle: RFC 8620 §5.3 — newState is present
    assert_eq!(result.new_state, "state-msg-002");
}

// ---------------------------------------------------------------------------
// Test 32: message_set_destroy — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 — Message/set destroy response shape: newState, destroyed list.
/// Fixture hand-written from §5.3 /set response definition.
///
/// Body matcher: verifies destroy list is sent (not create or update).
#[tokio::test]
async fn message_set_destroy_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Message/set", {
                "accountId": "account1",
                "destroy": ["01HV5Z6QKWJ7N3P8R2X4YTMD42"]
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("message_set_destroy_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .message_set_destroy(&test_session(&api_url), &["01HV5Z6QKWJ7N3P8R2X4YTMD42"])
        .await
        .expect("message_set_destroy must succeed");

    // Oracle: RFC 8620 §5.3 — destroyed list contains the id
    assert_eq!(result.new_state, "state-msg-002");
    let destroyed = result.destroyed.expect("destroyed list must be present");
    assert_eq!(destroyed, vec!["01HV5Z6QKWJ7N3P8R2X4YTMD42"]);
}

// ---------------------------------------------------------------------------
// Test 33: message_set_destroy — empty ids guard
// ---------------------------------------------------------------------------

/// Oracle: message_set_destroy must reject an empty ids slice before any
/// network call (same pattern as message_get empty ids guard).
#[tokio::test]
async fn message_set_destroy_rejects_empty_ids() {
    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, "http://127.0.0.1:1")
        .expect("client construction must succeed");

    let err = client
        .message_set_destroy(&test_session("http://127.0.0.1:1/api"), &[])
        .await
        .expect_err("empty ids must be rejected");

    assert!(
        matches!(&err, ClientError::InvalidArgument(msg) if msg.contains("ids")),
        "expected InvalidArgument mentioning 'ids', got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 34: message_query_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.6 — Message/queryChanges response shape: added items.
/// Fixture hand-written from §5.6 /queryChanges response definition.
#[tokio::test]
async fn message_query_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture("message_query_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .message_query_changes(&test_session(&api_url), "msg-qs-000", None)
        .await
        .expect("message_query_changes must succeed");

    // Oracle: RFC 8620 §5.6 — added contains the newly visible message
    assert_eq!(result.added.len(), 1);
    assert_eq!(result.added[0].id, "01HV5Z6QKWJ7N3P8R2X4YTMDCC");
    assert_eq!(result.added[0].index, 0);
    assert_eq!(result.old_query_state, "msg-qs-000");
}

// ---------------------------------------------------------------------------
// Test 35: message_query with text filter — sends correct body
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.5 / JMAP Chat §4.5 — Message/query with text filter
/// sends the text field in the filter object. Servers that do not support
/// full-text search return unsupportedFilter; the client sends the field
/// unconditionally when provided.
///
/// Body matcher: verifies both chatId and text appear in the filter, and that
/// position/limit are absent when not provided.
#[tokio::test]
async fn message_query_with_text_filter_sends_correct_body() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Message/query", {
                "accountId": "account1",
                "filter": {
                    "chatId": "01HV5Z6QKWJ7N3P8R2X4YTMD3G",
                    "text": "hello"
                },
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
                text: Some("hello"),
                ..Default::default()
            },
        )
        .await
        .expect("message_query with text filter must succeed");

    // Oracle: RFC 8620 §5.5 — ids is non-empty, queryState is present
    assert!(!result.ids.is_empty(), "ids must have length > 0");
    assert!(
        !result.query_state.is_empty(),
        "query_state must not be empty"
    );
}

// ---------------------------------------------------------------------------
// Test 36: chat_create_direct — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 / JMAP Chat §Chat/set — create direct chat response shape:
/// newState, created map with server-assigned id. Fixture hand-written from spec.
///
/// Body matcher: verifies kind:"direct" and contactId are present in the create object.
#[tokio::test]
async fn chat_create_direct_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Chat/set", {
                "accountId": "account1",
                "create": {
                    "client-direct-001": {
                        "kind": "direct",
                        "contactId": "contact-id-001"
                    }
                }
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture("chat_set_create_direct_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_create_direct(
            &test_session(&api_url),
            &ChatCreateDirectInput {
                client_id: "client-direct-001",
                contact_id: "contact-id-001",
            },
        )
        .await
        .expect("chat_create_direct must succeed");

    // Oracle: RFC 8620 §5.3 — newState is present, created map contains the client key
    assert_eq!(result.new_state, "chat-state-001");
    assert!(
        result
            .created
            .as_ref()
            .unwrap()
            .contains_key("client-direct-001"),
        "created map must contain client-direct-001"
    );
}

// ---------------------------------------------------------------------------
// Test 37: chat_create_group — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 / JMAP Chat §Chat/set — create group chat response shape:
/// newState, created map with server-assigned id. Fixture hand-written from spec.
///
/// Body matcher: verifies kind:"group", name, and memberIds are present.
/// Optional fields (description, avatarBlobId, messageExpirySeconds) are absent
/// from the request body because they are None, confirming conditional serialization.
#[tokio::test]
async fn chat_create_group_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Chat/set", {
                "accountId": "account1",
                "create": {
                    "client-group-001": {
                        "kind": "group",
                        "name": "Test Group",
                        "memberIds": ["contact-id-002"]
                    }
                }
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture("chat_set_create_group_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_create_group(
            &test_session(&api_url),
            &ChatCreateGroupInput {
                client_id: "client-group-001",
                name: "Test Group",
                member_ids: &["contact-id-002"],
                description: None,
                avatar_blob_id: None,
                message_expiry_seconds: None,
            },
        )
        .await
        .expect("chat_create_group must succeed");

    // Oracle: RFC 8620 §5.3 — newState is present, created map contains the client key
    assert_eq!(result.new_state, "chat-state-001");
    assert!(
        result
            .created
            .as_ref()
            .unwrap()
            .contains_key("client-group-001"),
        "created map must contain client-group-001"
    );
}

// ---------------------------------------------------------------------------
// Test 38: chat_set_update (muted) — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 / JMAP Chat §Chat/set — update chat response shape:
/// newState, updated map. Fixture hand-written from spec.
///
/// Body matcher: verifies that only the muted field appears in the patch
/// (absent options are not serialized), confirming conditional patch building.
#[tokio::test]
async fn chat_set_update_muted_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Chat/set", {
                "accountId": "account1",
                "update": {
                    "01HV5Z6QKWJ7N3P8R2X4YTMDAA": {
                        "muted": true
                    }
                }
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("chat_set_update_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_set_update(
            &test_session(&api_url),
            &ChatUpdateInput {
                id: "01HV5Z6QKWJ7N3P8R2X4YTMDAA",
                muted: Some(true),
                mute_until: None,
                receive_typing_indicators: None,
                pinned_message_ids: None,
                message_expiry_seconds: None,
                receipt_sharing: None,
                name: None,
                description: None,
                avatar_blob_id: None,
                add_members: &[],
                remove_members: &[],
                update_member_roles: &[],
            },
        )
        .await
        .expect("chat_set_update must succeed");

    // Oracle: RFC 8620 §5.3 — newState reflects the post-update state
    assert_eq!(result.new_state, "chat-state-002");
}

// ---------------------------------------------------------------------------
// Test 39: chat_typing — happy path
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §Chat/typing — typing response shape: accountId only.
/// Fixture hand-written from spec.
///
/// Body matcher: verifies accountId, chatId, and typing flag are all present.
#[tokio::test]
async fn chat_typing_sends_correct_args() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Chat/typing", {
                "accountId": "account1",
                "chatId": "01HV5Z6QKWJ7N3P8R2X4YTMDAA",
                "typing": true
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("chat_typing_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_typing(&test_session(&api_url), "01HV5Z6QKWJ7N3P8R2X4YTMDAA", true)
        .await
        .expect("chat_typing must succeed");

    // Oracle: JMAP Chat §Chat/typing — accountId is echoed back
    assert_eq!(result.account_id, "account1");
}

// ---------------------------------------------------------------------------
// Test 40: chat_typing — empty chat_id guard
// ---------------------------------------------------------------------------

/// Oracle: chat_typing must reject an empty chat_id before any network call,
/// returning InvalidArgument (same guard pattern as message_set_destroy empty ids).
#[tokio::test]
async fn chat_typing_rejects_empty_chat_id() {
    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, "http://127.0.0.1:1")
        .expect("client construction must succeed");

    let result = client
        .chat_typing(&test_session("http://127.0.0.1:1/api"), "", true)
        .await;

    assert!(
        matches!(result.unwrap_err(), ClientError::InvalidArgument(_)),
        "empty chat_id must produce InvalidArgument"
    );
}

// ---------------------------------------------------------------------------
// Test 41: chat_query_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.6 — Chat/queryChanges response shape: oldQueryState,
/// newQueryState, removed, added. Fixture hand-written from §5.6 definition.
///
/// Body matcher: verifies sinceQueryState is sent and maxChanges key is absent
/// when None (omit-when-None pattern), confirming conditional serialization.
#[tokio::test]
async fn chat_query_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Chat/queryChanges", {
                "accountId": "account1",
                "sinceQueryState": "chat-qs-000"
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("chat_query_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_query_changes(&test_session(&api_url), "chat-qs-000", None)
        .await
        .expect("chat_query_changes must succeed");

    // Oracle: RFC 8620 §5.6 — states are echoed, removed is empty, added has one entry
    assert_eq!(result.old_query_state, "chat-qs-000");
    assert_eq!(result.new_query_state, "chat-qs-001");
    assert!(result.removed.is_empty(), "removed must be empty");
    assert_eq!(result.added.len(), 1, "added must have one entry");
    assert_eq!(result.added[0].id, "01HV5Z6QKWJ7N3P8R2X4YTMDBB");
}

// ---------------------------------------------------------------------------
// Test 42: chat_set_update with add_members + role — serialization coverage
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §Chat/set update — addMembers array with explicit role
/// must include {"id": "...", "role": "admin"} in the patch. Verifies that
/// the serde_json::to_value(role) path in chat_set_update is exercised and
/// produces the correct camelCase wire value ("admin", not "Admin").
#[tokio::test]
async fn chat_set_update_with_add_members_role_serializes_correctly() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Chat/set", {
                "accountId": "account1",
                "update": {
                    "01HV5Z6QKWJ7N3P8R2X4YTMDAA": {
                        "addMembers": [{"id": "contact-id-003", "role": "admin"}]
                    }
                }
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("chat_set_update_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let members = [AddMemberInput {
        id: "contact-id-003",
        role: Some(jmap_chat::types::ChatMemberRole::Admin),
    }];
    let result = client
        .chat_set_update(
            &test_session(&api_url),
            &ChatUpdateInput {
                id: "01HV5Z6QKWJ7N3P8R2X4YTMDAA",
                muted: None,
                mute_until: None,
                receive_typing_indicators: None,
                pinned_message_ids: None,
                message_expiry_seconds: None,
                receipt_sharing: None,
                name: None,
                description: None,
                avatar_blob_id: None,
                add_members: &members,
                remove_members: &[],
                update_member_roles: &[],
            },
        )
        .await
        .expect("chat_set_update must succeed");

    // Oracle: chat_set_update_response.json — newState is "chat-state-002"
    assert_eq!(result.new_state, "chat-state-002");
}

// ---------------------------------------------------------------------------
// Test 43: chat_contact_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.2 — ChatContact/changes response shape: oldState, newState,
/// hasMoreChanges, updated list. Fixture hand-written from §5.2 /changes definition.
///
/// Body matcher: verifies sinceState is sent and maxChanges key is absent
/// when None (RFC 8620 §5.2: omit to let the server choose the limit).
#[tokio::test]
async fn chat_contact_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["ChatContact/changes", {
                "accountId": "account1",
                "sinceState": "contact-state-000"
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("chat_contact_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_contact_changes(&test_session(&api_url), "contact-state-000", None)
        .await
        .expect("chat_contact_changes must succeed");

    // Oracle: RFC 8620 §5.2 — states echoed, updated has one entry
    assert_eq!(result.old_state, "contact-state-000");
    assert_eq!(result.new_state, "contact-state-001");
    assert!(!result.has_more_changes);
    assert_eq!(result.updated.len(), 1);
    assert_eq!(result.updated[0], "01HV5Z6QKWJ7N3P8R2X4YTMDCC");
}

// ---------------------------------------------------------------------------
// Test 44: chat_contact_set — update blocked flag
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §ChatContact/set — update patch must contain only the
/// fields supplied; with display_name:None only blocked appears in the patch.
/// Fixture hand-written from §5.3 /set response definition.
///
/// Body matcher: verifies patch contains only {"blocked": true}, no displayName key.
#[tokio::test]
async fn chat_contact_set_blocked_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["ChatContact/set", {
                "accountId": "account1",
                "update": {
                    "01HV5Z6QKWJ7N3P8R2X4YTMDCC": {"blocked": true}
                }
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("chat_contact_set_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_contact_set(
            &test_session(&api_url),
            &ChatContactSetInput {
                id: "01HV5Z6QKWJ7N3P8R2X4YTMDCC",
                blocked: Some(true),
                display_name: None,
            },
        )
        .await
        .expect("chat_contact_set must succeed");

    // Oracle: chat_contact_set_response.json — newState updated, id present in updated map
    assert_eq!(result.new_state, "contact-state-002");
    assert!(
        result
            .updated
            .as_ref()
            .unwrap()
            .contains_key("01HV5Z6QKWJ7N3P8R2X4YTMDCC"),
        "updated map must contain the patched id"
    );
}

// ---------------------------------------------------------------------------
// Test 45: chat_contact_query — filter + sort
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.5 — ChatContact/query response shape: queryState, ids.
/// Fixture hand-written from §5.5 /query response definition.
///
/// Body matcher: verifies filter has {"blocked": false}, sort has lastSeenAt
/// ascending, and position/limit keys are absent when None.
#[tokio::test]
async fn chat_contact_query_filters_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["ChatContact/query", {
                "accountId": "account1",
                "filter": {"blocked": false},
                "sort": [{"property": "lastSeenAt", "isAscending": true}]
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("chat_contact_query_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_contact_query(
            &test_session(&api_url),
            &ChatContactQueryInput {
                filter_blocked: Some(false),
                filter_presence: None,
                position: None,
                limit: None,
                sort_property: Some("lastSeenAt"),
                sort_ascending: Some(true),
            },
        )
        .await
        .expect("chat_contact_query must succeed");

    // Oracle: chat_contact_query_response.json — ids has one entry
    assert_eq!(result.ids.len(), 1);
    assert_eq!(result.ids[0], "01HV5Z6QKWJ7N3P8R2X4YTMDCC");
}

// ---------------------------------------------------------------------------
// Test 46: chat_contact_query_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.6 — ChatContact/queryChanges response shape:
/// oldQueryState, newQueryState, removed, added. Fixture hand-written from
/// §5.6 definition.
///
/// Body matcher: verifies sinceQueryState is sent and maxChanges key is absent
/// when None (omit-when-None pattern, same as Chat/queryChanges).
#[tokio::test]
async fn chat_contact_query_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["ChatContact/queryChanges", {
                "accountId": "account1",
                "sinceQueryState": "contact-qs-000"
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture("chat_contact_query_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_contact_query_changes(&test_session(&api_url), "contact-qs-000", None)
        .await
        .expect("chat_contact_query_changes must succeed");

    // Oracle: RFC 8620 §5.6 — states echoed, removed empty, added has one entry
    assert_eq!(result.old_query_state, "contact-qs-000");
    assert_eq!(result.new_query_state, "contact-qs-001");
    assert!(result.removed.is_empty(), "removed must be empty");
    assert_eq!(result.added.len(), 1, "added must have one entry");
    assert_eq!(result.added[0].id, "01HV5Z6QKWJ7N3P8R2X4YTMDCC");
}

// ---------------------------------------------------------------------------
// Test 47: space_get — happy path
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §Space/get response shape: state, list with one Space, notFound empty.
/// Fixture hand-written from §4.15 Space object definition.
#[tokio::test]
async fn space_get_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("space_get_response.json")))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_get(
            &test_session(&api_url),
            Some(&["01HV5Z6QKWJ7N3P8R2X4YTMDSP"]),
            None,
        )
        .await
        .expect("space_get must succeed");

    // Oracle: space_get_response.json — list has one Space with id and name
    assert_eq!(result.state, "state-space-001");
    assert_eq!(result.list.len(), 1);
    assert_eq!(result.list[0].id.as_str(), "01HV5Z6QKWJ7N3P8R2X4YTMDSP");
    assert_eq!(result.list[0].name, "Engineering");
    assert_eq!(result.list[0].member_count, 1);
    assert!(result.not_found.unwrap_or_default().is_empty());
}

// ---------------------------------------------------------------------------
// Test 48: space_changes — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.2 — Space/changes response: oldState, newState,
/// hasMoreChanges, created/updated/destroyed lists.
///
/// Body matcher: verifies sinceState is sent and maxChanges key is absent
/// when None (RFC 8620 §5.2: omit to let the server choose the limit).
#[tokio::test]
async fn space_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Space/changes", {
                "accountId": "account1",
                "sinceState": "state-space-000"
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("space_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_changes(&test_session(&api_url), "state-space-000", None)
        .await
        .expect("space_changes must succeed");

    // Oracle: space_changes_response.json — one created id, updated/destroyed empty
    assert_eq!(result.old_state, "state-space-000");
    assert_eq!(result.new_state, "state-space-001");
    assert!(!result.has_more_changes);
    assert_eq!(result.created.len(), 1);
    assert_eq!(result.created[0], "01HV5Z6QKWJ7N3P8R2X4YTMDSP");
    assert!(result.updated.is_empty());
    assert!(result.destroyed.is_empty());
}

// ---------------------------------------------------------------------------
// Test 49: space_create — body shape + happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 — Space/set create response: created map with server id.
///
/// Body matcher: verifies name is sent and optional fields absent when None.
#[tokio::test]
async fn space_create_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Space/set", {
                "accountId": "account1",
                "create": {
                    "client-space-001": {
                        "name": "Engineering"
                    }
                }
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("space_set_create_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_create(
            &test_session(&api_url),
            &SpaceCreateInput {
                client_id: "client-space-001",
                name: "Engineering",
                description: None,
                icon_blob_id: None,
            },
        )
        .await
        .expect("space_create must succeed");

    // Oracle: space_set_create_response.json — created map has one entry
    let created = result.created.expect("created must be Some");
    assert!(created.contains_key("client-space-001"));
}

// ---------------------------------------------------------------------------
// Test 50: space_set_update — body shape + happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 — Space/set update response: updated map.
///
/// Body matcher: verifies metadata-only patch (name key only).
#[tokio::test]
async fn space_set_update_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Space/set", {
                "accountId": "account1",
                "update": {
                    "01HV5Z6QKWJ7N3P8R2X4YTMDSP": {
                        "name": "Engineering Team"
                    }
                }
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("space_set_update_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_set_update(
            &test_session(&api_url),
            &SpaceUpdateInput {
                id: "01HV5Z6QKWJ7N3P8R2X4YTMDSP",
                name: Some("Engineering Team"),
                description: None,
                icon_blob_id: None,
                is_public: None,
                is_publicly_previewable: None,
                add_members: &[],
                remove_members: &[],
                update_members: &[],
                add_channels: &[],
                remove_channels: &[],
            },
        )
        .await
        .expect("space_set_update must succeed");

    // Oracle: space_set_update_response.json — updated map has one entry
    let updated = result.updated.expect("updated must be Some");
    assert!(updated.contains_key("01HV5Z6QKWJ7N3P8R2X4YTMDSP"));
}

// ---------------------------------------------------------------------------
// Test 51: space_set_destroy — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 — Space/set destroy response: destroyed list.
///
/// Body matcher: verifies the destroy key is present with the correct id array.
#[tokio::test]
async fn space_set_destroy_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Space/set", {
                "accountId": "account1",
                "destroy": ["01HV5Z6QKWJ7N3P8R2X4YTMDSP"]
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("space_set_destroy_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_set_destroy(&test_session(&api_url), &["01HV5Z6QKWJ7N3P8R2X4YTMDSP"])
        .await
        .expect("space_set_destroy must succeed");

    // Oracle: space_set_destroy_response.json — destroyed list has one id
    let destroyed = result.destroyed.expect("destroyed must be Some");
    assert_eq!(destroyed.len(), 1);
    assert_eq!(destroyed[0], "01HV5Z6QKWJ7N3P8R2X4YTMDSP");
}

// ---------------------------------------------------------------------------
// Test 52: space_query — body shape + happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.5 — Space/query response: queryState, ids, canCalculateChanges.
///
/// Body matcher: verifies filter with isPublic and position.
#[tokio::test]
async fn space_query_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Space/query", {
                "accountId": "account1",
                "filter": {"isPublic": true},
                "position": 0,
                "limit": 10
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("space_query_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_query(
            &test_session(&api_url),
            &SpaceQueryInput {
                filter_name: None,
                filter_is_public: Some(true),
                position: Some(0),
                limit: Some(10),
            },
        )
        .await
        .expect("space_query must succeed");

    // Oracle: space_query_response.json — ids has one entry
    assert_eq!(result.ids.len(), 1);
    assert_eq!(result.ids[0], "01HV5Z6QKWJ7N3P8R2X4YTMDSP");
    assert!(result.can_calculate_changes);
}

// ---------------------------------------------------------------------------
// Test 53: space_query_changes — maxChanges absent when None
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.6 — Space/queryChanges response shape.
///
/// Body matcher: verifies sinceQueryState is sent and maxChanges key is absent
/// when None (omit-when-None pattern).
#[tokio::test]
async fn space_query_changes_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Space/queryChanges", {
                "accountId": "account1",
                "sinceQueryState": "space-qs-000"
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("space_query_changes_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_query_changes(&test_session(&api_url), "space-qs-000", None)
        .await
        .expect("space_query_changes must succeed");

    // Oracle: space_query_changes_response.json — removed empty, added has one entry
    assert_eq!(result.old_query_state, "space-qs-000");
    assert_eq!(result.new_query_state, "space-qs-001");
    assert!(result.removed.is_empty());
    assert_eq!(result.added.len(), 1);
    assert_eq!(result.added[0].id, "01HV5Z6QKWJ7N3P8R2X4YTMDSP");
}

// ---------------------------------------------------------------------------
// Test 54: space_join — invite_code path, body shape
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §Space/join — SpaceJoinResponse with accountId and spaceId.
///
/// Body matcher: verifies inviteCode present and spaceId absent (invite path).
#[tokio::test]
async fn space_join_by_invite_code() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Space/join", {
                "accountId": "account1",
                "inviteCode": "INVITE-XYZ"
            }, "r1"]]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("space_join_response.json")))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_join(
            &test_session(&api_url),
            &SpaceJoinInput::InviteCode("INVITE-XYZ"),
        )
        .await
        .expect("space_join must succeed");

    // Oracle: space_join_response.json — accountId and spaceId present
    assert_eq!(result.account_id, "account1");
    assert_eq!(result.space_id, "01HV5Z6QKWJ7N3P8R2X4YTMDSP");
}

// ---------------------------------------------------------------------------
// Test 55: space_join — SpaceId path body shape
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §Space/join — SpaceJoinResponse with accountId and spaceId.
///
/// Body matcher: verifies spaceId present and inviteCode absent (SpaceId path).
/// Covers the second enum variant so both join paths are wire-verified.
#[tokio::test]
async fn space_join_by_space_id() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Space/join", {
                "accountId": "account1",
                "spaceId": "01HV5Z6QKWJ7N3P8R2X4YTMDSP"
            }, "r1"]]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("space_join_response.json")))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .space_join(
            &test_session(&api_url),
            &SpaceJoinInput::SpaceId("01HV5Z6QKWJ7N3P8R2X4YTMDSP"),
        )
        .await
        .expect("space_join must succeed");

    // Oracle: space_join_response.json — accountId and spaceId present
    assert_eq!(result.account_id, "account1");
    assert_eq!(result.space_id, "01HV5Z6QKWJ7N3P8R2X4YTMDSP");
}

// ---------------------------------------------------------------------------
// Test 56: push_subscription_set — body shape + happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §7.2 — PushSubscription/set response with created map.
/// draft-atwood-jmap-chat-push-00 §3.1 — chatPush property in create object.
///
/// Body matcher: verifies the `using` array includes `urn:ietf:params:jmap:chat:push`,
/// `chatPush` is present with the correct account id key, and urgency is included.
#[tokio::test]
async fn push_subscription_set_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat:push"],
            "methodCalls": [["PushSubscription/set", {
                "create": {
                    "client-push-001": {
                        "deviceClientId": "device-abc",
                        "url": "https://push.example.com/endpoint",
                        "types": ["Message"],
                        "chatPush": {
                            "account1": {
                                "urgency": "normal",
                                "mentionUrgency": "high"
                            }
                        }
                    }
                }
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture("push_subscription_set_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let push_config = jmap_chat::types::ChatPushConfig {
        kinds: None,
        chat_ids: None,
        properties: None,
        urgency: Some("normal".to_string()),
        mention_urgency: Some("high".to_string()),
    };
    let result = client
        .push_subscription_set(
            &test_session(&api_url),
            &PushSubscriptionCreateInput {
                client_id: "client-push-001",
                device_client_id: "device-abc",
                url: "https://push.example.com/endpoint",
                expires: None,
                types: Some(&["Message"]),
                chat_push: Some(&[("account1", push_config)]),
            },
        )
        .await
        .expect("push_subscription_set must succeed");

    // Oracle: push_subscription_set_response.json — created map has one entry; accountId null
    let created = result.created.expect("created must be Some");
    assert!(created.contains_key("client-push-001"));
    assert!(
        result.account_id.is_none(),
        "accountId must be null for PushSubscription/set"
    );
}

// ---------------------------------------------------------------------------
// Test 57: blob_lookup — happy path
// ---------------------------------------------------------------------------

/// Oracle: blob_lookup_response.json — list[0].matched_ids["Message"] contains
/// two IDs; notFound contains one ID. Verifies BlobLookupResponse deserialization
/// and correct use of urn:ietf:params:jmap:blob2 capability.
#[tokio::test]
async fn blob_lookup_returns_typed_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("blob_lookup_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must not fail");
    let session = test_session(&server.uri());

    let result = client
        .blob_lookup(&session, &["blob-001", "blob-missing"], Some(&["Message"]))
        .await
        .expect("blob_lookup must succeed");

    // Oracle: fixture list[0].id == "blob-001" with two matched Message IDs
    assert_eq!(result.list.len(), 1);
    assert_eq!(result.list[0].id, "blob-001");
    let msgs = result.list[0]
        .matched_ids
        .get("Message")
        .expect("Message key must be present");
    assert_eq!(msgs.len(), 2);
    assert!(msgs.contains(&"msg-123".to_string()));
    assert!(msgs.contains(&"msg-456".to_string()));

    // Oracle: fixture notFound == ["blob-missing"]
    assert!(result.not_found.contains(&"blob-missing".to_string()));
}

// ---------------------------------------------------------------------------
// Test: message_set_update (readAt) — marks message read timestamp
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §4.5 — readAt patch sets the read timestamp on a Message.
/// Body matcher: verifies readAt is present in the update patch with full-precision value.
#[tokio::test]
async fn message_set_update_read_at_sends_correct_patch() {
    let server = MockServer::start().await;
    let read_at = jmap_chat::jmap::UTCDate::from_trusted("2024-01-01T10:05:00Z");

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Message/set", {
                "accountId": "account1",
                "update": {
                    "01HV5Z6QKWJ7N3P8R2X4YTMD42": {
                        "readAt": "2024-01-01T10:05:00Z"
                    }
                }
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("message_set_update_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    client
        .message_set_update(
            &test_session(&api_url),
            &MessageUpdateInput {
                id: "01HV5Z6QKWJ7N3P8R2X4YTMD42",
                body: None,
                body_type: None,
                reaction_changes: &[],
                read_at: Some(&read_at),
                deleted_at: None,
                deleted_for_all: None,
            },
        )
        .await
        .expect("message_set_update with readAt must succeed");
}

// ---------------------------------------------------------------------------
// Test: message_query (threadRootId) — filter to a message thread
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §Message/query — threadRootId filter restricts results to a thread.
/// Body matcher: verifies threadRootId is sent in the filter object.
#[tokio::test]
async fn message_query_with_thread_root_id_sends_correct_filter() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Message/query", {
                "accountId": "account1",
                "filter": {
                    "chatId": "chat-001",
                    "threadRootId": "msg-root-001"
                },
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
    client
        .message_query(
            &test_session(&api_url),
            &MessageQueryInput {
                chat_id: Some("chat-001"),
                thread_root_id: Some("msg-root-001"),
                ..Default::default()
            },
        )
        .await
        .expect("message_query with threadRootId must succeed");
}

// ---------------------------------------------------------------------------
// Test: message_create rateLimited — returns ClientError::RateLimited
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat slow-mode — server returns rateLimited SetError with serverRetryAfter.
/// Fixture hand-written from JMAP Chat SetError spec (rateLimited type).
#[tokio::test]
async fn message_create_rate_limited_returns_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("message_set_rate_limited.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let sent_at = jmap_chat::jmap::UTCDate::from_trusted("2024-01-01T10:00:30Z");
    let err = client
        .message_create(
            &test_session(&api_url),
            &MessageCreateInput {
                client_id: "client-id-001",
                chat_id: "chat-001",
                body: "Hello",
                body_type: "text/plain",
                sent_at: &sent_at,
                reply_to: None,
            },
        )
        .await
        .expect_err("message_create must fail when rateLimited");

    match err {
        ClientError::RateLimited { retry_after } => {
            assert_eq!(retry_after.as_str(), "2024-01-01T10:01:00Z");
        }
        other => panic!("expected RateLimited, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test: chat_set_destroy — happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §5.3 — Chat/set destroy response: destroyed list contains the ID.
/// Fixture hand-written from §5.3 /set response definition.
#[tokio::test]
async fn chat_set_destroy_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Chat/set", {
                "accountId": "account1",
                "destroy": ["01HV5Z6QKWJ7N3P8R2X4YTMDCH"]
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fixture("chat_set_destroy_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_set_destroy(&test_session(&api_url), &["01HV5Z6QKWJ7N3P8R2X4YTMDCH"])
        .await
        .expect("chat_set_destroy must succeed");

    // Oracle: fixture destroyed list contains the chat ID
    let destroyed = result.destroyed.expect("destroyed list must be present");
    assert_eq!(destroyed.len(), 1);
    assert_eq!(destroyed[0], "01HV5Z6QKWJ7N3P8R2X4YTMDCH");
}

// ---------------------------------------------------------------------------
// Test: chat_create_channel — happy path
// ---------------------------------------------------------------------------

/// Oracle: JMAP Chat §Chat/set create/channel — server assigns an ID in the created map.
/// Body matcher: verifies kind, spaceId, name are sent.
#[tokio::test]
async fn chat_create_channel_returns_typed_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"],
            "methodCalls": [["Chat/set", {
                "accountId": "account1",
                "create": {
                    "client-ch-001": {
                        "kind": "channel",
                        "spaceId": "space-001",
                        "name": "general"
                    }
                }
            }, "r1"]]
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture("chat_set_create_channel_response.json")),
        )
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");

    let api_url = format!("{}/api", server.uri());
    let result = client
        .chat_create_channel(
            &test_session(&api_url),
            &ChatCreateChannelInput {
                client_id: "client-ch-001",
                space_id: "space-001",
                name: "general",
                description: None,
            },
        )
        .await
        .expect("chat_create_channel must succeed");

    // Oracle: fixture created map contains client_id → server-assigned ID
    let created = result.created.expect("created map must be present");
    let chat = created
        .get("client-ch-001")
        .expect("created map must contain client_id");
    assert_eq!(
        chat.get("id").and_then(|v| v.as_str()),
        Some("01HV5Z6QKWJ7N3P8R2X4YTMDCN")
    );
}
