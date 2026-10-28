"""Fingerprint the correction implementation and bundled resources for release evidence."""
import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def source_digest():
    paths = [ROOT / "Cargo.lock", ROOT / "Cargo.toml"]
    for base in [ROOT / "crates", ROOT / "src-tauri"]:
        paths.extend(p for p in base.rglob("*") if p.is_file() and p.suffix in {".rs", ".mm", ".cpp", ".h"} and "target" not in p.parts and "gen" not in p.parts)
    paths.extend((ROOT / "resources" / "wordfreq").glob("*"))
    digest = hashlib.sha256()
    paths.extend((ROOT / "scripts").glob("*.py"))
    for path in sorted(set(paths), key=lambda path: path.relative_to(ROOT).as_posix()):
        digest.update(path.relative_to(ROOT).as_posix().encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
    return digest.hexdigest()
