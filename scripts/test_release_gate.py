import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import check_release

class ReleaseGateTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.addCleanup(self.directory.cleanup)
        self.addCleanup(patch.stopall)
        patch.object(check_release, "ROOT", self.root).start()
        patch.object(check_release, "source_digest", return_value="current-source").start()
        self.readiness = {"approved": True, "policy": "conservative-v2", "benchmark_report": "benchmarks/report.json", "manual_validation": {"macos_apple_silicon": True, "macos_intel": True, "windows_11_x64": True}}
        self.metadata = {"held_out": True, "independently_annotated": True, "provenance": "Test fixture only; never release evidence"}
        self.write("benchmarks/held-out.jsonl", "test-corpus")
        self.write("benchmarks/held-out.meta.json", self.metadata)
        self.report = {"source_sha256": "current-source", "corpus": "benchmarks/held-out.jsonl", "corpus_sha256": self.hash("benchmarks/held-out.jsonl"), "metadata_sha256": self.hash("benchmarks/held-out.meta.json"), "policy": "conservative-v2", "true_positive": 300, "negative_cases": 6000, "precision": 0.999, "by_layout": {layout: {"true_positive": 150, "negative_cases": 3000, "precision": 0.999} for layout in ["english", "russian"]}}
        self.write("resources/wordfreq/sample", "resource")
        self.write("resources/wordfreq/manifest.json", {"files": {"sample": {"sha256": self.hash("resources/wordfreq/sample")}}})

    def hash(self, relative):
        return hashlib.sha256((self.root / relative).read_bytes()).hexdigest()

    def write(self, relative, value):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value if isinstance(value, str) else json.dumps(value), encoding="utf-8")

    def run_gate(self):
        self.write("release/readiness.json", self.readiness)
        self.write("benchmarks/report.json", self.report)
        check_release.check()

    def test_complete_current_evidence_passes(self):
        self.run_gate()

    def test_missing_approval_and_platform_checks_fail(self):
        self.readiness["approved"] = False
        with self.assertRaisesRegex(ValueError, "not been approved"): self.run_gate()
        self.readiness["approved"] = True
        self.readiness["manual_validation"]["windows_11_x64"] = False
        with self.assertRaisesRegex(ValueError, "Platform"): self.run_gate()

    def test_precision_and_coverage_fail_in_either_direction(self):
        self.report["by_layout"]["russian"]["precision"] = 0.994
        with self.assertRaisesRegex(ValueError, "russian"): self.run_gate()
        self.report["by_layout"]["russian"]["precision"] = 1.0
        self.report["true_positive"] = 50
        with self.assertRaisesRegex(ValueError, "coverage"): self.run_gate()

    def test_stale_source_or_corpus_fails(self):
        self.report["source_sha256"] = "old-source"
        with self.assertRaisesRegex(ValueError, "stale"): self.run_gate()
        self.report["source_sha256"] = "current-source"
        self.write("benchmarks/held-out.jsonl", "changed-corpus")
        with self.assertRaisesRegex(ValueError, "corpus changed"): self.run_gate()

    def test_synthetic_annotations_do_not_pass(self):
        self.metadata["independently_annotated"] = False
        self.write("benchmarks/held-out.meta.json", self.metadata)
        self.report["metadata_sha256"] = self.hash("benchmarks/held-out.meta.json")
        with self.assertRaisesRegex(ValueError, "independently annotated"): self.run_gate()

    def test_modified_resources_fail(self):
        self.write("resources/wordfreq/sample", "changed")
        with self.assertRaisesRegex(ValueError, "checksum"): self.run_gate()

if __name__ == "__main__": unittest.main()
