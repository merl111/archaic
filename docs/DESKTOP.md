# Desktop integration and release archives

Updated October 1, 2026. Native integration uses Rust platform adapters alongside the separately linked CUI library. Windows/macOS code is included, but runtime verification remains deferred. Portable archives are unsigned; native installer/signing support is described in [Release packaging](RELEASE.md).

## Profiles and account switching

Launch `archaic --profile work`, or enter `work` in the account-profile field and choose **Switch account**. Names are 1–40 ASCII letters, numbers, underscores or hyphens. `default` preserves the original profile path. Other profiles live under `profiles/NAME` beneath the application data directory, each with a separate credential-vault entry, encrypted session and Matrix device store.

Switching waits for dirty composer drafts to save, pauses the previous outgoing queue and closes that account's connection. A draft write failure leaves the current account available and retries saving; it does not silently discard the draft. Switching does not log out the previous device. Returning restores the existing device and its queue. Typing and automatic receipts reset to off. Desktop notification preferences are saved per profile and restored after connecting. The saved-profile selector lists valid local profile directories without opening their vaults; use the text field to create/select another profile. Only the active profile syncs.

## Notifications, tray and badges

Enable **Desktop notifications** in a connected account. Native notifications contain a sanitized room name and generic activity label, never the message body. Initial sync, own messages, blocked senders and activity in the focused composer are suppressed. Four fixed workers and an eight-item queue bound pending notifications; excess notifications are dropped. Queued notifications are invalidated when switching/signing out. Notifications already delivered to the OS are not recalled. Delivery requires a functioning OS notification service. Clicking the default notification action opens its account, room and event context through the same validated link path as desktop activation. Stale actions after switching/signing out or disabling notifications are discarded. Non-secret notification preferences are stored per profile in `desktop.json`; an unreadable file defaults notifications off and is never overwritten silently.

The native tray/status item offers **Show Archaic**, an unread count and **Quit**. Its icon marks unread activity and its tooltip includes the count. macOS additionally sets the dock badge. Linux uses StatusNotifierItem/D-Bus without adding GTK 3 or AppIndicator. A desktop shell must support status items to show one. Windows taskbar overlays and configurable close-to-tray behavior remain open: closing the main window currently exits. Graceful exit drains queued command acknowledgements and saves composer drafts; failures/timeouts are reported on stderr.

## Links and shortcuts

Paste a `matrix:` or `https://matrix.to/` link into **Matrix link** and choose **Open link**. Command-line forms are `archaic --open 'matrix:…'` or a positional link. Joined rooms open directly, including event context. Other rooms/users populate a join/new-chat form for confirmation. Up to eight valid federation `via` hints accompany the confirmed join. Links never sign in or join automatically.

Normal launches forward to the running instance over an authenticated loopback channel with a private per-user lock file. `--profile NAME` explicitly selects the target account; without it the current account is used. Busy operations defer activation, and links arriving before sign-in wait for room synchronization. Fixture, temporary and smoke-test launches bypass single-instance routing. macOS app bundles register the `matrix:` scheme and handle native URL-open events; that platform path still needs native verification.

| Shortcut | Action |
| --- | --- |
| Ctrl/Cmd+K | Focus room filter |
| Ctrl/Cmd+N | New conversation |
| Ctrl/Cmd+J | Join room |
| Ctrl/Cmd+F | Search selected room |
| Ctrl/Cmd+R | Refresh |
| Ctrl/Cmd+Shift+O | Rooms and organization |
| Ctrl/Cmd+Shift+S | Security and recovery |
| Ctrl/Cmd+L | Focus Matrix link |

The commands also appear in the native Navigation menu and respect connected/busy state. Autostart and spellcheck integration are not included.

## Create a release archive

On the target OS, install the normal build prerequisites and run:

```sh
python3 tools/package.py
```

This builds pinned CUI and Archaic in release mode, then creates an archive and SHA-256 sidecar under `dist/`. To package a native binary already built on this OS:

```sh
python3 tools/package.py --binary target/release/archaic --output dist
```

On Windows use `py -3` and `target/release/archaic.exe`. The helper does not cross-compile or inspect foreign ABIs. CUI and SQLite are linked into the binary. GTK is **not bundled**. Linux needs compatible GTK 4.6+, linked system libraries and a Secret Service provider for saved credentials. Build on the oldest supported distribution and test there.

- Linux: `.tar.gz` with `bin/archaic`, desktop entry, documentation and metadata. Add `bin` to PATH; optionally install the desktop entry under `~/.local/share/applications/` and register it with `xdg-mime default org.archaic.desktop.desktop x-scheme-handler/matrix`.
- macOS: `.zip` with `Archaic.app`, executable and `Info.plist` including Matrix URL registration. No signing, hardened-runtime setup or notarization.
- Windows: `.zip` with `archaic.exe`. No Authenticode signing, installer/uninstaller or automatic registry changes. URI registration is an installation step still to implement.

No packaging command changes OS handlers. Each archive records source/CUI revisions, architecture, build OS and executable hash; existing archives are not overwritten. For Debian, signed/notarized macOS PKG and per-user Windows NSIS installer commands, see [Release packaging](RELEASE.md). License selection/review, signing credentials, automatic updates and platform release acceptance remain required before production distribution.

## Code and checks

`desktop.rs` owns preferences/shortcuts/link presentation, `notifications.rs` owns bounded notification delivery and action routing, `instance.rs` owns bounded authenticated activation, `status_item.rs` owns tray/badge lifetime, and `apple_links.rs` owns macOS URL events. `tools/package.py` stages archives; it is separate from installation.

Run `python3 tools/dev.py test` for protocol/unit checks. On Linux, `python3 tools/test_desktop_tray.py` runs a disposable D-Bus watcher and Xvfb application, verifying registration, icon data and Show/Quit menu actions. It uses no real Matrix account. `python3 tools/test_notifications.py` runs a private notification service and verifies escaped text, default-action delivery and account/event routing. Native Linux keyboard checks additionally exercise search and room administration. `python3 tools/test_package.py` covers all three archive layouts without claiming native Windows/macOS execution.
