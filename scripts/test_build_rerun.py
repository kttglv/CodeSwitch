"""build.rs bakes the release-gate result into the binary. Cargo only re-runs it when
a watched path changes, so every file the release evidence fingerprints must be
watched; otherwise a cached build could keep a stale approval."""
from pathlib import Path
import re
import unittest
from source_digest import ROOT, digest_paths

BUILD_RS = ROOT / "src-tauri" / "build.rs"

def watched_paths():
    text = BUILD_RS.read_text(encoding="utf-8")
    block = re.search(r"for path in \[(.*?)\]\s*\{\s*println!\(\"cargo:rerun-if-changed=\{path\}\"\);", text, re.S)
    literals = re.findall(r'cargo:rerun-if-changed=([^"{]+)"', text)
    raw = (re.findall(r'"([^"]+)"', block.group(1)) if block else []) + literals
    return [(BUILD_RS.parent / path).resolve() for path in raw]

class BuildRerunCoverageTests(unittest.TestCase):
    def test_every_fingerprinted_file_is_watched(self):
        watched = watched_paths()
        self.assertTrue(watched)
        unwatched = [path.relative_to(ROOT).as_posix() for path in digest_paths()
                     if not any(path.resolve() == w or w in path.resolve().parents for w in watched)]
        self.assertEqual(unwatched, [])

    def test_gate_inputs_outside_the_digest_are_watched(self):
        watched = watched_paths()
        for required in ["release", "benchmarks"]:
            self.assertIn((ROOT / required).resolve(), watched)

    def test_build_output_directories_are_not_watched(self):
        for path in watched_paths():
            self.assertNotIn("target", path.relative_to(ROOT.resolve()).parts)
            self.assertNotEqual(path, (ROOT / "src-tauri").resolve())

if __name__ == "__main__": unittest.main()
