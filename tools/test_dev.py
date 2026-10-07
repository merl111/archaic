#!/usr/bin/env python3
"""Build-helper platform contracts; no SDK, compiler or foreign binary is executed."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("archaic_dev", Path(__file__).with_name("dev.py"))
dev = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dev)


class DevelopmentBuild(unittest.TestCase):
    def exercise(self, platform, release=False, missing=False):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "cui-revision").write_text("revision")
            source = root / "source-cui"
            native = root / "target/cui"
            config = "Release" if release else "Debug"
            calls = []

            def run(args, **kwargs):
                calls.append(([str(a) for a in args], kwargs))
                if args[0:2] == ["cmake", "--build"]:
                    output = native / config
                    output.mkdir(parents=True)
                    (output / ("cui.lib" if platform == "win32" else "libcui.a")).write_bytes(b"library")
                    if platform == "win32":
                        (output / "cui.dll").write_bytes(b"cui runtime")
                        if not missing:
                            (output / "Microsoft.WindowsAppRuntime.Bootstrap.dll").write_bytes(b"bootstrap")

            argv = ["dev.py", "build", "--offline"] + (["--release"] if release else [])
            with mock.patch.object(dev, "ROOT", root), mock.patch.object(dev.sys, "platform", platform), \
                 mock.patch.object(dev.sys, "argv", argv), mock.patch.object(dev, "run", side_effect=run), \
                 mock.patch.object(dev.subprocess, "check_output", side_effect=["revision", ""]), \
                 mock.patch.dict(os.environ, {"CUI_SOURCE_DIR": str(source), "CARGO_TARGET_DIR": str(root / "cargo"), "CARGO_BUILD_TARGET": ""}):
                if missing:
                    with self.assertRaises(SystemExit): dev.main()
                    self.assertFalse(any(c[0][0] == "cargo" for c in calls))
                    return
                dev.main()
            windows = platform == "win32"
            self.assertIn(f"-DCUI_BUILD_SHARED={'ON' if windows else 'OFF'}", calls[0][0])
            self.assertIn("cui_winui_build" if windows else "cui", calls[1][0])
            cargo, kwargs = calls[-1]
            self.assertEqual(cargo[0], "cargo")
            self.assertIn("--locked", cargo)
            self.assertEqual(kwargs["env"]["CUI_LIB_DIR"], str(native / config))
            if windows:
                output = root / "cargo" / ("release" if release else "debug")
                for folder in [output, output / "deps"]:
                    self.assertEqual((folder / "cui.dll").read_bytes(), b"cui runtime")
                    self.assertEqual((folder / "Microsoft.WindowsAppRuntime.Bootstrap.dll").read_bytes(), b"bootstrap")

    def test_windows_debug_and_release_stage_runtime(self):
        for release in [False, True]:
            with self.subTest(release=release): self.exercise("win32", release)

    def test_linux_and_macos_remain_static(self):
        for platform in ["linux", "darwin"]:
            with self.subTest(platform=platform): self.exercise(platform)

    def test_missing_windows_runtime_stops_before_cargo(self):
        self.exercise("win32", missing=True)


if __name__ == "__main__":
    unittest.main()
