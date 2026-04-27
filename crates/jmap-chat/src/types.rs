use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use crate::jmap::{Id, UTCDate};

/// Serde default helper returning `true`. Used for boolean fields whose absent/default value is `true`.
pub(crate) fn default_true() -> bool {
    true
}

/// Deserializes an optionally-present, nullable string field into `Option<Option<String>>`.
///
/// - Field absent  → `None`           (no change; serde uses `#[serde(default)]`)
/// - Field `null`  → `Some(None)`     (explicit clear)
/// - Field `"s"`   → `Some(Some(s))`  (update)
pub(crate) fn deserialize_optional_nullable_string<'de, D>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
    match v {
        serde_json::Value::Null => Ok(Some(None)),
        serde_json::Value::String(s) => Ok(Some(Some(s))),
        other => Err(serde::de::Error::custom(format!(
            "expected null or string, got {other:?}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Attachment
// ---------------------------------------------------------------------------

/// File attachment metadata for a Message.
/// Spec: draft-atwood-jmap-chat-00 §4.1
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    /// Opaque server-assigned blob identifier.
    pub blob_id: Id,
    /// Original filename. MUST NOT contain `/`, `\`, or null bytes.
    pub filename: String,
    /// Valid MIME type string.
    pub content_type: String,
    /// Blob size in bytes.
    pub size: u64,
    /// Lowercase hex SHA-256 of blob content.
    pub sha256: String,
}

// ---------------------------------------------------------------------------
// Rich Body
// ---------------------------------------------------------------------------

/// The type of a rich body span.
/// Spec: draft-atwood-jmap-chat-00 §Rich Body Format
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpanType {
    Text,
    Bold,
    Italic,
    BoldItalic,
    Code,
    Codeblock,
    Blockquote,
    Mention,
    Link,
    /// Catch-all for unrecognized span types. Clients MUST render `Span.text` as plain text.
    #[serde(other)]
    Unknown,
}

/// A single styled or annotated run of text in a rich message body.
/// Spec: draft-atwood-jmap-chat-00 §Rich Body Format
///
/// `text` is present on ALL span types and MUST be rendered as plain text fallback for unknown types.
/// Type-specific fields are absent when not applicable to the span type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    /// Span type discriminator.
    #[serde(rename = "type")]
    pub span_type: SpanType,
    /// Plaintext content. Present on all span types; serves as fallback for unknown types.
    pub text: String,
    /// Language hint for syntax highlighting. Present only when `span_type` is `Codeblock`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// The mentioned user's ChatContact.id. Present only when `span_type` is `Mention`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Id>,
    /// Link URI. Present only when `span_type` is `Link`. MUST be treated as untrusted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

/// Parsed content of a `body` field when `bodyType` is `"application/jmap-chat-rich"`.
///
/// # Parsing
///
/// Per spec (IMPROVEMENTS.md §13), `Message.body` for rich messages is a JSON-encoded *string*
/// (not an embedded object). Parse with:
/// ```rust,no_run
/// # use jmap_chat::types::RichBody;
/// # let message_body = String::new();
/// let rich: RichBody = serde_json::from_str(&message_body).unwrap();
/// ```
///
/// # Security
///
/// `body` originates from a remote user. Validate length before calling `from_str`
/// and cap at `ChatCapability.max_body_bytes`. Unknown span types MUST render `Span.text`
/// as plain text without additional interpretation.
///
/// Spec: draft-atwood-jmap-chat-00 §Rich Body Format
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RichBody {
    /// Ordered array of text spans, left to right.
    pub spans: Vec<Span>,
}

// ---------------------------------------------------------------------------
// Endpoint
// ---------------------------------------------------------------------------

/// Known `type` URI values for an [`Endpoint`].
///
/// `Endpoint.endpoint_type` is typed as `String` on the wire to accept future extension types.
/// Use this enum to match against known values without losing forward compatibility.
///
/// Spec: draft-atwood-jmap-chat-00 §4.2
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointType {
    /// Video/voice teleconference (`urn:jmap:chat:cap:vtc`).
    Vtc,
    /// Payment receiving endpoint (`urn:jmap:chat:cap:payment`).
    Payment,
    /// Out-of-band file transfer endpoint (`urn:jmap:chat:cap:blob`).
    Blob,
    /// Calendar event link or meeting invite (`urn:jmap:chat:cap:calendar-event`).
    CalendarEvent,
    /// Free/busy availability lookup (`urn:jmap:chat:cap:availability`).
    Availability,
    /// Task/to-do item link (`urn:jmap:chat:cap:task`).
    Task,
    /// File storage node link (`urn:jmap:chat:cap:filenode`).
    Filenode,
    /// Any other type URI not recognized by this client.
    Other(String),
}

impl EndpointType {
    /// The canonical URI for this endpoint type, or the raw string for `Other`.
    pub fn as_uri(&self) -> &str {
        match self {
            Self::Vtc => "urn:jmap:chat:cap:vtc",
            Self::Payment => "urn:jmap:chat:cap:payment",
            Self::Blob => "urn:jmap:chat:cap:blob",
            Self::CalendarEvent => "urn:jmap:chat:cap:calendar-event",
            Self::Availability => "urn:jmap:chat:cap:availability",
            Self::Task => "urn:jmap:chat:cap:task",
            Self::Filenode => "urn:jmap:chat:cap:filenode",
            Self::Other(s) => s.as_str(),
        }
    }

    /// Parse from the raw URI string in `Endpoint.endpoint_type`.
    pub fn from_uri(s: &str) -> Self {
        match s {
            "urn:jmap:chat:cap:vtc" => Self::Vtc,
            "urn:jmap:chat:cap:payment" => Self::Payment,
            "urn:jmap:chat:cap:blob" => Self::Blob,
            "urn:jmap:chat:cap:calendar-event" => Self::CalendarEvent,
            "urn:jmap:chat:cap:availability" => Self::Availability,
            "urn:jmap:chat:cap:task" => Self::Task,
            "urn:jmap:chat:cap:filenode" => Self::Filenode,
            other => Self::Other(other.to_string()),
        }
    }
}

/// An out-of-band capability endpoint advertised on a ChatContact or Session.
/// Spec: draft-atwood-jmap-chat-00 §4.2
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    /// URI identifying the capability type.
    #[serde(rename = "type")]
    pub endpoint_type: String,
    /// The endpoint URI. Format is type-specific.
    pub uri: String,
    /// Human-readable label for this endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Type-specific key-value pairs. Clients MUST ignore unknown keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// MessageAction
// ---------------------------------------------------------------------------

/// A per-message out-of-band action invitation.
/// Spec: draft-atwood-jmap-chat-00 §4.3
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageAction {
    /// URI identifying the action type (same namespace as Endpoint.type).
    #[serde(rename = "type")]
    pub action_type: String,
    /// The action URI. Peer-supplied; MUST be treated as untrusted.
    pub uri: String,
    /// Human-readable label for the action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Time after which the action is no longer valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<UTCDate>,
    /// Type-specific key-value pairs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Mention
// ---------------------------------------------------------------------------

/// A structured @mention annotation within a message body.
/// Spec: draft-atwood-jmap-chat-00 §4.4
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mention {
    /// The ChatContact.id (userId) of the mentioned participant.
    pub id: Id,
    /// Byte offset into `body` where the mention text begins.
    pub offset: u64,
    /// Byte length of the mention text.
    pub length: u64,
}

// ---------------------------------------------------------------------------
// MessageRevision
// ---------------------------------------------------------------------------

/// One historical version of a Message body (edit history entry).
/// Spec: draft-atwood-jmap-chat-00 §4.5
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRevision {
    /// The prior body text.
    pub body: String,
    /// The prior MIME type.
    pub body_type: String,
    /// The time this version was superseded by an edit.
    pub edited_at: UTCDate,
}

// ---------------------------------------------------------------------------
// Reaction
// ---------------------------------------------------------------------------

/// An emoji reaction to a Message, stored in the `reactions` map.
/// Spec: draft-atwood-jmap-chat-00 §4.6
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reaction {
    /// A non-empty string identifying the reaction (Unicode emoji or token).
    pub emoji: String,
    /// The id of a Space-scoped custom emoji, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_emoji_id: Option<Id>,
    /// `"self"` for the owner's reaction, or a ChatContact.id.
    pub sender_id: SenderIdOrSelf,
    /// Time the reaction was added.
    pub sent_at: UTCDate,
}

// ---------------------------------------------------------------------------
// ChatContact
// ---------------------------------------------------------------------------

/// A remote user known to this mailbox.
/// Spec: draft-atwood-jmap-chat-00 §4.7
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatContact {
    /// The userId provided by the authentication layer.
    pub id: Id,
    /// A non-empty human-readable identifier for this contact.
    pub login: String,
    /// Human-readable display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Time this ChatContact was first recorded.
    pub first_seen_at: UTCDate,
    /// Time of most recent interaction with this ChatContact's mailbox.
    pub last_seen_at: UTCDate,
    /// Last known presence state. Absent when the server has no presence data for this contact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence: Option<ContactPresence>,
    /// Time the ChatContact was last observed to be active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<UTCDate>,
    /// Short custom status message set by the contact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_text: Option<String>,
    /// Single emoji or shortcode representing the contact's status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_emoji: Option<String>,
    /// Out-of-band capability endpoints advertised by this ChatContact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoints: Option<Vec<Endpoint>>,
    /// When `true`, messages from this ChatContact are silently dropped.
    pub blocked: bool,
}

/// Last known presence state for a ChatContact.
/// Spec: draft-atwood-jmap-chat-00 §4.7
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContactPresence {
    Online,
    Away,
    Busy,
    Invisible,
    Offline,
    /// Catch-all for any unrecognized wire value, including legacy `"unknown"` from old servers.
    #[serde(other)]
    Unknown,
}

// ---------------------------------------------------------------------------
// ChatMember
// ---------------------------------------------------------------------------

/// One participant in a group Chat.
/// Spec: draft-atwood-jmap-chat-00 §4.8
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMember {
    /// The participant's ChatContact.id / userId.
    pub id: Id,
    /// Either `"admin"` or `"member"`.
    pub role: ChatMemberRole,
    /// Time this participant joined the chat.
    pub joined_at: UTCDate,
    /// The ChatContact.id of the member who added this participant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invited_by: Option<Id>,
}

/// Role of a participant in a group Chat.
/// Spec: draft-atwood-jmap-chat-00 §4.8
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatMemberRole {
    Admin,
    Member,
    /// Catch-all for any unrecognized wire value from a future spec version.
    /// If serialized, produces the literal string `"unknown"` — not the original wire value.
    #[serde(other)]
    Unknown,
}

// ---------------------------------------------------------------------------
// ChannelPermission
// ---------------------------------------------------------------------------

/// Per-channel permission override for a specific role or member.
/// Spec: draft-atwood-jmap-chat-00 §4.14
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelPermission {
    /// A SpaceRole id or a SpaceMember ChatContact.id.
    pub target_id: Id,
    /// `"role"` or `"member"`.
    pub target_type: ChannelPermissionTargetType,
    /// Permissions explicitly granted in this channel.
    pub allow: Vec<String>,
    /// Permissions explicitly denied in this channel.
    pub deny: Vec<String>,
}

/// Whether a ChannelPermission targets a role or a member.
/// Spec: draft-atwood-jmap-chat-00 §4.14
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelPermissionTargetType {
    Role,
    Member,
    /// Catch-all for any unrecognized wire value from a future spec version.
    /// If serialized, produces the literal string `"unknown"` — not the original wire value.
    #[serde(other)]
    Unknown,
}

// ---------------------------------------------------------------------------
// Chat
// ---------------------------------------------------------------------------

/// A JMAP Chat object (JMAP Chat §4.1).
///
/// This type is **deserialization-only**: it is populated from server responses.
/// Field applicability by `kind` is enforced by the server, not by this struct.
/// Constructing a `Chat` in application code is not supported — use the
/// `Chat/get` method instead.
///
/// A conversation between two or more participants.
/// Spec: draft-atwood-jmap-chat-00 §4.9
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    /// A ULID assigned per §4.9.1.
    pub id: Id,
    /// `"direct"`, `"group"`, or `"channel"`.
    pub kind: ChatKind,

    // --- direct only ---
    /// Direct chats only: ChatContact.id of the other participant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<Id>,

    // --- group and channel ---
    /// Group and channel Chats: display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    // --- group only ---
    /// Group chats only: short description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Group chats only: blobId of the group avatar image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_blob_id: Option<Id>,
    /// Group chats only: full membership list including the owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub members: Option<Vec<ChatMember>>,

    // --- channel only ---
    /// Channel Chats only: the id of the containing Space.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<Id>,
    /// Channel Chats only: the Category id within the Space.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_id: Option<Id>,
    /// Channel Chats only: sort order within the category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<u64>,
    /// Channel Chats only: short description shown in the channel header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    /// Channel Chats only: minimum seconds between messages per member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slow_mode_seconds: Option<u64>,
    /// Channel Chats only: per-channel permission overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_overrides: Option<Vec<ChannelPermission>>,

    // --- all kinds ---
    /// Per-chat preference: when `false`, server silently drops typing push events for this chat.
    /// Spec: draft-atwood-jmap-chat-00 §4.9 (default: true)
    #[serde(default = "default_true")]
    pub receive_typing_indicators: bool,
    /// Per-chat override of `PresenceStatus.receiptSharing`. When absent, account-level applies.
    /// Spec: draft-atwood-jmap-chat-00 §4.9
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_sharing: Option<bool>,
    /// Time this chat was first recorded on this mailbox.
    pub created_at: UTCDate,
    /// Received time of the most recent message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_at: Option<UTCDate>,
    /// Count of unread Messages for this Chat.
    pub unread_count: u64,
    /// Ordered list of pinned Message ids, most-recently-pinned first.
    pub pinned_message_ids: Vec<Id>,
    /// When `true`, push notifications for this chat are suppressed.
    pub muted: bool,
    /// Muting expires at this time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute_until: Option<UTCDate>,
    /// Local expiry policy: messages older than this many seconds are deleted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_expiry_seconds: Option<u64>,
}

/// The kind of a Chat conversation.
/// Spec: draft-atwood-jmap-chat-00 §4.9
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatKind {
    Direct,
    Group,
    Channel,
    /// Catch-all for any unrecognized wire value from a future spec version.
    /// If serialized, produces the literal string `"unknown"` — not the original wire value.
    #[serde(other)]
    Unknown,
}

// ---------------------------------------------------------------------------
// DeliveryReceipt (nested in Message.deliveryReceipts)
// ---------------------------------------------------------------------------

/// Per-recipient delivery/read receipt for group message delivery tracking.
/// Spec: draft-atwood-jmap-chat-00 §4.10
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryReceipt {
    /// Time the message was delivered to this recipient, or null.
    pub delivered_at: Option<UTCDate>,
    /// Device-level delivery confirmation (APNs/FCM). Absent if platform cannot confirm.
    /// Spec: draft-atwood-jmap-chat-00 §4.10 (IMPROVEMENTS.md §4.1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_delivered_at: Option<UTCDate>,
    /// Time this recipient read the message, or null.
    pub read_at: Option<UTCDate>,
}

// ---------------------------------------------------------------------------
// SenderIdOrSelf
// ---------------------------------------------------------------------------

/// The sender of a message or reaction.
///
/// Wire value `"self"` is the spec-defined sentinel (draft-atwood-jmap-chat-00 §4.10);
/// empty strings are rejected as they are not valid ChatContact IDs.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum SenderIdOrSelf {
    /// The mailbox owner composed this message. Wire value: `"self"`.
    SelfSender,
    /// An inbound message from a contact. Wire value: the contact's
    /// `ChatContact.id` string. Empty strings are rejected at construction.
    Contact(crate::jmap::Id),
}

impl serde::Serialize for SenderIdOrSelf {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            SenderIdOrSelf::SelfSender => s.serialize_str("self"),
            SenderIdOrSelf::Contact(id) => s.serialize_str(id.as_str()),
        }
    }
}

impl<'de> serde::Deserialize<'de> for SenderIdOrSelf {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s == "self" {
            Ok(SenderIdOrSelf::SelfSender)
        } else {
            crate::jmap::Id::new(s)
                .map(SenderIdOrSelf::Contact)
                .map_err(serde::de::Error::custom)
        }
    }
}

impl std::fmt::Display for SenderIdOrSelf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SenderIdOrSelf::SelfSender => f.write_str("self"),
            SenderIdOrSelf::Contact(id) => f.write_str(id.as_str()),
        }
    }
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

/// A single transmission within a Chat.
/// Spec: draft-atwood-jmap-chat-00 §4.10
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Receiver-assigned ULID.
    pub id: Id,
    /// The sender-assigned ULID carried in `Peer/deliver`.
    pub sender_msg_id: Id,
    /// ID of the containing Chat.
    pub chat_id: Id,
    /// `"self"` for owner-composed messages; the sender's ChatContact.id otherwise.
    pub sender_id: SenderIdOrSelf,
    /// Message content.
    pub body: String,
    /// MIME type of `body`.
    pub body_type: String,
    /// File attachments.
    pub attachments: Vec<Attachment>,
    /// Structured @mention annotations.
    pub mentions: Vec<Mention>,
    /// Out-of-band action invitations.
    pub actions: Vec<MessageAction>,
    /// Emoji reactions, keyed by `senderReactionId`.
    pub reactions: HashMap<String, Reaction>,

    /// The receiver-assigned `id` of the Message this replies to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<Id>,
    /// The receiver-assigned `id` of the thread root message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_root_id: Option<Id>,
    /// Count of messages in this chat with `replyTo` equal to this message's `id`.
    pub reply_count: u64,
    /// Count of unread replies to this message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unread_reply_count: Option<u64>,

    /// Sender's claimed composition time.
    pub sent_at: UTCDate,
    /// Time this mailbox stored the message.
    pub received_at: UTCDate,
    /// Sender-set hard-deletion deadline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_expires_at: Option<UTCDate>,
    /// When `true`, permanently hard-delete after the owner reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burn_on_read: Option<bool>,

    /// Delivery state across all recipients.
    pub delivery_state: DeliveryState,
    /// Per-recipient delivery/read receipts (group, owner-sent messages only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_receipts: Option<HashMap<String, DeliveryReceipt>>,
    /// Time the first outbound delivery was acknowledged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivered_at: Option<UTCDate>,
    /// Time the owner acknowledged reading this message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_at: Option<UTCDate>,

    /// Time of the most recent edit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<UTCDate>,
    /// Prior versions, oldest first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_history: Option<Vec<MessageRevision>>,

    /// Time the message was deleted (tombstone).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<UTCDate>,
    /// `true` when deletion was propagated to all participants.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_for_all: Option<bool>,
}

/// Delivery state of a Message.
/// Spec: draft-atwood-jmap-chat-00 §4.10
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryState {
    Pending,
    Delivered,
    Failed,
    Received,
    /// Catch-all for any unrecognized wire value from a future spec version.
    /// If serialized, produces the literal string `"unknown"` — not the original wire value.
    #[serde(other)]
    Unknown,
}

// ---------------------------------------------------------------------------
// SpaceRole
// ---------------------------------------------------------------------------

/// A named set of permissions within a Space.
/// Spec: draft-atwood-jmap-chat-00 §4.11
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceRole {
    /// A ULID assigned by the server.
    pub id: Id,
    /// Display name of the role.
    pub name: String,
    /// Hex color string (e.g., `"#5865f2"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Named permissions this role grants.
    pub permissions: Vec<String>,
    /// Role hierarchy position. Higher values outrank lower ones.
    pub position: u64,
}

// ---------------------------------------------------------------------------
// SpaceMember
// ---------------------------------------------------------------------------

/// One participant in a Space.
/// Spec: draft-atwood-jmap-chat-00 §4.12
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceMember {
    /// The participant's ChatContact.id.
    pub id: Id,
    /// SpaceRole ids held by this member. Empty means only `@everyone`.
    pub role_ids: Vec<Id>,
    /// Space-specific display name override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nick: Option<String>,
    /// Time this member joined the Space.
    pub joined_at: UTCDate,
}

// ---------------------------------------------------------------------------
// Category
// ---------------------------------------------------------------------------

/// A named grouping of channels within a Space.
/// Spec: draft-atwood-jmap-chat-00 §4.13
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    /// A ULID assigned by the server.
    pub id: Id,
    /// Display name of the category.
    pub name: String,
    /// Sort order among categories. Lower values appear first.
    pub position: u64,
    /// Ordered list of channel Chat ids in this category.
    pub channel_ids: Vec<Id>,
}

// ---------------------------------------------------------------------------
// Space
// ---------------------------------------------------------------------------

/// A named container for channel Chats, members, roles, and categories.
/// Spec: draft-atwood-jmap-chat-00 §4.15
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    /// A ULID assigned by the server.
    pub id: Id,
    /// Display name of the Space.
    pub name: String,
    /// Short description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// blobId of the Space icon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_blob_id: Option<Id>,
    /// Named roles defined for this Space, ordered by `position` descending.
    pub roles: Vec<SpaceRole>,
    /// Full membership list including the owner.
    pub members: Vec<SpaceMember>,
    /// Categories, ordered by `position`.
    pub categories: Vec<Category>,
    /// Ordered list of channel Chat ids not assigned to any category.
    pub uncategorized_channel_ids: Vec<Id>,
    /// Time this Space was created.
    pub created_at: UTCDate,
    /// If `true`, any user may join without an invite code.
    pub is_public: bool,
    /// If `true`, non-members may query this Space via `Space/query`.
    pub is_publicly_previewable: bool,
    /// Current number of members in this Space.
    pub member_count: u64,
}

// ---------------------------------------------------------------------------
// CustomEmoji
// ---------------------------------------------------------------------------

/// A server- or Space-scoped custom emoji image.
/// Spec: draft-atwood-jmap-chat-00 §4.16
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomEmoji {
    /// A ULID assigned by the server.
    pub id: Id,
    /// The shortcode name, without colons (e.g., `catjam`).
    pub name: String,
    /// blobId of the emoji image.
    pub blob_id: Id,
    /// The id of the Space this emoji belongs to; absent means server-global.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<Id>,
    /// ChatContact.id of the user who created this emoji.
    pub created_by: Id,
    /// Time this emoji was created.
    pub created_at: UTCDate,
}

// ---------------------------------------------------------------------------
// SpaceInvite
// ---------------------------------------------------------------------------

/// A pending invitation to join a Space via a shared invite code.
/// Spec: draft-atwood-jmap-chat-00 §4.17
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceInvite {
    /// Opaque server-assigned JMAP identifier for this invite.
    pub id: Id,
    /// The user-shareable invite code.
    pub code: String,
    /// The Space this invite grants access to.
    pub space_id: Id,
    /// Chat id of the channel to highlight when a new member arrives.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_channel_id: Option<Id>,
    /// ChatContact.id of the member who created this invite.
    pub created_by: Id,
    /// Expiry time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<UTCDate>,
    /// Maximum redemption count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<u64>,
    /// Number of times this invite has been redeemed.
    pub uses: u64,
    /// Time this invite was created.
    pub created_at: UTCDate,
}

// ---------------------------------------------------------------------------
// SpaceBan
// ---------------------------------------------------------------------------

/// A ban record preventing a user from participating in a Space.
/// Spec: draft-atwood-jmap-chat-00 §4.18
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceBan {
    /// A ULID assigned by the server.
    pub id: Id,
    /// The id of the Space this ban applies to.
    pub space_id: Id,
    /// The ChatContact.id of the banned user.
    pub user_id: Id,
    /// The ChatContact.id of the Space member who issued this ban.
    pub banned_by: Id,
    /// Human-readable reason for the ban.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Time this ban was created.
    pub created_at: UTCDate,
    /// If present, the ban expires at this time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<UTCDate>,
}

// ---------------------------------------------------------------------------
// ReadPosition
// ---------------------------------------------------------------------------

/// Tracks the owner's read state within a Chat.
/// Spec: draft-atwood-jmap-chat-00 §4.19
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadPosition {
    /// A ULID assigned by the server.
    pub id: Id,
    /// The id of the Chat this position tracks.
    pub chat_id: Id,
    /// The `id` of the most recent Message the owner has read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_read_message_id: Option<Id>,
    /// Time the `lastReadMessageId` was last updated.
    pub last_read_at: UTCDate,
}

// ---------------------------------------------------------------------------
// PresenceStatus
// ---------------------------------------------------------------------------

/// The owner's self-reported availability and custom status (singleton).
/// Spec: draft-atwood-jmap-chat-00 §4.20
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceStatus {
    /// A ULID assigned by the server.
    pub id: Id,
    /// The owner's self-reported availability.
    pub presence: OwnerPresence,
    /// A short custom status message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_text: Option<String>,
    /// A single emoji or shortcode representing the owner's status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_emoji: Option<String>,
    /// If set, clear `statusText`/`statusEmoji` and reset `presence` at this time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<UTCDate>,
    /// When `false`, server MUST NOT invoke Peer/receipt on behalf of this account.
    /// Opt-out is bidirectional. Spec: draft-atwood-jmap-chat-00 §4.20 (default: true)
    #[serde(default = "default_true")]
    pub receipt_sharing: bool,
    /// Time the owner last updated this record.
    pub updated_at: UTCDate,
}

/// Self-reported availability for a PresenceStatus record.
/// Spec: draft-atwood-jmap-chat-00 §4.20
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OwnerPresence {
    Online,
    Away,
    Busy,
    Invisible,
    Offline,
    /// Catch-all for any unrecognized wire value from a future spec version.
    /// If serialized, produces the literal string `"unknown"` — not the original wire value.
    #[serde(other)]
    Unknown,
}

// ---------------------------------------------------------------------------
// WebSocket message types (draft-atwood-jmap-chat-wss-00)
// ---------------------------------------------------------------------------
// These types are used over the RFC 8887 WebSocket transport when the server
// advertises `urn:ietf:params:jmap:chat:websocket` capability.
// The `@type` field is the JSON discriminator; clients dispatch on it manually.
// ---------------------------------------------------------------------------

/// Sent by the client to subscribe to ephemeral typing and presence events.
/// Spec: draft-atwood-jmap-chat-wss-00
///
/// A subsequent `ChatStreamEnable` replaces the prior subscription entirely.
/// Must be re-sent after every WebSocket reconnect (subscriptions are session-scoped).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamEnable {
    /// Discriminator. Always `"ChatStreamEnable"` on the wire.
    #[serde(rename = "@type")]
    pub(crate) msg_type: String,
    /// Which event categories to receive: `"typing"` and/or `"presence"`.
    /// Unrecognized values alongside recognized ones are silently ignored by the server.
    pub data_types: Vec<String>,
    /// Chat IDs to receive typing events for. `None` = all member chats.
    /// Only relevant when `"typing"` is in `data_types`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_ids: Option<Vec<Id>>,
    /// Contact IDs to receive presence events for. `None` = all known contacts.
    /// Only relevant when `"presence"` is in `data_types`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_ids: Option<Vec<Id>>,
}

impl ChatStreamEnable {
    /// Construct a new `ChatStreamEnable` message.
    pub fn new(
        data_types: Vec<String>,
        chat_ids: Option<Vec<Id>>,
        contact_ids: Option<Vec<Id>>,
    ) -> Self {
        Self {
            msg_type: "ChatStreamEnable".to_string(),
            data_types,
            chat_ids,
            contact_ids,
        }
    }
}

/// Sent by the client to stop all ephemeral event delivery.
/// Spec: draft-atwood-jmap-chat-wss-00
///
/// Server MUST stop delivery silently even if no subscription is active.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamDisable {
    /// Discriminator. Always `"ChatStreamDisable"` on the wire.
    #[serde(rename = "@type")]
    pub(crate) msg_type: String,
}

impl Default for ChatStreamDisable {
    fn default() -> Self {
        Self {
            msg_type: "ChatStreamDisable".to_string(),
        }
    }
}

/// Pushed by the server when a participant in a subscribed chat is typing.
/// Spec: draft-atwood-jmap-chat-wss-00
///
/// If no event is received for a `(chat_id, sender_id)` pair within 10 seconds,
/// the client MUST hide the typing indicator regardless (decay timer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTypingEvent {
    /// Discriminator. Always `"ChatTypingEvent"` on the wire.
    #[serde(rename = "@type")]
    pub(crate) msg_type: String,
    /// The chat in which typing occurred.
    pub chat_id: Id,
    /// ChatContact.id of the user who is typing. MUST NOT be `"self"`.
    pub sender_id: Id,
    /// `true` = typing; `false` = stopped.
    pub typing: bool,
}

/// Pushed by the server when a subscribed contact's presence state changes.
/// Spec: draft-atwood-jmap-chat-wss-00
///
/// Partial update semantics: absent optional fields mean "no change to that field."
/// `null` explicitly clears `status_text` or `status_emoji`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPresenceEvent {
    /// Discriminator. Always `"ChatPresenceEvent"` on the wire.
    #[serde(rename = "@type")]
    pub(crate) msg_type: String,
    /// The ChatContact whose presence changed.
    pub contact_id: Id,
    /// Updated presence state.
    pub presence: ContactPresence,
    /// Updated last-active timestamp, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<UTCDate>,
    /// Updated status text. Outer `None` = absent (no change). `Some(None)` = explicit null (clear).
    /// `Some(Some(s))` = update to new value.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_nullable_string"
    )]
    pub status_text: Option<Option<String>>,
    /// Updated status emoji. Same null/absent semantics as `status_text`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_nullable_string"
    )]
    pub status_emoji: Option<Option<String>>,
}

// ---------------------------------------------------------------------------
// Push Notification Types
// ---------------------------------------------------------------------------

/// Per-account push configuration for a PushSubscription.
/// Spec: draft-atwood-jmap-chat-push-00 §3.2
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPushConfig {
    /// Chat kinds for which push is enabled (`"direct"`, `"group"`, `"channel"`).
    /// `None` enables push for all kinds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<String>>,
    /// Explicit Chat ids for which push is enabled. `None` enables push for
    /// all Chats of the matching kinds the account is a member of.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_ids: Option<Vec<String>>,
    /// `ChatMessageEntry` fields to include in each payload entry.
    /// `None` uses the server default set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<String>>,
    /// Web Push urgency for notifications from this config (default `"normal"`).
    /// MUST be a value in `ChatPushCapability.supported_urgency_values`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urgency: Option<String>,
    /// Overrides `urgency` for payloads containing a direct or Space-wide mention.
    /// MUST be a value in `ChatPushCapability.supported_urgency_values`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mention_urgency: Option<String>,
}

/// One message entry in a `ChatMessagePush` payload.
/// Spec: draft-atwood-jmap-chat-push-00 §4.2
///
/// `body_snippet` is absent when `encrypted` is `true` (spec §4.2 MUST).
/// This is a server-enforced invariant; the type does not reject a misbehaving
/// server that sends both — callers should treat `body_snippet` as unreliable
/// when `encrypted` is `true`.
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageEntry {
    /// Server-assigned id of the new message.
    pub message_id: String,
    /// Id of the Chat containing the message.
    pub chat_id: String,
    /// Kind of the Chat: `"direct"`, `"group"`, or `"channel"`.
    pub chat_kind: String,
    /// Display name of the Chat. Present for group and channel chats.
    pub chat_name: Option<String>,
    /// For channel chats: id of the containing Space.
    pub space_id: Option<String>,
    /// For channel chats: display name of the containing Space.
    pub space_name: Option<String>,
    /// ChatContact.id of the message sender (authoritative identity).
    pub sender_id: String,
    /// Sender display name at push-generation time (snapshot; not authoritative).
    pub sender_display_name: Option<String>,
    /// Sender's claimed composition time.
    pub sent_at: UTCDate,
    /// `true` if the account owner's ChatContact.id appears in `mentions`.
    pub has_mention: bool,
    /// `true` if the message carried a Space-wide mention scope.
    pub has_mention_all: bool,
    /// `true` if the message body is end-to-end encrypted; `bodySnippet` is absent when `true`.
    pub encrypted: bool,
    /// Truncated plaintext rendering of the message body. Absent when `encrypted` is `true`.
    pub body_snippet: Option<String>,
}

/// Push payload delivered directly to the registered push endpoint.
/// Spec: draft-atwood-jmap-chat-push-00 §4.1
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessagePush {
    /// MUST be `"ChatMessagePush"`.
    #[serde(rename = "@type")]
    pub type_name: String,
    /// The account id for which this payload was generated.
    pub account_id: String,
    /// The `Message` state after all entries in `messages` are applied.
    pub state: String,
    /// Message entries that triggered this push.
    pub messages: Vec<ChatMessageEntry>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/types/{}",
            env!("CARGO_MANIFEST_DIR"),
            name
        );
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read fixture {path}: {e}"))
    }

    /// Oracle: spec §4.9 — Chat object fields, hand-written from spec definition.
    #[test]
    fn test_chat_fixture_deserializes_correctly() {
        let json = fixture("chat.json");
        let chat: Chat = serde_json::from_str(&json).expect("chat.json must parse");

        // Oracle: spec §4.9 — kind values are "direct", "group", "channel"
        assert_eq!(chat.kind, ChatKind::Group);
        assert_eq!(chat.id, "01HV5Z6QKWJ7N3P8R2X4YTMD3G");
        assert_eq!(chat.name.as_deref(), Some("Engineering Team"));
        assert_eq!(chat.unread_count, 3);
        assert!(!chat.muted);
        assert!(chat.pinned_message_ids.is_empty());

        let members = chat.members.as_ref().expect("group chat must have members");
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].id, "user:alice@example.com");
        assert_eq!(members[0].role, ChatMemberRole::Admin);
        assert_eq!(members[1].id, "user:bob@example.com");
        assert_eq!(members[1].role, ChatMemberRole::Member);
    }

    /// Oracle: spec §4.10 — Message object fields, hand-written from spec definition.
    #[test]
    fn test_message_fixture_deserializes_correctly() {
        let json = fixture("message.json");
        let msg: Message = serde_json::from_str(&json).expect("message.json must parse");

        // Oracle: spec §4.10 — senderId is "self" for owner-composed messages
        assert_eq!(msg.id, "01HV5Z6QKWJ7N3P8R2X4YTMD00");
        assert_eq!(msg.sender_id, SenderIdOrSelf::SelfSender);
        assert_eq!(msg.body, "Hello, world!");
        assert_eq!(msg.body_type, "text/plain");
        assert_eq!(msg.delivery_state, DeliveryState::Delivered);
        assert!(msg.attachments.is_empty());
        assert!(msg.mentions.is_empty());
        assert!(msg.actions.is_empty());
        assert!(msg.reactions.is_empty());
        assert_eq!(msg.reply_count, 0);
        assert!(msg.reply_to.is_none());
        assert!(msg.deleted_at.is_none());
    }

    /// Oracle: spec §4.9 — optional fields absent from JSON deserialize to None.
    #[test]
    fn test_chat_optional_fields_absent_become_none() {
        let json = fixture("chat_direct.json");
        let chat: Chat = serde_json::from_str(&json).expect("chat_direct.json must parse");

        // Oracle: spec §4.9 — direct chats have no name, no members, no spaceId
        assert_eq!(chat.kind, ChatKind::Direct);
        assert!(chat.name.is_none(), "direct chat must not have name");
        assert!(chat.members.is_none(), "direct chat must not have members");
        assert!(chat.space_id.is_none(), "direct chat must not have spaceId");
        assert!(chat.description.is_none());
        assert!(chat.last_message_at.is_some());
        // contactId is required for direct chats
        assert_eq!(chat.contact_id.as_deref(), Some("user:carol@example.com"));
    }

    /// Oracle: spec §4.10 — DeliveryState serializes to the correct lowercase string.
    #[test]
    fn test_delivery_state_serializes_to_spec_string() {
        // Oracle: spec §4.10 text: "pending", "delivered", "failed", "received"
        let pending = serde_json::to_string(&DeliveryState::Pending).unwrap();
        let delivered = serde_json::to_string(&DeliveryState::Delivered).unwrap();
        let failed = serde_json::to_string(&DeliveryState::Failed).unwrap();
        let received = serde_json::to_string(&DeliveryState::Received).unwrap();

        assert_eq!(pending, "\"pending\"");
        assert_eq!(delivered, "\"delivered\"");
        assert_eq!(failed, "\"failed\"");
        assert_eq!(received, "\"received\"");
    }

    /// Oracle: spec §4.20 — OwnerPresence serializes to spec-defined lowercase strings.
    #[test]
    fn test_owner_presence_serializes_to_spec_string() {
        // Oracle: spec §4.20: "online", "away", "busy", "invisible", "offline"
        let cases = [
            (OwnerPresence::Online, "\"online\""),
            (OwnerPresence::Away, "\"away\""),
            (OwnerPresence::Busy, "\"busy\""),
            (OwnerPresence::Invisible, "\"invisible\""),
            (OwnerPresence::Offline, "\"offline\""),
        ];
        for (variant, expected) in cases {
            let got = serde_json::to_string(&variant).unwrap();
            assert_eq!(
                got, expected,
                "OwnerPresence::{variant:?} wrong serialization"
            );
        }
    }

    /// Oracle: spec §4.8 — ChatMemberRole serializes to "admin" or "member".
    #[test]
    fn test_chat_member_role_serializes_to_spec_string() {
        // Oracle: spec §4.8: role is "admin" or "member"
        assert_eq!(
            serde_json::to_string(&ChatMemberRole::Admin).unwrap(),
            "\"admin\""
        );
        assert_eq!(
            serde_json::to_string(&ChatMemberRole::Member).unwrap(),
            "\"member\""
        );
    }

    /// Oracle: spec §4.19 — ReadPosition with absent lastReadMessageId deserializes to None.
    #[test]
    fn test_read_position_absent_last_read_is_none() {
        let json = fixture("read_position.json");
        let rp: ReadPosition = serde_json::from_str(&json).expect("read_position.json must parse");

        // Oracle: spec §4.19 — lastReadMessageId is optional; absent means no messages read
        assert!(
            rp.last_read_message_id.is_none(),
            "lastReadMessageId must be None when absent from JSON"
        );
        assert_eq!(rp.chat_id, "01HV5Z6QKWJ7N3P8R2X4YTMD3G");
    }

    /// Oracle: spec §ChatContact — absent presence field deserializes to None.
    #[test]
    fn test_chat_contact_absent_presence_is_none() {
        let json = fixture("chat_contact_no_presence.json");
        let contact: ChatContact = serde_json::from_str(&json).expect("must parse");
        assert!(
            contact.presence.is_none(),
            "absent presence must be None, not Offline"
        );
        assert!(contact.status_text.is_none());
        assert!(contact.status_emoji.is_none());
    }

    /// Oracle: spec §ChatContact — presence, statusText, statusEmoji all present.
    #[test]
    fn test_chat_contact_with_status_fields() {
        let json = fixture("chat_contact_with_status.json");
        let contact: ChatContact = serde_json::from_str(&json).expect("must parse");
        assert_eq!(contact.presence, Some(ContactPresence::Busy));
        assert_eq!(contact.status_text.as_deref(), Some("In a meeting"));
        assert_eq!(contact.status_emoji.as_deref(), Some("🗓️"));
    }

    /// Oracle: spec ContactPresence — Busy and Invisible deserialize from wire strings.
    #[test]
    fn test_contact_presence_busy_invisible_deserialize() {
        let busy: ContactPresence = serde_json::from_str("\"busy\"").unwrap();
        let invisible: ContactPresence = serde_json::from_str("\"invisible\"").unwrap();
        assert_eq!(busy, ContactPresence::Busy);
        assert_eq!(invisible, ContactPresence::Invisible);
    }

    /// Oracle: spec ContactPresence — unrecognized wire value deserializes to Unknown (catch-all).
    #[test]
    fn test_contact_presence_unknown_catch_all() {
        let unknown: ContactPresence = serde_json::from_str("\"some-future-value\"").unwrap();
        assert_eq!(unknown, ContactPresence::Unknown);
        // Legacy "unknown" wire value also maps to Unknown
        let legacy: ContactPresence = serde_json::from_str("\"unknown\"").unwrap();
        assert_eq!(legacy, ContactPresence::Unknown);
    }

    /// Oracle: spec §Chat — absent receiveTypingIndicators defaults to true.
    #[test]
    fn test_chat_receive_typing_indicators_defaults_true() {
        // Use the existing chat.json fixture which does not include this field
        let json = fixture("chat.json");
        let chat: Chat = serde_json::from_str(&json).expect("must parse");
        assert!(
            chat.receive_typing_indicators,
            "absent field must default to true"
        );
        assert!(
            chat.receipt_sharing.is_none(),
            "absent receipt_sharing must be None"
        );
    }

    /// Oracle: spec §RichBody — spans array with multiple span types.
    #[test]
    fn test_rich_body_fixture_parses_correctly() {
        let json = fixture("rich_body.json");
        let rb: RichBody = serde_json::from_str(&json).expect("must parse");
        assert_eq!(rb.spans.len(), 7);
        assert_eq!(rb.spans[0].span_type, SpanType::Text);
        assert_eq!(rb.spans[0].text, "Hello ");
        assert_eq!(rb.spans[1].span_type, SpanType::Bold);
        assert_eq!(rb.spans[3].span_type, SpanType::Link);
        assert_eq!(rb.spans[3].uri.as_deref(), Some("https://example.com"));
        assert_eq!(rb.spans[5].span_type, SpanType::Codeblock);
        assert_eq!(rb.spans[5].lang.as_deref(), Some("rust"));
        assert_eq!(rb.spans[6].span_type, SpanType::Mention);
        assert_eq!(
            rb.spans[6].user_id.as_deref(),
            Some("user:alice@example.com")
        );
    }

    /// Oracle: spec §RichBody — unknown span type deserializes to SpanType::Unknown with text preserved.
    #[test]
    fn test_rich_body_unknown_span_type_uses_text_fallback() {
        let json = r#"{"spans": [{"type": "future-type", "text": "fallback text"}]}"#;
        let rb: RichBody = serde_json::from_str(json).expect("must parse");
        assert_eq!(rb.spans[0].span_type, SpanType::Unknown);
        assert_eq!(rb.spans[0].text, "fallback text");
    }

    /// Oracle: spec §ChatTypingEvent — fixture round-trip.
    #[test]
    fn test_chat_typing_event_deserializes() {
        let json = fixture("chat_typing_event.json");
        let evt: ChatTypingEvent = serde_json::from_str(&json).expect("must parse");
        assert_eq!(evt.chat_id, "01HV5Z6QKWJ7N3P8R2X4YTMD3G");
        assert_eq!(evt.sender_id, "user:bob@example.com");
        assert!(evt.typing);
    }

    /// Oracle: spec §ChatPresenceEvent — fixture with statusText and statusEmoji.
    #[test]
    fn test_chat_presence_event_deserializes() {
        let json = fixture("chat_presence_event.json");
        let evt: ChatPresenceEvent = serde_json::from_str(&json).expect("must parse");
        assert_eq!(evt.contact_id, "user:carol@example.com");
        assert_eq!(evt.presence, ContactPresence::Busy);
        assert_eq!(evt.status_text, Some(Some("Do not disturb".to_string())));
        assert_eq!(evt.status_emoji, Some(Some("🔕".to_string())));
    }

    /// Oracle: spec §ChatPresenceEvent — null statusText/statusEmoji means explicit clear.
    #[test]
    fn test_chat_presence_event_null_clears_status() {
        let json = fixture("chat_presence_event_clear_status.json");
        let evt: ChatPresenceEvent = serde_json::from_str(&json).expect("must parse");
        assert_eq!(evt.presence, ContactPresence::Online);
        assert_eq!(
            evt.status_text,
            Some(None),
            "null in JSON must become Some(None)"
        );
        assert_eq!(
            evt.status_emoji,
            Some(None),
            "null in JSON must become Some(None)"
        );
    }

    /// Oracle: spec §ChatPresenceEvent — absent statusText/statusEmoji means no change.
    #[test]
    fn test_chat_presence_event_absent_status_means_no_change() {
        let json =
            r#"{"@type":"ChatPresenceEvent","contactId":"user:x@example.com","presence":"online"}"#;
        let evt: ChatPresenceEvent = serde_json::from_str(json).expect("must parse");
        assert!(
            evt.status_text.is_none(),
            "absent means no change (outer None)"
        );
        assert!(
            evt.status_emoji.is_none(),
            "absent means no change (outer None)"
        );
    }

    /// Oracle: spec §ChatStreamEnable — fixture serializes/deserializes correctly.
    #[test]
    fn test_chat_stream_enable_round_trip() {
        let json = fixture("chat_stream_enable.json");
        let msg: ChatStreamEnable = serde_json::from_str(&json).expect("must parse");
        assert_eq!(msg.msg_type, "ChatStreamEnable");
        assert_eq!(msg.data_types, vec!["typing", "presence"]);
        assert!(msg.contact_ids.is_none());
        // Test constructor
        let constructed = ChatStreamEnable::new(
            vec!["typing".to_string(), "presence".to_string()],
            Some(vec![Id::from_trusted("01HV5Z6QKWJ7N3P8R2X4YTMD3G")]),
            None,
        );
        assert_eq!(constructed.msg_type, "ChatStreamEnable");
    }

    /// Oracle: EndpointType::from_uri parses all known spec URIs correctly.
    #[test]
    fn test_endpoint_type_from_uri() {
        assert_eq!(
            EndpointType::from_uri("urn:jmap:chat:cap:vtc"),
            EndpointType::Vtc
        );
        assert_eq!(
            EndpointType::from_uri("urn:jmap:chat:cap:payment"),
            EndpointType::Payment
        );
        assert_eq!(
            EndpointType::from_uri("urn:jmap:chat:cap:blob"),
            EndpointType::Blob
        );
        assert_eq!(
            EndpointType::from_uri("urn:jmap:chat:cap:calendar-event"),
            EndpointType::CalendarEvent
        );
        assert_eq!(
            EndpointType::from_uri("urn:jmap:chat:cap:availability"),
            EndpointType::Availability
        );
        assert_eq!(
            EndpointType::from_uri("urn:jmap:chat:cap:task"),
            EndpointType::Task
        );
        assert_eq!(
            EndpointType::from_uri("urn:jmap:chat:cap:filenode"),
            EndpointType::Filenode
        );
        assert_eq!(
            EndpointType::from_uri("urn:example:custom"),
            EndpointType::Other("urn:example:custom".to_string())
        );
    }

    /// Oracle: EndpointType::as_uri round-trips through from_uri for all known types.
    #[test]
    fn test_endpoint_type_as_uri_round_trips() {
        for et in [
            EndpointType::Vtc,
            EndpointType::Payment,
            EndpointType::Blob,
            EndpointType::CalendarEvent,
            EndpointType::Availability,
            EndpointType::Task,
            EndpointType::Filenode,
        ] {
            let uri = et.as_uri();
            assert_eq!(
                EndpointType::from_uri(uri),
                et,
                "round-trip failed for {uri}"
            );
        }
    }

    /// Oracle: unknown ChatKind wire value must deserialize to Unknown, not fail.
    /// Trigger: server sends a new chat kind (e.g. "thread") not yet in this crate.
    #[test]
    fn test_chat_kind_unknown_wire_value_becomes_unknown() {
        let v: ChatKind = serde_json::from_str("\"thread\"").unwrap();
        assert_eq!(v, ChatKind::Unknown);
    }

    /// Oracle: Unknown catch-all variants serialize as the literal string "unknown".
    /// The original wire value is NOT preserved — this is a known serde #[serde(other)] limitation.
    /// These types are deserialization-only in practice; this test documents the fallback behavior.
    #[test]
    fn test_unknown_catch_all_variants_serialize_as_literal_unknown() {
        assert_eq!(
            serde_json::to_string(&ChatKind::Unknown).unwrap(),
            "\"unknown\""
        );
        assert_eq!(
            serde_json::to_string(&DeliveryState::Unknown).unwrap(),
            "\"unknown\""
        );
        assert_eq!(
            serde_json::to_string(&OwnerPresence::Unknown).unwrap(),
            "\"unknown\""
        );
    }

    /// Oracle: unknown ChatMemberRole wire value must deserialize to Unknown, not fail.
    #[test]
    fn test_chat_member_role_unknown_wire_value_becomes_unknown() {
        let v: ChatMemberRole = serde_json::from_str("\"owner\"").unwrap();
        assert_eq!(v, ChatMemberRole::Unknown);
    }

    /// Oracle: unknown DeliveryState wire value must deserialize to Unknown, not fail.
    #[test]
    fn test_delivery_state_unknown_wire_value_becomes_unknown() {
        let v: DeliveryState = serde_json::from_str("\"bounced\"").unwrap();
        assert_eq!(v, DeliveryState::Unknown);
    }

    /// Oracle: unknown OwnerPresence wire value must deserialize to Unknown, not fail.
    #[test]
    fn test_owner_presence_unknown_wire_value_becomes_unknown() {
        let v: OwnerPresence = serde_json::from_str("\"dnd\"").unwrap();
        assert_eq!(v, OwnerPresence::Unknown);
    }

    /// Oracle: unknown ChannelPermissionTargetType wire value must deserialize to Unknown, not fail.
    #[test]
    fn test_channel_permission_target_type_unknown_wire_value_becomes_unknown() {
        let v: ChannelPermissionTargetType = serde_json::from_str("\"group\"").unwrap();
        assert_eq!(v, ChannelPermissionTargetType::Unknown);
    }

    /// Oracle: ChatStreamEnable with None optional fields must serialize WITHOUT those keys.
    /// Spec: absent = all member chats/contacts. Sending null is non-conformant.
    #[test]
    fn test_chat_stream_enable_none_fields_absent_in_serialization() {
        let msg = ChatStreamEnable::new(vec!["typing".to_string()], None, None);
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            parsed.get("chatIds").is_none(),
            "chatIds must be absent when None, got: {json}"
        );
        assert!(
            parsed.get("contactIds").is_none(),
            "contactIds must be absent when None, got: {json}"
        );
    }

    /// Oracle: deserialize_optional_nullable_string must error on non-null, non-string JSON values.
    #[test]
    fn test_deserialize_optional_nullable_string_rejects_non_string() {
        // statusText: 42 (number) — must not parse to Ok
        let json =
            r#"{"@type":"ChatPresenceEvent","contactId":"x","presence":"online","statusText":42}"#;
        let result: Result<ChatPresenceEvent, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "number for statusText must be a deserialization error"
        );
    }

    #[test]
    fn sender_id_deserialize_self() {
        let v: SenderIdOrSelf = serde_json::from_str("\"self\"").unwrap();
        assert_eq!(v, SenderIdOrSelf::SelfSender);
    }

    #[test]
    fn sender_id_deserialize_contact() {
        let v: SenderIdOrSelf = serde_json::from_str("\"contact-abc123\"").unwrap();
        assert_eq!(
            v,
            SenderIdOrSelf::Contact(crate::jmap::Id::from_trusted("contact-abc123"))
        );
    }

    #[test]
    fn sender_id_serialize_self() {
        let v = SenderIdOrSelf::SelfSender;
        assert_eq!(serde_json::to_string(&v).unwrap(), "\"self\"");
    }

    #[test]
    fn sender_id_reject_empty() {
        let result: Result<SenderIdOrSelf, _> = serde_json::from_str("\"\"");
        assert!(result.is_err());
    }

    /// Oracle: draft-atwood-jmap-chat-push-00 §4 example payload (§4.1 / §4.2).
    /// Fixture hand-written from the spec example at §4.1.
    #[test]
    fn chat_message_push_from_fixture() {
        let text = fixture("chat_message_push.json");
        let push: ChatMessagePush =
            serde_json::from_str(&text).expect("deserialize ChatMessagePush");

        assert_eq!(push.type_name, "ChatMessagePush");
        assert_eq!(push.account_id, "u1");
        assert_eq!(push.state, "d35ecb040aab");
        assert_eq!(push.messages.len(), 1);

        let entry = &push.messages[0];
        assert_eq!(entry.message_id, "01J3YKZQP5MWVT8PPBEHTJ3HX");
        assert_eq!(entry.chat_id, "01J3XKZQN4MWVT8PPBEHTJ3HX");
        assert_eq!(entry.chat_kind, "channel");
        assert_eq!(entry.chat_name.as_deref(), Some("general"));
        assert_eq!(entry.space_id.as_deref(), Some("01J2WKZQM3LVST7OOBDHSI2GW"));
        assert_eq!(entry.space_name.as_deref(), Some("ACME Corp"));
        assert_eq!(entry.sender_id, "user:alice@example.com");
        assert_eq!(entry.sender_display_name.as_deref(), Some("Alice"));
        assert!(entry.has_mention);
        assert!(!entry.has_mention_all);
        assert!(!entry.encrypted);
        assert_eq!(
            entry.body_snippet.as_deref(),
            Some("Hey @bob, the deploy is ready for review")
        );
    }

    /// Oracle: spec §4.2 — when `encrypted` is true, `bodySnippet` MUST be absent.
    #[test]
    fn chat_message_push_encrypted_no_snippet() {
        let json = r#"{
            "@type": "ChatMessagePush",
            "accountId": "u1",
            "state": "abc",
            "messages": [{
                "messageId": "01HV5Z6QKWJ7N3P8R2X4YTMDSP",
                "chatId": "01HV5Z6QKWJ7N3P8R2X4YTMDCT",
                "chatKind": "direct",
                "senderId": "user:bob@example.com",
                "sentAt": "2026-04-26T14:32:00Z",
                "hasMention": false,
                "hasMentionAll": false,
                "encrypted": true
            }]
        }"#;
        let push: ChatMessagePush =
            serde_json::from_str(json).expect("deserialize encrypted ChatMessagePush");
        let entry = &push.messages[0];
        assert!(entry.encrypted);
        assert!(
            entry.body_snippet.is_none(),
            "bodySnippet must be absent when encrypted"
        );
    }

    /// Oracle: spec §4.10 — deviceDeliveredAt round-trips as Option<UTCDate>.
    /// Fixture hand-written from spec field definition.
    #[test]
    fn test_delivery_receipt_device_delivered_at_round_trips() {
        let json = fixture("delivery_receipt_with_device.json");
        let receipt: DeliveryReceipt = serde_json::from_str(&json).expect("must parse");
        assert_eq!(
            receipt.delivered_at.as_ref().map(|d| d.as_str()),
            Some("2024-01-01T10:00:00Z"),
        );
        assert_eq!(
            receipt.device_delivered_at.as_ref().map(|d| d.as_str()),
            Some("2024-01-01T10:00:02Z"),
        );
        assert_eq!(
            receipt.read_at.as_ref().map(|d| d.as_str()),
            Some("2024-01-01T10:05:00Z"),
        );
    }

    /// Oracle: spec §4.20 — receiptSharing MUST default to true when absent from JSON.
    /// Fixture deliberately omits the field; serde(default = "default_true") provides it.
    #[test]
    fn test_presence_status_receipt_sharing_defaults_true() {
        let json = fixture("presence_status_receipt_sharing.json");
        let ps: PresenceStatus = serde_json::from_str(&json).expect("must parse");
        assert!(
            ps.receipt_sharing,
            "absent receiptSharing must default to true"
        );
    }
}
