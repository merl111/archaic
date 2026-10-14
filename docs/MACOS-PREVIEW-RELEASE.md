A native macOS Apple Silicon build with verified application startup and corrected chat rendering.

Download **archaic-0.1.0-macos.1-arm64.zip**, extract it, and open **Archaic.app**. This is an ARM64 development build for Apple Silicon, tested on macOS 14.8.9. It is unsigned and not notarized; macOS may require approving it in Privacy & Security. It is not an Intel or universal build.

### Changes

- Restored canvas text, icons, avatars' initials and emoji. The previous bitmap format did not provide a valid AppKit drawing context.
- Corrected text orientation, translucent pixel handling and single-line reply previews.
- Compact navigation, consistent header heights, style padding and button colors in light and dark themes.
- Hidden panels no longer permanently enlarge the window during construction; native fields retain their text insets and unnecessary composer scrollbars disappear.
- Focus outlines stay disabled across theme changes.

### Validation

The packaged `.app` passed direct startup/shutdown and LaunchServices startup checks on a native macOS runner. Actual login and light/dark chat screenshots were reviewed. The native regression tests also check rendered text/icon pixels and initial window geometry. The checks use temporary sessions and synthetic conversations, not a real Matrix account or credential vault.

[Application build and native captures](https://github.com/merl111/archaic/actions/runs/38602631254).

`build.json` records the exact source and CUI revisions, target architecture and executable hash. A SHA-256 sidecar accompanies the archive. Source changes: [CUI #1](https://github.com/merl111/cui/pull/1), [Archaic #1](https://github.com/merl111/archaic/pull/1).
