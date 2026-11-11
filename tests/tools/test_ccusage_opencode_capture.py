"""Check complete OpenCode captures and private-source refusal in the Linux candidate."""
from contextlib import closing
import copy
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, PIN, ROOT, FormatComparisons, file_hashes, make_source
from test_ccusage_opencode import cases, populate
import test_ccusage_pi_candidate as reference_checks
from pi_scale import MIB, pad


class OpenCodeCaptureCandidateTests(FormatComparisons, unittest.TestCase):
    CANDIDATE_ENV = 'OPENUSAGE_CCUSAGE_OPENCODE_CAPTURE_CANDIDATE'
    run_cli = reference_checks.PiUsageCandidateTests.run_cli
    test_enabled_tools_match_the_reference = reference_checks.PiUsageCandidateTests.test_enabled_tools_still_match_the_reference

    @classmethod
    def setUpClass(cls):
        selected = os.environ.get(cls.CANDIDATE_ENV)
        if not selected:
            raise unittest.SkipTest(f'Set {cls.CANDIDATE_ENV} for this Linux candidate.')
        cls.binary = Path(selected).resolve(strict=True)
        if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != os.environ.get(cls.CANDIDATE_ENV + '_SHA256'):
            raise ValueError('The OpenCode capture candidate digest does not match.')
        cls.reference = ROOT / 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu'
        if hashlib.sha256(cls.reference.read_bytes()).hexdigest() != json.loads(PIN.read_text())['executableSha256']:
            raise ValueError('The production scanner does not match its pin.')
        cls.inputs = json.loads((FIXTURES / 'inputs.json').read_text())
        cls.expected = json.loads((FIXTURES / 'expected.json').read_text())

    def capture(self, root, roots, kind, unavailable=False):
        arguments = ['daily', '--by-agent'] if kind == 'daily' else ['opencode', 'session']
        with tempfile.TemporaryDirectory(dir=root, prefix='empty-home-') as home:
            result = subprocess.run([self.binary, *arguments, '--json', '--offline', '--no-color',
                                     '--mode', 'calculate', '--timezone', 'UTC'],
                                    cwd=home, env={'HOME': home, 'OPENCODE_DATA_DIR': ','.join(map(str, roots)),
                                                   'NO_COLOR': '1', 'TZ': 'UTC'},
                                    stdin=subprocess.DEVNULL, capture_output=True, timeout=20)
            self.assertEqual(list(Path(home).iterdir()), [])
            self.assertLess(len(result.stdout) + len(result.stderr), 128 * 1024)
            if unavailable:
                self.assertGreater(result.returncode, 0)
                self.assertEqual(result.stdout, b'')
                self.assertIn(b'OpenCode', result.stderr)
                self.assertNotIn(str(root).encode(), result.stderr)
                self.assertNotIn(b'private-marker', result.stderr)
                return None
            self.assertEqual(result.returncode, 0, result.stderr[:1000])
            return json.loads(result.stdout)

    def test_every_required_baseline_observation_or_refusal(self):
        recorded = json.loads((FIXTURES / 'opencode-qualification.json').read_text())
        recorded.update(json.loads((FIXTURES / 'opencode-observation-candidate.json').read_text()))
        for name, case in cases().items():
            with self.subTest(case=name), tempfile.TemporaryDirectory(prefix='openusage-opencode-capture-') as temporary:
                root = Path(temporary)
                make_source(root / 'not-selected', self.inputs['pi'])
                roots = []
                for index, spec in enumerate(case['roots']):
                    folder = root / f'root-{index}'
                    if not spec.get('missing'):
                        folder.mkdir(mode=0o700)
                        populate(folder, spec)
                    roots.append(folder)
                before = file_hashes(root)
                for kind in ('daily', 'session'):
                    report = self.capture(root, roots, kind, unavailable=case['required'] is None)
                    if report is None:
                        continue
                    self.assertEqual(report['totals']['totalTokens'], case['required'])
                    expected = recorded[name]['reports'][kind]
                    key = 'period' if kind == 'daily' else 'sessionId'
                    rows = report['daily' if kind == 'daily' else 'sessions']
                    self.assertEqual([[row[key], row['totalTokens']] for row in rows], expected['rows'])
                self.assertEqual(file_hashes(root), before)

    def test_invalid_present_counts_and_duplicate_keys_refuse_whole_capture(self):
        base = next(iter(self.inputs['opencode']['files'].values()))
        for fault in ('negative', 'fractional', 'inconsistent', 'duplicate-key'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory(prefix='openusage-opencode-shape-') as temporary:
                root = Path(temporary)
                folder = root / 'selected'
                folder.mkdir()
                populate(folder, {'json': [base]})
                bad = copy.deepcopy(base)
                bad['id'] = 'bad-record'
                if fault == 'negative':
                    bad['tokens']['input'] = -1
                elif fault == 'fractional':
                    bad['tokens']['input'] = 1.5
                if fault == 'inconsistent':
                    bad['tokens']['total'] = 1
                payload = json.dumps(bad)
                if fault == 'duplicate-key':
                    payload = payload[:-1] + ',"tokens":{"total":17}}'
                (folder / 'storage/message/session-z/bad.json').write_text(payload)
                before = file_hashes(root)
                for kind in ('daily', 'session'):
                    self.capture(root, [folder], kind, unavailable=True)
                self.assertEqual(file_hashes(root), before)

    def test_json_byte_limit_accepts_boundary_and_refuses_one_more_byte(self):
        base = next(iter(self.inputs['opencode']['files'].values()))
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-bytes-') as temporary:
            root = Path(temporary)
            selected = root / 'selected'
            selected.mkdir()
            populate(selected, {'json': [base]})
            path = selected / 'storage/message/session-z/copy-0.json'
            pad(path, MIB)
            for kind in ('daily', 'session'):
                self.assertEqual(self.capture(root, [selected], kind)['totals']['totalTokens'], 173)
            with path.open('ab') as stream:
                stream.write(b' ')
            for kind in ('daily', 'session'):
                self.capture(root, [selected], kind, unavailable=True)

    @unittest.skipUnless(os.name == 'posix', 'requires Linux file types')
    def test_links_and_special_files_refuse(self):
        base = next(iter(self.inputs['opencode']['files'].values()))
        for fault in ('file-link', 'directory-link', 'database-link', 'fifo'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory(prefix='openusage-opencode-link-') as temporary:
                root = Path(temporary)
                selected = root / 'selected'
                outside = root / 'outside'
                selected.mkdir()
                outside.mkdir()
                populate(outside, {'json': [base], 'db': [base]})
                messages = selected / 'storage/message'
                if fault == 'directory-link':
                    messages.parent.mkdir()
                    messages.symlink_to(outside / 'storage/message', target_is_directory=True)
                elif fault == 'database-link':
                    (selected / 'opencode.db').symlink_to(outside / 'opencode.db')
                else:
                    messages.mkdir(parents=True)
                    path = messages / 'source.json'
                    if fault == 'file-link':
                        path.symlink_to(outside / 'storage/message/session-z/copy-0.json')
                    else:
                        os.mkfifo(path)
                for kind in ('daily', 'session'):
                    self.capture(root, [selected], kind, unavailable=True)

    def test_committed_wal_is_explicitly_unavailable(self):
        base = next(iter(self.inputs['opencode']['files'].values()))
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-wal-') as temporary:
            root = Path(temporary)
            selected = root / 'selected'
            selected.mkdir()
            with closing(sqlite3.connect(selected / 'opencode.db')) as database:
                self.assertEqual(database.execute('PRAGMA journal_mode=WAL').fetchone()[0], 'wal')
                database.execute('CREATE TABLE message (id TEXT, session_id TEXT, data TEXT)')
                database.execute('INSERT INTO message VALUES (?,?,?)', ('message-z', 'session-z', json.dumps(base)))
                database.commit()
                self.assertTrue((selected / 'opencode.db-wal').is_file())
                before = file_hashes(root)
                for kind in ('daily', 'session'):
                    self.capture(root, [selected], kind, unavailable=True)
                self.assertEqual(file_hashes(root), before)
            # A checkpoint can remove the WAL peer while the main file keeps its WAL-mode header.
            before = file_hashes(root)
            for kind in ('daily', 'session'):
                self.capture(root, [selected], kind, unavailable=True)
            self.assertEqual(file_hashes(root), before)

    def test_database_shapes_and_journal_peers_refuse_whole_capture(self):
        base = next(iter(self.inputs['opencode']['files'].values()))
        for fault in ('blob', 'invalid-text', 'null-id', 'oversize-row', 'view', 'journal'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory(prefix='openusage-opencode-database-') as temporary:
                root = Path(temporary)
                selected = root / 'selected'
                selected.mkdir()
                populate(selected, {'json': [base]})
                with closing(sqlite3.connect(selected / 'opencode.db')) as database, database:
                    if fault == 'view':
                        database.execute('CREATE VIEW message AS SELECT 1 AS id, 2 AS session_id, 3 AS data')
                    else:
                        database.execute('CREATE TABLE message (id TEXT, session_id TEXT, data TEXT)')
                        payload = json.dumps(dict(base, id='database-message'))
                        identifier = None if fault == 'null-id' else 'database-message'
                        if fault == 'blob':
                            payload = payload.encode()
                        elif fault == 'oversize-row':
                            payload += ' ' * MIB
                        if fault == 'invalid-text':
                            database.execute("INSERT INTO message VALUES (?, ?, CAST(x'fffe' AS TEXT))",
                                             (identifier, 'session-z'))
                        else:
                            database.execute('INSERT INTO message VALUES (?,?,?)',
                                             (identifier, 'session-z', payload))
                if fault == 'journal':
                    (selected / 'opencode.db-journal').write_bytes(b'Synthetic private-marker journal.\n')
                before = file_hashes(root)
                for kind in ('daily', 'session'):
                    self.capture(root, [selected], kind, unavailable=True)
                self.assertEqual(file_hashes(root), before)


if __name__ == '__main__':
    unittest.main()
