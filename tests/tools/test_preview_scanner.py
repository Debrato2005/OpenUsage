"""Check that preview scanner acceptance refuses bytes outside the pin."""

from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class PreviewScannerTests(unittest.TestCase):
    @unittest.skipUnless(sys.platform == 'linux' and platform.machine() == 'x86_64', 'Linux pin fixture')
    def test_refuses_unpinned_bytes_before_launch(self):
        with tempfile.TemporaryDirectory() as temporary:
            candidate = Path(temporary) / 'scanner'
            marker = Path(temporary) / 'launched'
            candidate.write_text(f'#!/bin/sh\ntouch "{marker}"\n')
            candidate.chmod(0o700)
            result = subprocess.run([
                sys.executable, str(ROOT / 'tests/tools/preview_scanner.py'),
                '--target', 'x86_64-unknown-linux-gnu', '--binary', str(candidate),
            ], capture_output=True, text=True, timeout=15)
            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn('The scanner does not match its pin.', result.stderr)
            self.assertFalse(marker.exists())
