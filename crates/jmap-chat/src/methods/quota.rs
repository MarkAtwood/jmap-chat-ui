// Quota/get — RFC 8621 (urn:ietf:params:jmap:quotas)
//
// Retrieves storage quota information from the server.  Only call when
// `Session::supports_quotas()` returns true.

use serde::Deserialize;

/// A single JMAP Quota object (RFC 8621 §2).
///
/// Describes a storage limit that applies to one or more data types within
/// a given scope.  Poll with [`JmapChatClient::quota_get`] to display storage
/// usage in the UI and warn the user when approaching limits.
///
/// Spec: RFC 8621 §2
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    /// Server-assigned identifier.
    pub id: String,
    /// Human-readable name for this quota (e.g. "Message Storage").
    pub name: String,
    /// Scope of the quota: `"account"`, `"domain"`, or `"global"`.
    pub scope: String,
    /// Data type names covered by this quota (e.g. `["Message", "Chat"]`).
    pub data_types: Vec<String>,
    /// Bytes currently consumed.
    pub used: u64,
    /// Hard limit in bytes; requests that would exceed this MUST fail.
    pub hard_limit: u64,
    /// Warning threshold in bytes; clients SHOULD warn the user above this.
    #[serde(default)]
    pub warn_limit: Option<u64>,
    /// Soft limit in bytes (server may begin rejecting requests above this).
    #[serde(default)]
    pub soft_limit: Option<u64>,
    /// Optional human-readable description.
    #[serde(default)]
    pub description: Option<String>,
}

impl super::SessionClient<'_> {
    /// Fetch all Quota objects for the account (RFC 8621 §2.1 Quota/get).
    ///
    /// Returns all quota records for the primary JMAP Chat account.  Each
    /// [`Quota`] includes `used`, `hard_limit`, and optional `warn_limit` fields
    /// that callers can use to display storage bars and warnings.
    ///
    /// Only call when [`Session::supports_quotas`](crate::jmap::Session::supports_quotas)
    /// returns `true`.  Returns `ClientError::InvalidSession` if the session has
    /// no primary JMAP Chat account.
    ///
    /// Spec: RFC 8621 §2
    pub async fn quota_get(&self) -> Result<Vec<Quota>, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": serde_json::Value::Null,
        });
        let req = crate::jmap::JmapRequestBuilder::new(vec![
            "urn:ietf:params:jmap:core".to_string(),
            "urn:ietf:params:jmap:quotas".to_string(),
        ])
        .add_call("Quota/get", args, "r1")
        .build();

        let resp = self.call(api_url, &req).await?;
        let get_resp = crate::client::extract_response::<QuotaGetResponse>(resp, "r1")?;
        Ok(get_resp.list)
    }
}

/// Wire shape for the `Quota/get` method response.
#[derive(Debug, Deserialize)]
struct QuotaGetResponse {
    list: Vec<Quota>,
}
