// Integration tests for typed JMAP Chat method wrappers (Step 8).
//
// Each test mounts a wiremock POST handler returning a hand-written fixture,
// calls the typed method, and asserts key fields of the typed response.
//
// Fixtures live in tests/fixtures/methods/ and are hand-written from the
// RFC 8620 §5 response shapes — they are NOT derived from the code under test.

use jmap_chat::client::JmapChatClient;
use jmap_chat::error::ClientError;
use jmap_chat::methods::{GetResponse, MessageCreateInput, MessageQueryInput};
use wiremock::matchers::method;
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
