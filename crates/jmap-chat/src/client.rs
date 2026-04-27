// JmapChatClient: fetch_session, call, subscribe_events (Steps 5-7)

use std::sync::Arc;

use crate::auth::AuthProvider;
use crate::error::ClientError;
use crate::jmap::{JmapRequest, JmapResponse, Session};
use crate::sse::{parse_sse_block, SseFrame};
use futures::StreamExt;

/// Auth-agnostic JMAP Chat HTTP client.
///
/// Construct with [`JmapChatClient::new`], then call [`fetch_session`] to
/// obtain a [`Session`] before issuing any JMAP method calls.
///
/// [`fetch_session`]: JmapChatClient::fetch_session
#[derive(Clone)]
pub struct JmapChatClient {
    pub(crate) base_url: String,
    pub(crate) auth: Arc<dyn AuthProvider>,
    pub(crate) http: reqwest::Client,
}

impl JmapChatClient {
    /// Create a new client.
    ///
    /// `auth` provides both the HTTP client configuration (trust roots, client
    /// certificates) and per-request header injection. `base_url` must be the
    /// server origin without a trailing slash or path component, e.g.
    /// `"https://100.64.1.1:8008"`.
    pub fn new(auth: impl AuthProvider + 'static, base_url: &str) -> Result<Self, ClientError> {
        if base_url.is_empty() {
            return Err(ClientError::InvalidArgument("base_url may not be empty".into()));
        }
        let parsed = url::Url::parse(base_url)
            .map_err(|e| ClientError::InvalidArgument(format!("base_url is not a valid URL: {e}")))?;
        let path = parsed.path();
        if path != "/" {
            return Err(ClientError::InvalidArgument(
                format!("base_url must not have a path component, got: {path:?}")
            ));
        }
        if parsed.query().is_some() {
            return Err(ClientError::InvalidArgument(
                "base_url must not have a query string".into(),
            ));
        }
        if parsed.fragment().is_some() {
            return Err(ClientError::InvalidArgument(
                "base_url must not have a fragment".into(),
            ));
        }
        // Store without trailing slash
        let base_url = base_url.trim_end_matches('/').to_string();
        let http = auth.build_client()?;
        Ok(Self {
            base_url,
            auth: Arc::new(auth),
            http,
        })
    }

    pub(crate) fn check_auth_status(status: reqwest::StatusCode) -> Result<(), ClientError> {
        if status == 401 || status == 403 {
            Err(ClientError::AuthFailed(status.as_u16()))
        } else {
            Ok(())
        }
    }

    /// Fetch the JMAP Session object from `{base_url}/.well-known/jmap`.
    ///
    /// Returns `ClientError::AuthFailed` on HTTP 401 or 403 so the caller can
    /// distinguish auth failures (which will not resolve on retry) from
    /// transient errors.
    pub async fn fetch_session(&self) -> Result<Session, ClientError> {
        let url = format!("{}/.well-known/jmap", self.base_url.trim_end_matches('/'));

        let mut req = self
            .http
            .get(&url)
            .timeout(std::time::Duration::from_secs(30));
        if let Some((name, value)) = self.auth.auth_header() {
            req = req.header(name, value);
        }

        let resp = req.send().await.map_err(ClientError::Http)?;

        let status = resp.status();
        Self::check_auth_status(status)?;

        let resp = resp.error_for_status().map_err(ClientError::Http)?;

        let session: Session = resp
            .json()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))?;

        if session.api_url.is_empty() {
            return Err(ClientError::InvalidSession("apiUrl is empty"));
        }
        if session.event_source_url.is_empty() {
            return Err(ClientError::InvalidSession("eventSourceUrl is empty"));
        }
        if session.upload_url.is_empty() {
            return Err(ClientError::InvalidSession("uploadUrl is empty"));
        }
        if session.download_url.is_empty() {
            return Err(ClientError::InvalidSession("downloadUrl is empty"));
        }

        Ok(session)
    }

    /// POST a [`JmapRequest`] to `api_url` and return the parsed [`JmapResponse`].
    ///
    /// `api_url` is taken as an explicit parameter (not from `self`) because the
    /// caller has a [`Session`] and selects the correct URL from it.
    ///
    /// Returns [`ClientError::AuthFailed`] on HTTP 401 or 403 so the caller can
    /// distinguish auth failures from transient errors.
    pub async fn call(
        &self,
        api_url: &str,
        req: &JmapRequest,
    ) -> Result<JmapResponse, ClientError> {
        let mut builder = self.http.post(api_url).json(req);

        if let Some((name, value)) = self.auth.auth_header() {
            builder = builder.header(name, value);
        }

        builder = builder.timeout(std::time::Duration::from_secs(30));

        let resp = builder.send().await.map_err(ClientError::Http)?;

        let status = resp.status();
        Self::check_auth_status(status)?;

        let resp = resp.error_for_status().map_err(ClientError::Http)?;

        let jmap_resp: JmapResponse = resp
            .json()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))?;

        Ok(jmap_resp)
    }

    /// POST a multi-method [`JmapRequest`] and return all responses indexed by call id.
    ///
    /// Thin wrapper over [`call`](JmapChatClient::call) for batch requests
    /// built with [`JmapRequestBuilder`](crate::jmap::JmapRequestBuilder).
    /// Returns a `HashMap` from call id to the raw JSON args of each response.
    ///
    /// JMAP `"error"` responses are included in the map with their original
    /// args; callers must check the `"type"` field if they need to distinguish
    /// errors from successful responses for individual invocations.
    ///
    /// Returns [`ClientError::Parse`] if the server returns duplicate call ids,
    /// which violates RFC 8620 §3.3 ("each method call id MUST be unique within
    /// a request").
    ///
    /// Spec: RFC 8620 §3.3 / §3.4
    pub async fn call_batch(
        &self,
        api_url: &str,
        req: &JmapRequest,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>, ClientError> {
        let resp = self.call(api_url, req).await?;
        let mut map = std::collections::HashMap::with_capacity(resp.method_responses.len());
        for inv in resp.method_responses {
            if map.insert(inv.call_id.clone(), inv.args).is_some() {
                return Err(ClientError::Parse(format!(
                    "server returned duplicate call id {:?} in batch response",
                    inv.call_id
                )));
            }
        }
        Ok(map)
    }

    /// Open an SSE connection to `event_source_url` and return an async stream
    /// of parsed frames.
    ///
    /// Each stream item is an [`SseFrame`] carrying the parsed event and the
    /// `id:` field value (if any). Callers should track the last non-None
    /// `SseFrame::id` and send it as `Last-Event-ID` on reconnect per RFC 8620
    /// §7.3.
    ///
    /// The returned stream ends when the server closes the connection. Callers
    /// are responsible for reconnection with exponential backoff.
    ///
    /// If `last_event_id` is `Some`, sends a `Last-Event-ID` header so the
    /// server can resume from where the previous stream left off (RFC 8620 §7.3).
    ///
    /// Returns [`ClientError::AuthFailed`] on HTTP 401 or 403 before the stream
    /// starts; callers must not retry on auth failures.
    ///
    /// Buffer growth is capped at 1 MiB per frame. If a single SSE frame
    /// exceeds this limit the stream yields [`ClientError::SseFrameTooLarge`]
    /// and terminates.
    pub async fn subscribe_events(
        &self,
        event_source_url: &str,
        last_event_id: Option<&str>,
    ) -> Result<impl futures::Stream<Item = Result<SseFrame, ClientError>> + Send, ClientError> {
        let mut req = self
            .http
            .get(event_source_url)
            .header("Accept", "text/event-stream");

        if let Some(id) = last_event_id {
            req = req.header("Last-Event-ID", id);
        }
        if let Some((name, value)) = self.auth.auth_header() {
            req = req.header(name, value);
        }

        let resp = req.send().await.map_err(ClientError::Http)?;

        let status = resp.status();
        Self::check_auth_status(status)?;

        let resp = resp.error_for_status().map_err(ClientError::Http)?;

        let byte_stream = resp.bytes_stream();

        Ok(futures::stream::unfold(
            Some((byte_stream, String::new(), 0usize)),
            |state| async move {
                let (mut stream, mut buf, mut scan_from) = state?;
                loop {
                    // Search for any double-newline delimiter (LF/CRLF/CR variants).
                    // scan_from is set to old_len.saturating_sub(3) after each append
                    // so we only re-scan the overlap region rather than the whole buffer.
                    // 3 bytes back covers the longest incomplete delimiter prefix that
                    // can straddle the chunk boundary: `\r\n\r` (a 3-byte prefix of
                    // `\r\n\r\n`).
                    // Find the earliest occurrence and record its byte length so we
                    // extract exactly the right number of bytes.
                    let frame_end = [
                        buf[scan_from..]
                            .find("\r\n\r\n")
                            .map(|p| (scan_from + p, 4usize)),
                        buf[scan_from..]
                            .find("\n\n")
                            .map(|p| (scan_from + p, 2usize)),
                        buf[scan_from..]
                            .find("\r\r")
                            .map(|p| (scan_from + p, 2usize)),
                    ]
                    .into_iter()
                    .flatten()
                    .min_by_key(|&(pos, _)| pos);

                    if let Some((pos, delim_len)) = frame_end {
                        // Extract frame as O(frame) copy, then split off remainder in O(1).
                        let raw_frame = buf[..pos].to_string();
                        let suffix = buf.split_off(pos + delim_len);
                        buf = suffix;
                        scan_from = 0;
                        // Normalize line endings only in the extracted frame, not the
                        // whole buffer, so cost is O(frame) not O(total buffer).
                        let frame = raw_frame.replace("\r\n", "\n").replace('\r', "\n");
                        let sse_frame = parse_sse_block(&frame);
                        return Some((Ok(sse_frame), Some((stream, buf, scan_from))));
                    }

                    // Need more data from the network.
                    match stream.next().await {
                        None => return None,
                        Some(Err(e)) => {
                            return Some((
                                Err(ClientError::Http(e)),
                                Some((stream, buf, scan_from)),
                            ));
                        }
                        Some(Ok(bytes)) => {
                            // Reject invalid UTF-8 rather than silently replacing with U+FFFD.
                            let text = match String::from_utf8(bytes.to_vec()) {
                                Ok(s) => s,
                                Err(_) => {
                                    return Some((
                                        Err(ClientError::Parse(
                                            "invalid UTF-8 in SSE stream".into(),
                                        )),
                                        None,
                                    ));
                                }
                            };
                            // Advance scan_from to 3 bytes before the new data so we catch
                            // delimiters that span the old/new boundary.
                            let old_len = buf.len();
                            // Append raw bytes; normalization happens at frame extraction time.
                            buf.push_str(&text);
                            scan_from = old_len.saturating_sub(3);
                            // Walk backward to a char boundary so that
                            // buf[scan_from..] never panics on multibyte UTF-8.
                            while scan_from > 0 && !buf.is_char_boundary(scan_from) {
                                scan_from -= 1;
                            }
                            // Guard against unbounded buffer growth. Yield the error and
                            // terminate the stream (state = None) so no further items follow.
                            if buf.len() > 1024 * 1024 {
                                return Some((Err(ClientError::SseFrameTooLarge), None));
                            }
                        }
                    }
                }
            },
        ))
    }
}

/// Find the method response matching `call_id` in `resp` and deserialize its
/// arguments into `T`.
///
/// Returns [`ClientError::MethodNotFound`] if no invocation with the given
/// call_id exists. Returns [`ClientError::MethodError`] if the matched
/// invocation is a JMAP `"error"` response.
pub(crate) fn extract_response<T: serde::de::DeserializeOwned>(
    resp: JmapResponse,
    call_id: &str,
) -> Result<T, ClientError> {
    let inv = resp
        .method_responses
        .into_iter()
        .find(|inv| inv.call_id == call_id)
        .ok_or_else(|| ClientError::MethodNotFound(call_id.to_string()))?;

    if inv.method == "error" {
        let err_type = inv.args
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("serverError")
            .to_string();
        let description = inv.args
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        return Err(ClientError::MethodError {
            error_type: err_type,
            description,
        });
    }

    serde_json::from_value(inv.args).map_err(|e| ClientError::Parse(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Oracle: `JmapChatClient` must implement `Clone` (compile-time check).
    /// The Arc<dyn AuthProvider> field and reqwest::Client are both Clone.
    #[test]
    fn client_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<JmapChatClient>();
    }

    fn session_fixture() -> serde_json::Value {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/jmap/session.json"),
        )
        .expect("cannot read session.json fixture");
        serde_json::from_str(&text).expect("session.json is not valid JSON")
    }

    /// Oracle: RFC 8620 §2 — fetch_session returns a Session with the fields
    /// from the hand-written fixture.
    #[tokio::test]
    async fn fetch_session_success() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/.well-known/jmap"))
            .respond_with(ResponseTemplate::new(200).set_body_json(session_fixture()))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let session = client
            .fetch_session()
            .await
            .expect("fetch_session must succeed");

        assert_eq!(session.username, "alice@example.com");
        assert_eq!(session.api_url, "https://jmap.example.com/api");
        assert_eq!(
            session.event_source_url,
            "https://jmap.example.com/eventsource/"
        );
        assert_eq!(session.state, "session-abc123");
        assert!(session.accounts.contains_key("account1"));
    }

    /// Oracle: RFC 8620 §2 — chat_account_id() returns the primary account for
    /// "urn:ietf:params:jmap:chat" from the fixture's primaryAccounts map.
    #[tokio::test]
    async fn fetch_session_chat_account_id() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/.well-known/jmap"))
            .respond_with(ResponseTemplate::new(200).set_body_json(session_fixture()))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let session = client
            .fetch_session()
            .await
            .expect("fetch_session must succeed");

        assert_eq!(session.chat_account_id(), Some("account1"));
    }

    /// Oracle: RFC 8620 §2 — HTTP 401 must surface as ClientError::AuthFailed(401).
    #[tokio::test]
    async fn fetch_session_401_returns_auth_failed() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/.well-known/jmap"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let err = client.fetch_session().await.expect_err("401 must fail");
        assert!(
            matches!(err, ClientError::AuthFailed(401)),
            "expected AuthFailed(401), got {err:?}"
        );
    }

    /// Oracle: RFC 8620 §2 — HTTP 403 must surface as ClientError::AuthFailed(403).
    #[tokio::test]
    async fn fetch_session_403_returns_auth_failed() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/.well-known/jmap"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let err = client.fetch_session().await.expect_err("403 must fail");
        assert!(
            matches!(err, ClientError::AuthFailed(403)),
            "expected AuthFailed(403), got {err:?}"
        );
    }

    /// Oracle: RFC 8620 §2 — HTTP 500 must surface as ClientError::Http (not AuthFailed).
    #[tokio::test]
    async fn fetch_session_500_returns_http_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/.well-known/jmap"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let err = client.fetch_session().await.expect_err("500 must fail");
        assert!(
            matches!(err, ClientError::Http(_)),
            "expected Http error, got {err:?}"
        );
    }

    /// Oracle: RFC 8620 §2 — a response body that is not valid JSON must surface
    /// as ClientError::Parse.
    #[tokio::test]
    async fn fetch_session_invalid_json_returns_parse_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/.well-known/jmap"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let err = client
            .fetch_session()
            .await
            .expect_err("bad JSON must fail");
        assert!(
            matches!(err, ClientError::Parse(_)),
            "expected Parse error, got {err:?}"
        );
    }

    /// Oracle: validation requirement — a Session with empty apiUrl must return
    /// ClientError::InvalidSession.
    #[tokio::test]
    async fn fetch_session_empty_api_url_returns_invalid_session() {
        let server = MockServer::start().await;

        let mut body = session_fixture();
        body["apiUrl"] = serde_json::Value::String(String::new());

        Mock::given(method("GET"))
            .and(path("/.well-known/jmap"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let err = client
            .fetch_session()
            .await
            .expect_err("empty apiUrl must fail");
        assert!(
            matches!(err, ClientError::InvalidSession(_)),
            "expected InvalidSession, got {err:?}"
        );
    }

    /// Oracle: validation requirement — a Session with empty eventSourceUrl must
    /// return ClientError::InvalidSession.
    #[tokio::test]
    async fn fetch_session_empty_event_source_url_returns_invalid_session() {
        let server = MockServer::start().await;

        let mut body = session_fixture();
        body["eventSourceUrl"] = serde_json::Value::String(String::new());

        Mock::given(method("GET"))
            .and(path("/.well-known/jmap"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let err = client
            .fetch_session()
            .await
            .expect_err("empty eventSourceUrl must fail");
        assert!(
            matches!(err, ClientError::InvalidSession(_)),
            "expected InvalidSession, got {err:?}"
        );
    }

    fn call_response_fixture() -> serde_json::Value {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/jmap/call_response.json"),
        )
        .expect("cannot read call_response.json fixture");
        serde_json::from_str(&text).expect("call_response.json is not valid JSON")
    }

    fn minimal_request() -> crate::jmap::JmapRequest {
        crate::jmap::JmapRequest {
            using: vec![
                "urn:ietf:params:jmap:core".to_string(),
                "urn:ietf:params:jmap:chat".to_string(),
            ],
            method_calls: vec![crate::jmap::Invocation::new(
                "Chat/get",
                serde_json::json!({"accountId": "account1", "ids": null}),
                "r1",
            )],
        }
    }

    /// Oracle: RFC 8620 §3.3/§3.4 — a successful POST to apiUrl returns a
    /// JmapResponse parsed from the hand-written call_response.json fixture.
    #[tokio::test]
    async fn call_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(200).set_body_json(call_response_fixture()))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let api_url = format!("{}/api", server.uri());
        let resp = client
            .call(&api_url, &minimal_request())
            .await
            .expect("call must succeed");

        assert_eq!(resp.method_responses[0].method, "Chat/get");
        assert_eq!(resp.session_state, "sess1");
    }

    /// Oracle: RFC 8620 §3.3 — HTTP 401 from apiUrl must surface as
    /// ClientError::AuthFailed(401).
    #[tokio::test]
    async fn call_401_returns_auth_failed() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let api_url = format!("{}/api", server.uri());
        let err = client
            .call(&api_url, &minimal_request())
            .await
            .expect_err("401 must fail");
        assert!(
            matches!(err, ClientError::AuthFailed(401)),
            "expected AuthFailed(401), got {err:?}"
        );
    }

    /// Oracle: RFC 8620 §3.3 — HTTP 403 from apiUrl must surface as
    /// ClientError::AuthFailed(403).
    #[tokio::test]
    async fn call_403_returns_auth_failed() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let api_url = format!("{}/api", server.uri());
        let err = client
            .call(&api_url, &minimal_request())
            .await
            .expect_err("403 must fail");
        assert!(
            matches!(err, ClientError::AuthFailed(403)),
            "expected AuthFailed(403), got {err:?}"
        );
    }

    /// Oracle: RFC 8620 §3.3 — HTTP 500 from apiUrl must surface as
    /// ClientError::Http (not AuthFailed).
    #[tokio::test]
    async fn call_500_returns_http_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let api_url = format!("{}/api", server.uri());
        let err = client
            .call(&api_url, &minimal_request())
            .await
            .expect_err("500 must fail");
        assert!(
            matches!(err, ClientError::Http(_)),
            "expected Http error, got {err:?}"
        );
    }

    /// Oracle: RFC 8620 §3.4 — a response body that is not valid JSON must
    /// surface as ClientError::Parse.
    #[tokio::test]
    async fn call_bad_json_returns_parse_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(200).set_body_string("bad json"))
            .mount(&server)
            .await;

        let client = JmapChatClient::new(crate::auth::NoneAuth, &server.uri())
            .expect("client construction must succeed");

        let api_url = format!("{}/api", server.uri());
        let err = client
            .call(&api_url, &minimal_request())
            .await
            .expect_err("bad JSON must fail");
        assert!(
            matches!(err, ClientError::Parse(_)),
            "expected Parse error, got {err:?}"
        );
    }

    /// Oracle: RFC 8620 §3.4 — extract_response finds the matching invocation
    /// by call_id and deserializes its arguments into the requested type.
    #[test]
    fn extract_response_success() {
        let resp = crate::jmap::JmapResponse {
            method_responses: vec![crate::jmap::Invocation::new(
                "Chat/get",
                serde_json::json!({"accountId": "account1", "state": "s1", "list": [], "notFound": []}),
                "r1",
            )],
            session_state: "sess1".to_string(),
            created_ids: None,
        };

        let val = super::extract_response::<serde_json::Value>(resp, "r1");
        assert!(val.is_ok(), "extract_response must succeed: {val:?}");
    }

    /// Oracle: RFC 8620 §3.4 — extract_response returns ClientError::MethodNotFound
    /// when no invocation with the given call_id exists in method_responses.
    #[test]
    fn extract_response_method_not_found() {
        let resp = crate::jmap::JmapResponse {
            method_responses: vec![crate::jmap::Invocation::new(
                "Chat/get",
                serde_json::json!({}),
                "r1",
            )],
            session_state: "sess1".to_string(),
            created_ids: None,
        };

        let err = super::extract_response::<serde_json::Value>(resp, "r99")
            .expect_err("wrong call_id must fail");
        assert!(
            matches!(err, ClientError::MethodNotFound(_)),
            "expected MethodNotFound, got {err:?}"
        );
    }

    /// Oracle: char-boundary invariant — when a 4-byte UTF-8 character is split
    /// across two chunks such that old_len.saturating_sub(3) lands inside the
    /// character, buf[scan_from..] would panic without the fix.
    ///
    /// "a😀" is bytes [0x61, 0xF0, 0x9F, 0x98, 0x80].  With old_len=5,
    /// saturating_sub(3)==2, which is byte 0x9F — not a char boundary.
    /// The fix must walk backward to byte 1 (the emoji start) so the slice is safe.
    #[test]
    fn sse_scan_from_char_boundary_fix() {
        // "a😀": 1 ASCII byte + 4-byte emoji = 5 bytes total.
        let mut buf = String::from("a");
        buf.push('😀');
        // Simulate: the previous chunk ended at byte 5 (inside the emoji).
        let old_len = 5usize;
        let naive = old_len.saturating_sub(3); // 2 — inside the emoji, not a boundary
        assert!(
            !buf.is_char_boundary(naive),
            "test setup: byte {naive} must not be a char boundary (confirms bug triggers)"
        );
        // Apply the fix (mirrors the production code exactly).
        let mut scan_from = naive;
        while scan_from > 0 && !buf.is_char_boundary(scan_from) {
            scan_from -= 1;
        }
        assert!(
            buf.is_char_boundary(scan_from),
            "after fix, scan_from={scan_from} must be a char boundary"
        );
        // Must not panic.
        let _slice = &buf[scan_from..];
        // The fix must land on byte 1 — the start of the emoji.
        assert_eq!(scan_from, 1, "fix must land on the emoji's start byte");
    }

    /// Oracle: base_url validation — a URL with a path component must be rejected.
    #[test]
    fn new_rejects_base_url_with_path() {
        let result = JmapChatClient::new(crate::auth::NoneAuth, "https://host/path")
            .map(|_| ());
        match result {
            Err(ClientError::InvalidArgument(_)) => {}
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// Oracle: base_url validation — an invalid URL string must be rejected.
    #[test]
    fn new_rejects_invalid_url() {
        let result = JmapChatClient::new(crate::auth::NoneAuth, "not a url")
            .map(|_| ());
        match result {
            Err(ClientError::InvalidArgument(_)) => {}
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// Oracle: base_url validation — a valid origin URL must be accepted.
    #[test]
    fn new_accepts_valid_base_url() {
        let result = JmapChatClient::new(crate::auth::NoneAuth, "https://example.com");
        assert!(result.is_ok(), "valid base_url must be accepted");
    }

    /// Oracle: base_url validation — an IP:port URL (as documented in the constructor) must be accepted.
    #[test]
    fn new_accepts_ip_port_base_url() {
        let result = JmapChatClient::new(crate::auth::NoneAuth, "https://100.64.1.1:8008")
            .map(|_| ());
        assert!(result.is_ok(), "IP:port base_url must be accepted: {result:?}");
    }

    /// Oracle: base_url validation — an IPv6 literal URL must be accepted.
    #[test]
    fn new_accepts_ipv6_base_url() {
        let result = JmapChatClient::new(crate::auth::NoneAuth, "https://[::1]:8008")
            .map(|_| ());
        assert!(result.is_ok(), "IPv6 base_url must be accepted: {result:?}");
    }

    /// Oracle: base_url validation — a URL with a query string must be rejected.
    #[test]
    fn new_rejects_base_url_with_query() {
        let err = JmapChatClient::new(crate::auth::NoneAuth, "https://host?foo=1")
            .map(|_| ())
            .expect_err("base_url with query must be rejected");
        assert!(
            matches!(err, ClientError::InvalidArgument(_)),
            "expected InvalidArgument, got {err:?}"
        );
    }

    /// Oracle: base_url validation — a URL with a fragment must be rejected.
    #[test]
    fn new_rejects_base_url_with_fragment() {
        let err = JmapChatClient::new(crate::auth::NoneAuth, "https://host#anchor")
            .map(|_| ())
            .expect_err("base_url with fragment must be rejected");
        assert!(
            matches!(err, ClientError::InvalidArgument(_)),
            "expected InvalidArgument, got {err:?}"
        );
    }

    /// Oracle: RFC 8620 §3.6.1 — when an invocation has method name "error",
    /// extract_response returns ClientError::MethodError with the type and
    /// description from the error arguments.
    #[test]
    fn extract_response_method_error() {
        let resp = crate::jmap::JmapResponse {
            method_responses: vec![crate::jmap::Invocation::new(
                "error",
                serde_json::json!({"type": "serverFail", "description": "oops"}),
                "r1",
            )],
            session_state: "sess1".to_string(),
            created_ids: None,
        };

        let err = super::extract_response::<serde_json::Value>(resp, "r1")
            .expect_err("error invocation must fail");
        assert!(
            matches!(
                &err,
                ClientError::MethodError { error_type, description }
                    if error_type == "serverFail" && description == "oops"
            ),
            "expected MethodError{{serverFail, oops}}, got {err:?}"
        );
    }
}
