// Integration tests for batch calls and Quota/get.
//
// Each test mounts a wiremock handler returning a hand-written fixture and
// asserts key fields of the response.  Fixtures are in tests/fixtures/methods/
// (response shapes) and tests/fixtures/jmap/ (request shapes).

use jmap_chat::client::JmapChatClient;
use jmap_chat::jmap::JmapRequestBuilder;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_session(api_url: &str) -> jmap_chat::jmap::Session {
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

fn method_fixture(name: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/methods")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {name}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {name} is not valid JSON: {e}"))
}

// ---------------------------------------------------------------------------
// call_batch tests
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §3.3/§3.4 — call_batch returns all responses indexed by
/// call id; fixture hand-written from spec response structure.
#[tokio::test]
async fn call_batch_returns_all_responses() {
    let server = MockServer::start().await;
    let fixture = method_fixture("batch_response.json");
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&fixture))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri()).unwrap();
    let session = test_session(&server.uri());

    let req = JmapRequestBuilder::new(vec![
        "urn:ietf:params:jmap:core".to_string(),
        "urn:ietf:params:jmap:chat".to_string(),
    ])
    .add_call(
        "Chat/get",
        serde_json::json!({"accountId": "account1", "ids": null}),
        "r1",
    )
    .add_call(
        "Message/query",
        serde_json::json!({"accountId": "account1", "chatId": "chat-001", "limit": 10}),
        "r2",
    )
    .build();

    let responses = client
        .call_batch(&session.api_url, &req)
        .await
        .expect("call_batch must succeed");

    assert_eq!(responses.len(), 2, "must return two responses");
    assert!(responses.contains_key("r1"), "must contain r1");
    assert!(responses.contains_key("r2"), "must contain r2");

    // Verify r1 is the Chat/get result and r2 is Message/query result.
    assert_eq!(responses["r1"]["accountId"], "account1");
    assert_eq!(responses["r2"]["accountId"], "account1");
    assert!(responses["r2"].get("queryState").is_some());
}

// ---------------------------------------------------------------------------
// quota_get tests
// ---------------------------------------------------------------------------

/// Oracle: RFC 8621 §2 — quota_get returns typed Quota objects; fixture
/// hand-written from RFC 8621 §2 Quota object definition.
#[tokio::test]
async fn quota_get_returns_quota_list() {
    let server = MockServer::start().await;
    let fixture = method_fixture("quota_get.json");
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&fixture))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri()).unwrap();
    let session = test_session(&server.uri());

    let quotas = client
        .with_session(&session)
        .quota_get()
        .await
        .expect("quota_get must succeed");

    assert_eq!(quotas.len(), 2, "fixture has two quota objects");

    let msg_quota = quotas
        .iter()
        .find(|q| q.id == "quota-msg-1")
        .expect("quota-msg-1 must be present");
    assert_eq!(msg_quota.name, "Message Storage");
    assert_eq!(msg_quota.scope, "account");
    assert_eq!(msg_quota.data_types, vec!["Message"]);
    assert_eq!(msg_quota.used, 52428800);
    assert_eq!(msg_quota.hard_limit, 1073741824);
    assert_eq!(msg_quota.warn_limit, Some(858993459));
    assert_eq!(msg_quota.soft_limit, None);

    let chat_quota = quotas
        .iter()
        .find(|q| q.id == "quota-chat-1")
        .expect("quota-chat-1 must be present");
    assert_eq!(chat_quota.data_types, vec!["Chat", "Space"]);
    assert_eq!(chat_quota.used, 1024);
    assert_eq!(chat_quota.warn_limit, None);
    assert_eq!(chat_quota.description, None);
}
