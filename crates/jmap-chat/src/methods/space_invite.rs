use super::{ChangesResponse, GetResponse, SetResponse, SpaceInviteCreateInput};

impl super::SessionClient<'_> {
    /// Fetch SpaceInvite objects by IDs (JMAP Chat §4.17 SpaceInvite/get).
    ///
    /// If `ids` is `None`, returns all SpaceInvite objects for the account.
    /// Pass `properties: None` to return all fields.
    pub async fn space_invite_get(
        &self,
        ids: Option<&[&str]>,
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::SpaceInvite>, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
            "properties": properties,
        });
        let (call_id, req) = super::build_request("SpaceInvite/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Fetch changes to SpaceInvite objects since `since_state` (RFC 8620 §5.2 / SpaceInvite/changes).
    ///
    /// If `has_more_changes` is true in the response, call again with `new_state`
    /// as `since_state` until the flag is false.
    pub async fn space_invite_changes(
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
        let (call_id, req) = super::build_request("SpaceInvite/changes", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Create a SpaceInvite (RFC 8620 §5.3 / SpaceInvite/set create).
    ///
    /// When `input.client_id` is `None`, a ULID is generated automatically.
    pub async fn space_invite_create(
        &self,
        input: &SpaceInviteCreateInput<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let mut obj = serde_json::json!({ "spaceId": input.space_id });
        if let Some(ch) = input.default_channel_id {
            obj["defaultChannelId"] = ch.into();
        }
        if let Some(ea) = input.expires_at {
            obj["expiresAt"] = ea.as_str().into();
        }
        if let Some(mu) = input.max_uses {
            obj["maxUses"] = mu.into();
        }
        let mut buf = String::new();
        let client_id = super::resolve_client_id(input.client_id, &mut buf);
        let args = serde_json::json!({
            "accountId": account_id,
            "create": { client_id: obj },
        });
        let (call_id, req) = super::build_request("SpaceInvite/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Destroy SpaceInvite objects (RFC 8620 §5.3 / SpaceInvite/set destroy).
    ///
    /// `ids` must be non-empty; the guard fires before any network call.
    pub async fn space_invite_destroy(
        &self,
        ids: &[&str],
    ) -> Result<SetResponse, crate::error::ClientError> {
        if ids.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "space_invite_destroy: ids may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "destroy": ids,
        });
        let (call_id, req) = super::build_request("SpaceInvite/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }
}
