"""Reproduce worker-thread layout queries without keyboard access or injection."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(sys.platform == "darwin", "requires macOS HIToolbox")
class MacOSLayoutThreadingTests(unittest.TestCase):
    def test_native_layout_calls_from_main_and_worker_threads(self):
        with tempfile.TemporaryDirectory(prefix="codeswitch-layout-test-") as directory:
            executable = Path(directory) / "layout-threading"
            subprocess.run([
                "xcrun", "clang++", "-std=c++17", "-fobjc-arc", "-fblocks",
                "-mmacosx-version-min=13.0",
                str(ROOT / "crates/platform/native/tests/macos_layout_threading.mm"),
                str(ROOT / "crates/platform/native/undo.cpp"),
                "-framework", "Cocoa", "-framework", "ApplicationServices",
                "-framework", "Carbon", "-o", str(executable),
            ], check=True, capture_output=True, text=True, timeout=120)
            # Timeout also detects accidentally dispatching synchronously to the
            # main queue while already on that queue.
            subprocess.run([str(executable)], check=True, capture_output=True,
                           text=True, timeout=15)


if __name__ == "__main__":
    unittest.main()
