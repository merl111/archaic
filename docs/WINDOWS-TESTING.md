# Windows test builds

Archaic uses the x64 Windows App Runtime **1.8** and the Microsoft Visual C++ v14 x64 Redistributable. A newer App Runtime major/minor version does not replace 1.8; install them side by side.

- [Windows App Runtime 1.8 x64 installer](https://aka.ms/windowsappsdk/1.8/1.8.260921001/windowsappruntimeinstall-x64.exe)
- [Visual C++ v14 x64 installer](https://aka.ms/vc14/vc_redist.x64.exe)

Extract the entire release ZIP before starting `archaic.exe`. Keep `cui.dll` and `Microsoft.WindowsAppRuntime.Bootstrap.dll` alongside it. Builds are unsigned.

If startup exits without a window, open Command Prompt in the extracted directory and run:

```bat
archaic.exe
echo Exit code: %ERRORLEVEL%
```

`VCRUNTIME140.dll` missing means the Visual C++ runtime is unavailable. WinUI initialization error `0x80670016` means a required runtime package could not be resolved; check that the x64 1.8 runtime is installed for the current user.

## Windows UI correction

The pinned CUI update skips unchanged layout properties during Archaic's 50 ms UI polling, respects the requested chat canvas sizes, reclaims hidden sidebar space, and suppresses delayed canvas focus callbacks during shutdown. It preserves the public C ABI.

Windows labels now center within their allocated rows. Container padding follows the same style precedence as GTK, and styled text fields retain the selected palette while hovered, focused, or disabled. The composer uses a consistent inner inset. Archaic also styles the organization navigation button with its neighbors and disables focus outlines at startup and after theme changes.

The native Windows validation includes a repeated-layout benchmark, window responsiveness checks, and login/chat screenshots with temporary sessions and synthetic messages. It does not access a real Matrix account.
