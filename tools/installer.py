#!/usr/bin/env python3
"""Build a native installer. Never installs it or changes the host's URL handlers."""
import argparse
import hashlib
import io
import json
import platform
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import zipfile
import package as archives

ROOT = Path(__file__).resolve().parents[1]


def run(args):
    subprocess.run([str(a) for a in args], check=True)


def ar_member(name, data):
    """System V ar member, as required for Debian binary package containers."""
    if len(name) > 15:
        raise ValueError("ar member name too long")
    header = f"{name + '/':<16}{0:<12}{0:<6}{0:<6}{'100644':<8}{len(data):<10}`\n".encode("ascii")
    return header + data + (b"\n" if len(data) % 2 else b"")


def tar_bytes(root):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as archive:
        for path in sorted(root.rglob("*")):
            def owner(info):
                info.uid = info.gid = 0
                info.uname = info.gname = "root"
                # Package permissions are target metadata, independent of the
                # host filesystem (Windows does not preserve POSIX chmod bits).
                info.mode = 0o755 if info.isdir() or info.name == "./usr/bin/archaic" else 0o644
                return info
            archive.add(path, arcname="./" + path.relative_to(root).as_posix(), recursive=False, filter=owner)
    return output.getvalue()


def deb(stage, output, version, architecture, depends, maintainer):
    """Stage package files without invoking privileged installation tools."""
    if architecture not in {"amd64", "arm64"} or not re.fullmatch(r"[0-9][0-9A-Za-z.+~:-]*", version):
        raise ValueError("invalid Debian version/architecture")
    if any("\n" in text or "\r" in text for text in (depends, maintainer)) or not depends or not maintainer:
        raise ValueError("explicit, single-line dependency and maintainer metadata required")
    with tempfile.TemporaryDirectory(prefix="archaic-deb-") as temporary:
        temporary = Path(temporary)
        control, data = temporary / "control", temporary / "data"
        control.mkdir()
        (data / "usr/bin").mkdir(parents=True)
        (data / "usr/share/applications").mkdir(parents=True)
        doc = data / "usr/share/doc/archaic"
        doc.mkdir(parents=True)
        shutil.copy2(stage / "bin/archaic", data / "usr/bin/archaic")
        shutil.copy2(stage / "share/applications/org.archaic.desktop.desktop", data / "usr/share/applications/org.archaic.desktop.desktop")
        shutil.copytree(stage / "share/icons", data / "usr/share/icons")
        for name in ("README.txt", "build.json"):
            shutil.copy2(stage / name, doc / name)
        shutil.copytree(stage / "licenses", doc / "licenses")
        size = sum(p.stat().st_size for p in data.rglob("*") if p.is_file())
        (control / "control").write_text(f"Package: archaic\nVersion: {version}\nArchitecture: {architecture}\nMaintainer: {maintainer}\nDepends: {depends}\nInstalled-Size: {(size + 1023) // 1024}\nSection: net\nPriority: optional\nDescription: Native Matrix desktop client\n Archaic uses the native CUI library. GTK is a system dependency.\n")
        payload = b"!<arch>\n" + ar_member("debian-binary", b"2.0\n") + ar_member("control.tar.gz", tar_bytes(control)) + ar_member("data.tar.gz", tar_bytes(data))
        with output.open("xb") as handle:
            handle.write(payload)


def native(stage, output, options, system):
    if system == "Linux":
        architecture = {"x86_64": "amd64", "aarch64": "arm64"}.get(platform.machine())
        deb(stage, output, options.version, architecture, options.depends, options.maintainer)
    elif system == "Darwin":
        app = stage / "Archaic.app"
        (app / "Contents/MacOS/archaic").chmod(0o755)
        if options.app_identity:
            run(["codesign", "--force", "--options", "runtime", "--timestamp", "--sign", options.app_identity, app])
            run(["codesign", "--verify", "--deep", "--strict", app])
        args = ["pkgbuild", "--component", app, "--install-location", "/Applications", "--identifier", "org.archaic.desktop", "--version", options.version]
        if options.installer_identity:
            args += ["--sign", options.installer_identity]
        run(args + [output])
        if options.notary_profile:
            # Credential material stays in the developer's Keychain profile.
            result = subprocess.check_output(["xcrun", "notarytool", "submit", str(output), "--keychain-profile", options.notary_profile, "--wait", "--output-format", "json"])
            if json.loads(result).get("status") != "Accepted":
                raise RuntimeError("notarization was not accepted; installer must not be distributed")
            run(["xcrun", "stapler", "staple", output])
            run(["xcrun", "stapler", "validate", output])
    elif system == "Windows":
        if options.windows_certificate:
            sign_windows(stage / "archaic.exe", options)
        run(["makensis", "/V2", f"/DOUTPUT={output}", f"/DSTAGE={stage}", f"/DVERSION={options.version}", ROOT / "packaging/windows/archaic.nsi"])
        if options.windows_certificate:
            sign_windows(output, options)
    else:
        raise ValueError("unsupported native target")


def sign_windows(path, options):
    if not re.fullmatch(r"[0-9a-fA-F]{40}", options.windows_certificate) or not options.timestamp_url.startswith("https://"):
        raise ValueError("SHA-1 certificate thumbprint and HTTPS timestamp service required")
    run(["signtool", "sign", "/sha1", options.windows_certificate, "/fd", "SHA256", "/tr", options.timestamp_url, "/td", "SHA256", path])
    run(["signtool", "verify", "/pa", path])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--licenses", type=Path, required=True, help="Reviewed dependency/application license bundle")
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    parser.add_argument("--depends", default="", help="Verified target-distribution Debian dependencies")
    parser.add_argument("--maintainer", default="")
    parser.add_argument("--unsigned-development", action="store_true")
    parser.add_argument("--app-identity", default="")
    parser.add_argument("--installer-identity", default="")
    parser.add_argument("--notary-profile", default="")
    parser.add_argument("--windows-certificate", default="")
    parser.add_argument("--timestamp-url", default="")
    options = parser.parse_args()
    system = platform.system()
    if not re.fullmatch(r"[0-9]+(?:\.[0-9]+){1,3}", options.version):
        parser.error("use a numeric release version, for example 0.1.0")
    if not options.licenses.is_dir() or not any(options.licenses.iterdir()):
        parser.error("a nonempty reviewed license bundle is required")
    if not options.unsigned_development:
        if system == "Darwin" and not all([options.app_identity, options.installer_identity, options.notary_profile]):
            parser.error("release PKG requires application/installer identities and a notary Keychain profile")
        if system == "Windows" and not all([options.windows_certificate, options.timestamp_url]):
            parser.error("release EXE requires certificate thumbprint and timestamp URL")
    if system == "Linux" and not all([options.depends, options.maintainer]):
        parser.error("build on the supported Debian distribution and supply verified --depends and --maintainer")
    options.output.mkdir(parents=True, exist_ok=True)
    suffix = {"Linux": ".deb", "Darwin": ".pkg", "Windows": "-setup.exe"}[system]
    output = options.output.resolve() / f"archaic-{options.version}-{platform.machine()}{suffix}"
    if output.exists():
        parser.error(f"refusing to overwrite {output}")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if subprocess.check_output(["git", "diff", "--name-only", "HEAD"], cwd=ROOT):
        revision += "-dirty"
    with tempfile.TemporaryDirectory(prefix="archaic-installer-") as temporary:
        temporary = Path(temporary)
        archive = archives.package(options.binary.resolve(), temporary, system, platform.machine(), options.version, revision)
        if system == "Linux":
            with tarfile.open(archive) as handle:
                handle.extractall(temporary / "stage", filter="data")
        else:
            with zipfile.ZipFile(archive) as handle:
                handle.extractall(temporary / "stage")
        stage = next((temporary / "stage").iterdir())
        # This records the input, before platform signing changes executable bytes.
        metadata = json.loads((stage / "build.json").read_text())
        metadata["input_binary_sha256"] = metadata.pop("binary_sha256")
        metadata.pop("signed", None)
        metadata["artifact_kind"] = "native-installer-input"
        (stage / "build.json").write_text(json.dumps(metadata, indent=2) + "\n")
        shutil.copytree(options.licenses, stage / "licenses")
        if system == "Darwin":
            resources = stage / "Archaic.app/Contents/Resources"
            shutil.copytree(stage / "licenses", resources / "licenses")
            for name in ("README.txt", "build.json"):
                shutil.copy2(stage / name, resources / name)
        pending = temporary / output.name
        native(stage, pending, options, system)
        # Publish only after packaging, signing and notarization succeed.
        with output.open("xb") as handle:
            with pending.open("rb") as source:
                shutil.copyfileobj(source, handle)
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_name(output.name + ".sha256").write_text(f"{digest}  {output.name}\n")
    print(output)


if __name__ == "__main__":
    main()
