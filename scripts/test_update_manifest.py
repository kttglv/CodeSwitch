import json
from pathlib import Path
import tempfile
import subprocess
import unittest
from unittest.mock import patch
import update_manifest as updater


class UpdateManifestTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.value = "9.9.9"
        fixture = updater.ROOT / "src-tauri/tests/fixtures/updater"
        for target in updater.TARGETS:
            name = updater.asset_name(target, self.value)
            (self.directory / name).write_bytes((fixture / "payload.txt").read_bytes())
            (self.directory / (name + ".sig")).write_bytes((fixture / "payload.sig").read_bytes())

    def test_manifest_contains_all_platforms_and_unique_https_assets(self):
        result = updater.manifest(self.directory, self.value, "kttglv/CodeSwitch")
        self.assertEqual(set(result["platforms"]), {"darwin-aarch64", "darwin-x86_64", "windows-x86_64"})
        urls = {entry["url"] for entry in result["platforms"].values()}
        self.assertEqual(len(urls), 3)
        self.assertTrue(all(url.startswith("https://github.com/kttglv/CodeSwitch/releases/download/v9.9.9/") for url in urls))

    def test_missing_package_or_wrong_version_signature_prevents_publication(self):
        name = updater.asset_name("aarch64-apple-darwin", self.value)
        (self.directory / name).unlink()
        with self.assertRaisesRegex(ValueError, "Missing"): updater.manifest(self.directory, self.value, "kttglv/CodeSwitch")
        with self.assertRaisesRegex(ValueError, "signature"):
            updater.signature(self.directory / (name + ".sig"), "9.9.10")

    def test_malformed_signatures_untrusted_origins_and_unsafe_versions_fail(self):
        with self.assertRaises(ValueError): updater.manifest(self.directory, self.value, "other/repo")
        for value in ["../9.9.9", "v1.2.3", "1.2", "1.2.3/asset", "01.2.3", "1.2.3-01", "1.2.3-a..b"]:
            with self.assertRaises(ValueError): updater.asset_name("aarch64-apple-darwin", value)
        file = self.directory / "bad.sig"
        file.write_text("invalid signature")
        with self.assertRaises(ValueError): updater.signature(file, self.value)

    def test_release_versions_must_match(self):
        root = self.directory / "project"
        (root / "src-tauri").mkdir(parents=True)
        (root / "src-tauri/tauri.conf.json").write_text(json.dumps({"version": "1.2.3"}))
        (root / "package.json").write_text(json.dumps({"version": "1.2.4"}))
        (root / "Cargo.toml").write_text('[workspace.package]\nversion = "1.2.3"\n')
        with patch.object(updater, "ROOT", root):
            with self.assertRaisesRegex(ValueError, "together"): updater.version()

    def test_draft_uses_reviewed_commit_and_never_replaces_an_existing_tag(self):
        commit = "a" * 40
        with patch.object(updater.subprocess, "run") as run:
            run.return_value = subprocess.CompletedProcess([], 0, "", "")
            with self.assertRaisesRegex(ValueError, "already exists"):
                updater.create_draft(self.directory, self.value, "kttglv/CodeSwitch", commit)
            self.assertEqual(run.call_count, 1)
        with patch.object(updater.subprocess, "run") as run:
            run.side_effect = [subprocess.CompletedProcess([], 1, "", "gh: Not Found (HTTP 404)"),
                               subprocess.CompletedProcess([], 0)]
            updater.create_draft(self.directory, self.value, "kttglv/CodeSwitch", commit)
            command = run.call_args.args[0]
            self.assertIn("--draft", command)
            self.assertEqual(command[command.index("--target") + 1], commit)
        with patch.object(updater.subprocess, "run") as run:
            run.return_value = subprocess.CompletedProcess([], 1, "", "authentication failed")
            with self.assertRaisesRegex(ValueError, "Could not verify"):
                updater.create_draft(self.directory, self.value, "kttglv/CodeSwitch", commit)


if __name__ == "__main__": unittest.main()
