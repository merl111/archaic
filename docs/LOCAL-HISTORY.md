# Local history and appearance

## Startup and history

Persistent profiles restore the Matrix SDK's encrypted room state and saved sync
token. The UI shows a restoration screen while the store opens, then exposes
cached conversations before waiting for network sync. Selecting a cached room
loads its newest 50 message roots and their relations from local SQLite. Older
loads expand that viewport by 50. Scrolling upward near the first message requests
an older page and preserves the reading anchor. At 500 events the viewport moves
to an overlapping context window so browsing can continue with bounded UI memory.
The archive has no 500-event cap. Context navigation uses the homeserver.

After the initial live sync, a separate background task walks all joined,
non-space rooms in 50-event pages,
with one request at a time. It resumes persisted cursors after restart, detects
repeated pagination tokens, retries failed rooms, and fills gaps reported by
limited sync responses. Live sync merges new events and relations into the same
archive. Recovering room keys also retries cached encrypted events in bounded
batches. Background catch-up is incremental: a new profile does not have all
history immediately, and server retention, visibility rules or missing keys can
make older messages unavailable. Temporary sessions do not persist or perform
full-history backfill.

## Storage and search

Each device store contains `history.sqlite3` (with SQLite WAL/SHM files) and a
protected `archive-key.enc`. Message payloads, relation content and pagination
state use authenticated encryption through the Matrix SDK's StoreCipher. Room
and event identifiers and search trigrams are keyed hashes. SQLite can still
reveal timestamps, record counts, sizes and equality patterns: this is encrypted
content, not whole-file database encryption. Do not copy the database alone as a
usable backup; it needs its profile/device keys.

The previous encrypted search snapshot migrates once into the archive. Searches
verify matching text against the current projected message (including edits and
redactions), and retain the existing sender, date and attachment filters.
Redaction tombstones prevent a later history page from restoring removed text.
Search results are capped at 1,000 per query; narrow the query when truncated.
Removing a saved device/profile removes its archive with the existing store.

## Appearance and interactions

Settings opens a category sidebar for Account, Appearance, Notifications &
privacy, Security & recovery, and Advanced. Appearance provides System, Light
and Dark themes, plus a global Bubbles/Compact message layout shared by chats
and threads. Layout persists in `desktop.json` beside the existing appearance
preference; account notification settings remain independent.

Message bodies no longer open the action menu on left click. Right click opens
it at the pointer; the toolbar retains reaction/reply/thread/more actions.
HTTP(S) links use native hit regions and open in the system browser. Clicking a
sender avatar opens their profile in the room sidebar, with a return to room
details, messaging, receipt navigation, copying, mentioning and ignoring actions.
Verification badges reflect SDK identity verification, not room encryption.
Direct conversations without room art use the other member's avatar.

Emoji pickers provide searchable pages and skin-tone entries. Typed Unicode
emoji remain supported; rendering depends on installed system emoji fonts.
Archaic disables visible focus outlines while retaining keyboard interaction.

## Validation

`cargo test --workspace --locked` covers archive retention, atomic cursor updates,
redaction/edit replay, encrypted payload identity, cache-first offline restore,
links and preference persistence. `tools/test_daylight_ui.py` uses a private
loopback Matrix fixture and temporary session. Run under Xvfb:

```sh
xvfb-run -a -s '-screen 0 1600x1050x24' python3 tools/test_daylight_ui.py
xvfb-run -a -s '-screen 0 1600x1050x24' python3 tools/test_daylight_ui.py --navigation
xvfb-run -a -s '-screen 0 1600x1050x24' python3 tools/test_daylight_ui.py --polish --theme dark
```

Windows and macOS runtime verification remains pending.

Settings → Advanced shows background history progress: completed rooms, locally
cached events (including relations), encrypted events still waiting for keys, and
rooms being retried after an error. Counts describe available history, not a guessed
percentage of an unknown server total. Room backfill completion does not imply
that all encrypted events have their keys.

## Composer and notifications

The attachment button opens the native file picker directly. The chosen filename
appears above the composer; Send queues it and × removes it. Existing text drafts
are preserved. The overflow menu offers a file-backed sticker, a two-minute native
voice recording, and a single-answer poll (2–20 choices). Recording begins only
with Record, stops with Stop or dismissal, and requires a separate Send action.
Voice capture uses CPAL (ALSA on Linux, Core Audio on macOS, WASAPI on Windows);
the Linux build needs ALSA development headers. Microphone operation on Windows
and macOS still needs platform testing. SDK upload/event encryption is used in
encrypted rooms. Poll creation uses Element's MSC3381 event format; inline voting
and tallies and sticker-pack browsing are not implemented by this change.

Plain Matrix IDs and @room are highlighted and outgoing text includes m.mentions.
Desktop notifications still respect the profile's notification toggle and Matrix
push rules, including room mute/mention permissions. Native reaction tooltips show
the known reactors' Matrix IDs; counts and names are deduplicated by sender.
