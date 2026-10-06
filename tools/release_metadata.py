#!/usr/bin/env python3
"""Validate manual release inputs before native jobs or tag creation."""
import os
from pathlib import Path
import re
import tomllib


def metadata(root, tag):
    package = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]
    version = package["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version) or tag != f"v{version}":
        raise ValueError("Release tag must equal v + workspace.package.version in Cargo.toml")
    revision = (root / "cui-revision").read_text().strip()
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("CUI must be pinned to a full commit hash")
    rust = package["rust-version"]
    if not re.fullmatch(r"\d+\.\d+(?:\.\d+)?", rust):
        raise ValueError("Expected a numeric Rust toolchain version")
    # rustup uses full release names, while Cargo's MSRV may omit the patch.
    if rust.count(".") == 1:
        rust += ".0"
    return {"tag": tag, "cui": revision, "rust": rust}


if __name__ == "__main__":
    values = metadata(Path(__file__).resolve().parents[1], os.environ["RELEASE_TAG"])
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        for key, value in values.items():
            output.write(f"{key}={value}\n")
