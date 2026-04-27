use super::{ChangesResponse, GetResponse, SetResponse, SpaceBanCreateInput};

impl crate::client::JmapChatClient {
    /// Fetch SpaceBan objects by IDs (JMAP Chat §4.18 SpaceBan/get).
    ///
    /// If `ids` is `None`, returns all SpaceBan objects for the account.
    /// Pass `properties: None` to return all fields.
    pub async fn space_ban_get(
        &self,
        session: &crate::jmap::Session,
        ids: Option<&[&str]>,
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::SpaceBan>, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
            "properties": properties,
        });
        let (call_id, req) = super::build_request("SpaceBan/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Fetch changes to SpaceBan objects since `since_state` (RFC 8620 §5.2 / SpaceBan/changes).
    ///
    /// Only members with `"ban"` permission in the Space see all changes;
    /// other members see changes to their own bans only.
    pub async fn space_ban_changes(
        &self,
        session: &crate::jmap::Session,
        since_state: &str,
        max_changes: Option<u64>,
    ) -> Result<ChangesResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "sinceState": since_state,
            "maxChanges": max_changes,
        });
        let (call_id, req) = super::build_request("SpaceBan/changes", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Create or destroy SpaceBan objects (RFC 8620 §5.3 / SpaceBan/set).
    ///
    /// `input` describes a single ban to create. `destroy` is a list of
    /// existing SpaceBan IDs to delete.
    pub async fn space_ban_set(
        &self,
        session: &crate::jmap::Session,
        input: &SpaceBanCreateInput<'_>,
        destroy: &[&str],
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let mut create_obj = serde_json::json!({
            "spaceId": input.space_id,
            "userId": input.user_id,
        });
        if let Some(r) = input.reason {
            create_obj["reason"] = r.into();
        }
        if let Some(ea) = input.expires_at {
            create_obj["expiresAt"] = ea.as_str().into();
        }
        let args = serde_json::json!({
            "accountId": account_id,
            "create": { input.client_id: create_obj },
            "destroy": destroy,
        });
        let (call_id, req) = super::build_request("SpaceBan/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }
}
