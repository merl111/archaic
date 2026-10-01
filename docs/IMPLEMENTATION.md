# First implementation increment

October 1, 2026. This is a partial implementation of the approved plan; no entire parity milestone is complete. Current workflows are documented in [Security and messaging](SECURITY-MESSAGING.md), [Room tools](ROOM-TOOLS.md), [Organization](ORGANIZATION.md) and [Desktop integration](DESKTOP.md). Earlier increment sections below are historical records.

## Implemented

- Three-crate Rust workspace with a checked-in dependency lockfile and pinned sibling CUI revision.
- Static CUI build helper using CMake; no CUI source or backend implementation copied into Archaic.
- Native sign-in and split conversation view, system-themed controls, selectable message text, native password field and text composer.
- Embedded Fluent English/German catalogs, explicit locale selection, English fallback and catalog tests.
- Matrix SDK 0.19.1 password authentication, classic sync, joined rooms, recent events and bounded backward pagination, text sending and server-side logout.
- Tokio worker service with bounded channels; CUI handles remain on the main thread.
- Per-room drafts, stable transaction IDs for unchanged failed-send retries, stale-session/history rejection and private view clearing on sign-out.
- Network operations use finite request timeouts; sync retries with delay. No custom cryptographic implementation.

## Deliberate limits

Normal startup restores a saved device and SDK stores. SDK refresh callbacks now persist rotated tokens. Classic browser SSO, native SAS verification, cross-signing bootstrap, recovery setup/unlock, encrypted key-file import/export and room backup-key retrieval are implemented. Real Synapse tests cover two devices, SAS/recovery and encrypted sends. Delegated OAuth/PKCE with encrypted session restoration and same-device password soft logout now have protocol coverage. OAuth server compatibility, OAuth/SSO reauthorization, complete interactive authentication and broader interoperability remain unfinished.

Ordinary text uses the persistent SDK send queue, with retry/cancel and restart tests. The timeline applies incremental sync events to native CUI table rows; context and newer pagination supplement backward paging. Threads can be replied to and inspected through loaded relation pages. Unread counts and typing are exposed, with optional private receipts while composing. Rich message rendering/editing, mention completion, rich thread UX, unlimited scrollback and a full offline history browser remain open. Native message actions/media use SDK queues; ordinary unsent drafts now have encrypted persistence. Attachment previews, streamed integrity-checked downloads, progress/cancellation, persistent encrypted search with filters, live details and thread receipts are implemented within the documented bounds. See the current guide for exact limits.

English/German are the initial catalogs. Runtime language changes, OS locale detection, pluralized dynamic labels, translation workflow and RTL framework work remain open. CUI and app native Windows/macOS verification remain deferred as requested; no portability claim here substitutes for those runs.

## Remaining implementation order

1. Expand session lifecycle acceptance: OAuth/SSO soft logout, static OAuth registration, recovery-aware local-data purge and real-server failure/crash testing.
2. Expand device verification/recovery UX and cross-client interoperability beyond the tested two-device Synapse flows.
3. CUI dispatcher and disposable callback/subtree lifetime contracts; virtualized rich timeline and editable composer capabilities in CUI, with bindings and tests.
4. Incremental SDK timeline models, forward/context pagination, media previews/streaming, room directory and spaces. Room tools now implement bounded complete relation pagination, attachment transfer, explicit receipts and room-scoped search. Basic invitation actions, bounded backward pagination and loaded-message reply/edit/delete/reaction actions are implemented.
5. Continue the feature acceptance checks in the parity inventory. Calls, widgets, admin, notifications, accessibility and release packaging remain independent work packages.

## Repository boundary

Any reusable widget, rendering, input, accessibility or native-desktop capability is implemented in the separate CUI repository. The application owns Matrix protocol behavior, localization catalogs, account state, screens and packaging. Update `cui-revision` only after reviewing and checking the corresponding CUI change; build the Rust binding and static archive from that same source tree.

## Validation of this increment

Tested on Linux (Manjaro x86_64), Rust 1.98.1, GCC 16.2.1, GTK 4.22.4. CUI builds as a static archive. The workspace tests include four UI state tests, two localization tests, URL validation and two Matrix protocol integration tests. `cargo clippy --workspace --all-targets -- -D warnings` and formatting checks pass.

An isolated Xvfb run exercised the actual native controls using X11 mouse/keyboard input: entering a loopback test homeserver and credentials, selecting the synchronized room, typing/sending text, receiving that text in the timeline, and signing out. The HTTP server asserted the submitted credentials and message body. This used the real SDK transport with a local simulated homeserver, not a public account or full Synapse deployment. English/German login and conversation windows also completed graphical smoke runs. The documented build helper was checked with application arguments, and the German conversation view was visually checked at 200% display scaling.

Windows/macOS, real-server interoperability, E2EE multi-device behavior, long-session memory behavior and release packaging have not been validated for Archaic yet. The SDK's presence does not imply those acceptance checks are complete.

## Sign-in design update

The login is a compact centered card with intrinsic-width native fields, a violet/teal ambient background, localized inline errors and Enter-to-submit from the password field. Extra window space surrounds the card instead of stretching the fields; a scrollable root keeps the form accessible in short windows. The conversation footer appears only after login; the compact appearance toolbar is now available on both screens.

CUI now owns the reusable AMBIENT container role and platform implementations. GTK application button styles are scoped to the content root so native title-bar buttons retain their original metrics. The library regression compares close-button padding and measured size with a plain GTK window in both themes. The Rust binding and static library must be built from the updated pinned CUI commit.

Light/dark backgrounds are implemented; use `--theme system|light|dark` to select one at launch. The workspace increment below adds the visible, persistent System/Light/Dark switcher.

Checked on Linux: native login/send/logout mouse and keyboard flow, English light/dark layouts, German at 600×660 logical pixels and 200% scaling, nine app tests, strict Clippy, GTK title-bar/control/container tests and complete binding declaration coverage. Windows/macOS runtime verification remains deferred.

## Saved-session increment

Implemented automatic restoration of the same Matrix device, its encrypted SDK SQLite store, and a checked device-key fingerprint. Session tokens are serialized only into an authenticated XChaCha20-Poly1305 envelope; an OS-vault key unlocks it and the SDK stores. The profile is exclusively locked, record replacement is atomic, and malformed/oversized records, missing keys and missing crypto databases are rejected without replacing the saved login.

The native sign-in card now has restoration progress, localized retry errors and an explicit temporary-session action. The conversation footer distinguishes saved from unsaved sessions. Revoked tokens stop synchronization and disable messaging; logout retries distinguish failed server revocation from failed local cleanup. Temporary/fixture/smoke modes do not open the real profile. App state and all SDK calls remain separate from CUI handles.

Sign-out clears the saved token record but retains encrypted SDK databases and the OS-vault key. There is no secure-erase or full-disk-encryption claim. The SDK's encrypted values do not hide all SQLite metadata. Refresh tokens are deliberately not requested yet. No existing in-memory session can be recovered retroactively from an older build.

Validation includes isolated real-SDK protocol tests for restart without another login, logout/cleanup retry and revoked tokens, storage corruption/lost-key/locking tests, and UI state/catalog tests. Native Linux checks exercise temporary login, room selection, text sending and sign-out, and inspect light/dark, compact German and 200% layouts. `tools/test_linux_vault.py` provides an isolated real Secret Service check, separate from the user's keyring. Windows/macOS adapters are implemented through the keyring crate but still await platform qualification.

The accepted login composition is preserved. The main timeline is still the initial text view; verification/recovery, rich timeline and attachments remain upcoming work rather than implied completed features.

Checked this increment on Linux: 18 automated workspace tests, a separate passing real Secret Service round trip in an isolated keyring, strict Clippy, formatting, the native login/send/logout flow and rendered vault-error recovery controls. Structural review prompted separation of error/status/logout handling; remaining ripwire flags concern short wrapper duplication, recent UI churn, Rust test-module size and callback discovery rather than a passing structural gate.

## Conversation workspace and joining

Added a compact appearance toolbar shared by sign-in and conversations, sidebar account controls, a grouped composer hidden until a room is selected, and an inline Join a room form. Native window chrome is unchanged. The view continues to use CUI public controls; no framework change or pin update is needed for this increment. At that increment the timeline was limited to the latest 50 events; the history increment below adds bounded backward loading.

System/Light/Dark applies immediately. Normal-mode preferences are atomically replaced in a separate configuration file, with a visible save-failure warning. Launch overrides do not automatically rewrite the preference. Explicit temporary, fixture and smoke launches bypass preference storage. English and German strings cover the new controls and failure states.

Joining uses the SDK's authenticated join-by-ID-or-alias operation. Invalid input fails before a network join; forbidden/not-found/other failures preserve the entered address. Successful joining selects the returned room ID, clears the room filter, and retains existing per-room drafts. The worker updates the room list and loads recent history. Private group/direct-chat creation is added below. Public-room creation, directory browsing, Matrix URI parsing, explicit `via` routing and restricted/knock flows remain unfinished. Basic invitation and leave support was added in the membership increment below.

Validated on Linux with 22 passing automated workspace tests (the isolated real-vault test remains a separate opt-in check), strict Clippy and formatting. Tests exercise rejected input, authorization failure, alias retry, ID join, authenticated SDK requests, selected-room history, preference replacement/failure and view-state preservation. Native Xvfb input checks cover sign-in, send, join rejection/retry, room selection, live theme switching and sign-out. Light/dark, compact German and 200% layouts were rendered and inspected. Windows/macOS and live-server interoperability remain pending.

Structural review separated the join form and join-result reducer. Remaining ripwire findings include recent UI churn, declarative view/bootstrap length and Rust callback/test discovery; this is not a clean structural-gate claim.

## Existing Element account onboarding (researched, not implemented)

The desired additional entry point is “Connect an existing account,” alongside normal sign-in. Local Element account/server detection can be investigated as an opt-in convenience; an installation or saved-profile marker does not prove a live session. No Element profile or credentials are read by the current app.

Prefer supported authorization of a new Archaic device: capability-discovered one-time login-token flows, or OAuth/device/QR authorization when both homeserver and authorizing client support it. The login-token endpoint is optional and requires explicit user verification; this is not a universal password-free handoff. Verification/recovery of encrypted history is part of the acceptance criteria, not something guaranteed by obtaining a token.

Do not run Archaic and Element with a copied device token and divergent crypto stores. An exact local-session import would require explicit consent, a fully stopped Element instance, versioned credential/store conversion, preservation of encryption state, and rollback/interoperability tests. It remains a research task rather than a supported feature.

References checked October 1, 2026: [Element session lifecycle](https://github.com/element-hq/element-web/blob/develop/apps/web/src/Lifecycle.ts), [Matrix login-token specification](https://spec.matrix.org/v1.16/client-server-api/#post_matrixclientv1loginget_token), [MAS authorization flows](https://matrix-org.github.io/matrix-authentication-service/topics/authorization.html). The pinned Rust SDK 0.19.1 also exposes OAuth QR-login support; enabling a compatible desktop flow still needs application UI and interoperability tests.

## Room membership increment

Functionality work only; the existing layout is retained with the controls needed for these operations. Room summaries now distinguish joined rooms from incoming invitations and include the inviter ID when present in stripped room state. Invitations sort before joined rooms and are marked in the localized room list. Selecting an invitation never requests its message history, and both the reducer and SDK worker reject sending to non-joined rooms.

Implemented real SDK invitation acceptance, confirmed rejection, inviting a user by full Matrix ID, and confirmed leaving. Pending operations disable duplicate submission; errors are localized and retryable. The invite field is cleared only after a successful invitation or switching rooms. Leave/reject confirmation is tied to a specific room and cancelled when selection or membership changes. Successful departure clears that room's visible messages, drafts and outgoing transaction state; other-room drafts remain intact. Event epochs and room/action identity guard asynchronous completion.

The SDK's `Room::leave` also leaves known predecessor rooms in upgraded-room chains; the confirmation discloses that scope. Rejected invitations are automatically forgotten by the SDK where possible. This is not a local cache purge or secure erase. A separate forget/archive browser, member directory, permission preflight, moderation and knock/restricted-room guidance remain unfinished. The server remains authoritative for invite permissions.

Validation: 28 automated workspace tests pass, including three new membership protocol scenarios and three reducer tests. Protocol checks cover stripped invite state, inviter identity, rejected acceptance followed by retry, malformed invitee IDs, authenticated user invitations, leave failure/retry, rejection without history access/sending, and externally withdrawn invitations. Native Linux input checks cover accept, invite failure/retry, leave cancel/confirm and decline confirmation. Strict Clippy and formatting pass. These use disposable local test servers, not a real account or full homeserver deployment; Windows/macOS and E2EE interoperability remain deferred.

Structural review extracted membership operations, their state reducer and functional controls into dedicated modules. Remaining reported debt concerns short callback wrapper similarity, sequential construction/dispatch/test length and recent churn; no clean ripwire gate is claimed. CUI itself is unchanged.

Protocol reference: [Matrix room membership](https://spec.matrix.org/v1.16/client-server-api/#room-membership), plus the pinned matrix-sdk 0.19.1 `Room::join`, `Room::leave`, `Room::invite_user_by_id` and `Room::invite_details` implementations.


## Backward history increment

Added **Load older** and **Latest** to the existing native conversation controls. The worker retains a chronological event cache for the selected joined room, requests backward pages of 50 events with the server-provided cursor, and deduplicates by event ID. Overlapping recent responses replace the refreshed portion while retaining the older prefix and its backward cursor. Non-message event IDs participate in overlap detection; re-fetched redacted messages remove their previous visible body. This does not implement full live redaction or relation aggregation for older, unrefreshed messages.

The cache is capped at 500 raw events and 128 traversed backward cursors. New recent events trim the oldest entries when needed. Reaching either limit disables backward loading until **Latest** resets the window. An empty page with a new cursor can still be followed; a missing end cursor marks the start of available history. Repeated or cyclic cursors, malformed events and pages exceeding the requested limit report localized errors without advancing the cursor or replacing the current cache. If a recent page has no overlapping event IDs, it replaces the old window and sets a persistent gap notice, cleared by **Latest**. No automatic request loop attempts to bridge arbitrarily large gaps.

Commands, completion and errors carry the room ID and operation kind; session epochs and current joined membership guard UI updates. Background recent-page success does not clear a failed older-page error or finish a pending older/latest request. Room changes, membership loss and logout clear view pagination state; the worker also drops inaccessible-room history. The existing draft preservation and transaction handling remain in place.

Limitations: this is a selected-room in-memory window, not an offline history browser. Changing rooms/restarting reloads recent history. The event count bounds the window, not the byte size of individual server events. The native text view does not maintain a pixel scroll anchor after replacement. Bidirectional/context pagination, stable viewport anchoring, full edits/redactions/threads and a virtualized rich view are unfinished; ARC-03-04 is **partial**, not accepted parity.

Validation on Manjaro Linux x86_64: 37 workspace tests pass, with the optional real-vault check still separately ignored; strict Clippy and formatting pass. Added tests cover cache ordering/deduplication, overlap/gap handling, refreshed redactions, event/cursor caps, room/session/membership guards and background-refresh/error interaction. Real-SDK HTTP tests assert authorization, direction, page size, exact cursor reuse after a 503, malformed/oversized-page rejection, empty pages, repeated cursors, sync merging, end-of-history and Latest reset. An isolated Xvfb mouse/keyboard run exercised the actual buttons, a failed-page retry, live refresh retaining older messages, disabled end-of-history and loading again after Latest; screenshots were inspected. No real account or desktop credential vault was used. Windows/macOS and real-homeserver interoperability remain unverified.

Validation used an isolated export and static build of pinned CUI revision `1224059d991f7fee4fb019aadef154d275e77c42`, because unrelated CUI development was in progress in the sibling working tree. This increment does not change CUI or its pin. Structural review consolidated pagination resets and event guards; remaining ripwire findings include common callback-wrapper structure, test-module/protocol-scenario size, recent churn and Rust callback/test discovery. No clean structural gate is claimed.

Protocol reference: [Matrix room messages pagination](https://spec.matrix.org/v1.16/client-server-api/#get_matrixclientv3roomsroomidmessages) and the pinned matrix-sdk 0.19.1 `MessagesOptions`/`Room::messages` implementation.


## New conversations increment

Added **New** beside Conversations, with direct-chat and private-group modes using existing CUI controls. Direct chats accept a full Matrix ID; groups accept a required name, optional topic, and up to 20 distinct invitees. Inputs are trimmed and validated before network creation; duplicate invitees are removed. App limits are 255 UTF-8 bytes for names and 4096 bytes for topics. New rooms use the private-chat preset and an initial Megolm encryption state event; they have no public alias or directory publication. This requests encryption through the SDK; it is not acceptance of production E2EE or a replacement for device verification/recovery.

The service reopens an existing joined DM identified by the SDK, choosing deterministically by room ID when several exist. Existing room settings are preserved, including an existing unencrypted room's settings. A session-local cache also reopens a newly created DM before its account-data sync arrives, or if saving the direct-chat label failed. Left rooms are not reused. This cache is not a durable cross-restart deduplication ledger.

Creation uses the SDK's authenticated create-room request and existing no-automatic-retry configuration. A forbidden response is reported as such. Ambiguous network/server failures explicitly say that creation could not be confirmed and tell the user to inspect the room list before retrying: Matrix create-room has no message-style transaction ID. SDK create-room can succeed while its `m.direct` update fails; Archaic separately confirms the account-data mapping, opens the already-created room, and displays a warning if the mapping cannot be confirmed. It does not report that room as uncreated or automatically create it again. Repairing failed DM labels across restart remains unfinished.

Pending creation disables duplicate submission and conflicting room operations. Cancellation before submission preserves the current room/draft and sends no request. Once submitted, the form remains busy until a result; it does not imply that an in-flight server operation can be cancelled. Failures preserve input for correction/retry; success clears the form and room filter and opens the room while keeping other-room drafts. Direct-chat input supports Enter. English/German catalogs cover all controls and errors. Session epochs reject stale results, and expiry leaves sign-out reachable.

Validation on Linux against pinned CUI `1224059d991f7fee4fb019aadef154d275e77c42`: 44 workspace tests, strict Clippy and formatting pass; the isolated real-vault check remains separately opt-in. Added tests cover input validation, private/encrypted request fields, invite deduplication, explicit forbidden retry, existing DM reuse without creation, failed direct tagging without duplicate creation, draft preservation and stale/expired-session state. Native Xvfb checks exercise cancellation, invalid ID rejection, private-group creation/retry and direct-chat creation/reopening; captured requests assert encryption configuration and account-data updates. These are SDK/local-protocol and native UI checks, not live-server or multi-device E2EE acceptance. Windows/macOS remain deferred.

The creation dispatcher and shared Enter-key registration were factored out during structural review. Remaining ripwire findings concern recent churn, test/scenario size, declarative views and function-pointer callback discovery; no clean structural gate is claimed. Existing sibling CUI changes and the separate design directory were not modified or included.

ARC-06-01 remains partial: public/directory/restricted-room flows and full conversation lifecycle acceptance are still missing. Device verification/recovery, a rich timeline with replies/edits/reactions, attachments, search, notifications, spaces, moderation and calling remain substantial independent work.

Reference: [Matrix create-room API](https://spec.matrix.org/v1.16/client-server-api/#post_matrixclientv3createroom), with the pinned matrix-sdk 0.19.1 `Client::create_room`, `Client::get_dm_rooms` and account-data implementations.


## Message actions increment

Added a native message selector and **Reply**, **Edit**, **Delete**, and **React** controls beneath the existing text timeline. The separate action editor preserves the normal composer draft. Edit opens with the displayed current text; deletion requires explicit confirmation. Before submission, Cancel sends nothing. While a request is pending, duplicate submission/cancellation is disabled; a failed operation retains its input. Room navigation and conflicting composer/membership/creation operations are disabled while an action is open. Sign-out remains reachable. Controls, hints and failures have English/German translations.

The SDK builds unthreaded replies without implicit mention notifications, replacement events for edits, annotation events for reactions, and redactions for deletion/removal. Edits and deletion are limited to the user's own text messages; this is not moderation. The worker fetches and validates the target event, author, membership, identity and room before mutation. The server remains authoritative for permissions. Blank/oversized text and invalid reaction keys are rejected. Reply/edit input is capped at 32,000 UTF-8 bytes; reaction keys at 128 bytes without control characters. No executable HTML is rendered.

A request carries room, target, action and transaction identity. Unchanged retries preserve the exact transaction; changing input creates a new one. Reaction toggle resolves to a specific add or removal operation before sending, so background sync cannot reverse a retry's meaning. Removing duplicate own reactions validates all loaded target IDs before writing and assigns each a stable sub-transaction. Partial failure preserves that full removal set. Retrying deletion after a lost acknowledgement accepts the already-redacted own event. Epoch and exact-request matching reject stale completions; membership loss/logout clear private action state, and token expiry stops further mutations.

The bounded history projection associates loaded edits/reactions/redactions with the original event. Same-author replacement events are ordered by server timestamp then event ID; redacted revisions are ignored and an earlier loaded revision can become visible again. Original redaction hides all edited content and reactions. Reaction counts deduplicate senders while retaining event IDs needed to remove duplicate own reactions. Replies retain their original target and show a loaded preview or the target ID. Redaction target fields follow the room version's rules. Relation events still count toward the existing 500-event bound.

Limits remain explicit: no durable action queue across restart, full edit-history browser, thread navigation, formatted editor, custom reaction images/emoji picker, or complete relation fetching outside the loaded window. Server-bundled relation summaries are not yet consumed. Cancelling after an ambiguous network failure cannot undo a server operation; retrying the retained action is the way to recover its acknowledgement. SDK encryption is used where applicable, but these checks do not establish encrypted multi-device interoperability or production E2EE readiness.

Validation on Linux: **61 automated workspace tests pass**, with the optional isolated real-vault test separately ignored. Strict Clippy and formatting pass. New checks cover relation ordering/redaction, original-author and target guards, room-version redactions, retry identity, stale epochs, ordinary draft preservation, partial reaction-removal failures, already-redacted deletion retry, token expiry, cross-room responses and rejection of replacements targeting undecrypted placeholders. Native Xvfb input checks exercise edit failure/retry, replies, adding/removing reactions, delete cancellation/confirmation, and successful sending of the preserved composer draft. Requests use the real SDK against disposable loopback mocks; screenshots were inspected. No real account or credential vault was accessed. Windows/macOS and live-homeserver acceptance remain deferred.

Validation uses isolated pinned CUI revision `1224059d991f7fee4fb019aadef154d275e77c42`. CUI and its pin are unchanged, and concurrent CUI/design work is excluded. Structural review extracted message dispatch, form rendering, reaction retry resolution and edit projection into focused functions; recent churn, long sequential UI/test code and callback discovery still trigger ripwire findings, so no clean structural gate is claimed.

Protocol reference: [Matrix event relationships and replacements](https://spec.matrix.org/v1.16/client-server-api/#event-replacements), plus pinned matrix-sdk 0.19.1 reply/edit/send/redaction implementations. ARC-03-09, ARC-04-03, ARC-04-05 and ARC-04-06 are now partial, not accepted parity.


## Attachments, receipts, search and complete relation pagination

Added **Room tools** with four native modes and English/German controls. The [room tools guide](ROOM-TOOLS.md) documents user workflows, the command/result contract, retry/privacy semantics and exact limits. Existing CUI file dialogs, selectors, lists and text controls suffice; no CUI source, revision or design-work changes are included.

Attachments use native choose/save dialogs, worker-thread file IO and SDK encrypted/authenticated media support. Uploads retain immutable bytes and the same message transaction on unchanged retry. Downloads validate media integrity through the SDK and save atomically without overwriting. The 25 MiB application limit is enforced before upload and before saving downloads, but SDK response buffering is not a strict memory/network cap. No inline gallery, streaming, progress percentage, upload caption editor or durable transfer queue is claimed. Downloads currently save the original event's media.

Explicit public/private unthreaded receipts are available for the selected message; opening or inspecting messages sends nothing automatically. Details shows public receipts anchored on that event from the sync store, not an exhaustive reader list. Automatic viewport receipts, unread accounting and threads remain unfinished.

Server search uses room-filtered, recent-first Matrix search and blocks queries in encrypted rooms. Local search scans SDK-decrypted history pages without sending the query or writing a disk index. Both have independent cursors, bounded results, overlap deduplication and atomic retry. Local historical text can differ from current edits; opening a result resolves details. Unreadable events are counted separately. Limits (1,000 hits/4,096 cursors) produce an explicit incomplete error.

Message details fetches the original event, up to 100 relations per page, older reply originals, revision history and reactions independently of the 500-event text timeline. More relations exhausts server-returned pages, up to 10,000 related events/4,096 cursors. Bundled replacements are also consumed by the normal timeline. Refresh starts a new snapshot so withdrawn relations do not linger. This is complete pagination of available relations, not guaranteed access to deleted/retained-away/missing-key events, automatic live relation refresh, thread presentation or context navigation. Search results can invoke the existing message actions outside the timeline window.

Pending operations guard conflicting actions and preserve the ordinary composer. Exact request, room and epoch guards reject stale completions. File dialog cancellation, discarded-upload cleanup, missing reply originals, missing keys, server errors, cursor loops, malformed/cross-room data and token expiry have explicit paths. Sign-out remains reachable.

Validation: **79 automated workspace tests pass** (one real-vault test remains separately opt-in), strict Clippy and formatting pass. Linux, pinned CUI `1224059d991f7fee4fb019aadef154d275e77c42`, real matrix-sdk 0.19.1 transport against disposable loopback mocks. Automated checks cover stable upload bytes/transactions, original filename handling, size bounds/no-clobber saves, authenticated encrypted-media decryption and tamper rejection, explicit receipt types and sync-backed reader lists, search pagination/retry/privacy, unavailable encrypted content, bundled edit guards, redactions, outside-window reply originals and relation continuation/refresh. Native Xvfb input checks cover local search/open details, both receipt types, file selection/upload/save and preserved composer drafts. These are not real-homeserver, multi-device E2EE or Windows/macOS acceptance.

Structural review extracted attachment/bundle decoding and native dialog completion. Remaining structural findings concern dispatcher/view/test length, callback reachability and recent churn; a clean ripwire gate is not claimed. All parity families touched here remain **partial**.

## Priorities 1–3 increment

See [Security and messaging workflows](SECURITY-MESSAGING.md) for instructions, code ownership and remaining acceptance work. Added SDK-native account security/recovery, classic browser SSO, encrypted refresh-token persistence, a durable ordinary-text queue, typing/unread/private receipt controls, incremental native table rows, thread replies, text forwarding and context/newer pagination. This is progress across all three priorities, not completion of them.

Linux validation includes 86 normal workspace tests (two opt-in tests excluded), the additional passing real-Synapse two-device security/encrypted-message test, and native Xvfb interaction/layout checks. The live-test helper pins its Synapse image, creates isolated accounts and removes its own container/volume. Tests never reuse Element credentials or the desktop credential vault. CUI remains pinned at `1224059d991f7fee4fb019aadef154d275e77c42`; Windows/macOS verification and the design overhaul remain deferred.

Structural review extracted sync processing, security result handling, queue observation and forward-page handling. Some dispatcher/view/test size, callback-discovery and recent-churn findings remain; no clean ripwire structural gate is claimed.


## Media, organization and desktop batch

This batch adds the workflows in [Room tools](ROOM-TOOLS.md), [Organization](ORGANIZATION.md), [Desktop integration](DESKTOP.md), and the updated [authentication/draft/queue guide](SECURITY-MESSAGING.md). The pinned CUI library is unchanged. Existing unrelated design work is outside this implementation.

Validation on Linux: 102 Rust unit/protocol tests, strict all-target Clippy and formatting; a separate real Synapse test passed two-device verification/recovery, encrypted queued messages and an encrypted attachment upload/download with exact-byte verification. The ordinary test run leaves the live-server and real-vault tests ignored unless explicitly invoked. Native Xvfb interaction exercised password login, room selection, search and organization shortcuts; a private D-Bus/Xvfb test verified tray registration, icon data, Show and orderly Quit. Standard-library packaging tests verified archive layouts/hashes/Matrix URL metadata and overwrite rejection for all three OS layouts, without executing foreign binaries.

Structural review identified growing command/model dispatch functions and repeated guard/dispatch patterns. Sync indexing was separated and disk writes batched once per sync; receipt dispatch was separated from room-tool selection. Remaining explicit dispatch branches and the two distinct encrypted record writers are retained deliberately for this batch. The ripwire structural gate remains nonzero; a passing compiler/test suite is not a claim of zero structural debt. Rich UI/state decomposition, scalable indexing and broader platform acceptance remain follow-up work.

## Reauthorization, advanced room controls and installer batch

Added same-device classic SSO/OAuth reauthorization, cancellation and identity validation. OAuth can use issuer-bound pre-registration, a fixed callback port and a configured HTTPS client URI. Browser recovery suspends persistence until the renewed user/device is verified; wrong identities leave the encrypted record unchanged. Shutdown during recovery saves drafts. Normal-session reauthorization requests are rejected without replacing the active session.

Room administration now includes aliases/canonical aliases, publishing, avatars, low-priority tags, restricted membership, per-event power, retention state, upgrades, reporting/moderator redaction, knocks, parent links, thread catalogs and server-dependent subscriptions. Thread results open in live details while retaining room/draft scope; catalogs reject repeated pagination tokens and omit blocked users. See [Organization](ORGANIZATION.md) for fields and limits; receipt markers are not a complete unread-thread count.

Added saved-profile selection, per-profile notification preferences, bounded native notification activation and stale-account action rejection. Installer tooling covers Debian payloads, macOS signing/notarization commands and Windows per-user NSIS/signing commands, with explicit license inputs and final artifact hashes. See [Release packaging](RELEASE.md); no signed production artifact or target-OS installation has been produced.

Validation on Linux against pinned CUI `1224059d991f7fee4fb019aadef154d275e77c42`: **106 normal Rust tests passed** (27 application, 2 localization, 31 Matrix unit and 46 protocol), with the native-notification, live-server and real-vault tests separately opt-in. The native-notification test was additionally run on a private D-Bus service and passed delivery/escaping/default-action/account-event routing. Private D-Bus/Xvfb tray Show/Quit and native login/search/organization/thread-catalog interaction checks passed. Strict all-target Clippy, formatting, one archive-layout test and three installer contract tests passed. Protocol cases include SSO cancellation and wrong-user rejection, OAuth wrong-device rejection followed by successful same-device recovery, restricted/power/retention validation, forbidden alias removal and thread pagination. Tests use disposable protocol fixtures; real authentication-service interoperability is not established by them.

Thread listing, notification delivery, preferences, OAuth registration and organization result state were separated into focused modules/helpers. Structural review still reports dispatcher/test length and recent-churn findings; no clean ripwire gate is claimed. CUI, its pin and unrelated `design/` work are untouched. Remaining product work is represented as partial/planned in `parity.json`, including full unread-thread workflow, nested-space policy, wider interactive authentication, calls/widgets and broader Element parity. Signing/license decisions and platform/distribution release acceptance are external prerequisites; Windows/macOS runtime verification remains deferred.
