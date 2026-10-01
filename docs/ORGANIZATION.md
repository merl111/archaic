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

The directory and hierarchy use 50-result pages. **Continue** keeps the same operation/arguments and rejects repeated cursors. Hierarchy requests descend up to three levels. **Join room** uses the selected room; joining remains an explicit action. Existing invitation accept/decline and private group/direct-chat creation remain available in the conversation sidebar.

Room information includes name, topic and power-level state. Member results include membership and power. SDK permission checks reject known forbidden changes before sending; the homeserver remains authoritative if state changes concurrently. A requested power level cannot exceed your own. Moderation confirms scope and displays errors. Space linking requires space permissions and rejects a direct self-child; it does not establish reciprocal `m.space.parent` state automatically.

Favorites synchronize through Matrix room tags and sort first in the conversation list. Blocking uses Matrix ignored-user account data. Success updates the current account's block set immediately; sync applies remote changes. Timeline, details, search and notifications omit blocked senders, including cached or delayed UI results. Unblocking allows new content; refresh/reopen to repopulate messages removed from the in-memory view. Blocking is not a server ban or a promise that the blocked person cannot contact the account elsewhere.

## Limits and implementation contract

Supported settings above are independent operations; there is no multi-setting server transaction. Large member lists currently load through the SDK rather than a virtualized, paginated profile browser. Restricted join-rule configuration, avatar/alias/publishing controls, low-priority tags, room upgrades/retention, reporting, comprehensive nested-space cycle handling and invitation policies remain open. These do not become implemented merely because the generic state protocol exists.

`archaic-matrix/src/organization.rs` owns typed operations, validation and SDK calls. `archaic-app/src/organization.rs` owns inputs, confirmation, pagination and request identity. Responses must match the active account epoch and pending transaction. UI controls never issue SDK calls directly. English/German catalogs contain the chrome and errors.

Run `python3 tools/dev.py test`. Protocol tests cover directory/hierarchy pagination, settings/moderation/tag/block/space-child request payloads, forbidden writes and power escalation; native Linux interaction checks reach this panel through its shortcut. Windows/macOS runtime validation is deferred.
