#!/usr/bin/env python3
"""Verify archive structure without compiling or executing foreign-platform binaries."""
import hashlib
import importlib.util
import json
from pathlib import Path
import plistlib
import tarfile
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location("archaic_package", Path(__file__).with_name("package.py"))
packaging = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packaging)


class Packages(unittest.TestCase):
    def assert_platform_icons(self, files, prefix, system):
        if system == "Windows":
            self.assertEqual(files[prefix + "archaic.ico"][:4], b"\0\0\1\0")
        elif system == "Darwin":
            info = plistlib.loads(files[prefix + "Archaic.app/Contents/Info.plist"])
            self.assertEqual(info["CFBundleIconFile"], "archaic.icns")
            self.assertEqual(files[prefix + "Archaic.app/Contents/Resources/archaic.icns"][:4], b"icns")
        else:
            desktop = files[prefix + "share/applications/org.archaic.desktop.desktop"].decode()
            self.assertIn("Icon=org.archaic.desktop", desktop)
            self.assertIn("StartupWMClass=org.archaic.desktop", desktop)
            for size in (16, 24, 32, 48, 64, 128, 256, 512, 1024):
                icon = files[prefix + f"share/icons/hicolor/{size}x{size}/apps/org.archaic.desktop.png"]
                self.assertTrue(icon.startswith(b"\x89PNG"))

    def test_windows_rejects_an_archive_without_runtime_libraries(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            binary = root / "archaic.exe"
            binary.write_bytes(b"test")
            with self.assertRaisesRegex(ValueError, "missing Windows runtime"):
                packaging.package(binary, root / "dist", "Windows", "x64", "0.1.0", "test")
            self.assertFalse((root / "dist").exists())

    def test_layout_hashes_url_metadata_and_no_overwrite(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            binary = root / "fixture-binary"
            binary.write_bytes(b"test archive payload, not an executable")
            runtimes = {name: name.encode() for name in ("cui.dll", "Microsoft.WindowsAppRuntime.Bootstrap.dll")}
            for name, data in runtimes.items(): (root / name).write_bytes(data)
            for system in ["Linux", "Darwin", "Windows"]:
                with self.subTest(system=system):
                    archive = packaging.package(binary, root / "dist", system, "test-arch", "0.1.0", "test-revision")
                    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
                    self.assertEqual(archive.with_name(archive.name + ".sha256").read_text(), f"{digest}  {archive.name}\n")
                    if system == "Linux":
                        with tarfile.open(archive) as handle:
                            files = {m.name: handle.extractfile(m).read() for m in handle.getmembers() if m.isfile()}
                    else:
                        with zipfile.ZipFile(archive) as handle:
                            files = {n: handle.read(n) for n in handle.namelist()}
                    prefix = f"archaic-0.1.0-{system.lower()}-test-arch/"
                    meta = json.loads(files[prefix + "build.json"])
                    self.assertEqual(meta["target_os"], system)
                    self.assertFalse(meta["signed"])
                    self.assertFalse(meta["gtk_bundled"])
                    self.assertEqual(meta["binary_sha256"], hashlib.sha256(binary.read_bytes()).hexdigest())
                    executable = {"Linux": "bin/archaic", "Darwin": "Archaic.app/Contents/MacOS/archaic", "Windows": "archaic.exe"}[system]
                    self.assertEqual(files[prefix + executable], binary.read_bytes())
                    if system == "Windows":
                        for name, data in runtimes.items():
                            self.assertEqual(files[prefix + name], data)
                            self.assertEqual(meta["runtime_sha256"][name], hashlib.sha256(data).hexdigest())
                        self.assertEqual(meta["windows_app_runtime"], "1.8")
                    if system == "Darwin":
                        info = plistlib.loads(files[prefix + "Archaic.app/Contents/Info.plist"])
                        self.assertEqual(info["CFBundleURLTypes"][0]["CFBundleURLSchemes"], ["matrix"])
                    if system == "Linux":
                        desktop = files[prefix + "share/applications/org.archaic.desktop.desktop"].decode()
                        self.assertIn("x-scheme-handler/matrix", desktop)
                        self.assertIn("Exec=archaic %u", desktop)
                    self.assert_platform_icons(files, prefix, system)
                    with self.assertRaisesRegex(ValueError, "refusing to overwrite"):
                        packaging.package(binary, root / "dist", system, "test-arch", "0.1.0", "test-revision")


if __name__ == "__main__":
    unittest.main()
