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

impl super::SessionClient<'_> {
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
        blob_ids: &[&str],
        type_names: Option<&[&str]>,
    ) -> Result<BlobLookupResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
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

/// Response to a Blob/convert call (JMAP-BLOBEXT §7).
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobConvertResponse {
    /// Account the conversion was run against.
    pub account_id: String,
    /// blobId of the converted output blob.
    pub blob_id: String,
    /// MIME type of the output blob.
    #[serde(rename = "type")]
    pub content_type: String,
}

impl super::SessionClient<'_> {
    /// Convert a blob to a different MIME type (JMAP-BLOBEXT §7 / blob2 capability).
    ///
    /// Typical use: request a thumbnail (`image/webp`) from an image blob without
    /// downloading the original. The server MUST advertise
    /// `urn:ietf:params:jmap:blob2` in Session capabilities.
    ///
    /// `width` and `height` are optional hint dimensions; the server may ignore
    /// or clamp them. Pass `None` to omit both.
    pub async fn blob_convert(
        &self,
        from_blob_id: &str,
        content_type: &str,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<BlobConvertResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let mut args = serde_json::json!({
            "accountId": account_id,
            "fromBlobId": from_blob_id,
            "type": content_type,
        });
        if let Some(w) = width {
            args["width"] = w.into();
        }
        if let Some(h) = height {
            args["height"] = h.into();
        }
        let req = crate::jmap::JmapRequest {
            using: vec![
                "urn:ietf:params:jmap:core".to_string(),
                "urn:ietf:params:jmap:blob2".to_string(),
            ],
            method_calls: vec![("Blob/convert".to_string(), args, super::CALL_ID.to_string())],
        };
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }
}
