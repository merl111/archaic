#!/usr/bin/env python3
"""Build the pinned sibling CUI and Archaic using only Python's standard library."""
import argparse
import json
import os
import shutil
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def run(args, **kwargs):
    subprocess.run([str(arg) for arg in args], cwd=ROOT, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["build", "run", "test", "check"])
    parser.add_argument("--release", action="store_true")
    parser.add_argument("--allow-dirty-cui", action="store_true")
    arguments = sys.argv[1:]
    boundary = arguments.index("--") if "--" in arguments else len(arguments)
    options, extra = parser.parse_known_args(arguments[:boundary])
    extra += arguments[boundary:]
    source = Path(os.environ.get("CUI_SOURCE_DIR", ROOT.parent / "cui")).resolve()
    revision = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    expected = (ROOT / "cui-revision").read_text().strip()
    if revision != expected:
        parser.error(f"CUI revision mismatch: expected {expected}, found {revision}")
    dirty = subprocess.check_output(["git", "-C", str(source), "status", "--porcelain", "--untracked-files=no"], text=True)
    if dirty and not options.allow_dirty_cui:
        parser.error("CUI has tracked changes; review them and pass --allow-dirty-cui for local development")
    native = ROOT / "target" / "cui"
    config = "Release" if options.release else "Debug"
    windows = sys.platform == "win32"
    run(["cmake", "-S", source, "-B", native, f"-DCUI_BUILD_SHARED={'ON' if windows else 'OFF'}", "-DCUI_BUILD_EXAMPLES=OFF", "-DCUI_BUILD_TESTS=OFF", "-DCUI_BUILD_BINDING_TESTS=OFF", f"-DCMAKE_BUILD_TYPE={config}"])
    run(["cmake", "--build", native, "--config", config, "--target", "cui_winui_build" if windows else "cui", "--parallel", "4"])
    name = "cui.lib" if sys.platform == "win32" else "libcui.a"
    lib = next((directory for directory in [native / config, native] if (directory / name).exists()), None)
    if lib is None:
        parser.error(f"CUI build did not produce {name}")
    env = dict(os.environ, CUI_LIB_DIR=str(lib))
    if windows:
        # Windows consumers link an import library and load WinUI through cui.dll.
        # Stage beside both normal binaries and Cargo's test executables.
        target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
        if not target.is_absolute():
            target = ROOT / target
        target_triple = os.environ.get("CARGO_BUILD_TARGET")
        for index, arg in enumerate(extra):
            if arg == "--": break
            if arg == "--target" and index + 1 < len(extra): target_triple = extra[index + 1]
            elif arg.startswith("--target="): target_triple = arg.split("=", 1)[1]
        if target_triple: target /= target_triple
        output = target / ("release" if options.release else "debug")
        for name in ("cui.dll", "Microsoft.WindowsAppRuntime.Bootstrap.dll"):
            dll = lib / name
            if not dll.is_file(): parser.error(f"CUI build did not produce required runtime {dll}")
            for destination in (output, output / "deps"):
                destination.mkdir(parents=True, exist_ok=True)
                shutil.copy2(dll, destination / name)
        env["PATH"] = str(lib) + os.pathsep + env.get("PATH", "")
    cmd = ["cargo", "--config", "patch.crates-io.cui.path=" + json.dumps(str(source / "bindings" / "rust")), options.command, "--locked"]
    if options.command == "run":
        cmd += ["-p", "archaic"]
    else:
        cmd += ["--workspace"]
    if options.release:
        cmd += ["--release"]
    run(cmd + extra, env=env)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.returncode) from None
