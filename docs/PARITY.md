# Element parity inventory

Baseline: [Element v1.12.30](https://github.com/element-hq/element-web/releases/tag/v1.12.30), commit `f19cfd9030429240a4209bf5175b372175cf0464`, researched October 1, 2026.

94 feature families mapped. 52 families now have **partial** implementations; none has passed full parity acceptance. The first native client increment is described in [implementation status](IMPLEMENTATION.md). Windows/macOS verification remains deferred. This is an initial source-based inventory, not an exhaustive runtime audit. Phase P0 expands families into scenarios and reconciles the full settings/test index.

The [machine-readable inventory](parity.json) contains acceptance criteria and platform status. The [upstream index](upstream-index.json) records 176 setting entries, 24 feature-marked entries, release configuration, 158 reference test files and hashes of 63 retrieved source files. Source/test presence identifies a feature surface, not whether a particular deployment enables it.

## How to use this checklist

For each row, add concrete test cases, SDK capability evidence, CUI requirements, implementation revision and Linux/Windows/macOS verification. Include success, cancellation, errors, offline/restart, accessibility and localization where relevant. A feature is complete only when its real behavior passes, not because the SDK exposes a type or the UI contains a button.

The reference release turns on both video-room flags in its release configs. User status defaults on. Labs documentation alone can be stale; settings, release config, runtime capabilities and observed behavior determine the effective baseline. Optional features remain visible in the roadmap, with a separate decision about which block the parity release.

## Identity and sessions

[Reference surface](https://github.com/element-hq/element-web/tree/f19cfd9030429240a4209bf5175b372175cf0464/apps/web/src/components/structures/auth)

| ID | Feature | Phase | Availability | Acceptance |
| --- | --- | --- | --- | --- |
| ARC-01-01 | Discovery and custom homeservers | P2 | Server-dependent | Resolve Matrix IDs/well-known and manual homeservers; show TLS/network/discovery failures. |
| ARC-01-02 | Password login and interactive authentication | P2 | Server-dependent | Sign in to a classic homeserver; complete supported UI-auth stages; never log secrets. |
| ARC-01-03 | SSO and delegated OAuth/OIDC | P2 | Server-dependent | Use a system browser with validated return flow; handle cancellation, registration and expired tokens. |
| ARC-01-04 | Registration, consent and account recovery | P4 | Server-dependent | Handle supported registration/terms/reset flows or approved browser flows without claiming native completion. |
| ARC-01-05 | Restore session, refresh and soft logout | P2 | Reference | Restart without a fresh device; refresh safely; reauthenticate without losing recoverable data. |
| ARC-01-06 | Logout and session revocation | P2 | Reference | Revoke the correct session and remove its local secrets/caches; handle offline failure honestly. |
| ARC-01-07 | Profile, avatar, presence and status | P4 | Server-dependent | Update profile and show presence/status when server/client capabilities permit. |
| ARC-01-08 | Email/phone/identity-service and account management | P4 | Server-dependent | Support configured account/identity workflows and deactivation; never assume an identity service. |
| ARC-01-09 | Independent profiles and account switching | P4 | Reference + enhancement | Match independent desktop profiles and add in-app switching with separate stores/crypto state. |

## Encryption and trust

[Reference surface](https://github.com/element-hq/element-web/tree/f19cfd9030429240a4209bf5175b372175cf0464/apps/web/playwright/e2e/crypto)

| ID | Feature | Phase | Availability | Acceptance |
| --- | --- | --- | --- | --- |
| ARC-02-01 | Encrypted room messaging | P2 | Reference | Exchange encrypted messages with Element; never downgrade an encrypted room to plaintext. |
| ARC-02-02 | Device list and session management | P2 | Reference | Inspect, rename and revoke sessions; expose meaningful device trust state. |
| ARC-02-03 | SAS/emoji and QR verification | P2 | Reference | Verify between Archaic and Element, including cancellation, timeout and mismatched codes. |
| ARC-02-04 | Cross-signing bootstrap and trust changes | P2 | Reference | Bootstrap and restore identity; explain resets and changed identities without silently trusting them. |
| ARC-02-05 | Secret storage and recovery key/passphrase | P2 | Reference | Create and unlock recovery, handle wrong/lost credentials without destructive reset by default. |
| ARC-02-06 | Backup, restore, key import/export | P2 | Reference | Recover encrypted history on a fresh device and validate import/export error handling. |
| ARC-02-07 | Missing keys, key requests and decryption recovery | P2 | Reference | Differentiate unavailable keys, untrusted history and malformed data; retry when keys arrive. |
| ARC-02-08 | Encrypted attachments and authenticated media | P2 | Reference | Encrypt/decrypt files and thumbnails; keep content keys and authorization out of external URLs/logs. |
| ARC-02-09 | History sharing and device dehydration | P4 | Server-dependent | Validate supported history-sharing/dehydration flows against the pinned SDK and server capabilities. |
| ARC-02-10 | Optional strict device policy and encrypted state | P5 | Labs/server-dependent | Track reference flags and SDK capability; do not conflate experimental support with baseline E2EE. |

## Synchronization and timeline

[Reference surface](https://github.com/element-hq/element-web/tree/f19cfd9030429240a4209bf5175b372175cf0464/apps/web/playwright/e2e/timeline)

| ID | Feature | Phase | Availability | Acceptance |
| --- | --- | --- | --- | --- |
| ARC-03-01 | Initial and incremental sync | P2 | Reference | Catch up live events, state and unread counts without freezing the UI. |
| ARC-03-02 | Classic sync compatibility | P2 | Compatibility requirement | Operate on a homeserver without MSC4186; validate adapters instead of assuming high-level SDK fallback. |
| ARC-03-03 | Sliding sync and capability negotiation | P2 | Server-dependent | Use supported efficient sync; recover from expired positions and reconnect correctly. |
| ARC-03-04 | Historical pagination and jump to event | P2 | Reference | Load both directions around an event without duplicates or unexpected viewport movement. |
| ARC-03-05 | Local echoes and durable send queue | P2 | Reference | Reconcile transaction IDs, retries and server echoes across restart without duplicate sends. |
| ARC-03-06 | Offline mode and network recovery | P2 | Reference | Read cached history, preserve drafts and unsent state; handle rate limits and partial connectivity. |
| ARC-03-07 | Unread markers and read receipts | P2 | Reference | Respect receipt/privacy settings and thread-aware unread behavior across clients. |
| ARC-03-08 | Typing indicators and membership/state events | P2 | Reference | Expire typing state and render meaningful state changes without overwhelming the timeline. |
| ARC-03-09 | Redacted, edited and unknown events | P3 | Reference | Preserve event identity and safe fallback rendering; update relations after redaction. |
| ARC-03-10 | Jump to date, retention and room predecessors | P4 | Labs/server-dependent | Detect relevant capabilities, preserve room-upgrade navigation, and apply supported retention behavior. |

## Composer and messages

[Reference surface](https://github.com/element-hq/element-web/tree/f19cfd9030429240a4209bf5175b372175cf0464/apps/web/playwright/e2e/composer)

| ID | Feature | Phase | Availability | Acceptance |
| --- | --- | --- | --- | --- |
| ARC-04-01 | Plain text and formatted message rendering | P3 | Reference | Render Matrix formatting, links, lists, quotes, code and spoiler content with selection and no executable HTML. |
| ARC-04-02 | Markdown and rich-text composing | P3 | Reference / selectable editor | Support formatted editing and plain mode with predictable round trips, undo and IME behavior. |
| ARC-04-03 | Reply and quoted context | P3 | Reference | Create/follow replies including missing originals, edits and encrypted events. |
| ARC-04-04 | Threads and thread activity | P3 | Reference | Read/send threaded replies, navigate context and show appropriate unread state. |
| ARC-04-05 | Edit, edit history and deletion | P3 | Reference | Edit own messages, inspect history and apply permission-aware redaction across clients. |
| ARC-04-06 | Reactions, emoji and custom reaction images | P3 | Reference + Labs extension | Toggle standard reactions without duplicate state; track custom-image flag separately. |
| ARC-04-07 | Mentions and notification intent | P3 | Reference | Autocomplete users/rooms and send correct intentional mention metadata with permission-aware room mentions. |
| ARC-04-08 | Slash commands and emotes | P3 | Reference | Audit shipped commands and provide equivalent outcomes, discoverability and useful errors. |
| ARC-04-09 | Per-room drafts and send preferences | P2 | Reference | Restore drafts per account/room/thread; support multiline and configurable Enter behavior. |
| ARC-04-10 | Message actions, permalinks and copy | P3 | Reference | Copy readable text/links and open actions through keyboard/context menu with access checks. |
| ARC-04-11 | Poll creation, voting, closing and history | P3 | Reference | Interoperate for poll lifecycle, including edits where permitted and encrypted room behavior. |
| ARC-04-12 | Pinned/saved messages and reminders of context | P3 | Reference / audit reference availability | Match pinned-message management and available saved-message workflows; distinguish local convenience from protocol state. |
| ARC-04-13 | URL previews and preview bundles | P3 | Server-dependent + Labs extension | Respect privacy settings and server support; distinguish server previews from encrypted preview bundles. |
| ARC-04-14 | Math rendering and expanded composer features | P5 | Labs | Track optional LaTeX and newer editor modes; native implementation must have explicit supported syntax. |

## Attachments and location

[Reference surface](https://github.com/element-hq/element-web/tree/f19cfd9030429240a4209bf5175b372175cf0464/apps/web/src/components/views/messages)

| ID | Feature | Phase | Availability | Acceptance |
| --- | --- | --- | --- | --- |
| ARC-05-01 | Upload files, images and captions | P2 | Reference | Choose/drop/paste files, preview and cancel; report size limits/progress/retry accurately. |
| ARC-05-02 | Downloads, save/open and file safety | P3 | Reference | Sanitize filenames and require intended user actions before opening external handlers. |
| ARC-05-03 | Image/sticker gallery and animated media | P3 | Reference | Zoom/navigate supported media with bounded decoding and correct orientation/scale. |
| ARC-05-04 | Audio and video playback | P3 | Reference | Play/pause/seek/volume and device changes work with documented codec/platform coverage. |
| ARC-05-05 | Voice-message recording and playback | P3 | Reference | Record, preview, cancel and send interoperable voice messages with waveform/duration. |
| ARC-05-06 | Shared files and media browser | P3 | Reference | Browse/paginate room media and navigate back to originating messages. |
| ARC-05-07 | PDF preview | P5 | Labs | Audit optional embedded viewer; external opening is an interim fallback, not embedded parity. |
| ARC-05-08 | Static location and map display | P4 | Reference | Send/render supported location events and link coordinates; disclose map-provider networking. |
| ARC-05-09 | Live location sharing | P5 | Labs/server-dependent | Consent, expiry, stopping and receiving updates work; no undisclosed background tracking. |

## Rooms, spaces and moderation

[Reference surface](https://github.com/element-hq/element-web/tree/f19cfd9030429240a4209bf5175b372175cf0464/apps/web/src/components/views/room_settings)

| ID | Feature | Phase | Availability | Acceptance |
| --- | --- | --- | --- | --- |
| ARC-06-01 | DMs, group rooms and room directory | P2 | Reference | Create/find/join supported conversations and distinguish room membership states. |
| ARC-06-02 | Public/private/restricted/knock join rules | P4 | Server-dependent | Render room access requirements and complete invite/knock/accept/reject flows. |
| ARC-06-03 | Invites, membership, leave and forget | P4 | Reference | Use permission-aware controls and recover from blocked invites or already-changed membership. |
| ARC-06-04 | Spaces, hierarchy and room discovery | P4 | Reference | Create/manage nested spaces, browse children and obey restricted membership rules. |
| ARC-06-05 | Favorites, low priority, unread filters and ordering | P3 | Reference | Sync applicable tags/account data and preserve sensible local sorting/navigation. |
| ARC-06-06 | Room name/topic/avatar/aliases and publishing | P4 | Reference | Edit metadata/aliases and directory visibility with explicit permission errors. |
| ARC-06-07 | Member profiles, search and invitations | P4 | Reference | Search/paginate large member lists and open contextual actions without loading every avatar. |
| ARC-06-08 | Power levels and room permissions | P4 | Reference | Inspect/edit permissions and reflect remote changes immediately. |
| ARC-06-09 | Kick, ban, unban, redact and report | P4 | Reference | Perform supported moderation operations, confirm scope and handle partial failures. |
| ARC-06-10 | Ignore/block and invitation rules | P4 | Reference/server-dependent | Apply supported account ignore/invite policies and explain their effect. |
| ARC-06-11 | Room versions/upgrades, history visibility and retention | P4 | Reference/server-dependent | Handle upgrades/tombstones, security/history settings and supported server retention. |
| ARC-06-12 | Ban lists, bridge information and pending moderation | P5 | Labs/integration-dependent | Track supported policy/bridge state and experimental moderation without implementing a server-side bridge. |

## Search and navigation

[Reference surface](https://github.com/element-hq/element-web/tree/f19cfd9030429240a4209bf5175b372175cf0464/apps/web/src/components/structures)

| ID | Feature | Phase | Availability | Acceptance |
| --- | --- | --- | --- | --- |
| ARC-07-01 | Room/user quick switcher and command palette | P3 | Reference | Keyboard-search rooms/users/actions with stable selection and correct account scope. |
| ARC-07-02 | Server-side message search | P3 | Server-dependent | Search supported unencrypted history with filters, pagination and jump-to-result. |
| ARC-07-03 | Local encrypted-history search | P3 | Desktop reference | Index decrypted content locally with defined at-rest protection, incremental updates and coverage reporting. |
| ARC-07-04 | Search filters, thread context and highlighting | P3 | Reference | Filter scope/sender/time where supported and navigate results without losing search state. |
| ARC-07-05 | Chat export and event inspection | P4 | Reference | Export supported formats safely; show raw event diagnostics on demand; redact secrets in support exports. |

## Notifications and desktop integration

[Reference surface](https://github.com/element-hq/element-web/tree/f19cfd9030429240a4209bf5175b372175cf0464/apps/desktop/src)

| ID | Feature | Phase | Availability | Acceptance |
| --- | --- | --- | --- | --- |
| ARC-08-01 | Global/per-room notification rules and mentions | P3 | Reference/server-dependent | Persist Matrix push rules, room overrides and sound/highlight choices consistently. |
| ARC-08-02 | Native desktop notifications and activation | P3 | Desktop reference | Respect preview privacy and open correct account/room/event on activation. |
| ARC-08-03 | Notification/activity panels | P4 | Reference + Labs variant | Audit available activity views; maintain unread/thread state and honest encrypted-history coverage. |
| ARC-08-04 | Tray/status item, dock badges and close behavior | P6 | Desktop reference | Follow each OS convention; closing/hiding/quitting is explicit and does not lose sends. |
| ARC-08-05 | Deep links, matrix.to and single-instance routing | P4 | Desktop reference | Validate incoming links, select an account and navigate only after needed authentication. |
| ARC-08-06 | Autostart, menus, shortcuts and spellcheck | P6 | Desktop reference | Honor user preferences and keyboard layout; use optional OS language services with clear availability. |
| ARC-08-07 | Independent profile data and storage controls | P4 | Desktop reference | Separate profiles, expose cache/index controls and migrate safely. |
| ARC-08-08 | Updates, release channel and support diagnostics | P6 | Desktop reference | Verify update artifacts, handle rollback/migrations and provide opt-in redacted diagnostics. |

## Calls and integration surfaces

[Reference surface](https://github.com/element-hq/element-web/tree/f19cfd9030429240a4209bf5175b372175cf0464/apps/web/playwright/e2e/voip)

| ID | Feature | Phase | Availability | Acceptance |
| --- | --- | --- | --- | --- |
| ARC-09-01 | Native MatrixRTC audio/video calls | P5 | Release-enabled/server-dependent | Complete interoperable outgoing/incoming calls with Element using actual native capture/playback. |
| ARC-09-02 | Group/video rooms and call lifecycle | P5 | Release-enabled/server-dependent | Join/rejoin persistent/group calls and handle notifications, lobby and participant changes. |
| ARC-09-03 | Call E2EE and identity/permission state | P5 | Reference/server-dependent | Prove encrypted media interoperability and present failures; never silently disable encryption. |
| ARC-09-04 | Microphone/camera/speaker and device hotplug | P5 | Reference | Select devices, mute and recover from removal or denied OS permission. |
| ARC-09-05 | Screen/window sharing, PiP and layouts | P5 | Reference/platform-dependent | Share via OS-supported capture with visible state; resize and keyboard-control native call layouts. |
| ARC-09-06 | Legacy Matrix VoIP and TURN | P5 | Compatibility path | Test legacy signaling/ICE/TURN separately; MatrixRTC support alone is insufficient. |
| ARC-09-07 | Jitsi widgets and pop-out calls | P5 | Integration-dependent | Resolve compatibility-host policy and verify widget permissions/pop-out behavior. |
| ARC-09-08 | Arbitrary widgets, layouts and integration manager | P5 | Integration-dependent | Run compatible widget content with explicit capability grants in an isolated host; browser fallback is not full parity. |
| ARC-09-09 | Bridges, dialpad and telephony integrations | P5 | Server/integration-dependent | Support relevant client UI/state when configured; provide no fake calling or client-side bridge service. |

## Localization, accessibility and preferences

[Reference surface](https://github.com/element-hq/element-web/tree/f19cfd9030429240a4209bf5175b372175cf0464/apps/web/src/components/views/settings/tabs/user)

| ID | Feature | Phase | Availability | Acceptance |
| --- | --- | --- | --- | --- |
| ARC-10-01 | Translated UI and language selection | P1–P6 | Reference | Externalize all strings and validate fallback, plural/select rules and reviewed catalogs. |
| ARC-10-02 | RTL, bidi and international text input | P1–P6 | Reference | Test mirrored layout, mixed-direction text, CJK IME, grapheme-aware editing and fallback fonts. |
| ARC-10-03 | Locale date/time/number formatting | P1–P6 | Reference | Use locale/time-zone aware formatting and display unambiguous timestamps. |
| ARC-10-04 | Themes, density, text size and high DPI | P1–P6 | Reference | Native light/dark and high contrast with 4K/fractional scale and independent large text. |
| ARC-10-05 | Keyboard and assistive technology | P1–P6 | Reference | Verify native semantics, focus and live updates on each platform, including virtualized history. |
| ARC-10-06 | Media/privacy/appearance preferences | P3–P6 | Reference | Map all reference settings to explicit support, alternatives or documented differences. |
| ARC-10-07 | Labs, feature policy and deployment configuration | P0–P6 | Reference/config-dependent | Track every indexed setting/flag and server prerequisite; distinguish optional experiments from completed baseline. |
| ARC-10-08 | Enterprise modules and X.509 integrations | P5 | Optional/module-dependent | Inventory available module surfaces and decide supported adapters; no claim to execute arbitrary Element JavaScript modules. |

## High-risk parity gaps to resolve in P0

- Arbitrary web widgets require a browser compatibility environment. External-browser fallback alone is not embedded-widget parity.
- Native MatrixRTC calls require a media engine and interoperability work; SDK signaling methods do not provide finished calling.
- SDK high-level sync needs MSC4186; older-server compatibility needs a separately tested path.
- The current encrypted event index scans cached events; scaling, index maintenance and full-history coverage require further work.
- CUI has known gaps in rich/virtualized chat, worker dispatch, lifetime disposal and RTL/accessibility. See [CUI work packages](CUI-GAPS.md).
- Enterprise or deployment-specific modules cannot be assumed to work as native features; keep their support decisions explicit.

The room-tools increment adds partial attachment upload/save, authenticated encrypted media, explicit read receipts, server search and local decrypted-history scanning. Existing relation/reply/revision families gain separate pagination beyond the text timeline. See [workflows and limits](ROOM-TOOLS.md); no family has full parity acceptance.

Priorities 1–3 now include partial security/recovery, browser SSO, durable text queue, typing/unread updates, incremental rows, threads and context navigation. See [current workflows and remaining gaps](SECURITY-MESSAGING.md). No full parity acceptance is claimed.

The next batch adds encrypted drafts, OAuth/PKCE and password soft logout, queued message actions/media, bounded previews and streamed downloads, persistent filtered search, live details/thread receipts, room/space administration, blocking/favorites, account profiles and native desktop integration. [Organization](ORGANIZATION.md) and [Desktop integration](DESKTOP.md) document the additional workflows. Families remain partial wherever their complete acceptance scenarios are unmet. Native Windows/macOS verification remains deferred.
