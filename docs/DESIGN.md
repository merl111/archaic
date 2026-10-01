# Archaic desktop design and localization

Design proposal, not a screenshot of an implemented client. The deliverable will be a real CUI application. No browser-based shell or web screenshot should be presented as proof that a native view works.

## Visual direction

A quiet, precise communication app: generous but adjustable spacing, restrained accent color, sharp typography, high-quality avatars/icons and clear information hierarchy. The conversation owns the screen; settings and advanced tools appear in context. The app should feel deliberately designed on each OS rather than forcing an identical title bar and control treatment everywhere.

Use a warm near-white/light-neutral palette and a deep slate dark palette, with one muted teal or system-accent highlight. Contrast and semantic state must work without color. Use system fonts and native rendering, with approximately 14–16 logical-pixel body text and user-adjustable density/text scale. Design for font fallback rather than shipping a decorative font family. Use consistent vector icons with accessible names; icon-only actions need discoverable labels/tooltips.

Native window chrome/menu conventions on Windows, macOS and Linux. Use Win11-appropriate spacing, focus and system colors, AppKit conventions on macOS, and the user's GTK theme preferences on Linux. Application-level composition supplies the coherent Archaic identity. Avoid making optional translucency, backdrop blur or custom chrome a usability dependency.

## Main layout

```text
┌────────────────────────────────────────────────────────────────────────┐
│ Native title bar / app menu                                            │
├──────┬─────────────────────┬───────────────────────────┬───────────────┤
│      │ Search / jump to…   │ # Design         search · │ Thread        │
│ acct │                     │ Topic / encryption state  │ or details    │
│      │ Inbox               ├───────────────────────────┤               │
│      │ Favorites           │ Date separator            │ Contextual    │
│space │                     │ Avatar  Author · time     │ information   │
│ rail │ Rooms / direct msgs │         Message content   │               │
│      │ unread / mentions   │         Reactions/replies │ Close         │
│      │                     │                           │               │
│      │                     │ New messages ↓            │               │
│      │                     ├───────────────────────────┤               │
│prefs │ Connection status   │ Reply/edit context        │               │
│      │                     │ Composer                  │               │
│      │                     │ Attach · Format · Send    │               │
└──────┴─────────────────────┴───────────────────────────┴───────────────┘
```

Initial desktop proportions: account/space rail around 56–64 logical pixels, resizable room list around 240–300, flexible conversation and optional 300–380 inspector. These are design starting points, never fixed physical pixels. The inspector is closed unless a thread/detail workflow needs it. On narrow desktop windows, collapse the inspector and then navigation; do not shrink text or squeeze the composer into unusable widths. Persist panel widths per window.

The default timeline is an editorial message list with grouped authors and restrained hover actions. A comfortable/compact density preference changes spacing, not functionality. Full-width media is bounded by the reading column. Unread/failed/decrypting/sending states must be visibly different and explain recovery actions. Updates do not move the user's scroll position unless they were already following the bottom.

The composer grows to a bounded height, preserves per-room drafts and clearly distinguishes editing, replying and threading. Offer keyboard-first formatting and a discoverable toolbar, mentions, emoji and attachment previews. Never make a drawing surface impersonate a native text editor. Enter-to-send is configurable and respects IME composition; Shift+Enter and platform conventions remain consistent.

## Screens to design before implementing their workflows

1. Welcome/homeserver discovery; password, SSO and delegated-auth browser return; connectivity errors.
2. Recovery setup and device verification: understandable trust states, recovery-key handling and non-destructive failure paths.
3. Main conversation with long names, mixed message types, slow sync, encrypted history and a crowded room list.
4. Thread panel, room details/members/files, global search and keyboard command palette.
5. Attachment gallery, upload queue, audio/voice playback, polls and emoji/reaction picker.
6. Room/space creation, invites, permissions/moderation and server-denied actions.
7. Settings for accounts, notifications, appearance, accessibility, languages, storage and device sessions.
8. Incoming/active calls with device selection, screen sharing, PiP and connection/encryption indicators.

For each, include real loading, empty, offline, failure and permission-denied states. Avoid buttons that merely simulate completion. Early native fixtures are clearly labelled fixtures; real-client milestones require actual Matrix state changes.

## Keyboard and accessibility

Support platform-native command modifiers, a jump-to-room command, search, room-history navigation, message actions and predictable Escape/focus restoration. Discover shortcuts in menus and a searchable shortcut reference. Respect keyboard layout and text-entry contexts instead of intercepting global keys indiscriminately.

Model the timeline as an accessible collection with sender, time, body and actions. Virtualization must preserve meaningful navigation beyond currently visible rows. Notifications and live updates should not continuously interrupt a screen reader. Security state uses words and actions in addition to icons/color. Verify with Orca/AT-SPI, Narrator/UIA and VoiceOver/AX at the platform qualification gates.

## Localization from the first screen

Use [Fluent](https://projectfluent.org/) and its [Rust implementation](https://github.com/projectfluent/fluent-rs) in Archaic. Store complete messages with stable IDs, named arguments, plural/select variants and translator comments. Never assemble sentences by concatenating translated fragments. Matrix IDs, event IDs, commands and internal state keys remain independent of translated strings.

Use locale negotiation: explicit user preference, supported OS locale, then English fallback. Missing keys have a deterministic fallback and diagnostics. Language changes update visible app strings and layouts; let the OS own standard system-dialog translation. Archaic can supply translated labels for CUI-owned compositions through the planned CUI resolver without adding a localization dependency to the C library.

The format/locale layer handles dates, times, relative time, time zones, numbers, file sizes and plural categories. Evaluate a Rust locale-data implementation such as [ICU4X](https://github.com/unicode-org/icu4x) and limit the compiled data to required locales; measure the size cost. Fluent message selection alone does not solve every formatting or collation requirement. Define locale-sensitive searching without altering protocol identifiers.

English is the source catalog. Proposed first translated catalog: German, subject to language priorities and review. Include expanded pseudolocalization and an RTL pseudo-locale from P1. Use Arabic/Hebrew, Japanese/Korean/Chinese composition, combining marks, emoji sequences and mixed-direction messages as correctness fixtures even before full translations are available. Do not claim those languages are fully translated just because their text renders.

In RTL mode, mirror layout navigation and logical start/end spacing, but keep URLs, Matrix IDs and code isolated with their natural direction. Direction of message content is independent from the UI locale. Test focus order, selection, cursor movement, tooltip/popup placement and screen-reader reading order. Fonts and line height must accommodate fallback scripts without clipping.

CI checks catalog key coverage, argument/type consistency, missing or unused messages and pseudolocalized layout overflow. Human review validates tone, grammar and security-sensitive wording. Locale coverage is tracked separately from platform and feature completion.

## High DPI and responsiveness

Use logical units and vector assets, native font metrics, per-monitor scale updates and bounded image resolution. Exercise fractional scale, 4K displays, moving between monitors and enlarged text. Test 100%, 125%, 150%, 200% and 300% display scale where available, and independent large-text preferences. Keep controls reachable at smaller window sizes with localized labels.

Avoid rasterizing the whole application into one fixed-size surface. CUI's current surface limit is 4096 physical pixels per axis; use bounded/segmented surfaces and native widgets, or improve the library where a verified workload requires it. Prefer incremental visible-region rendering and load media only when needed.

## Privacy and user trust in the UI

Make sending, delivered/read, encryption and device verification distinct concepts. Treat unable-to-decrypt as a recoverable state with explanation, never as missing content. Confirm destructive account/security actions with precise scope. Settings for notification previews, cache retention, link previews and automatic media downloads should explain what is stored or sent.

External media and message HTML are untrusted. Render sanitized content as native document elements; never execute scripts from a message. Opening external links and compatibility widgets follows explicit application policy. Diagnostic exports redact tokens, keys and message bodies by default.
