#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1] / "resources" / "wordfreq"
manifest = json.loads((root / "manifest.json").read_text())
for name, entry in manifest["files"].items():
    if hashlib.sha256((root / name).read_bytes()).hexdigest() != entry["sha256"]:
        raise SystemExit(f"Resource checksum mismatch: {name}")
print(f"Verified {len(manifest['files'])} pinned wordfreq resources.")
