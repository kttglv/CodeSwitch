#!/usr/bin/env python3
"""Fail closed until independently annotated evidence matches the current source."""
import hashlib
import json
from pathlib import Path
import sys
from source_digest import ROOT, source_digest

def require(condition, message):
    if not condition:
        raise ValueError(message)

def check():
    readiness = json.loads((ROOT / "release/readiness.json").read_text())
    require(readiness["approved"] is True, "Public automatic correction has not been approved.")
    require(all(readiness["manual_validation"].get(k) is True for k in ["macos_apple_silicon", "macos_intel", "windows_11_x64"]), "Platform validation is incomplete.")
    report = json.loads((ROOT / readiness["benchmark_report"]).read_text())
    require(report["source_sha256"] == source_digest(), "Benchmark evidence is stale.")
    corpus = ROOT / report["corpus"]
    require(report["corpus_sha256"] == hashlib.sha256(corpus.read_bytes()).hexdigest(), "Benchmark corpus changed.")
    metadata = json.loads(corpus.with_suffix(".meta.json").read_text())
    require(report["metadata_sha256"] == hashlib.sha256(corpus.with_suffix(".meta.json").read_bytes()).hexdigest(), "Corpus metadata changed.")
    require(metadata["held_out"] is True and metadata["independently_annotated"] is True and metadata["provenance"].strip(), "Corpus must be held out and independently annotated.")
    require(report["policy"] == readiness["policy"] == "conservative-v3", "Detector policy changed.")
    require(report["true_positive"] >= 200 and report["negative_cases"] >= 5000, "Insufficient benchmark coverage.")
    require(report["precision"] >= 0.995, "Precision is below 99.5%.")
    for language in ["english", "russian"]:
        row = report["by_layout"][language]
        require(row["true_positive"] >= 100 and row["negative_cases"] >= 2500 and row["precision"] >= 0.995, f"Insufficient {language} precision/coverage.")
    manifest = json.loads((ROOT / "resources/wordfreq/manifest.json").read_text())
    for name, entry in manifest["files"].items():
        require(hashlib.sha256((ROOT / "resources/wordfreq" / name).read_bytes()).hexdigest() == entry["sha256"], f"Resource checksum mismatch: {name}")

if __name__ == "__main__":
    try:
        check()
    except (AssertionError, KeyError, OSError, ValueError) as error:
        if "--quiet" not in sys.argv: print(f"Release gate: {error}", file=sys.stderr)
        sys.exit(1)
    if "--quiet" not in sys.argv: print("Release gate passed.")
