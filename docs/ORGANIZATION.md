# Rooms, spaces and moderation

Open **Rooms & organization** (or Ctrl/Cmd+Shift+O) while connected. This is a functional administration surface; visual redesign remains separate. Select an operation and fill **Target**, **Value**, and **Extra** as below. Target defaults to the current room. Mutations require the confirmation checkbox. Select a result and choose **Use selection** to copy its room ID into Target or its user ID into Value.

| Operation | Target | Value | Extra |
| --- | --- | --- | --- |
| Public room directory (current homeserver) | — | Search text | — |
| Joined spaces | — | — | — |
| Space hierarchy | Space ID | — | — |
| Room information / members | Room ID | — | — |
| Create private space | — | Name | Topic |
| Change room name / topic | Room ID | New text | — |
| Join rule | Room ID | `public`, `invite` or `knock` | — |
| History visibility | Room ID | `world_readable`, `shared`, `invited` or `joined` | — |
| Guest access | Room ID | `can_join` or `forbidden` | — |
| User power level | Room ID | Full user ID | Integer level |
| Invite / kick / ban / unban | Room ID | Full user ID | Optional reason |
| Favorite / remove favorite | Room ID | — | — |
| Block / unblock | — | Full user ID | — |
| Blocked users | — | — | — |
| Add / remove space child | Space ID | Child room ID | — |
| Create/remove/canonical alias | Room ID | Full `#alias:server` | — |
| Publish / unpublish | Room ID | — | — |
| Low / normal priority | Room ID | — | — |
| Upload / clear avatar | Room ID | Local image path for upload | — |
| Restricted membership | Room ID | Space-separated allowed room IDs (1–20) | — |
| Event permission | Room ID | Event type, e.g. `m.room.message` | Integer level |
| Retention | Room ID | Minimum lifetime in milliseconds | Maximum lifetime in milliseconds |
| Upgrade version | Room ID | New room version | — |
| Report / moderator redact | Room ID | Event ID | Reason |
| Knock | Room ID or alias | Optional reason | — |
| Add/remove parent | Child room ID | Joined parent space ID | — |
| Browse / participated threads | Room ID | — | — |
| Subscribe/unsubscribe thread | Room ID | Thread-root event ID | — |

The directory, hierarchy and thread list use 50-result pages. **Continue** keeps the same operation/arguments and rejects repeated cursors. Hierarchy requests descend up to three levels. Thread rows omit blocked senders. Use selection on a thread opens its root in live message details; **Run** reloads the thread catalog, which is not continuously refreshed. The receipt marker ✓ means an own public/private receipt matches the latest bundled event; ● means no such match is known, not a proven unread count. **Join room** uses the selected room; joining remains an explicit action. Existing invitation accept/decline and private group/direct-chat creation remain available in the conversation sidebar.

Room information includes name, topic and power-level state. Member results include membership and power. SDK permission checks reject known forbidden changes before sending; the homeserver remains authoritative if state changes concurrently. A requested power level cannot exceed your own. Moderation confirms scope and displays errors. Space linking requires space permissions and rejects a direct self-child; it does not establish reciprocal `m.space.parent` state automatically.

Favorites synchronize through Matrix room tags and sort first in the conversation list. Blocking uses Matrix ignored-user account data. Success updates the current account's block set immediately; sync applies remote changes. Timeline, details, search and notifications omit blocked senders, including cached or delayed UI results. Unblocking allows new content; refresh/reopen to repopulate messages removed from the in-memory view. Blocking is not a server ban or a promise that the blocked person cannot contact the account elsewhere.

## Limits and implementation contract

Supported settings above are independent operations; there is no multi-setting server transaction. Large member lists currently load through the SDK rather than a virtualized, paginated profile browser. The additional operations are independent, permission-checked SDK requests. Alias removal/canonical changes first resolve the alias and reject another room. Event permissions preserve other power-level fields and cannot exceed the actor’s level. Retention requires 0 ≤ minimum ≤ maximum and depends on server policy/support; setting the event does not itself delete history. Upgrades return the replacement room ID. Parent and child links are separate operations, not an atomic reciprocal update. Direct self-links are rejected; comprehensive nested-space cycle handling and invitation policies remain open.

`archaic-matrix/src/organization.rs` and `room_settings.rs` own typed operations, validation and SDK calls; `room_threads.rs` owns decrypted thread pagination and scoped-receipt indicators. `archaic-app/src/organization.rs` owns inputs, confirmation, pagination and request identity. Responses must match the active account epoch and pending transaction. UI controls never issue SDK calls directly. English/German catalogs contain the chrome and errors.

Run `python3 tools/dev.py test`. Protocol tests cover directory/hierarchy pagination, settings/moderation/tag/block/space-child request payloads, forbidden writes and power escalation; native Linux interaction checks reach this panel through its shortcut. Windows/macOS runtime validation is deferred.
