#!/usr/bin/env python3
"""Run opt-in Matrix tests against an isolated, disposable loopback Synapse."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]
IMAGE = "ghcr.io/element-hq/synapse@sha256:38879c6039381b9b66a2adb11b92c63dd5f7ee6a443b98d1edf10ffe004e17e4"

def docker(*args, **kwargs):
    return subprocess.run(["docker", *args], check=True, text=True, **kwargs)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    name = "archaic-test-" + uuid.uuid4().hex[:12]
    docker("volume", "create", name, stdout=subprocess.DEVNULL)
    try:
        docker("run", "--rm", "-v", name + ":/data", "-e", "SYNAPSE_SERVER_NAME=archaic.test", "-e", "SYNAPSE_REPORT_STATS=no", IMAGE, "generate")
        config = """import yaml
p='/data/homeserver.yaml'
with open(p) as f: c=yaml.safe_load(f)
c.update(enable_registration=True, enable_registration_without_verification=True, federation_domain_whitelist=[])
c['listeners']=[dict(port=8008, type='http', tls=False, bind_addresses=['0.0.0.0'], resources=[dict(names=['client'], compress=False)])]
for key in ['rc_registration','rc_message','rc_joins','rc_invites']:
    if key not in ['rc_joins','rc_invites']: c[key]=dict(per_second=100, burst_count=1000)
c['rc_login']={k:dict(per_second=100,burst_count=1000) for k in ['address','account','failed_attempts']}
with open(p,'w') as f: yaml.safe_dump(c,f)
"""
        docker("run", "--rm", "-i", "--entrypoint", "python", "-v", name + ":/data", IMAGE, "-", input=config)
        docker("run", "-d", "--name", name, "-v", name + ":/data", "-p", "127.0.0.1::8008", IMAGE, stdout=subprocess.DEVNULL)
        address = docker("port", name, "8008/tcp", capture_output=True).stdout.strip().splitlines()[0]
        server = "http://" + address
        for _ in range(90):
            try:
                with urllib.request.urlopen(server + "/_matrix/client/versions", timeout=1) as response:
                    json.load(response)
                break
            except (OSError, ValueError):
                time.sleep(1)
        else:
            raise RuntimeError("Disposable Synapse did not start")
        command = ["cargo", "test", "--locked", "-p", "archaic-matrix", "--test", "live_security", "--", "--ignored"]
        subprocess.run(command, cwd=ROOT, env=dict(os.environ, ARCHAIC_TEST_SERVER=server), check=True)
    finally:
        subprocess.run(["docker", "rm", "-f", name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(["docker", "volume", "rm", name], stdout=subprocess.DEVNULL)

if __name__ == "__main__":
    main()
