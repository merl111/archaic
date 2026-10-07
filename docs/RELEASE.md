# Releases and native installers

## Manual GitHub releases

Use **Actions → Build release → Run workflow** on the desired source revision.
Enter a new tag matching `workspace.package.version`, currently `v0.1.0`.
Increment that version (and update Cargo.lock) before making another version.
The workflow rejects existing tags, builds native Linux, macOS and Windows
archives, checks their packaging, and creates a release with SHA-256 sidecars
only when all three builds succeed. Prerelease is enabled by default; a draft
checkbox is available. Uncheck **Create a GitHub release after building** to
validate all three platforms and download artifacts without creating a tag or
release. Pushes and tags never trigger this workflow automatically.

The workflow checks out the exact commit in `cui-revision` from `merl111/cui`.
`CUI_DEPLOY_KEY` is an encrypted Actions secret on Archaic with the private half
of a dedicated **read-only** CUI deploy key. It is already configured. The
checkout does not persist the key. If rotating it, replace the CUI repository's
“Archaic release builds (read only)” deploy key and update that secret together.
Do not use an account-wide token. The ordinary `GITHUB_TOKEN` publishes only to
this repository.

These are unsigned development archives, not signed installers. Native Windows
and macOS runtime verification remains separate from successful compilation.
Windows users still need Windows App Runtime 1.8 and the Microsoft VC++ runtime.
The Linux build uses Ubuntu 24.04 system libraries; it is not a universal binary
for older distributions. The archive names and build.json record the actual
runner architecture. This workflow does not change repository visibility.

## Application icons

The approved mark is embedded in the application and tray, and the README uses
the full logo. Packaging adds Linux hicolor PNGs and the desktop entry, a macOS
ICNS resource and Info.plist declaration, and the Windows ICO resource compiled
into the EXE. NSIS also uses the icon for setup/uninstall. See
[branding assets](../assets/branding/README.md) for regeneration and desktop
integration details. All assets ship in the checkout; builds need no graphics
generation service.

A Linux tar archive contains `bin/` and `share/`; install both under the same
prefix (for example `~/.local`), with its `bin` on PATH, so the desktop can find
`org.archaic.desktop.desktop` and its matching hicolor icons. Refresh the desktop's
icon cache if necessary. A bare ELF file has no portable embedded launcher icon.
macOS Finder uses the packaged `Archaic.app`; Windows Explorer reads the EXE's
embedded icon.

`tools/package.py` still creates unsigned portable archives. `tools/installer.py` adds native installers, a required license bundle, and signing commands. Run it **on the target OS**, against a native release binary built with the pinned CUI revision. Neither script installs the app or changes the developer machine's URL handlers.

## Preconditions

Build with `python3 tools/dev.py build --release`. Verify the binary and its runtime dependencies on the oldest supported OS/distribution. Select the application and CUI distribution licenses, collect the actual notices/license texts for shipped dependencies, and review that bundle before supplying `--licenses`. The repositories do not currently select a top-level application/CUI license; the script cannot make that decision. A nonempty folder check is not a license audit.

The release version must be numeric, for example `0.1.0`. Existing output files are never overwritten. Packaging/signing happens in a temporary staging directory; the output is published only after the platform commands succeed, with a SHA-256 sidecar. `build.json` records source/CUI revisions, target architecture, build OS and the **input** executable hash. Signing can change executable bytes, so the installer hash identifies the final artifact. No credential is embedded in the package.

## Linux: Debian package

```sh
python3 tools/installer.py --binary target/release/archaic \
  --version 0.1.0 --licenses /path/to/reviewed-licenses \
  --maintainer 'Release Maintainer <maintainer@example.org>' \
  --depends 'VERIFIED DEBIAN DEPENDENCY EXPRESSION' --output dist
```

Replace the dependency expression with the actual dependencies derived on the supported Debian-family distribution, for example with its `dpkg-shlibdeps` tooling. The script deliberately does not guess a portable glibc/GTK dependency set. GTK is not bundled. A compatible Secret Service is required for persistent sign-in, and desktop notifications need a functioning notification service. The app still supports temporary sessions without the credential vault.

The package supports amd64/arm64, installs `/usr/bin/archaic`, the Matrix-capable desktop entry, and notices/metadata under `/usr/share/doc/archaic`. Archive ownership is root and the executable is mode 0755. Installing/removing this package does not remove saved account data or force a default Matrix URL handler. This is a Debian installer, not a universal Linux package; RPM, Flatpak and AppImage are not supplied.

The file-format test verifies the ar members and their tar payloads directly. A target-distribution install/uninstall acceptance test remains necessary. Debian metadata follows [control fields](https://www.debian.org/doc/debian-policy/ch-controlfields.html) and [dependency relationships](https://www.debian.org/doc/debian-policy/ch-relationships.html).

## macOS: signed and notarized PKG

```sh
python3 tools/installer.py --binary target/release/archaic \
  --version 0.1.0 --licenses /path/to/reviewed-licenses \
  --app-identity 'Developer ID Application: YOUR IDENTITY' \
  --installer-identity 'Developer ID Installer: YOUR IDENTITY' \
  --notary-profile 'ARCHAIC_RELEASE' --output dist
```

Set up the Developer ID identities and a `notarytool` Keychain profile on the release machine first. The script signs the app with hardened runtime and a timestamp, verifies it, builds a signed PKG for `/Applications`, submits it to notarization, requires an `Accepted` result, then staples and validates the ticket. The app bundle includes licenses, build metadata and Matrix scheme registration. No key/password is passed through this script.

`--unsigned-development` allows a local unsigned PKG; it does not make it a distributable notarized release. Actual Developer ID signing, Gatekeeper acceptance, URL opening and install behavior require a macOS release machine and remain unverified here. See Apple's [distribution packaging](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution) and [notarization workflow](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).

## Windows: per-user NSIS installer

Install NSIS and Windows SDK SignTool on the Windows release machine:

```powershell
py -3 tools/installer.py --binary target/release/archaic.exe `
  --version 0.1.0 --licenses C:\release\reviewed-licenses `
  --windows-certificate YOUR_40_HEX_CERTIFICATE_THUMBPRINT `
  --timestamp-url https://YOUR_TIMESTAMP_SERVICE --output dist
```

The script signs/verifies the app with SHA-256, compiles `packaging/windows/archaic.nsi`, then signs/verifies the installer. Credentials stay in the Windows certificate store. The installer uses current-user permissions and defaults to `%LOCALAPPDATA%\Programs\Archaic`. It adds a Start menu shortcut, uninstall entry and Matrix URL-handler capability. The user chooses the default URL handler in Windows; an existing default is not replaced automatically. Uninstall retains encrypted app data and vault entries.

`--unsigned-development` allows local builds without signing. The generated uninstaller is not separately Authenticode-signed. NSIS compilation, installed shortcuts/default-app registration, native notifications and signing require Windows acceptance; the Linux test verifies command construction and installer source contracts only. See [NSIS execution levels](https://nsis.sourceforge.io/Reference/RequestExecutionLevel) and [NSIS installer commands](https://nsis.sourceforge.io/Docs/Chapter4.html).

## Validation and release limits

```sh
python3 tools/test_package.py
python3 tools/test_installer.py
```

Tests use synthetic payloads: archive contents/hashes/no-overwrite, Debian control/data/ownership/mode, signing order, failed notarization rejection and Windows per-user registration. They do not execute foreign binaries, install packages, access certificates or certify a production release. Automatic updates, release hosting, application icon assets, signing credentials, distribution licensing and platform acceptance remain release work. Windows/macOS runtime verification is deferred as requested.

Windows archives require `cui.dll` and `Microsoft.WindowsAppRuntime.Bootstrap.dll`
beside the supplied executable. `tools/dev.py` stages these from CUI's WinUI
build; packaging fails if either is missing and hashes both in `build.json`.
The installer includes and removes both files. The Windows App SDK 1.8 runtime
and Microsoft VC++ runtime are prerequisites, not bundled installers.
