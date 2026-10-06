#!/usr/bin/env python3
"""Create an unsigned native release archive. Run separately on each target OS."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def package(binary, destination, system, machine, version, revision):
    if system not in {"Linux", "Darwin", "Windows"}:
        raise ValueError("unsupported operating system")
    if not binary.is_file():
        raise ValueError(f"missing binary: {binary}")
    runtimes = []
    if system == "Windows":
        for name in ("cui.dll", "Microsoft.WindowsAppRuntime.Bootstrap.dll"):
            dll = binary.parent / name
            if not dll.is_file():
                raise ValueError(f"missing Windows runtime beside binary: {dll}")
            runtimes.append(dll)
    destination.mkdir(parents=True, exist_ok=True)
    name = f"archaic-{version}-{system.lower()}-{machine}"
    archive = destination / (name + (".tar.gz" if system == "Linux" else ".zip"))
    if archive.exists():
        raise ValueError(f"refusing to overwrite {archive}")
    with tempfile.TemporaryDirectory(prefix="archaic-package-") as temporary:
        stage = Path(temporary) / name
        stage.mkdir()
        if system == "Darwin":
            content = stage / "Archaic.app" / "Contents"
            executable = content / "MacOS" / "archaic"
            content.mkdir(parents=True)
            with (content / "Info.plist").open("wb") as handle:
                plistlib.dump({"CFBundleExecutable": "archaic", "CFBundleIdentifier": "org.archaic.desktop",
                              "CFBundleName": "Archaic", "CFBundleDisplayName": "Archaic",
                              "CFBundleIconFile": "archaic.icns",
                              "CFBundlePackageType": "APPL", "CFBundleShortVersionString": version,
                              "CFBundleVersion": version, "NSHighResolutionCapable": True,
                              "NSMicrophoneUsageDescription": "Record voice messages when you press Record.",
                              "CFBundleURLTypes": [{"CFBundleURLName": "Matrix", "CFBundleURLSchemes": ["matrix"]}]}, handle)
            resources = content / "Resources"
            resources.mkdir()
            shutil.copy2(ROOT / "assets/branding/archaic.icns", resources / "archaic.icns")
        elif system == "Windows":
            executable = stage / "archaic.exe"
            shutil.copy2(ROOT / "assets/branding/archaic.ico", stage / "archaic.ico")
        else:
            executable = stage / "bin" / "archaic"
            desktop = stage / "share" / "applications"
            desktop.mkdir(parents=True)
            shutil.copyfile(ROOT / "packaging" / "org.archaic.desktop.desktop", desktop / "org.archaic.desktop.desktop")
            for size in (16, 24, 32, 48, 64, 128, 256, 512, 1024):
                icons = stage / f"share/icons/hicolor/{size}x{size}/apps"
                icons.mkdir(parents=True)
                shutil.copy2(ROOT / f"assets/branding/icon-{size}.png", icons / "org.archaic.desktop.png")
        executable.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(binary, executable)
        executable.chmod(0o755)
        for dll in runtimes:
            shutil.copy2(dll, executable.parent / dll.name)
        shutil.copyfile(ROOT / "docs" / "DESKTOP.md", stage / "README.txt")
        metadata = {"version": version, "source_revision": revision,
                    "cui_revision": (ROOT / "cui-revision").read_text().strip(),
                    "build_os": platform.platform(), "target_os": system, "architecture": machine,
                    "signed": False, "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                    "gtk_bundled": False,
                    "runtime_sha256": {dll.name: hashlib.sha256(dll.read_bytes()).hexdigest() for dll in runtimes},
                    "windows_app_runtime": "1.8" if system == "Windows" else None}
        (stage / "build.json").write_text(json.dumps(metadata, indent=2) + "\n")
        if system == "Linux":
            with tarfile.open(archive, "w:gz") as handle:
                handle.add(stage, arcname=name)
        else:
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as handle:
                for file in sorted(stage.rglob("*")):
                    if file.is_file():
                        handle.write(file, file.relative_to(stage.parent))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n")
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, help="Package an existing native release binary instead of building")
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    options = parser.parse_args()
    if options.binary is None:
        subprocess.run([sys.executable, str(ROOT / "tools" / "dev.py"), "build", "--release"], cwd=ROOT, check=True)
    binary = options.binary or ROOT / "target" / "release" / ("archaic.exe" if os.name == "nt" else "archaic")
    # Cargo metadata keeps the version in one source of truth.
    cargo = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"], cwd=ROOT))
    version = next(p["version"] for p in cargo["packages"] if p["name"] == "archaic")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if subprocess.check_output(["git", "diff", "--name-only", "HEAD"], cwd=ROOT):
        revision += "-dirty"
    print(package(binary.resolve(), options.output.resolve(), platform.system(), platform.machine(), version, revision))


if __name__ == "__main__":
    main()
