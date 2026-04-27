# Test Fixtures

This directory contains JSON fixtures used as oracles for deserialization and
serialization tests in the `jmap-chat` crate.

## The Oracle Pattern

**Fixtures are written by hand from the spec, never generated from the code
under test.**

A test that serializes a value with function A and then deserializes it with
function A proves only internal consistency — it cannot detect a systematic
misreading of the spec.  Each fixture is an independent reference point:
a JSON document whose shape and field values are derived directly from the
JMAP Chat specification text, not from running any Rust code.

When a fixture is non-trivial to construct by hand (e.g., it contains
UTCDate strings that must satisfy RFC 3339, or ULID values that must be
lexicographically ordered), commit a Python or shell script alongside the
fixture that produced it.  The script is the oracle's provenance; without it
the fixture becomes an opaque blob whose correctness cannot be audited.

## Naming Convention

Fixtures are grouped by what layer of the stack they exercise:

```
tests/fixtures/
    types/      — single data-type objects (Chat, Message, ChatContact, etc.)
    jmap/       — JMAP core wire envelopes (JmapRequest, JmapResponse, Session)
    session/    — Session object variants (valid, error cases)
    methods/    — full JmapResponse bodies wrapping a single method response
```

File names follow the pattern `<subject>[_<variant>].json`, where the subject
is a snake_case rendering of the type or method being tested and the optional
variant distinguishes cases (e.g., `chat_contact_no_presence` vs
`chat_contact_with_status`).

## Fixture Index

### `types/` — Individual data-type objects

| File | Oracle for |
|------|-----------|
| `chat.json` | `Chat` — group chat with two members, admin and regular role |
| `chat_direct.json` | `Chat` — direct (kind=direct) chat variant |
| `chat_contact_with_status.json` | `ChatContact` — full presence, statusText, statusEmoji, lastActiveAt |
| `chat_contact_no_presence.json` | `ChatContact` — federated contact with no presence fields |
| `message.json` | `Message` — plain-text message, delivered state, empty collections |
| `read_position.json` | `ReadPosition` — minimal read-position object |
| `rich_body.json` | `RichBody` — span array covering text, bold, link, codeblock, mention |
| `chat_typing_event.json` | `ChatTypingEvent` — SSE typing notification |
| `chat_presence_event.json` | `ChatPresenceEvent` — presence update with status text and emoji |
| `chat_presence_event_clear_status.json` | `ChatPresenceEvent` — presence update clearing status (null fields) |
| `chat_stream_enable.json` | `ChatStreamEnable` — SSE subscription control message |

### `jmap/` — JMAP core wire envelopes

| File | Oracle for |
|------|-----------|
| `session.json` | `Session` — valid session with chat capability and account caps |
| `session_malformed_chat_cap.json` | `Session` — chat capability with wrong type for `maxBodyBytes` (reject/error case) |
| `session_with_ws_and_push.json` | `Session` — session advertising WebSocket and web-push capabilities |
| `request_chat_get.json` | `JmapRequest` — two-call batch: `Chat/get` + `Message/get` |
| `response_chat_get.json` | `JmapResponse` — two-call batch response: `Chat/get` + `Message/get` |
| `call_response.json` | `JmapResponse` — minimal single-call response (compact form) |

### `session/` — Session object validation cases

| File | Oracle for |
|------|-----------|
| `session_ok.json` | `Session` — full valid session with ownerUserId, ownerEndpoints |
| `session_missing_api_url.json` | `Session` — session with empty `apiUrl` (validation-failure case) |

### `methods/` — Full `JmapResponse` bodies for each method family

| File | Oracle for |
|------|-----------|
| `chat_get_response.json` | `Chat/get` response — list with one Chat object |
| `chat_query_response.json` | `Chat/query` response — ids list, canCalculateChanges |
| `chat_changes_response.json` | `Chat/changes` response — one created id, empty updated/destroyed |
| `message_get_response.json` | `Message/get` response — list with one Message object |
| `message_query_response.json` | `Message/query` response — ids list, canCalculateChanges |
| `message_changes_response.json` | `Message/changes` response — one created id, empty updated/destroyed |
| `message_create_response.json` | `Message/set` response (create path) — created map with server-assigned id |
| `message_set_update_response.json` | `Message/set` response (update path) — updated map with message id |
| `message_set_destroy_response.json` | `Message/set` response (destroy path) — destroyed list |
| `message_query_changes_response.json` | `Message/queryChanges` response — one added item |
| `chat_contact_get_response.json` | `ChatContact/get` response — list with one ChatContact |
| `read_position_get_response.json` | `ReadPosition/get` response — list with one ReadPosition |
| `read_position_set_response.json` | `ReadPosition/set` response — updated map |
| `presence_status_get_response.json` | `PresenceStatus/get` response — list with one PresenceStatus |
| `read_position_changes_response.json` | `ReadPosition/changes` response — one updated id |
| `presence_status_set_response.json` | `PresenceStatus/set` response — updated map |
| `presence_status_changes_response.json` | `PresenceStatus/changes` response — one updated id |
| `custom_emoji_get_response.json` | `CustomEmoji/get` response — list with one CustomEmoji |
| `custom_emoji_changes_response.json` | `CustomEmoji/changes` response — one created id |
| `custom_emoji_set_response.json` | `CustomEmoji/set` response — created map |
| `custom_emoji_query_response.json` | `CustomEmoji/query` response — ids list |
| `custom_emoji_query_changes_response.json` | `CustomEmoji/queryChanges` response — one added item |
| `space_ban_get_response.json` | `SpaceBan/get` response — list with one SpaceBan |
| `space_ban_changes_response.json` | `SpaceBan/changes` response — one created id, empty updated/destroyed |
| `space_ban_set_response.json` | `SpaceBan/set` response — created map |
| `space_invite_get_response.json` | `SpaceInvite/get` response — list with one SpaceInvite |
| `space_invite_changes_response.json` | `SpaceInvite/changes` response — one created id |
| `space_invite_set_response.json` | `SpaceInvite/set` response — created map |
| `method_error_response.json` | `JmapResponse` — `error` invocation (unknownMethod) |

## When to Commit a Generation Script

Commit a script when any of the following is true:

- The fixture contains UTCDate/RFC 3339 timestamps that were computed rather
  than typed from spec prose.
- The fixture contains ULID values that encode a specific timestamp or ordering
  property under test.
- The fixture contains cryptographic material (keys, signatures, hashes).
- The fixture was produced by running an external tool (e.g. `python3`,
  `openssl`, `jq`) and the exact invocation is not obvious from the file.

Name the script `<fixture_basename>.py` (or `.sh`) and place it in the same
directory as the fixture.  The script must be fully standalone: no network
access, no dependencies on the Rust code under test.

## Adding a New Fixture

1. Write the JSON by hand from the spec, or use an external tool (Python,
   `jq`, `openssl`).  Do not generate it from the Rust code under test.
2. Place it in the appropriate subdirectory (`types/`, `jmap/`, `session/`,
   or `methods/`).  Create a new subdirectory only if none of the existing
   categories fit and more than one fixture will share the category.
3. If you used a script, commit the script alongside the fixture.
4. Add a row to the table in this file.
5. Write (or extend) a test in `crates/jmap-chat/tests/` that loads the
   fixture using the `fixture(name)` helper (which calls `fs::read_to_string`
   at runtime) and asserts the deserialized value against a hand-constructed
   Rust value, or asserts a specific deserialization error for rejection cases.
   For method-wrapper tests, mount the fixture as a wiremock response body and
   add a `body_json` matcher to verify the outgoing request shape.
