# Archaic

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/branding/icon-master.png">
  <img src="assets/branding/logo.png" alt="Archaic logo" width="280">
</picture>

Archaic foundations, modern conversations. The name is a nod to building a modern desktop app on a C GUI library.

A native Matrix desktop client built with Rust and CUI, targeting Linux, Windows and macOS.

**Early development build.** Archaic connects to real Matrix homeservers with password, classic browser SSO or delegated OAuth, restores saved devices, and supports security/recovery controls, persistent outgoing text, incremental history, message actions, attachments and search. It is not yet an Element replacement. Use a test account while acceptance work continues. Closing the window keeps a saved session; **Sign out** revokes it. See [security and messaging workflows](docs/SECURITY-MESSAGING.md) for the current behavior and remaining work in priorities 1–3.

Manual native builds are available under **Actions → Build release → Run workflow**.
See [release instructions](docs/RELEASE.md) for prerequisites, icons and packaging.

## Build and run

Clone both private repositories with a GitHub account that has access, keeping them next to each other:

```sh
git clone https://github.com/merl111/cui.git
git clone https://github.com/merl111/archaic.git
git -C cui checkout "$(cat archaic/cui-revision)"
cd archaic
```

```text
workspace/
  cui/       # revision recorded in archaic/cui-revision
  archaic/
```

Requirements: Rust 1.96 or newer, Cargo, Python 3.10+, CMake, a native C compiler and Git. The first build downloads Rust dependencies, including the Matrix SDK and its TLS/cryptography implementation. Linux additionally needs GTK 4.6+ development files, Xext, D-Bus development files and pkg-config; runtime GTK and libdbus remain system dependencies. Saved sign-in also requires a running Secret Service provider (such as GNOME Keyring or a compatible KWallet configuration). SQLite is compiled into the app through the Matrix SDK. macOS uses the Xcode command-line tools/AppKit. Windows uses a Visual Studio 2022 MSVC developer terminal, the Windows SDK, NuGet, and the Windows App SDK 1.8 runtime. Native compilation has been verified on Linux, Windows and macOS. Broader Windows/macOS runtime verification remains pending.

From this directory:

```sh
python3 tools/dev.py run
python3 tools/dev.py run -- --locale de
python3 tools/dev.py run -- --theme dark
python3 tools/dev.py build --release
python3 tools/dev.py test
```

On Windows, use `py -3 tools/dev.py run` from the developer terminal. The helper builds CUI statically on Linux/macOS and as a WinUI DLL on Windows, configures `CUI_LIB_DIR`, and builds the public Rust binding from the same pinned source revision. Windows builds stage `cui.dll` and `Microsoft.WindowsAppRuntime.Bootstrap.dll` beside the app and test executables; release archives include them. Install the Windows App SDK 1.8 runtime and the matching Microsoft VC++ runtime separately. Set `CUI_SOURCE_DIR` to use another checkout of that revision. A revision mismatch or tracked modifications stop the build; `--allow-dirty-cui` permits reviewed local library work without silently changing the pin. `Cargo.lock` is checked in. CUI source is available in the private [merl111/cui](https://github.com/merl111/cui) repository; check out the commit recorded in `cui-revision` before building.

The development executable is `target/debug/archaic` (`archaic.exe` on Windows). Release builds use `target/release`. These are development binaries, not signed installers or macOS application bundles.

The UI uses the native components and visual language from CUI’s Rust `daylight.rs`: a cyan canvas, rounded white panels, a compact conversation composer and a room inspector. Sign-in and dialogs use the same palette and spacing. See [Daylight UI and controls](docs/DAYLIGHT.md). It follows system appearance by default. `--theme light`, `--theme dark` and `--theme system` let you preview both palettes now; the **Appearance** selector in **Account & preferences** applies changes immediately on both the sign-in and conversation screens. Normal launches save that preference; `--theme` overrides it for the launch until you change the selector. Temporary, fixture and smoke launches do not read or write appearance preferences. Native window controls retain the desktop theme.

## Connect

1. Enter the HTTPS homeserver base URL, your Matrix ID or username, and password. Loopback HTTP addresses are accepted for local testing only.
2. Sign in, wait for the room and invitation list, then select a conversation.
3. Write a message and press **Send**. The inspector’s **Messages** action opens history controls: **Load older** fetches the previous page and **Latest** resets the view to the newest page. **Refresh** merges recent events into the loaded history. Filtering the room list preserves selection and drafts.
4. Use **+** beside Conversations to join by `#room:server` alias (recommended) or `!room:server` ID. Enter submits; failures preserve the address for retry. A successful join opens the room and preserves other-room drafts. Directory browsing is available in Rooms & organization; Matrix links preserve federation routing hints for confirmed joins.
5. Select a conversation marked **Invitation** and open **Room tools** in the inspector to see the inviter (when supplied by the server). **Accept** joins it; **Decline** asks for confirmation before rejecting it. History and sending stay disabled until you join.
6. In a joined room, open **Room tools**, enter a full `@person:server` ID and press **Invite**. Permission/network errors preserve the ID for retry. **Leave room** asks for confirmation and warns that the unsent draft is discarded and returning may require an invitation. The SDK also leaves known predecessor versions of upgraded rooms. Cancelling makes no server request.
7. Use **New** beside Conversations to open a direct chat by full Matrix ID or create a private group with a name, optional topic and up to 20 invitees. New rooms request encryption. Existing joined direct chats are reused with their current settings; they are not automatically upgraded to encryption. **Cancel** returns to the previous conversation and its draft. A failed request preserves the form; if the result is unconfirmed, check the room list before retrying because the server may already have created a room.
8. Hover or keyboard-focus a message to reveal **React**, **Reply**, **Thread** and **More**. Replies and edits use the main composer with a context strip; Escape cancels and restores the ordinary draft. **Thread** opens a relation-backed side pane with its own composer and pagination. **More** opens a native anchored menu for copy, edit, delete, forward, details and surrounding history. Edit/delete require your own text message; deletion still requires confirmation. Click an existing reaction to add/remove your own matching reaction. The emoji button opens a searchable common-emoji picker; pasting another Unicode sequence is supported.
9. Use **Attachments** or **Search messages** in the inspector for file upload/save, local or server search, message details with complete relation pagination, and explicit private/public read receipts. Search results can open messages beyond the loaded timeline. See the [room tools guide](docs/ROOM-TOOLS.md) for steps and limits.
10. Open **Account & preferences** and use **Sign out** to revoke the current device session and clear the view. Failed server revocation permits retry. If revocation succeeds but clearing the local sign-in fails, messaging is disabled and **Sign out** retries only local cleanup.

Classic browser SSO and delegated OAuth/PKCE are available on compatible servers; saved OAuth sessions restore their client identity and tokens. Password soft logout resumes the existing device. See the authentication compatibility limits in the workflow guide. **Security & recovery** supports SAS verification, cross-signing bootstrap, recovery-key setup/unlock, room backup-key retrieval and encrypted key-file import/export. Two disposable devices have exchanged encrypted messages against a real local Synapse, but full production E2EE acceptance and Element interoperability remain open.

The timeline uses the same native CUI chat renderer as Daylight, populated with Matrix messages, timestamps, reply previews, attachment cards and reaction counts. **Show surrounding messages** opens selected or searched events with older/newer pagination; **Latest** returns to the live end. The window remains bounded to 500 raw events and 128 cursors per direction. Rich-text/Markdown editing and a global unread-thread inbox remain pending. A dedicated thread pane displays the root and loaded relation replies, loads additional relation pages, and sends through the SDK queue; Message details remain available for receipts and revisions. **Forward text** sends to a joined room by ID.

Ordinary composer text is persisted in the SDK send queue for saved sessions. Pending messages have native retry/cancel controls and survive restart; temporary sessions do not. Native replies, threads, edits, reactions, forwarding, deletion and attachments also enter the SDK queue. Target-dependent operations still validate/fetch their target before insertion. Ordinary composer drafts are stored separately with authenticated encryption. Incoming typing and unread notification counts are shown. Outgoing typing and private receipts while composing are opt-in; public receipts remain explicit. See the [workflow guide](docs/SECURITY-MESSAGING.md) and [room tools](docs/ROOM-TOOLS.md) for all limits.

Appearance preferences are stored separately from credentials: `$XDG_CONFIG_HOME/archaic/appearance` (or `~/.config/archaic/appearance`) on Linux, `~/Library/Preferences/archaic/appearance` on macOS, and `%APPDATA%\archaic\appearance` on Windows. Failed writes leave the selected theme active and show a warning. Missing/unrecognized preferences fall back to System.

## Rooms and desktop integration

**Rooms & organization** exposes directory/space discovery, space creation/children, room name/topic/access/history settings, member lists, power levels, invitation/kick/ban/unban, favorites and blocking. See the [operation/input guide](docs/ORGANIZATION.md).

The [desktop guide](docs/DESKTOP.md) covers isolated account profiles, account switching, opt-in native notifications, tray controls, macOS dock badges, shortcuts, Matrix links, existing-instance routing and release archives. Run `python3 tools/package.py` on each target OS to create an unsigned native development archive. GTK remains a Linux system dependency. Broader native Windows/macOS desktop integration testing and production signing/installers remain deferred.

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

Successful sign-out revokes the access token and replaces the saved-login record with an encrypted empty record. **Encrypted SDK caches, crypto stores and the vault key are currently retained**; sign-out is not a local-data purge or secure erase. Cache removal and recovery-aware device management need their own UI. Revoked/expired tokens stop synchronization and sending; explicit sign-out clears their saved login. Refresh tokens are requested and rotated tokens are saved through SDK callbacks. Password soft logout supports same-device reauthentication; OAuth/SSO reauthorization remains pending.

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

Protocol tests use a local HTTP mock server and the real Matrix SDK, without external accounts. They cover private group/direct-chat creation, existing DM reuse, partial DM-label failures, login, classic sync, room discovery, history ordering, backward cursors, retry/empty/repeated/malformed pages, refresh merging and Latest reset, authorization, transaction IDs, sending, invitation acceptance/decline, user invitations, leave retries, external membership changes, rejected credentials, restart without another login, corrupted saved data, token revocation and local logout cleanup retries. Storage tests cover encryption/authentication, missing keys/stores and exclusive profile locking. `python3 tools/test_linux_vault.py` additionally exercises the real Linux credential adapter in a disposable D-Bus session/keyring; it never uses the desktop vault. Additional real-server coverage is available through `python3 tools/test_live_matrix.py`; it creates a disposable loopback Synapse and tests two-device verification, recovery and encrypted messages. These tests do not establish complete Element interoperability.

## Roadmap and architecture

- [Daylight UI, controls and native validation](docs/DAYLIGHT.md)
- [Attachments, search, receipts and message relations](docs/ROOM-TOOLS.md)
- [Current implementation status](docs/IMPLEMENTATION.md)
- [Implementation plan](docs/PLAN.md): architecture, phases, risk decisions and release criteria.
- [Element parity inventory](docs/PARITY.md): versioned feature families, acceptance checks and source evidence.
- [CUI work required](docs/CUI-GAPS.md): reusable framework changes, kept in the separate CUI repository.
- [Design and localization](docs/DESIGN.md): desktop layouts, interaction rules and multilingual behavior.
- [Machine-readable inventory](docs/parity.json) and [upstream feature/settings index](docs/upstream-index.json).

Archaic consumes CUI's public Rust binding and C ABI. It does not copy private backend code or become part of CUI's examples.

Native installer and signing commands: [release packaging](docs/RELEASE.md). Room administration and thread browsing: [organization guide](docs/ORGANIZATION.md).
