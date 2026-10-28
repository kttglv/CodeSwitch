#!/usr/bin/env python3
"""Import a release certificate into an ephemeral CI-only keychain; never log secrets."""
import base64
import os
from pathlib import Path
import secrets
import shlex
import subprocess

required = ["APPLE_CERTIFICATE", "APPLE_CERTIFICATE_PASSWORD", "APPLE_SIGNING_IDENTITY", "APPLE_ID", "APPLE_PASSWORD", "APPLE_TEAM_ID"]
for name in required:
    if not os.environ.get(name):
        raise SystemExit(f"Missing release secret: {name}")
temporary = Path(os.environ["RUNNER_TEMP"])
certificate = temporary / "codeswitch-certificate.p12"
keychain = temporary / "codeswitch-signing.keychain-db"
password = secrets.token_urlsafe(32)
certificate.write_bytes(base64.b64decode(os.environ["APPLE_CERTIFICATE"], validate=True))
certificate.chmod(0o600)
def security(*args):
    subprocess.run(["security", *args], check=True, stdout=subprocess.DEVNULL)
try:
    existing = shlex.split(subprocess.check_output(["security", "list-keychains", "-d", "user"], text=True))
    security("create-keychain", "-p", password, str(keychain))
    security("set-keychain-settings", "-lut", "3600", str(keychain))
    security("unlock-keychain", "-p", password, str(keychain))
    security("import", str(certificate), "-k", str(keychain), "-P", os.environ["APPLE_CERTIFICATE_PASSWORD"], "-T", "/usr/bin/codesign")
    security("set-key-partition-list", "-S", "apple-tool:,apple:,codesign:", "-s", "-k", password, str(keychain))
    security("list-keychains", "-d", "user", "-s", str(keychain), *existing)
finally:
    certificate.unlink(missing_ok=True)
