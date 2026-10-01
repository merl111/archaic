#!/usr/bin/env python3
"""Test Linux Secret Service with a private D-Bus session and temporary keyring.

Requires dbus-run-session and gnome-keyring-daemon. Never uses the desktop vault.
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]

def main():
    if sys.argv[1:] == ["--inside"]:
        sandbox = Path(os.environ["ARCHAIC_TEST_VAULT_ROOT"])
        control = sandbox / "control"
        control.mkdir(mode=0o700)
        subprocess.run(
            ["gnome-keyring-daemon", "--daemonize", "--unlock", "--components=secrets",
             "--control-directory", str(control)],
            input=b"isolated-test-keyring-password", check=True,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15,
        )
        subprocess.run(
            ["cargo", "test", "--locked", "-p", "archaic-matrix",
             "storage::tests::system_vault_roundtrip", "--", "--ignored", "--exact"],
            cwd=ROOT, check=True,
        )
        return
    with tempfile.TemporaryDirectory(prefix="archaic-vault-test-") as directory:
        sandbox = Path(directory)
        data, runtime = sandbox / "data", sandbox / "runtime"
        data.mkdir(mode=0o700)
        runtime.mkdir(mode=0o700)
        env = dict(os.environ, XDG_DATA_HOME=str(data), XDG_RUNTIME_DIR=str(runtime),
                   ARCHAIC_TEST_VAULT_ROOT=str(sandbox))
        # Avoid inheriting any handle for the real desktop credential agent.
        env.pop("GNOME_KEYRING_CONTROL", None)
        env.pop("GNOME_KEYRING_PID", None)
        subprocess.run(["dbus-run-session", "--", sys.executable, str(Path(__file__).resolve()), "--inside"],
                       env=env, check=True, timeout=120)

if __name__ == "__main__":
    main()
