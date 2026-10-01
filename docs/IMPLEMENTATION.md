# First implementation increment

October 1, 2026. This is a partial implementation of the approved plan; no entire parity milestone is complete.

## Implemented

- Three-crate Rust workspace with a checked-in dependency lockfile and pinned sibling CUI revision.
- Static CUI build helper using CMake; no CUI source or backend implementation copied into Archaic.
- Native sign-in and split conversation view, system-themed controls, selectable message text, native password field and text composer.
- Embedded Fluent English/German catalogs, explicit locale selection, English fallback and catalog tests.
- Matrix SDK 0.19.1 password authentication, classic sync, joined rooms, latest 50 events, text sending and server-side logout.
- Tokio worker service with bounded channels; CUI handles remain on the main thread.
- Per-room drafts, stable transaction IDs for unchanged failed-send retries, stale-session/history rejection and private view clearing on sign-out.
- Network operations use finite request timeouts; sync retries with delay. No custom cryptographic implementation.

## Deliberate limits

Normal startup now restores a saved session and the SDK SQLite crypto/state stores. Tokens live in an authenticated encrypted record; its key lives in the platform credential vault. Explicit temporary sessions remain available. See the saved-session increment below and the README for storage/retention details. Refresh-token rotation, same-device soft logout, verification and backup/recovery remain unfinished.

The Matrix SDK has E2EE enabled, but production encrypted messaging is not an accepted feature yet. There are no multi-device verification/recovery tests or recovery UI. Password login only: SSO/OIDC and authentication challenges are not implemented. Never label a missing supported login method as a successful connection.

A latest-events text view is not a complete timeline. Edits, redactions, relation aggregation, pagination, attachments, reactions and receipts need a proper SDK-backed timeline model plus reusable CUI rich/virtualized controls. History currently reloads on successful sync and explicit refresh; this is a correctness-first scaffold and must be replaced with incremental updates before performance acceptance. Filtering uses current joined rooms only.

English/German are the initial catalogs. Runtime language changes, OS locale detection, pluralized dynamic labels, translation workflow and RTL framework work remain open. CUI and app native Windows/macOS verification remain deferred as requested; no portability claim here substitutes for those runs.

## Next implementation order

1. Complete session lifecycle acceptance: refresh-token rotation, same-device soft logout, recovery-aware local-data purge and real-server failure/crash testing. The initial vault/store/restore boundary is now implemented.
2. Device verification and recovery UX, real-homeserver integration tests with multiple devices, reconnect/token-expiry/logout cases.
3. CUI dispatcher and disposable callback/subtree lifetime contracts; virtualized rich timeline and editable composer capabilities in CUI, with bindings and tests.
4. Incremental SDK timeline models, pagination, edits, reactions, replies, attachments, room joining/invitations and spaces.
5. Continue the feature acceptance checks in the parity inventory. Calls, widgets, admin, notifications, accessibility and release packaging remain independent work packages.

## Repository boundary

Any reusable widget, rendering, input, accessibility or native-desktop capability is implemented in the separate CUI repository. The application owns Matrix protocol behavior, localization catalogs, account state, screens and packaging. Update `cui-revision` only after reviewing and checking the corresponding CUI change; build the Rust binding and static archive from that same source tree.

## Validation of this increment

Tested on Linux (Manjaro x86_64), Rust 1.98.1, GCC 16.2.1, GTK 4.22.4. CUI builds as a static archive. The workspace tests include four UI state tests, two localization tests, URL validation and two Matrix protocol integration tests. `cargo clippy --workspace --all-targets -- -D warnings` and formatting checks pass.

An isolated Xvfb run exercised the actual native controls using X11 mouse/keyboard input: entering a loopback test homeserver and credentials, selecting the synchronized room, typing/sending text, receiving that text in the timeline, and signing out. The HTTP server asserted the submitted credentials and message body. This used the real SDK transport with a local simulated homeserver, not a public account or full Synapse deployment. English/German login and conversation windows also completed graphical smoke runs. The documented build helper was checked with application arguments, and the German conversation view was visually checked at 200% display scaling.

Windows/macOS, real-server interoperability, E2EE multi-device behavior, long-session memory behavior and release packaging have not been validated for Archaic yet. The SDK's presence does not imply those acceptance checks are complete.

## Sign-in design update

The login is a compact centered card with intrinsic-width native fields, a violet/teal ambient background, localized inline errors and Enter-to-submit from the password field. Extra window space surrounds the card instead of stretching the fields; a scrollable root keeps the form accessible in short windows. The application header and conversation footer appear only after login.

CUI now owns the reusable AMBIENT container role and platform implementations. GTK application button styles are scoped to the content root so native title-bar buttons retain their original metrics. The library regression compares close-button padding and measured size with a plain GTK window in both themes. The Rust binding and static library must be built from the updated pinned CUI commit.

Light/dark backgrounds are implemented; use `--theme system|light|dark` to select one at launch. **Follow-up: add a visible System/Light/Dark switcher in Settings, persist the preference and apply it without restarting.** This switcher is not implemented yet.

Checked on Linux: native login/send/logout mouse and keyboard flow, English light/dark layouts, German at 600×660 logical pixels and 200% scaling, nine app tests, strict Clippy, GTK title-bar/control/container tests and complete binding declaration coverage. Windows/macOS runtime verification remains deferred.

## Saved-session increment

Implemented automatic restoration of the same Matrix device, its encrypted SDK SQLite store, and a checked device-key fingerprint. Session tokens are serialized only into an authenticated XChaCha20-Poly1305 envelope; an OS-vault key unlocks it and the SDK stores. The profile is exclusively locked, record replacement is atomic, and malformed/oversized records, missing keys and missing crypto databases are rejected without replacing the saved login.

The native sign-in card now has restoration progress, localized retry errors and an explicit temporary-session action. The conversation footer distinguishes saved from unsaved sessions. Revoked tokens stop synchronization and disable messaging; logout retries distinguish failed server revocation from failed local cleanup. Temporary/fixture/smoke modes do not open the real profile. App state and all SDK calls remain separate from CUI handles.

Sign-out clears the saved token record but retains encrypted SDK databases and the OS-vault key. There is no secure-erase or full-disk-encryption claim. The SDK's encrypted values do not hide all SQLite metadata. Refresh tokens are deliberately not requested yet. No existing in-memory session can be recovered retroactively from an older build.

Validation includes isolated real-SDK protocol tests for restart without another login, logout/cleanup retry and revoked tokens, storage corruption/lost-key/locking tests, and UI state/catalog tests. Native Linux checks exercise temporary login, room selection, text sending and sign-out, and inspect light/dark, compact German and 200% layouts. `tools/test_linux_vault.py` provides an isolated real Secret Service check, separate from the user's keyring. Windows/macOS adapters are implemented through the keyring crate but still await platform qualification.

The accepted login composition is preserved. The main timeline is still the initial text view; the conversation design pass, runtime appearance switcher, verification/recovery, rich timeline, invitations and attachments remain upcoming work rather than implied completed features.

Checked this increment on Linux: 18 automated workspace tests, a separate passing real Secret Service round trip in an isolated keyring, strict Clippy, formatting, the native login/send/logout flow and rendered vault-error recovery controls. Structural review prompted separation of error/status/logout handling; remaining ripwire flags concern short wrapper duplication, recent UI churn, Rust test-module size and callback discovery rather than a passing structural gate.
