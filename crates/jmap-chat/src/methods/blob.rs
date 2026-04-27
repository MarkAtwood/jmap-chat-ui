// Blob/lookup JMAP method (urn:ietf:params:jmap:blob2 capability)
// Spec: draft-ietf-jmap-blobext-01 §6

use std::collections::HashMap;

use serde::Deserialize;

/// A single entry in a Blob/lookup response.
/// Spec: draft-ietf-jmap-blobext-01 §6
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobLookupEntry {
    /// The blobId that was queried.
    pub id: String,
    /// Per-type reverse lookup: keys are data type names (e.g. `"Message"`),
    /// values are object IDs that reference this blob.
    pub matched_ids: HashMap<String, Vec<String>>,
}

/// Response to a Blob/lookup call.
/// Spec: draft-ietf-jmap-blobext-01 §6
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobLookupResponse {
    /// Account the query was run against.
    pub account_id: String,
    /// One entry per queried blobId.
    pub list: Vec<BlobLookupEntry>,
    /// blobIds that were not found or not accessible (access-control safe).
    /// An absent field and an empty array are semantically identical.
    #[serde(default)]
    pub not_found: Vec<String>,
}

impl crate::client::JmapChatClient {
    /// Reverse-lookup blobs: given a list of blob IDs and data type names,
    /// returns which objects of those types reference each blob.
    ///
    /// Uses capability `urn:ietf:params:jmap:blob2`; the server MUST advertise
    /// it in the Session for this method to succeed (RFC 8620 §3.3).
    ///
    /// `type_names` filters which data types to search. `None` queries all
    /// types registered on the server. For JMAP Chat, `"Message"` is the
    /// expected type.
    ///
    /// Security: blobs that are inaccessible or nonexistent are returned with
    /// empty `matchedIds` arrays rather than an error (draft-ietf-jmap-blobext
    /// §6), to avoid information leakage.
    pub async fn blob_lookup(
        &self,
        session: &crate::jmap::Session,
        blob_ids: &[&str],
        type_names: Option<&[&str]>,
    ) -> Result<BlobLookupResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": blob_ids,
            "typeNames": type_names,
        });
        let req = crate::jmap::JmapRequest {
            using: vec![
                "urn:ietf:params:jmap:core".to_string(),
                "urn:ietf:params:jmap:blob2".to_string(),
            ],
            method_calls: vec![("Blob/lookup".to_string(), args, super::CALL_ID.to_string())],
        };
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }
}
