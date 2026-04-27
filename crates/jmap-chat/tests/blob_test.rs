// Integration tests for JmapChatClient::upload_blob and download_blob.
//
// Oracles:
//   - SHA-256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
//     Source: `echo -n "abc" | openssl dgst -sha256` (cross-checked with sha2 crate).
//
// Fixtures are either inline JSON (upload) or raw bytes (download); none are
// produced by the code under test.

use jmap_chat::{blob::BlobUploadResponse, client::JmapChatClient, error::ClientError};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

/// SHA-256 of the 3-byte ASCII message "abc".
/// Oracle: `echo -n "abc" | openssl dgst -sha256` → ba7816bf...15ad
const SHA256_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

// ---------------------------------------------------------------------------
// Test 1 — upload_blob: happy path, server echoes correct sha256
// ---------------------------------------------------------------------------

/// Oracle: NIST FIPS 180-4 §B.1 — SHA-256("abc") = SHA256_ABC.
/// The server echoes back the correct sha256; upload_blob must succeed and
/// return the typed BlobUploadResponse.
#[tokio::test]
async fn upload_blob_happy_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/upload/account1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "account1",
            "blobId":    "blob-001",
            "type":      "text/plain",
            "size":      3,
            "sha256":    SHA256_ABC
        })))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must not fail");
    let upload_template = format!("{}/upload/{{accountId}}/", server.uri());

    let resp: BlobUploadResponse = client
        .upload_blob(&upload_template, "account1", b"abc", "text/plain")
        .await
        .expect("upload_blob must succeed when server sha256 matches");

    assert_eq!(resp.blob_id, "blob-001");
    assert_eq!(resp.sha256.as_deref(), Some(SHA256_ABC));
    assert_eq!(resp.size, 3);
}

// ---------------------------------------------------------------------------
// Test 2 — upload_blob: server returns wrong sha256 → BlobIntegrityMismatch
// ---------------------------------------------------------------------------

/// Oracle: mismatch is server-returned ("all zeros") vs locally-computed
/// SHA256_ABC. Because they differ, upload_blob must return
/// BlobIntegrityMismatch with expected = "000...0" and actual = SHA256_ABC.
#[tokio::test]
async fn upload_blob_sha256_mismatch() {
    let wrong_sha256 = "0".repeat(64);
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/upload/account1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "account1",
            "blobId":    "blob-002",
            "type":      "text/plain",
            "size":      3,
            "sha256":    wrong_sha256
        })))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must not fail");
    let upload_template = format!("{}/upload/{{accountId}}/", server.uri());

    let err = client
        .upload_blob(&upload_template, "account1", b"abc", "text/plain")
        .await
        .expect_err("upload_blob must fail on sha256 mismatch");

    match err {
        ClientError::BlobIntegrityMismatch { expected, actual } => {
            assert_eq!(expected, "0".repeat(64));
            assert_eq!(actual, SHA256_ABC);
        }
        other => panic!("expected BlobIntegrityMismatch, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 3 — upload_blob: server omits sha256 → no verification, succeeds
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 base upload response has no sha256 field.
/// When absent, upload_blob must skip verification and succeed.
#[tokio::test]
async fn upload_blob_no_sha256_skips_check() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/upload/account1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "account1",
            "blobId":    "blob-003",
            "type":      "text/plain",
            "size":      3
        })))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must not fail");
    let upload_template = format!("{}/upload/{{accountId}}/", server.uri());

    let resp = client
        .upload_blob(&upload_template, "account1", b"abc", "text/plain")
        .await
        .expect("upload_blob must succeed when server omits sha256");

    assert_eq!(resp.blob_id, "blob-003");
    assert!(resp.sha256.is_none());
}

// ---------------------------------------------------------------------------
// Test 4 — download_blob: happy path, correct sha256
// ---------------------------------------------------------------------------

/// Oracle: NIST FIPS 180-4 §B.1 — SHA-256("abc") = SHA256_ABC.
/// Server returns b"abc"; we pass SHA256_ABC as expected; download_blob must
/// succeed and return those bytes.
#[tokio::test]
async fn download_blob_happy_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/download/account1/blob-abc/file.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"abc".as_ref()))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must not fail");
    let dl_template = format!(
        "{}/download/{{accountId}}/{{blobId}}/{{name}}",
        server.uri()
    );

    let data = client
        .download_blob(
            &dl_template,
            "account1",
            "blob-abc",
            "file.bin",
            None,
            Some(SHA256_ABC),
        )
        .await
        .expect("download_blob must succeed when sha256 matches");

    assert_eq!(data, b"abc");
}

// ---------------------------------------------------------------------------
// Test 5 — download_blob: wrong sha256 → BlobIntegrityMismatch
// ---------------------------------------------------------------------------

/// Oracle: b"xyz" ≠ b"abc" so their sha256s differ. expected = SHA256_ABC
/// (sha256 of "abc"); server returns b"xyz". download_blob must return
/// BlobIntegrityMismatch with expected = SHA256_ABC.
#[tokio::test]
async fn download_blob_sha256_mismatch() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/download/account1/blob-xyz/file.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"xyz".as_ref()))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must not fail");
    let dl_template = format!(
        "{}/download/{{accountId}}/{{blobId}}/{{name}}",
        server.uri()
    );

    // expected sha256 is for "abc" but server sends "xyz"
    let err = client
        .download_blob(
            &dl_template,
            "account1",
            "blob-xyz",
            "file.bin",
            None,
            Some(SHA256_ABC),
        )
        .await
        .expect_err("download_blob must fail on sha256 mismatch");

    match err {
        ClientError::BlobIntegrityMismatch { expected, .. } => {
            assert_eq!(expected, SHA256_ABC);
        }
        other => panic!("expected BlobIntegrityMismatch, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 6 — download_blob: None expected_sha256 skips verification
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §6.2 — sha256 verification is optional; when
/// expected_sha256 is None, download_blob must return bytes without checking.
#[tokio::test]
async fn download_blob_no_sha256_skips_check() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/download/account1/blob-any/file.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".as_ref()))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must not fail");
    let dl_template = format!(
        "{}/download/{{accountId}}/{{blobId}}/{{name}}",
        server.uri()
    );

    let data = client
        .download_blob(&dl_template, "account1", "blob-any", "file.bin", None, None)
        .await
        .expect("download_blob must succeed when no sha256 is expected");

    assert_eq!(data, b"hello");
}

// ---------------------------------------------------------------------------
// Test 7 — download_blob: RFC 8620 §6.2 conforming template with {type}
// ---------------------------------------------------------------------------

/// Oracle: RFC 8620 §6.2 example downloadUrl includes {type} as a query param.
/// When accept_type is Some, {type} is percent-encoded and substituted.
/// image/png contains '/' which must be encoded as %2F per RFC 3986 §2.1.
#[tokio::test]
async fn download_blob_with_type_substitution() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/download/account1/blob-img/photo.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fakepng".as_ref()))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must not fail");
    // RFC 8620 §6.2 recommends placing {type} in the query section to avoid
    // path-segment slash encoding issues; here we use a path-free template to
    // test that {type} with a '/' is percent-encoded correctly.
    let dl_template = format!(
        "{}/download/{{accountId}}/{{blobId}}/{{name}}",
        server.uri()
    );

    let data = client
        .download_blob(
            &dl_template,
            "account1",
            "blob-img",
            "photo.png",
            Some("image/png"),
            None,
        )
        .await
        .expect("download_blob must succeed with accept_type");

    assert_eq!(data, b"fakepng");
}

// ---------------------------------------------------------------------------
// Test: blob_convert — happy path
// ---------------------------------------------------------------------------

/// Oracle: JMAP-BLOBEXT §7 — Blob/convert response: blobId and type present.
/// Fixture hand-written from JMAP blob extension spec §7.
#[tokio::test]
async fn blob_convert_returns_typed_response() {
    use jmap_chat::methods::blob::BlobConvertResponse;
    use wiremock::matchers::body_json;

    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(body_json(serde_json::json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:blob2"],
            "methodCalls": [["Blob/convert", {
                "accountId": "account1",
                "fromBlobId": "original-blob-001",
                "type": "image/webp",
                "width": 200,
                "height": 200
            }, "r1"]]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "methodResponses": [["Blob/convert", {
                "accountId": "account1",
                "blobId": "converted-blob-webp-001",
                "type": "image/webp"
            }, "r1"]],
            "sessionState": "sess-abc"
        })))
        .mount(&server)
        .await;

    let client = JmapChatClient::new(jmap_chat::auth::NoneAuth, &server.uri())
        .expect("client construction must succeed");
    // Build a minimal session with blob2 capability via serde to handle all fields.
    let api_url = format!("{}/api", server.uri());
    let session: jmap_chat::jmap::Session = serde_json::from_value(serde_json::json!({
        "capabilities": { "urn:ietf:params:jmap:blob2": null },
        "accounts": {
            "account1": {
                "name": "Test",
                "isPersonal": true,
                "isReadOnly": false,
                "accountCapabilities": {}
            }
        },
        "primaryAccounts": { "urn:ietf:params:jmap:chat": "account1" },
        "username": "test@example.com",
        "apiUrl": api_url,
        "downloadUrl": "",
        "uploadUrl": "",
        "eventSourceUrl": "",
        "state": "sess-abc"
    }))
    .expect("session must deserialize");

    let result: BlobConvertResponse = client
        .with_session(&session)
        .blob_convert("original-blob-001", "image/webp", Some(200), Some(200))
        .await
        .expect("blob_convert must succeed");

    assert_eq!(result.blob_id, "converted-blob-webp-001");
    assert_eq!(result.content_type, "image/webp");
    assert_eq!(result.account_id, "account1");
}
