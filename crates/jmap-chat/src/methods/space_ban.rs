use super::{ChangesResponse, GetResponse, SetResponse, SpaceBanCreateInput};

impl super::SessionClient<'_> {
    /// Fetch SpaceBan objects by IDs (JMAP Chat §4.18 SpaceBan/get).
    ///
    /// If `ids` is `None`, returns all SpaceBan objects for the account.
    /// Pass `properties: None` to return all fields.
    pub async fn space_ban_get(
        &self,
        ids: Option<&[&str]>,
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::SpaceBan>, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
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
        since_state: &str,
        max_changes: Option<u64>,
    ) -> Result<ChangesResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let mut args = serde_json::json!({
            "accountId": account_id,
            "sinceState": since_state,
        });
        if let Some(mc) = max_changes {
            args["maxChanges"] = mc.into();
        }
        let (call_id, req) = super::build_request("SpaceBan/changes", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Create a SpaceBan (RFC 8620 §5.3 / SpaceBan/set create).
    ///
    /// When `input.client_id` is `None`, a ULID is generated automatically.
    pub async fn space_ban_create(
        &self,
        input: &SpaceBanCreateInput<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
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
        let mut buf = String::new();
        let client_id = super::resolve_client_id(input.client_id, &mut buf);
        let args = serde_json::json!({
            "accountId": account_id,
            "create": { client_id: create_obj },
        });
        let (call_id, req) = super::build_request("SpaceBan/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Destroy SpaceBan objects (RFC 8620 §5.3 / SpaceBan/set destroy).
    ///
    /// `ids` must be non-empty; the guard fires before any network call.
    pub async fn space_ban_destroy(
        &self,
        ids: &[&str],
    ) -> Result<SetResponse, crate::error::ClientError> {
        if ids.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "space_ban_destroy: ids may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "destroy": ids,
        });
        let (call_id, req) = super::build_request("SpaceBan/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }
}
