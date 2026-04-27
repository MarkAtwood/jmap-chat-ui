// Typed JMAP Chat method wrappers (Step 8)
//
// Response types mirror RFC 8620 standard shapes (§5.1 /get, §5.5 /query,
// §5.2 /changes, §5.3 /set). Method implementations live on JmapChatClient
// and are the primary public API for callers that already hold a Session.

use std::collections::HashMap;

use serde::Deserialize;

use crate::jmap::Id;

pub mod blob;
pub mod chat;
pub mod contact;
pub mod custom_emoji;
pub mod message;
pub mod misc;
pub mod quota;
pub mod space;
pub mod space_ban;
pub mod space_invite;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// RFC 8620 §5.1 — /get response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetResponse<T> {
    pub account_id: Id,
    pub state: String,
    pub list: Vec<T>,
    pub not_found: Option<Vec<Id>>,
}

/// RFC 8620 §5.5 — /query response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResponse {
    pub account_id: Id,
    pub query_state: String,
    pub can_calculate_changes: bool,
    pub position: u64,
    pub ids: Vec<Id>,
    pub total: Option<u64>,
    pub limit: Option<u64>,
}

/// RFC 8620 §5.2 — /changes response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesResponse {
    pub account_id: Id,
    pub old_state: String,
    pub new_state: String,
    pub has_more_changes: bool,
    pub created: Vec<Id>,
    pub updated: Vec<Id>,
    pub destroyed: Vec<Id>,
}

/// RFC 8620 §5.3 — /set response.
///
/// Used for create (`message_create`, `custom_emoji_create`, `space_ban_create`,
/// `space_invite_create`), update (`read_position_update`, `presence_status_update`),
/// and destroy operations. All optional maps are `None` when absent in the
/// server response.
///
/// The type parameter `T` is the shape of each created/updated object.
/// Defaults to `serde_json::Value` so callers that don't need typed objects
/// can use `SetResponse` without a type argument.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "T: serde::de::DeserializeOwned"))]
pub struct SetResponse<T = serde_json::Value> {
    pub account_id: Id,
    pub old_state: Option<String>,
    pub new_state: String,
    /// Keys are caller-supplied creation keys (not server Ids); see RFC 8620 §5.3.
    pub created: Option<HashMap<String, T>>,
    /// Keys are server-assigned object Ids; see RFC 8620 §5.3.
    pub updated: Option<HashMap<String, T>>,
    pub destroyed: Option<Vec<Id>>,
    /// Keys are caller-supplied creation keys (not server Ids); see RFC 8620 §5.3.
    pub not_created: Option<HashMap<String, SetError>>,
    /// Keys are server-assigned object Ids; see RFC 8620 §5.3.
    pub not_updated: Option<HashMap<String, SetError>>,
    /// Keys are server-assigned object Ids; see RFC 8620 §5.3.
    pub not_destroyed: Option<HashMap<String, SetError>>,
}

/// Response to [`JmapChatClient::push_subscription_create`] (RFC 8620 §7.2).
///
/// `account_id` is always `null` for PushSubscription objects (they are not
/// account-scoped). `Option<String>` handles both the null case and servers
/// that echo the session accountId anyway.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushSubscriptionCreateResponse {
    #[serde(default)]
    pub account_id: Option<Id>,
    pub created: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub not_created: Option<HashMap<String, SetError>>,
}

/// A /set operation failure for a single object (RFC 8620 §5.3).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub description: Option<String>,
    /// Present only when `error_type == "rateLimited"` (JMAP Chat slow-mode).
    /// Callers should wait until this time before retrying.
    pub server_retry_after: Option<crate::jmap::UTCDate>,
}

impl std::fmt::Display for SetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.description {
            Some(desc) => write!(f, "{}: {}", self.error_type, desc),
            None => write!(f, "{}", self.error_type),
        }
    }
}

/// RFC 8620 §5.6 — /queryChanges response.
///
/// Reports which IDs were removed from and added to a query result set since
/// `old_query_state`. Used by `custom_emoji_query_changes` and any future
/// /queryChanges implementations.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryChangesResponse {
    pub account_id: Id,
    pub old_query_state: String,
    pub new_query_state: String,
    pub total: Option<u64>,
    pub removed: Vec<Id>,
    pub added: Vec<AddedItem>,
}

/// A single item added to a query result set (RFC 8620 §5.6).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedItem {
    pub id: Id,
    pub index: u64,
}

/// Response to a [`JmapChatClient::chat_typing`] call (JMAP Chat §Chat/typing).
///
/// The server echoes only `accountId`. No state token or object list is returned.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingResponse {
    pub account_id: Id,
}

// ---------------------------------------------------------------------------
// Patch<T>: three-way update value for nullable fields
// ---------------------------------------------------------------------------

/// Three-way patch value for nullable JMAP fields.
///
/// - `Keep` (default): the field is omitted from the patch — server leaves it unchanged.
/// - `Set(v)`: the field is included with value `v`.
/// - `Clear`: the field is included as JSON `null` (clears the server-side value).
///
/// Use `Patch::from(v)` or `.into()` to construct `Set(v)`. Use `Default::default()`
/// or `Patch::Keep` to leave the field unchanged.
///
/// # Serde usage
///
/// Fields of type `Patch<T>` **must** carry both attributes:
/// ```ignore
/// #[serde(default, skip_serializing_if = "Patch::is_keep")]
/// pub my_field: Patch<String>,
/// ```
/// - `default`: absent JSON key → `Patch::Keep` (no change).
/// - `skip_serializing_if`: omits the key from the output when the value is `Keep`.
///
/// Without `skip_serializing_if`, `Patch::Keep` serializes as `null`, which is
/// indistinguishable from `Patch::Clear` on the wire and will clear the field.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum Patch<T> {
    #[default]
    Keep,
    Set(T),
    Clear,
}

impl<T> Patch<T> {
    /// Returns `true` if this is `Patch::Keep` (field should be omitted from serialization).
    pub fn is_keep(&self) -> bool {
        matches!(self, Patch::Keep)
    }
}

impl<T> From<T> for Patch<T> {
    fn from(v: T) -> Self {
        Patch::Set(v)
    }
}

impl<T> From<Option<T>> for Patch<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => Patch::Set(v),
            None => Patch::Keep,
        }
    }
}

impl<T: serde::Serialize> Patch<T> {
    /// Returns `None` when `Keep` (omit key from patch),
    /// `Some(Value::Null)` when `Clear`, or `Some(serialized_value)` when `Set`.
    pub fn map_entry(&self) -> Result<Option<serde_json::Value>, serde_json::Error> {
        match self {
            Patch::Keep => Ok(None),
            Patch::Clear => Ok(Some(serde_json::Value::Null)),
            Patch::Set(v) => serde_json::to_value(v).map(Some),
        }
    }
}

impl<T: serde::Serialize> serde::Serialize for Patch<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Patch::Keep => {
                // Keep must never reach the serializer — see doc-level serde usage note.
                debug_assert!(
                    false,
                    "Patch::Keep must not be serialized directly; \
                     add #[serde(skip_serializing_if = \"Patch::is_keep\")] to the field"
                );
                s.serialize_none()
            }
            Patch::Clear => s.serialize_none(),
            Patch::Set(v) => v.serialize(s),
        }
    }
}

impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Patch<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // JSON absent (via #[serde(default)]) → Keep (default).
        // JSON null → Clear. JSON value → Set(v).
        Option::<T>::deserialize(d).map(|opt| match opt {
            None => Patch::Clear,
            Some(v) => Patch::Set(v),
        })
    }
}

// ---------------------------------------------------------------------------
// Input types for methods with many optional parameters
// ---------------------------------------------------------------------------

/// Input parameters for [`JmapChatClient::chat_query`].
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct ChatQueryInput {
    pub filter_kind: Option<crate::types::ChatKind>,
    pub filter_muted: Option<bool>,
    pub position: Option<u64>,
    pub limit: Option<u64>,
}

/// Input parameters for [`JmapChatClient::message_query`].
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct MessageQueryInput<'a> {
    pub chat_id: Option<&'a str>,
    pub has_mention: Option<bool>,
    pub has_attachment: Option<bool>,
    pub text: Option<&'a str>,
    pub thread_root_id: Option<&'a str>,
    /// Only include messages received after this time (exclusive).
    pub after: Option<&'a crate::jmap::UTCDate>,
    /// Only include messages received before this time (exclusive).
    pub before: Option<&'a crate::jmap::UTCDate>,
    pub position: Option<u64>,
    pub limit: Option<u64>,
    /// Sort by `sentAt` ascending (oldest first) when `true`.
    /// Defaults to `false` (descending, newest first), so `position:0, limit:N`
    /// returns the N most recent messages.
    pub sort_ascending: bool,
}

/// Input parameters for [`JmapChatClient::message_create`].
#[non_exhaustive]
#[derive(Debug)]
pub struct MessageCreateInput<'a> {
    /// Caller-supplied creation key. When `None`, a ULID is generated automatically.
    pub client_id: Option<&'a str>,
    pub chat_id: &'a str,
    pub body: &'a str,
    /// MIME type for the message body. Use [`crate::types::BodyType::Plain`],
    /// [`crate::types::BodyType::Markdown`], or [`crate::types::BodyType::Rich`]
    /// for spec-defined types; `Unknown(s)` passes `s` as-is.
    pub body_type: crate::types::BodyType,
    /// RFC 3339 timestamp (e.g. from `chrono::Utc::now().to_rfc3339()`).
    pub sent_at: &'a crate::jmap::UTCDate,
    pub reply_to: Option<&'a str>,
}

impl<'a> MessageCreateInput<'a> {
    /// Create a `MessageCreateInput` with required fields; optional fields default to `None`.
    pub fn new(
        chat_id: &'a str,
        body: &'a str,
        body_type: crate::types::BodyType,
        sent_at: &'a crate::jmap::UTCDate,
    ) -> Self {
        Self {
            client_id: None,
            chat_id,
            body,
            body_type,
            sent_at,
            reply_to: None,
        }
    }

    /// Set the caller-supplied creation key (overrides the auto-generated ULID).
    pub fn with_client_id(mut self, id: &'a str) -> Self {
        self.client_id = Some(id);
        self
    }

    /// Set the message this one replies to.
    pub fn with_reply_to(mut self, id: &'a str) -> Self {
        self.reply_to = Some(id);
        self
    }
}

/// A single reaction change in a Message/set patch (JMAP Chat §4.5).
///
/// The patch key is `reactions/<senderReactionId>` (JSON Pointer).
/// `senderReactionId` is a caller-generated ID (e.g. ULID) that uniquely
/// identifies this reaction slot for the sending user in this message.
#[non_exhaustive]
#[derive(Debug)]
pub enum ReactionChange<'a> {
    /// Add a reaction. Patch value: `{emoji, sentAt}`.
    Add {
        sender_reaction_id: &'a str,
        emoji: &'a str,
        sent_at: &'a crate::jmap::UTCDate,
    },
    /// Remove a reaction. Patch value: null.
    Remove { sender_reaction_id: &'a str },
}

/// Patch parameters for [`JmapChatClient::message_update`].
///
/// All fields are optional; absent fields (i.e. `None`) are not included in
/// the patch (the server leaves them unchanged). For chat-level deletion, set
/// `deleted_at` (soft-delete) and optionally `deleted_for_all: Some(true)`
/// (hard-delete, propagated to all participants).
///
/// Use `..Default::default()` to fill in unused fields.
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct MessagePatch<'a> {
    /// New message body text (author-only edit).
    pub body: Option<&'a str>,
    /// MIME type for `body`. Set alongside `body` in author-only edits.
    pub body_type: Option<crate::types::BodyType>,
    /// Reaction changes to apply. `None` (default) = no reaction changes.
    pub reaction_changes: Option<&'a [ReactionChange<'a>]>,
    /// Set the read-receipt timestamp (`Message.readAt`).
    pub read_at: Option<&'a crate::jmap::UTCDate>,
    /// Set the deletion timestamp for soft/hard delete.
    pub deleted_at: Option<&'a crate::jmap::UTCDate>,
    /// When `Some(true)` and `deleted_at` is also set, deletes for all
    /// participants (server sends `Peer/retract`).
    pub deleted_for_all: Option<bool>,
}

/// Patch parameters for [`JmapChatClient::presence_status_update`].
///
/// All fields are optional. A field that is `Patch::Keep` (default) is omitted
/// from the patch, leaving the server value unchanged. Use `Patch::Set(v)` to
/// set a value and `Patch::Clear` to null-clear a nullable field.
///
/// Use `..Default::default()` to fill in unused fields.
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct PresenceStatusPatch<'a> {
    pub presence: Option<crate::types::OwnerPresence>,
    pub status_text: Patch<&'a str>,
    pub status_emoji: Patch<&'a str>,
    /// Set or clear the auto-clear deadline. `Patch::Clear` removes any deadline.
    pub expires_at: Patch<&'a crate::jmap::UTCDate>,
    pub receipt_sharing: Option<bool>,
}

/// Input parameters for [`JmapChatClient::custom_emoji_query`].
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct CustomEmojiQueryInput<'a> {
    /// Filter to a specific Space's custom emojis. `None` returns all emojis
    /// visible to the account (Space-specific + server-global).
    pub filter_space_id: Option<&'a str>,
    pub position: Option<u64>,
    pub limit: Option<u64>,
}

/// Parameters for creating one CustomEmoji via [`JmapChatClient::custom_emoji_create`].
#[non_exhaustive]
#[derive(Debug)]
pub struct CustomEmojiCreateInput<'a> {
    /// Caller-supplied creation key. When `None`, a ULID is generated automatically.
    pub client_id: Option<&'a str>,
    /// Shortcode name without colons (e.g., `catjam`).
    pub name: &'a str,
    /// blobId of the emoji image (already uploaded).
    pub blob_id: &'a str,
    /// If `Some`, limits the emoji to the given Space. `None` = server-global.
    pub space_id: Option<&'a str>,
}

impl<'a> CustomEmojiCreateInput<'a> {
    /// Create a `CustomEmojiCreateInput` with required fields; optional fields default to `None`.
    pub fn new(name: &'a str, blob_id: &'a str) -> Self {
        Self {
            client_id: None,
            name,
            blob_id,
            space_id: None,
        }
    }

    /// Set the caller-supplied creation key (overrides the auto-generated ULID).
    pub fn with_client_id(mut self, id: &'a str) -> Self {
        self.client_id = Some(id);
        self
    }
}

/// Parameters for creating one SpaceInvite via [`JmapChatClient::space_invite_create`].
#[non_exhaustive]
#[derive(Debug)]
pub struct SpaceInviteCreateInput<'a> {
    /// Caller-supplied creation key. When `None`, a ULID is generated automatically.
    pub client_id: Option<&'a str>,
    pub space_id: &'a str,
    pub default_channel_id: Option<&'a str>,
    pub expires_at: Option<&'a crate::jmap::UTCDate>,
    pub max_uses: Option<u64>,
}

impl<'a> SpaceInviteCreateInput<'a> {
    /// Create a `SpaceInviteCreateInput` with required fields; optional fields default to `None`.
    pub fn new(space_id: &'a str) -> Self {
        Self {
            client_id: None,
            space_id,
            default_channel_id: None,
            expires_at: None,
            max_uses: None,
        }
    }

    /// Set the caller-supplied creation key (overrides the auto-generated ULID).
    pub fn with_client_id(mut self, id: &'a str) -> Self {
        self.client_id = Some(id);
        self
    }

    /// Set the maximum number of times this invite may be used.
    pub fn with_max_uses(mut self, max: u64) -> Self {
        self.max_uses = Some(max);
        self
    }
}

/// Parameters for creating one SpaceBan via [`JmapChatClient::space_ban_create`].
#[non_exhaustive]
#[derive(Debug)]
pub struct SpaceBanCreateInput<'a> {
    /// Caller-supplied creation key. When `None`, a ULID is generated automatically.
    pub client_id: Option<&'a str>,
    pub space_id: &'a str,
    /// ChatContact.id of the user to ban.
    pub user_id: &'a str,
    pub reason: Option<&'a str>,
    pub expires_at: Option<&'a crate::jmap::UTCDate>,
}

impl<'a> SpaceBanCreateInput<'a> {
    /// Create a `SpaceBanCreateInput` with required fields; optional fields default to `None`.
    pub fn new(space_id: &'a str, user_id: &'a str) -> Self {
        Self {
            client_id: None,
            space_id,
            user_id,
            reason: None,
            expires_at: None,
        }
    }

    /// Set the caller-supplied creation key (overrides the auto-generated ULID).
    pub fn with_client_id(mut self, id: &'a str) -> Self {
        self.client_id = Some(id);
        self
    }
}

/// Patch parameters for [`JmapChatClient::chat_contact_update`].
///
/// All fields are optional; absent fields are omitted from the patch. For the
/// nullable `display_name` field, use `Patch::Set(s)` to set and `Patch::Clear`
/// to clear. Use `..Default::default()` to fill in unused fields.
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct ChatContactPatch<'a> {
    pub blocked: Option<bool>,
    /// `Patch::Clear` clears `displayName`; `Patch::Set(s)` sets it.
    pub display_name: Patch<&'a str>,
}

/// Sort property for [`JmapChatClient::chat_contact_query`].
///
/// Spec: draft-atwood-jmap-chat-00 §4.3
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ContactSortProperty {
    LastSeenAt,
    Login,
    LastActiveAt,
}

/// Input parameters for [`JmapChatClient::chat_contact_query`].
///
/// All fields are optional; an empty filter shows all contacts.
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct ChatContactQueryInput {
    pub filter_blocked: Option<bool>,
    /// Filter to contacts with this exact presence state.
    ///
    /// `Unknown` is not representable here; use [`crate::types::ContactPresenceFilter`],
    /// which only carries the four wire-valid values.
    pub filter_presence: Option<crate::types::ContactPresenceFilter>,
    pub position: Option<u64>,
    pub limit: Option<u64>,
    /// Sort property: [`ContactSortProperty::LastSeenAt`], [`ContactSortProperty::Login`], or [`ContactSortProperty::LastActiveAt`].
    pub sort_property: Option<ContactSortProperty>,
    /// When `Some(false)` or `None`, sort descending. `Some(true)` sorts ascending.
    pub sort_ascending: Option<bool>,
}

/// Input parameters for [`JmapChatClient::space_create`].
#[non_exhaustive]
#[derive(Debug)]
pub struct SpaceCreateInput<'a> {
    /// Caller-supplied creation key. When `None`, a ULID is generated automatically.
    pub client_id: Option<&'a str>,
    /// Display name for the Space.
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub icon_blob_id: Option<&'a str>,
}

impl<'a> SpaceCreateInput<'a> {
    /// Create a `SpaceCreateInput` with required fields; optional fields default to `None`.
    pub fn new(name: &'a str) -> Self {
        Self {
            client_id: None,
            name,
            description: None,
            icon_blob_id: None,
        }
    }

    /// Set the caller-supplied creation key (overrides the auto-generated ULID).
    pub fn with_client_id(mut self, id: &'a str) -> Self {
        self.client_id = Some(id);
        self
    }
}

/// Input parameters for [`JmapChatClient::space_query`].
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct SpaceQueryInput<'a> {
    /// Filter by substring match on Space name.
    pub filter_name: Option<&'a str>,
    pub filter_is_public: Option<bool>,
    pub position: Option<u64>,
    pub limit: Option<u64>,
}

/// How to join a Space — passed to [`JmapChatClient::space_join`].
///
/// The enum makes invalid inputs unrepresentable: exactly one path is always
/// selected at construction time, so the runtime guard is not needed.
#[non_exhaustive]
#[derive(Debug)]
pub enum SpaceJoinInput<'a> {
    /// Redeem a SpaceInvite by its `code` field (not its `id`).
    InviteCode(&'a str),
    /// Join a public Space directly by its JMAP id.
    SpaceId(&'a str),
}

/// Response to [`JmapChatClient::space_join`] (JMAP Chat §Space/join).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceJoinResponse {
    pub account_id: Id,
    pub space_id: Id,
}

/// One entry in the `addMembers` patch key for [`JmapChatClient::chat_update`].
#[non_exhaustive]
#[derive(Debug)]
pub struct AddMemberInput<'a> {
    /// ChatContact.id of the member to add.
    pub id: &'a str,
    /// Role for the new member. `None` lets the server apply the default (`"member"`).
    pub role: Option<crate::types::ChatMemberRole>,
}

impl<'a> AddMemberInput<'a> {
    /// Create an `AddMemberInput`; `role` defaults to `None` (server assigns default).
    pub fn new(id: &'a str) -> Self {
        Self { id, role: None }
    }

    /// Set the role for this member.
    pub fn with_role(mut self, role: crate::types::ChatMemberRole) -> Self {
        self.role = Some(role);
        self
    }
}

/// One entry in the `updateMemberRoles` patch key for [`JmapChatClient::chat_update`].
#[non_exhaustive]
#[derive(Debug)]
pub struct UpdateMemberRoleInput<'a> {
    /// ChatContact.id of the member to update.
    pub id: &'a str,
    /// New role for this member.
    pub role: crate::types::ChatMemberRole,
}

impl<'a> UpdateMemberRoleInput<'a> {
    /// Create an `UpdateMemberRoleInput` with the target member and their new role.
    pub fn new(id: &'a str, role: crate::types::ChatMemberRole) -> Self {
        Self { id, role }
    }
}

/// Input parameters for [`JmapChatClient::chat_create`].
///
/// Discriminates the three Chat creation kinds from the spec. Each variant
/// carries the fields required for that kind plus an optional `client_id`;
/// when `None`, a ULID is generated automatically.
#[non_exhaustive]
#[derive(Debug)]
pub enum ChatCreateInput<'a> {
    /// Create a direct (one-to-one) chat.
    Direct {
        /// Caller-supplied creation key. When `None`, a ULID is generated automatically.
        client_id: Option<&'a str>,
        /// ChatContact.id of the other participant.
        contact_id: &'a str,
    },
    /// Create a group chat.
    Group {
        /// Caller-supplied creation key. When `None`, a ULID is generated automatically.
        client_id: Option<&'a str>,
        /// Display name for the group.
        name: &'a str,
        /// ChatContact.ids of initial non-owner members.
        member_ids: &'a [&'a str],
        description: Option<&'a str>,
        avatar_blob_id: Option<&'a str>,
        message_expiry_seconds: Option<u64>,
    },
    /// Create a channel chat inside a Space.
    Channel {
        /// Caller-supplied creation key. When `None`, a ULID is generated automatically.
        client_id: Option<&'a str>,
        /// The Space this channel belongs to.
        space_id: &'a str,
        /// Display name for the channel.
        name: &'a str,
        description: Option<&'a str>,
    },
}

/// Patch parameters for [`JmapChatClient::chat_update`].
///
/// All fields are optional; absent fields are not included in the patch (the
/// server leaves them unchanged). For nullable spec fields (`mute_until`,
/// `description`, `avatar_blob_id`) use `Patch::Set(v)` to set and
/// `Patch::Clear` to null-clear. Slice fields default to `None` (no change).
///
/// Use `..Default::default()` to fill in unused fields.
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct ChatPatch<'a> {
    pub muted: Option<bool>,
    /// `Patch::Clear` clears `muteUntil`; `Patch::Set(t)` sets it.
    pub mute_until: Patch<&'a crate::jmap::UTCDate>,
    pub receive_typing_indicators: Option<bool>,
    /// Replace the entire pinned-message list. `Some(&[])` clears all pins.
    pub pinned_message_ids: Option<&'a [&'a str]>,
    /// Spec defines this as `UnsignedInt` (non-nullable). To remove a previously
    /// set value, omit this field.
    pub message_expiry_seconds: Option<u64>,
    pub receipt_sharing: Option<bool>,
    /// New display name (group chats, admin only).
    pub name: Option<&'a str>,
    /// `Patch::Clear` clears; `Patch::Set(s)` sets (group chats, admin only).
    pub description: Patch<&'a str>,
    /// `Patch::Clear` clears; `Patch::Set(id)` sets (group chats, admin only).
    pub avatar_blob_id: Patch<&'a str>,
    /// Members to add (group chats, admin only). `None` = no change.
    pub add_members: Option<&'a [AddMemberInput<'a>]>,
    /// ChatContact.ids to remove (group chats, admin only). `None` = no change.
    pub remove_members: Option<&'a [&'a str]>,
    /// Role changes for existing members (group chats, admin only). `None` = no change.
    pub update_member_roles: Option<&'a [UpdateMemberRoleInput<'a>]>,
}

/// One member to add in the `addMembers` patch key of [`JmapChatClient::space_update`].
#[non_exhaustive]
#[derive(Debug)]
pub struct SpaceAddMemberInput<'a> {
    /// ChatContact.id of the member to add.
    pub id: &'a str,
    /// Initial role IDs for the new member. `None` grants no extra roles beyond `@everyone`.
    pub role_ids: Option<&'a [&'a str]>,
}

impl<'a> SpaceAddMemberInput<'a> {
    /// Create a `SpaceAddMemberInput`; `role_ids` defaults to `None`.
    pub fn new(id: &'a str) -> Self {
        Self { id, role_ids: None }
    }
}

/// One member update in the `updateMembers` patch key of [`JmapChatClient::space_update`].
#[non_exhaustive]
#[derive(Debug)]
pub struct SpaceUpdateMemberInput<'a> {
    /// ChatContact.id of the member to update.
    pub id: &'a str,
    pub role_ids: Option<&'a [&'a str]>,
    /// `Patch::Clear` clears the nick; `Patch::Set(s)` sets it.
    pub nick: Patch<&'a str>,
}

impl<'a> SpaceUpdateMemberInput<'a> {
    /// Create a `SpaceUpdateMemberInput`; optional fields default to `None`/`Keep`.
    pub fn new(id: &'a str) -> Self {
        Self {
            id,
            role_ids: None,
            nick: Patch::Keep,
        }
    }
}

/// One channel to add in the `addChannels` patch key of [`JmapChatClient::space_update`].
#[non_exhaustive]
#[derive(Debug)]
pub struct SpaceAddChannelInput<'a> {
    pub name: &'a str,
    pub category_id: Option<&'a str>,
    pub position: Option<u64>,
    pub topic: Option<&'a str>,
}

impl<'a> SpaceAddChannelInput<'a> {
    /// Create a `SpaceAddChannelInput`; optional fields default to `None`.
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            category_id: None,
            position: None,
            topic: None,
        }
    }
}

/// Patch parameters for [`JmapChatClient::space_update`].
///
/// All fields are optional. Absent fields are omitted from the patch.
/// Nullable fields (`description`, `icon_blob_id`) use `Patch::Set(v)` to set
/// and `Patch::Clear` to null-clear. Slice fields default to `None` (no change).
///
/// Scope: metadata + member + channel management. Role and category management
/// are out of scope for this epic.
///
/// Use `..Default::default()` to fill in unused fields.
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct SpacePatch<'a> {
    /// New display name (`manage_space` permission required).
    pub name: Option<&'a str>,
    /// `Patch::Clear` clears; `Patch::Set(s)` sets.
    pub description: Patch<&'a str>,
    /// `Patch::Clear` clears; `Patch::Set(id)` sets.
    pub icon_blob_id: Patch<&'a str>,
    pub is_public: Option<bool>,
    pub is_publicly_previewable: Option<bool>,
    /// Members to add (`manage_members` required). `None` = no change.
    pub add_members: Option<&'a [SpaceAddMemberInput<'a>]>,
    /// ChatContact.ids to remove (`manage_members` required). `None` = no change.
    pub remove_members: Option<&'a [&'a str]>,
    /// Member updates (`manage_members` required). `None` = no change.
    pub update_members: Option<&'a [SpaceUpdateMemberInput<'a>]>,
    /// Channels to add (`manage_channels` required). `None` = no change.
    pub add_channels: Option<&'a [SpaceAddChannelInput<'a>]>,
    /// Channel Chat ids to remove (`manage_channels` required). `None` = no change.
    pub remove_channels: Option<&'a [&'a str]>,
}

/// Input parameters for [`JmapChatClient::push_subscription_create`].
///
/// Creates a PushSubscription (RFC 8620 §7.2) with the optional `chatPush`
/// extension (draft-atwood-jmap-chat-push-00 §3.1).
///
/// `device_client_id` and `url` have no safe defaults and must always be supplied.
#[non_exhaustive]
#[derive(Debug)]
pub struct PushSubscriptionCreateInput<'a> {
    /// Caller-supplied creation key. When `None`, a ULID is generated automatically.
    pub client_id: Option<&'a str>,
    /// Stable client device identifier, used by the server to deduplicate subscriptions.
    pub device_client_id: &'a str,
    /// Push endpoint URL registered with the platform push service.
    pub url: &'a str,
    /// Subscription expiry time. `None` lets the server choose.
    pub expires: Option<&'a crate::jmap::UTCDate>,
    /// Data type names to include in StateChange notifications.
    /// `None` means the server delivers all changed types.
    pub types: Option<&'a [&'a str]>,
    /// Per-account ChatPushConfig entries for inline push. Each entry is
    /// `(accountId, config)`. Pass `None` to omit the `chatPush` property.
    pub chat_push: Option<&'a [(&'a str, crate::types::ChatPushConfig)]>,
}

impl<'a> PushSubscriptionCreateInput<'a> {
    /// Create a `PushSubscriptionCreateInput` with required fields; optional fields default to `None`.
    pub fn new(device_client_id: &'a str, url: &'a str) -> Self {
        Self {
            client_id: None,
            device_client_id,
            url,
            expires: None,
            types: None,
            chat_push: None,
        }
    }

    /// Set the caller-supplied creation key (overrides the auto-generated ULID).
    pub fn with_client_id(mut self, id: &'a str) -> Self {
        self.client_id = Some(id);
        self
    }

    /// Restrict StateChange notifications to these data type names.
    pub fn with_types(mut self, types: &'a [&'a str]) -> Self {
        self.types = Some(types);
        self
    }

    /// Attach per-account ChatPushConfig entries for inline push.
    pub fn with_chat_push(
        mut self,
        chat_push: &'a [(&'a str, crate::types::ChatPushConfig)],
    ) -> Self {
        self.chat_push = Some(chat_push);
        self
    }
}

// ---------------------------------------------------------------------------
// SessionClient — session-bound client (eliminates &Session threading)
// ---------------------------------------------------------------------------

/// A [`crate::client::JmapChatClient`] bound to a JMAP session.
///
/// Obtain via [`crate::client::JmapChatClient::with_session`]. All JMAP Chat
/// methods are available on this type without needing to pass `&Session` on
/// every call.
///
/// ```rust,no_run
/// # use jmap_chat::{JmapChatClient, NoneAuth};
/// # async fn example() -> Result<(), jmap_chat::ClientError> {
/// # let client = JmapChatClient::new(NoneAuth, "http://localhost").unwrap();
/// # let session: jmap_chat::Session = todo!();
/// let sc = client.with_session(&session);
/// let chats = sc.chat_get(None, None).await?;
/// # Ok(())
/// # }
/// ```
pub struct SessionClient<'s> {
    client: &'s crate::client::JmapChatClient,
    session: &'s crate::jmap::Session,
}

impl crate::client::JmapChatClient {
    /// Bind this client to a JMAP session, returning a [`SessionClient`] that
    /// exposes all JMAP Chat methods without a `&Session` parameter on each call.
    pub fn with_session<'s>(&'s self, session: &'s crate::jmap::Session) -> SessionClient<'s> {
        SessionClient {
            client: self,
            session,
        }
    }
}

impl SessionClient<'_> {
    /// Extract `(api_url, chat_account_id)` from the bound session.
    ///
    /// Returns `Err(InvalidSession)` if there is no primary account for
    /// `urn:ietf:params:jmap:chat`.
    pub(in crate::methods) fn session_parts(
        &self,
    ) -> Result<(&str, &str), crate::error::ClientError> {
        let api_url = self.session.api_url.as_str();
        let account_id = self.session.chat_account_id().ok_or_else(|| {
            crate::error::ClientError::InvalidSession(
                "no primary account for urn:ietf:params:jmap:chat in Session.primaryAccounts",
            )
        })?;
        Ok((api_url, account_id))
    }

    /// The JMAP API URL from the bound session.
    pub(in crate::methods) fn api_url(&self) -> &str {
        self.session.api_url.as_str()
    }

    /// Forward a JMAP request to the underlying HTTP client.
    pub(in crate::methods) async fn call(
        &self,
        api_url: &str,
        req: &crate::jmap::JmapRequest,
    ) -> Result<crate::jmap::JmapResponse, crate::error::ClientError> {
        self.client.call(api_url, req).await
    }
}

// ---------------------------------------------------------------------------
// Module-private helpers (accessible to child modules via super::)
// ---------------------------------------------------------------------------

/// The call-id embedded in every single-method JMAP request produced by
/// [`build_request`]. Returned alongside the request so callers pass it
/// directly to [`crate::client::extract_response`] — no separate import needed.
pub(super) const CALL_ID: &str = "r1";

/// Capability URIs for standard JMAP Chat method calls (RFC 8620 §3.3).
pub(super) const USING_CHAT: &[&str] = &["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:chat"];

/// Capability URIs for Quota method calls.
pub(super) const USING_QUOTAS: &[&str] =
    &["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:quotas"];

/// Capability URIs for PushSubscription method calls (RFC 8620 §7.2).
pub(super) const USING_CORE: &[&str] = &["urn:ietf:params:jmap:core"];

/// Capability URIs for PushSubscription/set with chat push extension.
pub(super) const USING_CHAT_PUSH: &[&str] = &[
    "urn:ietf:params:jmap:core",
    "urn:ietf:params:jmap:chat:push",
];

/// Build a single-method JMAP request.
///
/// `using` is the complete `using` array for the request (RFC 8620 §3.3).
/// Use the pre-defined constants [`USING_CHAT`], [`USING_QUOTAS`], or
/// [`USING_CORE`] to avoid per-call allocations; all capability URI strings
/// are static.
///
/// Returns `(call_id, request)`. Pass `call_id` to
/// `crate::client::extract_response` so the pairing is explicit and
/// compiler-visible rather than via a shared constant.
fn build_request(
    method_name: &str,
    args: serde_json::Value,
    using: &[&str],
) -> (&'static str, crate::jmap::JmapRequest) {
    let req = crate::jmap::JmapRequest {
        using: using.iter().map(|&s| s.to_string()).collect(),
        method_calls: vec![crate::jmap::Invocation::new(method_name, args, CALL_ID)],
    };
    (CALL_ID, req)
}

/// Resolve an optional caller-supplied client ID, generating a ULID if absent.
///
/// Returns `Cow::Borrowed(s)` when `id` is `Some(s)`, and `Cow::Owned(ulid)`
/// when `id` is `None`.
pub(super) fn resolve_client_id(id: Option<&str>) -> std::borrow::Cow<'_, str> {
    match id {
        Some(s) => std::borrow::Cow::Borrowed(s),
        None => std::borrow::Cow::Owned(ulid::Ulid::new().to_string()),
    }
}
