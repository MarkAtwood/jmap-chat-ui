//! Integration tests for `JmapChatClient::fetch_session()`.
//!
//! Oracle: all expected values are derived from hand-written JSON fixtures
//! and the algorithm described in RFC 8620 §2 and the research report
//! (audit/research_steps5to8.md §B). No values are derived from the
//! implementation code under test.
//!
//! Fixtures live in tests/fixtures/session/ and are committed alongside
//! this file as independent oracles.

use jmap_chat::{ClientError, JmapChatClient, NoneAuth};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The session fixture embedded at compile time — an independent oracle
/// hand-written from RFC 8620 §2 and draft-atwood-jmap-chat-00 §3.
fn session_ok_json() -> &'static str {
    include_str!("fixtures/session/session_ok.json")
}

/// Session fixture with `"apiUrl": ""` — used to test empty-apiUrl validation.
fn session_missing_api_url_json() -> &'static str {
    include_str!("fixtures/session/session_missing_api_url.json")
}

// ---------------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §2 — Session object shape, hand-written fixture
/// `session_ok.json` was constructed directly from the spec and the JMAP Chat
/// §3 extension fields without consulting the implementation.
///
/// Expected values (read from fixture, not from code):
/// - username:           "alice@example.com"
/// - chat account:       "account-alice"  (primaryAccounts["urn:ietf:params:jmap:chat"])
/// - api_url:            non-empty        ("https://jmap.example.com/api")
/// - event_source_url:   non-empty        ("https://jmap.example.com/events")
#[tokio::test]
async fn fetch_session_returns_parsed_session() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/jmap"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(session_ok_json(), "application/json"),
        )
        .mount(&server)
        .await;

    let client =
        JmapChatClient::new(NoneAuth, &server.uri()).expect("NoneAuth::build_client must succeed");

    let session = client
        .fetch_session()
        .await
        .expect("fetch_session must succeed with valid 200 response");

    // Oracle: fixture has username "alice@example.com"
    assert_eq!(session.username, "alice@example.com");

    // Oracle: fixture has primaryAccounts["urn:ietf:params:jmap:chat"] == "account-alice"
    assert_eq!(
        session.chat_account_id(),
        Some("account-alice"),
        "chat_account_id() must return the value from primaryAccounts"
    );

    // Oracle: RFC 8620 §2 — api_url must be non-empty (validated by fetch_session)
    assert!(
        !session.api_url.is_empty(),
        "api_url must be non-empty after fetch_session validation"
    );

    // Oracle: RFC 8620 §2 — event_source_url must be non-empty (validated by fetch_session)
    assert!(
        !session.event_source_url.is_empty(),
        "event_source_url must be non-empty after fetch_session validation"
    );
}

// ---------------------------------------------------------------------------
// HTTP error cases
// ---------------------------------------------------------------------------

/// Oracle: ClientError::AuthFailed(401) for HTTP 401 response.
///
/// RFC 8620 §2 requires the client to stop retrying on auth failures.
/// The implementation special-cases 401 before calling error_for_status()
/// so callers can distinguish auth failures from generic HTTP errors.
/// (research_steps5to8.md §B step 5)
#[tokio::test]
async fn fetch_session_401_returns_auth_failed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/jmap"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client =
        JmapChatClient::new(NoneAuth, &server.uri()).expect("NoneAuth::build_client must succeed");

    let result = client.fetch_session().await;

    assert!(result.is_err(), "HTTP 401 must produce an Err");
    assert!(
        matches!(result.unwrap_err(), ClientError::AuthFailed(401)),
        "HTTP 401 must map to ClientError::AuthFailed(401)"
    );
}

/// Oracle: ClientError::AuthFailed(403) for HTTP 403 response.
///
/// 403 Forbidden, like 401 Unauthorized, signals that retrying the same
/// request will not succeed. Both status codes map to AuthFailed per
/// research_steps5to8.md §B step 5.
#[tokio::test]
async fn fetch_session_403_returns_auth_failed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/jmap"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let client =
        JmapChatClient::new(NoneAuth, &server.uri()).expect("NoneAuth::build_client must succeed");

    let result = client.fetch_session().await;

    assert!(result.is_err(), "HTTP 403 must produce an Err");
    assert!(
        matches!(result.unwrap_err(), ClientError::AuthFailed(403)),
        "HTTP 403 must map to ClientError::AuthFailed(403)"
    );
}

/// Oracle: ClientError::Http for HTTP 500 response.
///
/// A 500 Internal Server Error is a server-side fault that may be transient.
/// It must map to ClientError::Http (not AuthFailed) so the caller can decide
/// whether to retry with backoff.
#[tokio::test]
async fn fetch_session_500_returns_http_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/jmap"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client =
        JmapChatClient::new(NoneAuth, &server.uri()).expect("NoneAuth::build_client must succeed");

    let result = client.fetch_session().await;

    assert!(result.is_err(), "HTTP 500 must produce an Err");
    assert!(
        matches!(result.unwrap_err(), ClientError::Http(_)),
        "HTTP 500 must map to ClientError::Http"
    );
}

// ---------------------------------------------------------------------------
// Validation error cases
// ---------------------------------------------------------------------------

/// Oracle: ClientError::InvalidSession when apiUrl is empty string.
///
/// RFC 8620 §2 requires api_url to be present and usable. The algorithm
/// (research_steps5to8.md §B step 7) validates that api_url is non-empty
/// after parsing and returns ClientError::InvalidSession on failure.
/// The fixture `session_missing_api_url.json` contains `"apiUrl": ""`.
#[tokio::test]
async fn fetch_session_empty_api_url_returns_invalid_session() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/jmap"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(session_missing_api_url_json(), "application/json"),
        )
        .mount(&server)
        .await;

    let client =
        JmapChatClient::new(NoneAuth, &server.uri()).expect("NoneAuth::build_client must succeed");

    let result = client.fetch_session().await;

    assert!(result.is_err(), "empty apiUrl must produce an Err");
    assert!(
        matches!(result.unwrap_err(), ClientError::InvalidSession(_)),
        "empty apiUrl must map to ClientError::InvalidSession"
    );
}

/// Oracle: ClientError::Parse when body is not valid JSON.
///
/// The algorithm (research_steps5to8.md §B step 6) maps JSON parse errors
/// to ClientError::Parse. The body "not json" is not valid JSON by inspection;
/// this is the independent oracle.
#[tokio::test]
async fn fetch_session_invalid_json_returns_parse_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/jmap"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("not json", "application/json"))
        .mount(&server)
        .await;

    let client =
        JmapChatClient::new(NoneAuth, &server.uri()).expect("NoneAuth::build_client must succeed");

    let result = client.fetch_session().await;

    assert!(result.is_err(), "invalid JSON body must produce an Err");
    assert!(
        matches!(result.unwrap_err(), ClientError::Parse(_)),
        "non-JSON body must map to ClientError::Parse"
    );
}
