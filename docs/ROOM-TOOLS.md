# Attachments, search, receipts and message relations

Updated October 1, 2026. Open a joined conversation and choose **Room tools**. Modes cover attachments, search, message details and receipts. English and German catalogs cover the controls and errors. This guide describes implemented behavior and limits; it is not a declaration of Element parity.

## Attachments

Choose a file, then **Send attachment**. Files must be regular files of at most **25 MiB**. The worker reads a bounded byte snapshot away from the UI thread and inserts it into the SDK's durable attachment queue. Saved accounts retain queued uploads across restart. Temporary sessions do not. Encryption and message transactions belong to the Matrix SDK. Recognized image signatures determine image message types; other files are sent as files.

The outgoing row shows upload progress when the SDK reports it, paused sends, retry and cancel. Queue insertion is not delivery. Cancellation races completion: it cannot guarantee recalling an already delivered message or deleting an orphaned server upload. Upload input is buffered once, up to 25 MiB; it is not an unbounded file stream.

Select an attachment message and open **Message details**:

- **Preview image** downloads on demand, verifies encrypted media, then decodes PNG/JPEG/GIF/WebP into a native CUI image. Decoding is bounded to 8192 pixels per dimension and a 64 MiB decode budget; previews fit within 1024×768 without enlarging small images. GIF is a still preview. No external handler runs automatically.
- **Save attachment…** uses a destination you choose. Downloads stream in bounded chunks through the authenticated Matrix media API. Both advertised size and bytes received are checked against 25 MiB. Encrypted attachments are decrypted into a private temporary file and their integrity is checked before publication. The destination must not already exist; saving never overwrites it.
- **Cancel transfer** cancels the active preview/download or queue-submission task. Temporary download files are cleaned up. There is one active transfer task per account; queued SDK uploads have their own outgoing controls.

Progress uses declared length when available; an unknown length cannot produce an accurate percentage. Authenticated media requires the v1 media endpoint, rejects redirects, and has a finite timeout. A token refresh is attempted once on authorization failure. Saving resolves the original attachment event. Caption editing, drag/drop/paste, a zoomable gallery, audio/video playback and forwarding media are not implemented.

## Search

Search is scoped to the current room. Enter a query, press Enter or **Search messages**, select a result and choose **Open result**. Results can open events beyond the timeline's 500-event window. **Show surrounding messages** opens their context, with older/newer navigation.

| Mode | Behavior |
| --- | --- |
| Local history scan | Requests 50-event backward pages and searches readable text without sending the query to the server. Continue scans another page. This session-local scan may encounter an edit on a different page. |
| Server search | Calls Matrix `/search` for this room's body field, recent first, with server pagination. Disabled for encrypted rooms. Queries are disclosed to the homeserver. |
| Persistent encrypted index | Searches locally retained, decrypted events; sync adds new events and relations. Continue explicitly backfills older 50-event pages. Cached results can be searched without another history request. |

The persistent mode accepts case-insensitive text plus `from:@user:server`, `has:file`, `after:TIMESTAMP` and `before:TIMESTAMP`. Timestamps are Unix milliseconds; after is inclusive and before exclusive. Example:

```text
release from:@alice:example.org after:1759276800000 has:file
```

A result snippet marks matching text with `⟦…⟧`, preserving Unicode boundaries. It is plain native text, not interpreted HTML. Filter-only queries are supported. Queries are limited to 1024 UTF-8 bytes. Changing a query or mode starts a new search. Failed pagination preserves the previous results and cursor; cyclic and malformed pages are rejected.

The persistent event index is encrypted with the profile key and device-specific authenticated context in `search-index.enc`. It survives restart in saved accounts and stays memory-only in temporary accounts. Edits update projected results; redactions erase cached bodies and retain tombstones so older responses cannot resurrect them. Sync persistence is batched once per sync and failed writes are retried. It is an event cache searched by scanning, not an inverted full-text engine. It is bounded to 100,000 events per room and a 64 MiB encrypted file; results stop at 1000 with a visible truncation indication. The separate history scan bounds continuation cursors to 4096.

Unreadable events and coverage are reported. Exhausted pagination does not mean all encrypted content was decryptable. Indexing covers observed/backfilled history, not an automatic download of every room. Global search, dedicated index maintenance controls and a scalable full-text engine remain open.

## Live message details and relations

Select a message and choose **Load / refresh details**. The worker fetches the original and up to 100 relation events, separately from the main timeline. **More relations** follows the relation cursor. Details show projected body, same-author edit history, reactions, thread replies, reply context, and known public receipts. An unavailable original is labeled; **Replied-to message** navigates to the original.

While details are open, sync applies relevant new edits, reactions, redactions, thread replies and receipt changes. A limited sync gap reloads the first relation page and exposes remaining pagination again; failure is visible rather than silently presenting the old snapshot as current. Live refresh does not fetch all historical pages on every sync.

Projection checks sender identity and event/room IDs, resolves edit order, excludes redacted edits, and removes redacted originals' bodies, attachments and reactions. A failed page does not partially replace the view or advance its cursor. Inspection is bounded to 10,000 relation events and 4096 cursors. “All server-returned relation pages loaded” only means that cursor is exhausted; permissions, retention and unavailable keys still limit coverage. A dedicated thread inbox and thread unread navigation remain open.

## Receipts

Choose **Private** or **Public** and mark the selected message as read. The thread option uses the selected reply's actual thread root, or the selected root event itself. Unthreaded receipts remain available. The SDK can clear the room's manual unread flag when sending a receipt.

Details separate public unthreaded and thread receipts anchored exactly on the selected event. These are the receipts known to the SDK, not every person who has read the message; other users' private receipts are never exposed. Opening details does not itself send a receipt. The separately enabled private-while-composing behavior is described in [messaging](SECURITY-MESSAGING.md).

## Agent implementation map and validation

- `archaic-matrix/src/transfers.rs`: background tasks, progress, cancellation, streaming/decrypting media and bounded image decoding.
- `attachments.rs`: shared media validation and compatibility direct-upload path.
- `search_index.rs`, `local_data.rs`, `storage.rs`: index projection and encrypted persistence.
- `details.rs`, `sync_activity.rs`, `worker.rs`: paginated/live relations, receipts and sync updates.
- `archaic-app/src/tool_controller.rs`, `tool_controls.rs`, `tool_state.rs`, `search_snippet.rs`: native UI state and epoch/request validation.

Commands/events are bounded and session-scoped. Native handles stay on the GUI thread. Stale room/request results are discarded; blocking filters apply to cached and arriving views. CUI remains a separate pinned dependency.

Protocol checks cover pagination, malformed responses, encrypted integrity failures, cancellation, preview decoding, live relation updates and thread receipt payloads. The disposable Synapse test additionally queues an encrypted attachment, decrypts it on another device and verifies downloaded bytes. Windows/macOS native validation remains deferred.
