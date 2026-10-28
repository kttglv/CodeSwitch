#!/usr/bin/env python3
"""Export dependency licenses from locked metadata without executing package scripts."""
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=root))
rust = [{key: package.get(key) for key in ["name", "version", "license", "license_file", "repository"]} for package in metadata["packages"]]
lock = json.loads((root / "package-lock.json").read_text())
javascript = [{"name": path.removeprefix("node_modules/"), "version": package.get("version"), "license": package.get("license"), "resolved": package.get("resolved")} for path, package in lock["packages"].items() if path]
destination = root / "artifacts" / "dependency-notices.json"
destination.parent.mkdir(exist_ok=True)
destination.write_text(json.dumps({"rust": rust, "javascript": javascript}, indent=2) + "\n")
print(f"Exported license metadata for {len(rust)} Rust and {len(javascript)} npm packages.")
