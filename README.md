# Archaic

A native Matrix desktop client built with Rust and CUI, targeting Linux, Windows and macOS.

**Early development build.** The first implementation connects to a real Matrix homeserver using password login, synchronizes joined rooms and invitations, loads recent text messages, sends text and signs out. It is not yet an Element replacement. Use a test account: device verification/recovery are not implemented. Saved sessions now reopen with the same Matrix device and encryption store. Closing the window keeps a saved session; **Sign out** revokes it.

## Build and run

Keep the repositories next to each other:

```text
workspace/
  cui/       # revision recorded in archaic/cui-revision
  archaic/
```

Requirements: Rust 1.96 or newer, Cargo, Python 3.10+, CMake, a native C compiler and Git. The first build downloads Rust dependencies, including the Matrix SDK and its TLS/cryptography implementation. Linux additionally needs GTK 4.6+ development files, Xext, D-Bus development files and pkg-config; runtime GTK and libdbus remain system dependencies. Saved sign-in also requires a running Secret Service provider (such as GNOME Keyring or a compatible KWallet configuration). SQLite is compiled into the app through the Matrix SDK. macOS uses the Xcode command-line tools/AppKit. Windows uses an MSVC developer terminal with the Windows SDK. Windows/macOS build and runtime verification are still pending.

From this directory:

```sh
python3 tools/dev.py run
python3 tools/dev.py run -- --locale de
python3 tools/dev.py run -- --theme dark
python3 tools/dev.py build --release
python3 tools/dev.py test
```

On Windows, use `py -3 tools/dev.py run` from the developer terminal. The helper builds CUI's static library, configures `CUI_LIB_DIR`, and builds the public Rust binding from the same pinned source revision. Set `CUI_SOURCE_DIR` to use another checkout of that revision. A revision mismatch or tracked modifications stop the build; `--allow-dirty-cui` permits reviewed local library work without silently changing the pin. `Cargo.lock` is checked in. CUI has no remote yet, so source acquisition remains explicit rather than using an invented repository URL.

The development executable is `target/debug/archaic` (`archaic.exe` on Windows). Release builds use `target/release`. These are development binaries, not signed installers or macOS application bundles.

The sign-in screen uses a centered native card and a CUI ambient background. It follows system appearance by default. `--theme light`, `--theme dark` and `--theme system` let you preview both palettes now; the **Appearance** selector applies changes immediately on both the sign-in and conversation screens. Normal launches save that preference; `--theme` overrides it for the launch until you change the selector. Temporary, fixture and smoke launches do not read or write appearance preferences. Native window controls retain the desktop theme.

## Connect

1. Enter the HTTPS homeserver base URL, your Matrix ID or username, and password. Loopback HTTP addresses are accepted for local testing only.
2. Sign in, wait for the room and invitation list, then select a conversation.
3. Write a message and press **Send**. **Load older** fetches the previous page; **Refresh** merges recent events into the loaded history. **Latest** resets the view to the newest page. Filtering the room list preserves selection and drafts.
4. Use **+** beside Conversations to join by `#room:server` alias (recommended) or `!room:server` ID. Enter submits; failures preserve the address for retry. A successful join opens the room and preserves other-room drafts. This does not yet include directory browsing, Matrix links or explicit federation routing servers.
5. Select a conversation marked **Invitation** to see the inviter (when supplied by the server). **Accept** joins it; **Decline** asks for confirmation before rejecting it. History and sending stay disabled until you join.
6. In a joined room, enter a full `@person:server` ID and press **Invite**. Permission/network errors preserve the ID for retry. **Leave room** asks for confirmation and warns that the unsent draft is discarded and returning may require an invitation. The SDK also leaves known predecessor versions of upgraded rooms. Cancelling makes no server request.
7. Use **New** beside Conversations to open a direct chat by full Matrix ID or create a private group with a name, optional topic and up to 20 invitees. New rooms request encryption. Existing joined direct chats are reused with their current settings; they are not automatically upgraded to encryption. **Cancel** returns to the previous conversation and its draft. A failed request preserves the form; if the result is unconfirmed, check the room list before retrying because the server may already have created a room.
8. Select a message in the dropdown beneath the timeline, then choose **Reply**, **Edit**, **Delete**, or **React**. Edit/delete are limited to your own text messages. **Confirm** sends the action; **Cancel** returns to your ordinary draft. For reactions, enter an emoji or short text: confirming adds it, or removes your matching loaded reaction. Deletion always asks for confirmation. Failures keep your input for retry.
9. Use **Sign out** to revoke the current device session and clear the view. Failed server revocation permits retry. If revocation succeeds but clearing the local sign-in fails, messaging is disabled and **Sign out** retries only local cleanup.

Only password login is implemented; OIDC/SSO and interactive authentication flows come later. The Matrix SDK's encryption implementation is enabled, but verification and backup/recovery UI remain unfinished. This is **not a production E2EE client**. Unavailable encrypted messages are shown as placeholders. Avoid using this build for valuable conversations.

The selectable native text timeline starts with up to 50 room events. **Load older** follows the server's backward cursor, deduplicates overlapping events and keeps loaded messages during overlapping sync/refresh updates. Failed pages leave the current messages and cursor intact; retry with the same button. **Latest** discards the loaded window and fetches the newest page. Controls and errors are translated into English and German.

This temporary text view keeps at most 500 raw events for the selected room and at most 128 backward page cursors. Either limit disables further backward loading; **Latest** starts a fresh window. Non-message events count toward the cap. Switching rooms resets the view; pagination position is not saved across room switches or restart. If recent events do not overlap the loaded window, Archaic shows the newest page and a gap notice instead of silently splicing disconnected history. Pixel scroll position is not preserved when text changes. Replies show their target, valid loaded edits replace the original text, deleted messages show a tombstone, and loaded reactions show distinct-sender counts (a check marks your participation). Relation handling covers only this loaded window; bundled/server-wide relations and edit-history browsing are unfinished. Forward pagination, jump-to-event, rich messages, threads, attachments, public-room creation/directory browsing, receipts and calling remain unfinished. The planned CUI virtualized rich timeline will replace this text view.

Appearance preferences are stored separately from credentials: `$XDG_CONFIG_HOME/archaic/appearance` (or `~/.config/archaic/appearance`) on Linux, `~/Library/Preferences/archaic/appearance` on macOS, and `%APPDATA%\archaic\appearance` on Windows. Failed writes leave the selected theme active and show a warning. Missing/unrecognized preferences fall back to System.

## Saved and temporary sessions

Normal sign-in saves the Matrix device session automatically. On restart, Archaic restores its existing device ID and checks the encryption-key fingerprint before starting synchronization. An exclusive profile lock prevents two persistent instances from using the same crypto store.

The session file uses authenticated XChaCha20-Poly1305 encryption with a random 256-bit key held by Linux Secret Service, macOS Keychain or Windows Credential Manager. The Matrix SDK's SQLite stores are configured with a passphrase derived from that key. This protects the SDK's encrypted values; it is **not whole-database or disk encryption**, and database structure/metadata can remain visible. No hand-written cryptographic primitives are used.

Default profile locations:

- Linux: `$XDG_DATA_HOME/archaic`, or `~/.local/share/archaic`.
- macOS: `~/Library/Application Support/Archaic`.
- Windows: `%LOCALAPPDATA%\Archaic`.

The vault entry uses service `org.archaic.desktop`, account `default-profile-v1`. Keep the profile and its credential-vault key together when backing up; copying only the directory is insufficient. The encrypted `session.enc` record is replaced atomically. Private profile directories use owner-only permissions on Unix; Windows uses the current user's profile-directory ACLs.

If the vault is unavailable or the saved session cannot be opened, **Retry saved sign-in** retries without deleting it. **Use a temporary session** switches explicitly to memory-only operation and leaves any saved profile intact. You can also launch with:

```sh
python3 tools/dev.py run -- --temporary-session
```

Temporary sessions create a new Matrix device. Use **Sign out** before closing to revoke that device. A disk failure after successful authentication leaves a usable temporary session with a visible warning; it is never reported as saved. If the process exits between server login and local persistence, remove the resulting device from another client.

Successful sign-out revokes the access token and replaces the saved-login record with an encrypted empty record. **Encrypted SDK caches, crypto stores and the vault key are currently retained**; sign-out is not a local-data purge or secure erase. Cache removal and recovery-aware device management need their own UI. Revoked/expired tokens stop synchronization and sending; explicit sign-out clears their saved login. Refresh tokens are not requested, and same-device soft-logout reauthentication remains unfinished.

## Development and verification

```sh
python3 tools/dev.py check
python3 tools/dev.py test
python3 tools/dev.py run -- --fixture --locale de
python3 tools/dev.py run -- --fixture --smoke-test
```

`--fixture` is an explicitly labeled offline visual-test screen with network actions disabled. Normal startup first tries the saved session; without one it shows real sign-in. `--fixture` and `--smoke-test` never open the profile or credential vault. `--smoke-test` exits after approximately 1.8 seconds and still requires a graphical display; Linux CI can use Xvfb.

- `crates/archaic-app`: native CUI views and UI state; no SDK calls on the UI thread.
- `crates/archaic-matrix`: bounded command/event channels, SDK client, classic sync and room operations.
- `crates/archaic-i18n`: embedded Fluent English/German catalogs with English fallback.
- `locales`: all application chrome and error messages. Choose the language with `--locale`; live locale switching, platform locale discovery and RTL layout remain unfinished.
- `tools/dev.py`: portable native-library and Rust build entry point.

The UI currently drains up to eight events per 50 ms CUI timer tick. A reusable CUI worker dispatcher remains a separate planned library change. Widget callbacks are registered once; room updates reuse existing controls. Worker shutdown aborts synchronization and bounds runtime shutdown. Passwords are never deliberately logged or saved; access tokens are saved only in the encrypted session record. The profile key and temporary decrypted record buffers use zeroizing wrappers; the password entry is cleared after submitting login. No claim is made that every temporary allocator copy is securely erased.

Protocol tests use a local HTTP mock server and the real Matrix SDK, without external accounts. They cover private group/direct-chat creation, existing DM reuse, partial DM-label failures, login, classic sync, room discovery, history ordering, backward cursors, retry/empty/repeated/malformed pages, refresh merging and Latest reset, authorization, transaction IDs, sending, invitation acceptance/decline, user invitations, leave retries, external membership changes, rejected credentials, restart without another login, corrupted saved data, token revocation and local logout cleanup retries. Storage tests cover encryption/authentication, missing keys/stores and exclusive profile locking. `python3 tools/test_linux_vault.py` additionally exercises the real Linux credential adapter in a disposable D-Bus session/keyring; it never uses the desktop vault. They do not replace interoperability or encrypted multi-device tests against a real homeserver.

## Roadmap and architecture

- [Current implementation status](docs/IMPLEMENTATION.md)
- [Implementation plan](docs/PLAN.md): architecture, phases, risk decisions and release criteria.
- [Element parity inventory](docs/PARITY.md): versioned feature families, acceptance checks and source evidence.
- [CUI work required](docs/CUI-GAPS.md): reusable framework changes, kept in the separate CUI repository.
- [Design and localization](docs/DESIGN.md): desktop layouts, interaction rules and multilingual behavior.
- [Machine-readable inventory](docs/parity.json) and [upstream feature/settings index](docs/upstream-index.json).

Archaic consumes CUI's public Rust binding and C ABI. It does not copy private backend code or become part of CUI's examples.
