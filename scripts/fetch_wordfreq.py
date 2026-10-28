#!/usr/bin/env python3
"""Fetch original, version-pinned resources; never flatten the data into a word list."""
import hashlib
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1] / "resources" / "wordfreq"
root.mkdir(parents=True, exist_ok=True)
revision = "42233e6c36ce792031bcccfa17cdd0cec9af5fa7"  # upstream tag v3.2
base = f"https://raw.githubusercontent.com/rspeer/wordfreq/{revision}/"
paths = {
    "large_en.msgpack.gz": "wordfreq/data/large_en.msgpack.gz",
    "large_ru.msgpack.gz": "wordfreq/data/large_ru.msgpack.gz",
    "LICENSE.txt": "LICENSE.txt",
    "NOTICE.md": "NOTICE.md",
    "UPSTREAM-README.md": "README.md",
    "UPSTREAM-lookup.py": "wordfreq/__init__.py",
    "APACHE-2.0.txt": "https://www.apache.org/licenses/LICENSE-2.0.txt",
    "CC-BY-SA-4.0.txt": "https://creativecommons.org/licenses/by-sa/4.0/legalcode.txt",
}
manifest = {"upstream": "https://github.com/rspeer/wordfreq", "version": "3.2", "revision": revision, "files": {}}
for name, source in paths.items():
    url = source if source.startswith("https://") else base + source
    data = subprocess.check_output(["curl", "--fail", "--silent", "--show-error", "--location", url])
    (root / name).write_bytes(data)
    manifest["files"][name] = {"url": url, "sha256": hashlib.sha256(data).hexdigest()}
    print(f"{name}: {len(data):,} bytes")
(root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
