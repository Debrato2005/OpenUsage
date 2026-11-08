"""Pinned report comparisons for 16 tools, with private synthetic logs only."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / 'crates/openusage-locallogs/tests/fixtures/remaining-tools'
PIN = ROOT / 'crates/openusage-locallogs/sources/ccusage-20.0.48-linux-x86_64.json'


def varint(value):
    result = bytearray()
    while value > 127:
        result.append((value & 127) | 128)
        value >>= 7
    result.append(value)
    return bytes(result)


def number(field, value):
    return varint(field << 3) + varint(value)


def blob(field, value):
    if isinstance(value, str):
        value = value.encode()
    return varint((field << 3) | 2) + varint(len(value)) + value


def make_source(root, fixture):
    folder = root / fixture['folder']
    folder.mkdir(parents=True)
    for name, value in fixture['files'].items():
        path = folder / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value) + '\n')
    for name, data in fixture['databases'].items():
        path = folder / name
        path.parent.mkdir(parents=True, exist_ok=True)
        connection = sqlite3.connect(path)
        try:
            connection.executescript(data['sql'])
            if 'message' in data:
                connection.execute('INSERT INTO message VALUES(?,?,?)',
                                   ('message-z', 'session-z', json.dumps(data['message'])))
            if data.get('protobuf'):
                usage = (number(2, 100) + number(3, 25) + number(4, 5) + number(5, 10)
                         + number(9, 5) + number(10, 20) + blob(11, 'response-a'))
                timestamp = number(1, 1780000000) + number(2, 0)
                metadata = blob(1, blob(4, usage) + blob(9, blob(4, timestamp))
                                + blob(19, 'gemini-2.5-pro'))
                connection.execute('INSERT INTO gen_metadata VALUES(?,?)', (1, metadata))
            connection.commit()
        finally:
            connection.close()
    return folder


def file_hashes(root):
    return {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in root.rglob('*') if path.is_file()}


class FormatComparisons:
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CCUSAGE_BIN to run the pinned binary.')
        cls.binary = Path(selected).resolve(strict=True)
        pin = json.loads(PIN.read_text())
        if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != pin['executableSha256']:
            raise ValueError('The selected binary does not match the reviewed pin.')
        cls.inputs = json.loads((FIXTURES / 'inputs.json').read_text())
        cls.expected = json.loads((FIXTURES / 'expected.json').read_text())

    def run_report(self, root, tool, fixture, folder, kind):
        with tempfile.TemporaryDirectory(prefix='empty-home-', dir=root) as empty:
            arguments = ['daily', '--by-agent'] if kind == 'daily' else [tool, 'session']
            result = subprocess.run(
                [str(self.binary), *arguments, '--json', '--offline', '--mode', 'calculate',
                 '--timezone', 'UTC', '--no-color'],
                cwd=empty, env={'HOME': empty, 'TZ': 'UTC', 'NO_COLOR': '1',
                                fixture['variable']: str(folder)},
                stdin=subprocess.DEVNULL, capture_output=True, timeout=20)
            self.assertEqual(result.returncode, 0, (tool, kind, result.stderr[:1000]))
            self.assertLess(len(result.stdout), 128 * 1024)
            self.assertEqual(list(Path(empty).iterdir()), [])
            return json.loads(result.stdout)

    def test_daily_and_session_contracts_repeat_without_source_changes(self):
        self.assertEqual(set(self.inputs), set(self.expected))
        self.assertEqual(len(self.inputs), 16)
        with tempfile.TemporaryDirectory(prefix='openusage-formats-') as temporary:
            root = Path(temporary)
            folders = {tool: make_source(root, fixture) for tool, fixture in self.inputs.items()}
            before = file_hashes(root)
            for tool, fixture in self.inputs.items():
                for kind in ('daily', 'session'):
                    with self.subTest(tool=tool, report=kind):
                        first = self.run_report(root, tool, fixture, folders[tool], kind)
                        self.assertEqual(first, self.expected[tool][kind])
                        self.assertEqual(self.run_report(root, tool, fixture, folders[tool], kind), first)
                        if kind == 'daily':
                            self.assertTrue(all([agent['agent'] for agent in day['agents']] == [tool]
                                                for day in first['daily']))
            self.assertEqual(file_hashes(root), before)

    def test_empty_roots_have_no_rows_and_qwen_has_null_session_totals(self):
        with tempfile.TemporaryDirectory(prefix='openusage-empty-formats-') as temporary:
            root = Path(temporary)
            for tool, fixture in self.inputs.items():
                folder = root / fixture['folder']
                folder.mkdir(parents=True)
                for kind in ('daily', 'session'):
                    with self.subTest(tool=tool, report=kind):
                        report = self.run_report(root, tool, fixture, folder, kind)
                        self.assertEqual(report['daily' if kind == 'daily' else 'sessions'], [])
                        if tool == 'qwen' and kind == 'session':
                            self.assertIsNone(report['totals'])
                        else:
                            self.assertEqual(report['totals']['totalTokens'], 0)
            self.assertEqual(file_hashes(root), {})


class CcusageFormatTests(FormatComparisons, unittest.TestCase):
    def test_pinned_amp_counts_a_copied_thread_twice(self):
        # This records an upstream limit. It does not qualify Amp for AT-65.
        with tempfile.TemporaryDirectory(prefix='openusage-amp-copy-') as temporary:
            root = Path(temporary)
            fixture = self.inputs['amp']
            folder = make_source(root, fixture)
            shutil.copyfile(folder / 'threads/thread-z.json', folder / 'threads/thread-copy.json')
            before = file_hashes(root)
            for kind in ('daily', 'session'):
                with self.subTest(report=kind):
                    report = self.run_report(root, 'amp', fixture, folder, kind)
                    self.assertEqual(report['totals']['totalTokens'], 324)
                    self.assertEqual(self.expected['amp'][kind]['totals']['totalTokens'], 162)
            self.assertEqual(file_hashes(root), before)


if __name__ == '__main__':
    unittest.main()
