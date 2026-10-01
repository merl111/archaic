# Archaic

A native Matrix desktop client built with Rust and CUI, targeting Linux, Windows and macOS.

**Early development build.** The first implementation connects to a real Matrix homeserver using password login, synchronizes joined rooms, loads recent text messages, sends text and signs out. It is not yet an Element replacement. Use a test account: sessions and encryption keys are currently memory-only, and device verification/recovery are not implemented. Closing the window does not revoke the server session; use **Sign out** while connected before closing, or remove the device in another client.

## Build and run

Keep the repositories next to each other:

```text
workspace/
  cui/       # revision recorded in archaic/cui-revision
  archaic/
```

Requirements: Rust 1.96 or newer, Cargo, Python 3.10+, CMake, a native C compiler and Git. The first build downloads Rust dependencies, including the Matrix SDK and its TLS/cryptography implementation. Linux additionally needs GTK 4.6+ development files, Xext and pkg-config; runtime GTK remains a system dependency. macOS uses the Xcode command-line tools/AppKit. Windows uses an MSVC developer terminal with the Windows SDK. Windows/macOS build and runtime verification are still pending.

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

The sign-in screen uses a centered native card and a CUI ambient background. It follows system appearance by default. `--theme light`, `--theme dark` and `--theme system` let you preview both palettes now; a persistent in-app System/Light/Dark switcher is planned for the settings screen. Native window controls retain the desktop theme.

## Connect

1. Enter the HTTPS homeserver base URL, your Matrix ID or username, and password. Loopback HTTP addresses are accepted for local testing only.
2. Sign in, wait for the joined room list, then select a conversation.
3. Write a message and press **Send**. **Refresh** reloads recent history. Filtering the room list preserves selection and drafts.
4. Use **Sign out** to revoke the current device session and clear the view. A failed sign-out leaves the session available for retry.

Only password login is implemented; OIDC/SSO and interactive authentication flows come later. The Matrix SDK's encryption implementation is enabled, but there is no verification, backup/recovery or persistence UI yet. This is **not a production E2EE client**. Unavailable encrypted messages are shown as placeholders. Avoid using this build for valuable conversations.

The timeline currently shows text from the latest 50 room events in a selectable native text view. Rich messages, history pagination, edits, threads, attachments, invitations, room creation, receipts and calling are unfinished. The native view is a temporary foundation for the planned CUI virtualized rich timeline.

## Development and verification

```sh
python3 tools/dev.py check
python3 tools/dev.py test
python3 tools/dev.py run -- --fixture --locale de
python3 tools/dev.py run -- --fixture --smoke-test
```

`--fixture` is an explicitly labeled offline visual-test screen with network actions disabled. Normal startup always shows real sign-in. `--smoke-test` exits after approximately 1.8 seconds and still requires a graphical display; Linux CI can use Xvfb.

- `crates/archaic-app`: native CUI views and UI state; no SDK calls on the UI thread.
- `crates/archaic-matrix`: bounded command/event channels, SDK client, classic sync and room operations.
- `crates/archaic-i18n`: embedded Fluent English/German catalogs with English fallback.
- `locales`: all application chrome and error messages. Choose the language with `--locale`; live locale switching, platform locale discovery and RTL layout remain unfinished.
- `tools/dev.py`: portable native-library and Rust build entry point.

The UI currently drains up to eight events per 50 ms CUI timer tick. A reusable CUI worker dispatcher remains a separate planned library change. Widget callbacks are registered once; room updates reuse existing controls. Worker shutdown aborts synchronization and bounds runtime shutdown. Credentials are never deliberately logged or persisted; the password entry is cleared after submitting login. No claim is made that every temporary allocator copy is securely erased.

Protocol tests use a local HTTP mock server and the real Matrix SDK, without external accounts. They cover login, classic sync, room discovery, history ordering, authorization, transaction IDs, sending, logout and rejected credentials. They do not replace interoperability or encrypted multi-device tests against a real homeserver.

## Roadmap and architecture

- [Current implementation status](docs/IMPLEMENTATION.md)
- [Implementation plan](docs/PLAN.md): architecture, phases, risk decisions and release criteria.
- [Element parity inventory](docs/PARITY.md): versioned feature families, acceptance checks and source evidence.
- [CUI work required](docs/CUI-GAPS.md): reusable framework changes, kept in the separate CUI repository.
- [Design and localization](docs/DESIGN.md): desktop layouts, interaction rules and multilingual behavior.
- [Machine-readable inventory](docs/parity.json) and [upstream feature/settings index](docs/upstream-index.json).

Archaic consumes CUI's public Rust binding and C ABI. It does not copy private backend code or become part of CUI's examples.
