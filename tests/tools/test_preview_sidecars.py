"""Exercise preview staging with small synthetic archives and no network."""

import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
TARGET = 'x86_64-pc-windows-msvc'


class PreviewSidecarsTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.archives = self.root / 'archives'
        self.archives.mkdir()
        self.output = self.root / 'output'
        self.pins = self.root / 'pins.json'
        self.sources = {
            'codex': self.source(
                f'https://github.com/openai/codex/releases/download/rust-test/codex-{TARGET}.exe.tar.gz',
                f'codex-{TARGET}.exe', b'synthetic codex', 'sha256',
            ),
            'ccusage': self.source(
                'https://registry.npmjs.org/@ccusage/ccusage-win32-x64/-/ccusage-win32-x64-test.tgz',
                'package/bin/ccusage.exe', b'synthetic ccusage', 'sha512',
            ),
        }

    def source(self, url, name, payload, algorithm, *, duplicate=False):
        archive = self.archives / url.rsplit('/', 1)[-1]
        with tarfile.open(archive, 'w:gz') as package:
            item = tarfile.TarInfo(name)
            item.size = len(payload)
            package.addfile(item, io.BytesIO(payload))
            if duplicate:
                package.addfile(item, io.BytesIO(payload))
        return {
            'sourceUrl': url,
            'archiveBytes': archive.stat().st_size,
            'archiveDigest': f'{algorithm}-{hashlib.new(algorithm, archive.read_bytes()).hexdigest()}',
            'executableMember': name,
            'executableBytes': len(payload),
            'executableSha256': hashlib.sha256(payload).hexdigest(),
        }

    def stage(self):
        checkout = self.root / 'checkout'
        checkout.mkdir(exist_ok=True)
        scripts = checkout / 'scripts'
        scripts.mkdir(exist_ok=True)
        packaging = checkout / 'packaging'
        packaging.mkdir(exist_ok=True)
        script = scripts / 'provision-preview-sidecars.py'
        original = ROOT / 'scripts' / script.name
        if original.is_file():
            shutil.copyfile(original, script)
        (packaging / 'preview-sidecars.json').write_text(json.dumps({TARGET: self.sources}))
        return subprocess.run([
            sys.executable, str(script), '--target', TARGET, '--out', str(self.output),
            '--archive-dir', str(self.archives),
        ], capture_output=True, text=True, timeout=20)

    def test_complete_set_uses_tauri_windows_names(self):
        result = self.stage()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual({path.name: path.read_bytes() for path in self.output.iterdir()}, {
            f'openusage-codex-{TARGET}.exe': b'synthetic codex',
            f'openusage-ccusage-{TARGET}.exe': b'synthetic ccusage',
        })

    def test_bad_archive_pin_leaves_no_output(self):
        self.sources['ccusage']['archiveDigest'] = f"sha512-{'0' * 128}"
        result = self.stage()
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn('ccusage archive does not match its pin', result.stderr)
        self.assertFalse(self.output.exists())

    def test_wrong_executable_pin_leaves_no_output(self):
        self.sources['ccusage']['executableSha256'] = '0' * 64
        result = self.stage()
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn('ccusage executable does not match its pin', result.stderr)
        self.assertFalse(self.output.exists())

    def test_duplicate_member_leaves_no_output(self):
        self.sources['ccusage'] = self.source(
            self.sources['ccusage']['sourceUrl'], 'package/bin/ccusage.exe',
            b'synthetic ccusage', 'sha512', duplicate=True,
        )
        result = self.stage()
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn('ccusage archive must contain one exact executable member', result.stderr)
        self.assertFalse(self.output.exists())

    def test_existing_directory_is_left_alone(self):
        self.output.mkdir()
        marker = self.output / 'keep'
        marker.write_text('existing data')
        result = self.stage()
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn('The output directory must be new', result.stderr)
        self.assertEqual(list(self.output.iterdir()), [marker])
        self.assertEqual(marker.read_text(), 'existing data')
