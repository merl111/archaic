# macOS native checks

The macOS archive contains `Archaic.app`. Extract it and open the application bundle. The archive name and `build.json` identify its architecture; an ARM64 build targets Apple Silicon and is not an Intel or universal build. Development archives are unsigned and not notarized.

The AppKit backend respects the requested canvas size, uses application style padding in layout, and applies the selected palette to borderless styled buttons and the message composer. Archaic disables focus outlines at startup and after theme changes, matching its other desktop builds.

## Reproduce startup and visual checks

Run on a Mac after building and packaging the native executable:

```sh
swiftc tools/capture_macos.swift -o /tmp/archaic-capture-window
python3 tools/capture_macos.py /path/to/Archaic.app/Contents/MacOS/archaic evidence /tmp/archaic-capture-window
open -W -n /path/to/Archaic.app --args --temporary-session --fixture --smoke-test
```

The helper launches a temporary login window and synthetic light/dark conversations, captures the actual application window, and checks clean startup/shutdown. It never reads a saved account or connects to a homeserver. Window capture requires a desktop session and screen-capture permission. Inspect the resulting PNGs as well as the exit codes; compilation and toolkit tests alone do not establish visual correctness.
