use super::{ChangesResponse, GetResponse, SetResponse, SpaceInviteCreateInput};

impl crate::client::JmapChatClient {
    /// Fetch SpaceInvite objects by IDs (JMAP Chat §4.17 SpaceInvite/get).
    ///
    /// If `ids` is `None`, returns all SpaceInvite objects for the account.
    /// Pass `properties: None` to return all fields.
    pub async fn space_invite_get(
        &self,
        session: &crate::jmap::Session,
        ids: Option<&[&str]>,
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::SpaceInvite>, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
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
        let (call_id, req) = super::build_request("SpaceInvite/changes", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Create or destroy SpaceInvite objects (RFC 8620 §5.3 / SpaceInvite/set).
    ///
    /// `input` describes a single invite to create; pass `None` to skip creation
    /// (destroy-only call). `destroy` is a list of existing SpaceInvite IDs to delete.
    ///
    /// When `input` is `None` the `create` key is omitted entirely from the request,
    /// satisfying RFC 8620 §5.3 which requires `create` to be absent or an object.
    pub async fn space_invite_set(
        &self,
        session: &crate::jmap::Session,
        input: Option<&SpaceInviteCreateInput<'_>>,
        destroy: &[&str],
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let mut args = serde_json::json!({
            "accountId": account_id,
            "destroy": destroy,
        });
        if let Some(inp) = input {
            let mut obj = serde_json::json!({ "spaceId": inp.space_id });
            if let Some(ch) = inp.default_channel_id {
                obj["defaultChannelId"] = ch.into();
            }
            if let Some(ea) = inp.expires_at {
                obj["expiresAt"] = ea.as_str().into();
            }
            if let Some(mu) = inp.max_uses {
                obj["maxUses"] = mu.into();
            }
            args["create"] = serde_json::json!({ inp.client_id: obj });
        }
        let (call_id, req) = super::build_request("SpaceInvite/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }
}
