# Archaic identity

The approved identity uses a turquoise abstract mark and a lowercase wordmark.
`logo.png` is the approved full logo. `icon-master.png` is the symbol-only raster
master. Both were made with the built-in image-generation tool; the icon was
derived from the approved logo, preserving its three-part silhouette.

The app embeds the mark in its header, sign-in screen and tray; native text keeps
the app name readable in both themes. The GitHub README uses the full logo on
light backgrounds and the standalone mark on dark backgrounds.

## Rebuild native assets

Install Pillow in a development environment, then run:

```sh
python3 tools/generate_branding.py
python3 tools/test_branding.py
```

All generated assets are committed. Normal application and release builds do
not need Pillow or the image-generation service. PNGs cover 16–1024 pixels; ICO
contains 16–256 pixel variants; ICNS includes Retina sizes through 1024 pixels.
The RGBA copies embed the mark without requiring file paths or image decoding at
runtime. This is a raster identity; no vector master is claimed.

Windows embeds `archaic.ico` as resource 1 in the executable, which CUI also uses
for its native window class. macOS packages `archaic.icns` in the app bundle and
sets the running Dock icon for command-line launches. Linux packages the named
hicolor icons and matching desktop entry; runtime GTK registration also works
when running directly from a build directory.

Linux file managers do not have a portable ELF embedded-icon convention: install
the desktop entry and hicolor icons to get the application launcher icon. On
macOS use the `.app` bundle for the Finder icon; the bare Unix executable is not
an application bundle.

## Icon extraction prompt

Prepare the approved logo in the reference as an app-icon master. Preserve the EXACT turquoise abstract three-part triangular symbol, its shape, proportions, curves and color; do not redesign it. Remove only the lowercase wordmark and any tiny stray artifacts. Center the symbol alone on a square TRUE transparent canvas, sized to occupy 80% of width, leaving even clear margins. Crisp clean edges. No background tile, no text, no shadow, no extra element. This is an asset extraction and cleanup, not a new logo concept.
