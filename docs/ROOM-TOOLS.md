# Attachments, search, receipts and message relations

Open a joined conversation and press **Room tools** in its header. The mode selector offers Attachments, Search messages, Message details and Read receipts. Press **Room tools** again to return to the timeline and its preserved composer draft. All controls and errors are available in English and German.

## Attachments

Choose **Attachments → Choose file…**. The native file chooser returns a path; nothing is sent until you press **Send attachment**. The selected basename is displayed. Cancel discards the selection and any retained upload bytes. Files must be regular files of at most **25 MiB**. PNG, JPEG and GIF signatures select the corresponding image message type; other files use a generic file message. No filename extension is trusted to determine executable behavior.

The worker reads an immutable byte snapshot off the UI thread, then uses Matrix SDK attachment sending. For encrypted rooms the SDK encrypts the attachment and message; Archaic does not implement cryptography. An unchanged failed upload retry retains the bytes and message transaction ID, even if the local file has changed. A retry can upload the media again before retrying the same message transaction: orphaned server uploads are possible, but the message transaction remains stable. Retry state is session-local, not a durable outbox. Selecting a different file creates a new transaction when submitted.

To save a received attachment, select its message below the timeline, open **Room tools → Message details**, and press **Save attachment…**. Choose a new destination filename. Archaic fetches the original event's media through the SDK, including authenticated media and decryption where required, writes a temporary file alongside the destination, and atomically persists it **without overwriting an existing file**. An untrusted filename from a message is never used as a destination path. Downloads are never automatically opened or executed. Saving currently uses the original event's attachment, even if later replacements change its caption or media.

The application rejects advertised files above 25 MiB and checks the actual downloaded size before saving. The SDK buffers the response before the latter check: **25 MiB is not a strict download-memory or network-transfer ceiling**. There is no streaming transfer, progress percentage, in-flight cancellation, caption editor, thumbnail gallery, media playback or durable upload queue yet. The native dialog can be cancelled before submission; once submitted, the visible busy state lasts until the SDK returns.

## Search

Search is scoped to the current room. Enter a query and press **Search messages** or Enter. Select a result and press **Open result** to inspect it, even if it is outside the 500-event timeline window. Results remain available when switching back from Message details.

- **Local history scan** requests backward pages of 50 events through the SDK, searches readable message text case-insensitively, and sends no search query to the server. This includes encrypted messages for which this device has keys. **Continue search** scans the next older page. It can continue beyond the main timeline's 500-event window. It is an on-demand scan, not a persistent encrypted search index. Results may contain historical text when a replacement lies on another page; opening the result resolves its current relations. Events without readable content are counted explicitly, so exhausted pagination does not imply complete decryption.
- **Server search** uses Matrix `/search`, filters to the current room and body field, and requests recent-first pages of 50 results. **Continue search** uses the returned search cursor. Servers may use their own tokenization and indexing rules. This mode is blocked for encrypted rooms before sending the query; choose Local history scan there. The homeserver must implement search and authorize access to the room.

Changing the query or mode requires a new search. A failed continuation preserves its last successful results and cursor; retry continues from that same cursor. Overlapping results are deduplicated by event ID. Repeated/cyclic cursors, oversized pages, malformed events and responses from another room are rejected. The scan stops with a visible incomplete-limit error after 1,000 hits or 4,096 continuation cursors. Queries are trimmed and limited to 1,024 UTF-8 bytes. There are no sender/date filters, highlighting, global search, thread navigation or disk index yet.

## Message details and complete relation pagination

Select a message, choose **Message details**, and press **Load / refresh details**. Search's **Open result** does this automatically. Archaic fetches the original event and up to 100 related events. **More relations** follows the server cursor until every server-returned page has been fetched. This uses an independent cache, not the timeline's 500-event window. The UI reports loaded counts, unavailable content and whether more pages exist. “All server-returned relation pages loaded” means the cursor is exhausted; it does not establish that inaccessible or undecryptable events are available.

Details show the current projected body, reactions, loaded non-redacted same-author edit history, known public read receipts, and the original replied-to message. The reply original is fetched even when absent from the loaded timeline; an unavailable reply is labeled. **Replied-to message** opens that original's own details and relation pages. The existing Reply/Edit/Delete/React controls also work on the message opened from search. This is targeted event inspection, not forward/backward context navigation around the event.

Valid server-bundled replacement events are also consumed by the normal timeline. Projection enforces author identity, redaction and deterministic edit ordering. Explicit relation pages override duplicate bundled event IDs. Original redaction removes its body, reactions and attachment; redacted revisions are excluded. Reactions preserve event IDs for the existing own-reaction removal action. Relation snapshots are not continuously refreshed: use Load / refresh details to see subsequent edits, redactions and reactions.

A failed page is atomic: it does not partially change the snapshot or advance its cursor. Relation loading allows at most 10,000 distinct related events and 4,096 cursors per inspection, then reports an incomplete-limit error. Server permissions, retention and missing encryption keys still constrain what is returned. Thread presentation, relation-aware incremental timeline updates and a full event-context navigator remain unfinished.

## Read receipts

Select a message, open **Read receipts**, choose **Private** (default) or **Public**, and press **Mark selected as read**. Private uses `m.read.private`; Public uses `m.read`. Both are explicitly unthreaded. Merely opening a room, search result or details sends no receipt. The SDK also clears the room's manual unread flag when sending a receipt. A homeserver may reject unsupported receipt types; errors remain visible.

Message details lists public receipts known to the SDK sync store that are anchored exactly on the selected event. This is **not an exhaustive list of everyone who has read it**, and it does not disclose other users' private receipts. Load / refresh details to refresh that list. Automatic viewport-based receipts, thread-specific receipts, unread counts, fully-read markers and cross-client unread acceptance remain future work.

## State, security and integration contract

`archaic-matrix::Command::RoomTool(ToolRequest)` carries the room, operation, transaction identity and arguments. `EventKind::RoomToolResult` echoes that request with `ToolData` or a localized error key. GUI state applies a response only for the current session epoch, room and exact pending request. `Command::DiscardUpload` releases the worker's retained upload snapshot. Room switches, membership loss and logout clear the tool state. Joined membership is rechecked in the worker; fetched event identity and room are validated before acting.

All SDK work runs on the Matrix worker, file reading/saving uses blocking workers, and native CUI handles stay on the main thread. Conflicting room/message operations are disabled while a tool or file dialog is active; sign-out remains reachable. Channels remain bounded. Server token expiry revokes the active worker session and stops sync. No plaintext search index is written to disk.

This batch uses the existing public CUI Rust binding and native file dialogs; CUI and its revision pin do not change. Tests use the real SDK against disposable loopback mocks and native Linux controls under Xvfb. These checks do not qualify production E2EE, real-server interoperability or Windows/macOS behavior.
