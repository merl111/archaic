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

This version stores sessions and crypto state only in memory. Each login creates a new server device. Successful explicit sign-out revokes it; closing or crashing the application does not. Secure persistence, access-token storage, refresh, verification and backup/recovery are the next security work package. These limits are displayed before login.

The Matrix SDK has E2EE enabled, but production encrypted messaging is not an accepted feature yet. There are no multi-device verification/recovery tests or recovery UI. Password login only: SSO/OIDC and authentication challenges are not implemented. Never label a missing supported login method as a successful connection.

A latest-events text view is not a complete timeline. Edits, redactions, relation aggregation, pagination, attachments, reactions and receipts need a proper SDK-backed timeline model plus reusable CUI rich/virtualized controls. History currently reloads on successful sync and explicit refresh; this is a correctness-first scaffold and must be replaced with incremental updates before performance acceptance. Filtering uses current joined rooms only.

English/German are the initial catalogs. Runtime language changes, OS locale detection, pluralized dynamic labels, translation workflow and RTL framework work remain open. CUI and app native Windows/macOS verification remain deferred as requested; no portability claim here substitutes for those runs.

## Next implementation order

1. Persistent encrypted state store and platform credential-vault boundary, crash-safe session restoration and explicit device cleanup/recovery behavior.
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
