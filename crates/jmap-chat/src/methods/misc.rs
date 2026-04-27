use super::{GetResponse, SetResponse};

impl crate::client::JmapChatClient {
    /// Fetch ReadPosition objects by IDs (JMAP Chat §5 ReadPosition/get).
    ///
    /// If `ids` is `None`, returns all ReadPosition records for the account.
    /// The server creates one ReadPosition per Chat automatically.
    pub async fn read_position_get(
        &self,
        session: &crate::jmap::Session,
        ids: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::ReadPosition>, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
        });
        let (call_id, req) = super::build_request("ReadPosition/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Update the read position for a Chat (JMAP Chat §5 ReadPosition/set).
    ///
    /// `read_position_id` is the server-assigned ReadPosition.id (from
    /// `read_position_get`). `last_read_message_id` is the Message.id of the
    /// most recent message read. The server updates `lastReadAt` and
    /// recomputes `Chat.unreadCount`.
    ///
    /// `create` and `destroy` are forbidden by the spec; only `update` is issued.
    pub async fn read_position_set(
        &self,
        session: &crate::jmap::Session,
        read_position_id: &str,
        last_read_message_id: &str,
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "update": {
                read_position_id: { "lastReadMessageId": last_read_message_id }
            },
        });
        let (call_id, req) = super::build_request("ReadPosition/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Fetch the singleton PresenceStatus record (JMAP Chat §5 PresenceStatus/get).
    ///
    /// Per spec there is exactly one PresenceStatus per account; `ids: null`
    /// retrieves it.
    pub async fn presence_status_get(
        &self,
        session: &crate::jmap::Session,
    ) -> Result<GetResponse<crate::types::PresenceStatus>, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": None::<&[&str]>,
        });
        let (call_id, req) = super::build_request("PresenceStatus/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }
}
