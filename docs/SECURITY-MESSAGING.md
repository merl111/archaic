# Security and messaging workflows

Updated October 1, 2026. This increment covers work in priorities 1–3. It does **not** complete those priorities or Element parity. CUI remains a separate pinned dependency; no backend code was copied into Archaic.

## Sign-in and token rotation

Password login requests refresh tokens. Where supported, the SDK refreshes expired access tokens automatically. A synchronous session callback writes the latest access and refresh tokens into the existing authenticated, encrypted `session.enc` record. Writes retain the same device, crypto store and fingerprint. A failed save produces the session-storage warning on the next successful sync and is retried from current in-memory tokens. No plaintext token file is created. A crash between server-side rotation and durable local storage can still require reauthentication.

**Sign in with your browser (SSO)** checks that the homeserver advertises classic Matrix SSO, opens its SDK-generated URL, and receives a one-time login token on an SDK-managed loopback listener. It uses the same encrypted persistence as password login. **Cancel** or a five-minute timeout drops the listener. Classic SSO remains distinct from **Sign in with OAuth**, which uses the SDK's delegated authentication discovery, dynamic native-client registration, authorization code with PKCE, and a bounded loopback callback. Callback host/state and response shape are checked; a bad state does not consume the login. OAuth client ID and rotating tokens are restored from encrypted storage. Set `ARCHAIC_OAUTH_CLIENT_URI` to the official HTTPS client URL when registration requires one. For a pre-registered native client, set `ARCHAIC_OAUTH_CLIENT_ID` and `ARCHAIC_OAUTH_ISSUER`; the issuer must exactly match discovered server metadata. `ARCHAIC_OAUTH_CALLBACK_PORT` optionally fixes the loopback port for registrations with a fixed redirect URI (`http://127.0.0.1:PORT/callback`). Without those overrides the SDK uses dynamic registration and an ephemeral port. Existing saved client IDs take precedence. Protocol tests cover PKCE, callback rejection/closure and restoration; a real authentication-service interoperability matrix remains pending.

A saved-session **soft logout** exposes same-device password, classic SSO or OAuth reauthentication. Choose the browser method matching the original session. The request supplies the original device ID, validates the returned identity/device and resumes the same crypto store and fingerprint. Pending drafts survive. Browser reauthorization disables queue submission and token-persistence callbacks until identity validation succeeds. Cancellation, timeout or a rejected identity discards the client and reloads the original encrypted session. OAuth checks both user and device from whoami; classic SSO sends the existing device ID in its token login. Broader interactive-auth stages remain open.

Closing a saved session keeps it. Sign-out revokes the login and clears the saved session record; encrypted stores remain retained. Temporary sessions keep credentials, encryption state and queued messages in memory and cannot survive process exit.

## Security & recovery

Open **Security & recovery** in the account sidebar. The mode selector exposes:

1. **Devices & verification.** Refresh fetches current devices. Select another device and choose **Verify selected device**. Accept an incoming request or start comparison when ready. Compare all emoji/descriptions or the decimal numbers on both devices. Choose **They match** only after comparison, or **They do not match** / **Cancel**. Buttons follow SDK verification state; stale flow IDs and premature confirmation are rejected. This implements SAS verification. QR scanning, full other-user verification discovery, device rename/revocation and automatic incoming-request notifications are still pending. Refresh the device list after verification to update cached trust labels.
2. **Recovery & identity.** For a new account, **Set up identity** asks the SDK to bootstrap missing cross-signing; password UI authentication is supported. Existing identities are not reset to work around missing private keys. Use **Unlock recovery** with an existing key/passphrase first. **Create recovery key** is available for a new setup and refuses to replace existing secret storage. Save the returned key somewhere safe, then explicitly acknowledge it to hide it. Secret command values have redacted Debug output and zeroizing application storage. Native password fields and SDK-internal buffers are not claimed to be guaranteed secure erasure.
3. **Encrypted room-key files.** Export uses the SDK Megolm export format and a passphrase of at least 12 characters. The destination must not exist; a private temporary file is synced and persisted without overwriting another file. Import checks regular-file size (25 MiB maximum) and asks the SDK to decrypt/import. Incorrect passphrases fail without plaintext output. **Recover selected room keys** downloads that room’s keys from the configured backup. Successful import/recovery reloads the selected history. Automatic per-event missing-key retries, trusted-history explanations and recovery-aware cache deletion still need work.

This is development software. The tests below establish specific real-server flows, not complete production encryption acceptance or interoperability with every Element version.

## Outgoing messages and connectivity

Ordinary composer text uses the Matrix SDK send queue. The composer clears only after queue insertion succeeds. Each queued item has an SDK-owned transaction ID. In saved sessions the SDK SQLite store retains pending messages across restart; retry reuses the transaction rather than creating another send. Existing queue tasks are restored when the account opens.

The outgoing row shows the selected room’s pending text and whether sending has paused. **Retry selected message** clears a blocking error and enables that room’s queue. **Cancel selected message** uses the SDK cancellation handle; if a send wins the race, the SDK may redact it. Successful sends disappear from the queue and arrive through sync. Recoverable queues resume when sync reconnects; permission/encryption failures can remain blocked for explicit retry. A successful queue write is not a delivery acknowledgement.

The native reply, thread, edit, reaction, forward and delete actions also insert into the SDK queue, as do attachment uploads. Target-dependent actions still fetch and validate their target before insertion, so creating every action while fully offline is not supported. Retry/cancel use SDK queue handles after insertion; queue acceptance is distinct from server delivery.

Unsent ordinary composer drafts are encrypted in device-scoped `drafts.enc`, saved after edits on the one-second activity tick, and restored per room. Saved accounts support up to 2048 nonempty drafts of 32,000 UTF-8 bytes each. Save errors remain visible and are retried; account switching waits for success. Graceful shutdown first drains prior send acknowledgements, then saves a final draft batch with a bounded wait. Abrupt termination can lose the latest unsaved edits. Reply/edit form drafts, configurable send preferences and a full offline conversation browser remain open.

## Typing, unread state and receipts

The room list displays server unread notification counts. Incoming typing names expire locally after 30 seconds if no update clears them.

**Share typing status** is off by default. When enabled, typing is announced only while the visible composer has focus and has changed recently. It stops after five idle seconds or when leaving the composer, with SDK throttling for continued typing. These controls reset on sign-out.

**Mark read privately while composing** is also off by default. It sends a private unthreaded receipt through the latest loaded message only while the visible composer has focus; repeated attempts are throttled. Opening a background room, a security page or room tools does not mark messages read. Public receipts remain an explicit Room tools action. These controls are not yet persisted preferences or a full viewport-based read tracker. Room tools separately offers explicit thread-scoped receipts, resolving the actual thread root. Details display known public thread receipts separately from unthreaded receipts.

## Timeline and conversation actions

The main timeline uses CUI’s native table model; Linux GTK recycles visible cells. It preserves event selection by ID, updates changed cells instead of rewriting every row, and shows plain-text message bodies, loaded replies, edits, reactions and attachments. This reuses the existing public CUI table API. It is not yet a dedicated rich chat component, and row insertion can still change scroll position.

Successful sync applies new timeline events to the loaded window and recomputes loaded relations. Unchanged syncs do not fetch `/messages` or emit duplicate history snapshots. Explicit Refresh and operations that require a fresh target still fetch history. Limited, disjoint syncs retain an explicit gap indication instead of joining disconnected ranges.

**Reply in thread** asks the SDK to construct the correct `m.thread` relationship, continuing an existing thread or starting one at the selected root. **Room tools → Message details** includes loaded thread replies and allows paging remaining relations. Open details now applies sync relations and receipt changes automatically, with first-page reload after a limited sync gap. **Rooms & organization → Browse threads / Threads I participated in** lists paginated thread roots and opens selected roots in live message details. Each row reports the server reply count and whether an own scoped receipt matches the latest bundled reply. This is an exact-receipt indicator, not a complete unread counter. Subscription controls use the SDK’s server-dependent thread-subscription extension; unsupported servers return an error.

**Forward text** asks for the destination’s full `!room:server` ID, then sends the selected original text with its sender attribution. The destination must be joined; the SDK encrypts it when required. Attachments cannot be forwarded by this control. **Show surrounding messages** requests `/context` for the selected message, including targets opened from search. **Load older** and **Newer messages** follow separate cursors. **Latest** returns to the live end. An anchored context stays fixed while newer pages remain available.

The selected window remains bounded to 500 raw events and 128 cursors per direction. Cursor loops and malformed/cross-room responses leave existing data intact. This is not unlimited scrollback or guaranteed pixel-stable positioning. Matrix formatted HTML, Markdown/rich editing, mention completion and a full unread-thread dashboard remain pending.

## Code map for agents

- `archaic-matrix/src/security.rs`: SDK verification/recovery operations, safe secret wrapper, export/import boundary.
- `connection.rs` / `oauth.rs` / `token_store.rs`: authentication, sync task, encrypted rotation callbacks, queue observer lifecycle.
- `local_data.rs`, `search_index.rs`: device-scoped encrypted drafts and incremental search data.
- `organization.rs` / `links.rs`: room administration and validated Matrix link targets.
- `archaic-app/src/instance.rs`, `desktop.rs`, `status_item.rs`: desktop activation, notifications, shortcuts and tray/dock integration.
- `outbox.rs`: queue projection and retry/cancel operations; SDK owns persistence and transactions.
- `history.rs` / `timeline.rs`: bounded cursors, context and incremental relation projection.
- `worker.rs`: serialized commands and epoch-scoped UI events; no CUI handles cross threads.
- `archaic-app/src/security.rs`, `outbox.rs`, `activity.rs`, `timeline_view.rs`: native controls and presentation state. English/German strings live in `locales`.

Keep authentication and sync epoch guards, preserve transaction identity, and never replace an existing cross-signing identity merely because this device lacks private keys. Add protocol and failure tests before expanding each boundary.

## Validation and reproducible live test

`python3 tools/dev.py test` runs the normal suite. The additional ignored test creates disposable accounts on an explicitly selected loopback Synapse. It covers two-device SAS code agreement, cross-signing, recovery on a second device, encrypted key export/import with a wrong-passphrase rejection, non-destructive recovery setup, encrypted messages in both directions, queued encrypted delivery, encrypted attachment upload/download with exact-byte verification, and room backup-key retrieval. It does not inspect the user's account, Element data or credential vault.

With Docker and the normal build prerequisites:

```sh
python3 tools/test_live_matrix.py
```

The helper uses an image pinned by digest, binds its registration-enabled server to loopback only, and removes its uniquely named container and volume on exit. Windows/macOS native UI verification remains deferred. A compatible server already running on loopback can instead be used with `ARCHAIC_TEST_SERVER=http://127.0.0.1:PORT` and the ignored `live_security` test. Never point this test at a production server.

Normal protocol tests additionally cover queue persistence/retry identity after restart, encrypted token rotation/restoration, classic SSO callbacks and listener cancellation, OAuth PKCE/state checks/restoration, same-device password soft logout, encrypted draft restart, thread/forward payloads, incremental edits without repeated history requests, and context/forward pagination. Native Xvfb checks inspect the Linux controls; no Windows/macOS run is claimed.
