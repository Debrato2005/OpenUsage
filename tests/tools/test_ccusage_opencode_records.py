"""Check validated producer metadata and explicitly measured zero usage."""
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import tempfile
import unittest

import test_ccusage_opencode_runtime as runtime_checks
import test_ccusage_opencode_wal_reuse as reuse_checks
from test_ccusage_formats import FIXTURES, file_hashes


class OpenCodeRecordsCaptureTests(reuse_checks.OpenCodeWalReuseCandidateTests):
    CANDIDATE_ENV = 'OPENUSAGE_CCUSAGE_OPENCODE_RECORDS_CANDIDATE'
    SUPPORTED_PRODUCER_METADATA = True


class OpenCodeRecordsRuntimeTests(runtime_checks.OpenCodeRuntimeTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        selected = os.environ.get('OPENUSAGE_CCUSAGE_OPENCODE_RECORDS_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CCUSAGE_OPENCODE_RECORDS_CANDIDATE for the record candidate.')
        cls.binary = Path(selected).resolve(strict=True)
        if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != os.environ.get('OPENUSAGE_CCUSAGE_OPENCODE_RECORDS_CANDIDATE_SHA256'):
            raise ValueError('The record candidate digest does not match.')

    def test_candidate_reports_and_refusals_repeat(self):
        result = self.collect(self.binary)
        expected = json.loads((FIXTURES / 'opencode-records-runtime-candidate.json').read_text())
        self.assertEqual(result, expected)
        for name, record in result.items():
            with self.subTest(case=name):
                required = runtime_checks.REQUIRED[name]
                for report in record['reports'].values():
                    if required is None:
                        self.assertEqual(report['status'], 'refused')
                    else:
                        self.assertEqual(report['status'], 'reported')
                        self.assertEqual(report['totalTokens'], required)
                        self.assertGreater(report['rowCount'], 0)
                        if required == 0:
                            self.assertEqual(report['rowCount'], 1)

    def test_malformed_metadata_and_incomplete_zero_refuse(self):
        faults = [
            ('agent_switched', 'agent-switched', ('agent',), 123, False),
            ('model_switched', 'model-switched', ('model', 'providerID'), False, False),
            ('synthetic', 'synthetic', ('text',), [], False),
            ('shell', 'shell', ('callID',), None, False),
            ('compaction', 'compaction', ('summary',), 123, False),
            ('reported_zero', 'assistant', ('tokens', 'output'), None, True),
            ('reported_zero', 'assistant', ('tokens', 'input'), None, False),
            ('agent_switched', 'agent-switched', ('tokens',), {'input': 1}, False),
            ('agent_switched', 'agent-switched', ('id',), 'conflicting-id', False),
            ('agent_switched', 'agent-switched', ('role',), 'assistant', False),
        ]
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-record-guards-') as temporary:
            root = Path(temporary)
            fixture = self.produce(root)
            for index, (case, kind, keys, value, remove) in enumerate(faults):
                with self.subTest(case=case, field=keys):
                    selected = root / f'fault-{index}'
                    shutil.copytree(fixture / case, selected)
                    with closing(sqlite3.connect(selected / 'opencode.db', isolation_level=None)) as database:
                        identifier, raw = database.execute('SELECT id,data FROM session_message WHERE type=?', (kind,)).fetchone()
                        payload = json.loads(raw)
                        target = payload
                        for key in keys[:-1]:
                            target = target[key]
                        if keys[-1] not in ('tokens', 'id', 'role'):
                            self.assertIn(keys[-1], target)
                        if remove:
                            del target[keys[-1]]
                        else:
                            target[keys[-1]] = value
                        database.execute('UPDATE session_message SET data=? WHERE id=?', (json.dumps(payload), identifier))
                    before = file_hashes(selected)
                    for report_kind in ('daily', 'session'):
                        with tempfile.TemporaryDirectory(dir=root, prefix='empty-home-') as home:
                            report = self.report(self.binary, [selected], report_kind, Path(home))
                            self.assertEqual(report.returncode, 1)
                            self.assertEqual(report.stdout, b'')
                            self.assertIn(b'OpenCode', report.stderr)
                            self.assertEqual(list(Path(home).iterdir()), [])
                    self.assertEqual(file_hashes(selected), before)

    def test_metadata_annotations_do_not_create_usage(self):
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-annotations-') as temporary:
            root = Path(temporary)
            selected = self.produce(root) / 'agent_switched'
            with closing(sqlite3.connect(selected / 'opencode.db', isolation_level=None)) as database:
                identifier, raw = database.execute("SELECT id,data FROM session_message WHERE type='agent-switched'").fetchone()
                payload = json.loads(raw)
                payload['metadata'] = {'tokens': {'total': 999}, 'usage': {'input': 999}}
                database.execute('UPDATE session_message SET data=? WHERE id=?', (json.dumps(payload), identifier))
            before = file_hashes(selected)
            for kind in ('daily', 'session'):
                with tempfile.TemporaryDirectory(dir=root, prefix='empty-home-') as home:
                    result = self.report(self.binary, [selected], kind, Path(home))
                    self.assertEqual(result.returncode, 0, result.stderr)
                    report = json.loads(result.stdout)
                    self.assertEqual(report['totals']['totalTokens'], 173)
                    self.assertEqual(len(report['daily' if kind == 'daily' else 'sessions']), 1)
                    self.assertEqual(list(Path(home).iterdir()), [])
            self.assertEqual(file_hashes(selected), before)

    def test_model_variant_copies_require_matching_source_fields(self):
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-variant-copies-') as temporary:
            root = Path(temporary)
            fixture = self.produce(root)
            folders = [root / 'first', root / 'second']
            for folder in folders:
                shutil.copytree(fixture / 'completed', folder)
                with closing(sqlite3.connect(folder / 'opencode.db', isolation_level=None)) as database:
                    identifier, raw = database.execute("SELECT id,data FROM session_message WHERE type='assistant'").fetchone()
                    payload = json.loads(raw)
                    payload['model']['variant'] = 'high'
                    database.execute('UPDATE session_message SET data=? WHERE id=?', (json.dumps(payload), identifier))
            for conflict in (False, True):
                if conflict:
                    with closing(sqlite3.connect(folders[1] / 'opencode.db', isolation_level=None)) as database:
                        identifier, raw = database.execute("SELECT id,data FROM session_message WHERE type='assistant'").fetchone()
                        payload = json.loads(raw)
                        payload['model']['variant'] = 'low'
                        database.execute('UPDATE session_message SET data=? WHERE id=?', (json.dumps(payload), identifier))
                before = {str(folder): file_hashes(folder) for folder in folders}
                for order in (folders, list(reversed(folders))):
                    for kind in ('daily', 'session'):
                        with tempfile.TemporaryDirectory(dir=root, prefix='empty-home-') as home:
                            result = self.report(self.binary, order, kind, Path(home))
                            self.assertEqual(result.returncode, 1 if conflict else 0, result.stderr)
                            if conflict:
                                self.assertEqual(result.stdout, b'')
                                self.assertIn(b'OpenCode', result.stderr)
                            else:
                                self.assertEqual(json.loads(result.stdout)['totals']['totalTokens'], 173)
                            self.assertEqual(list(Path(home).iterdir()), [])
                self.assertEqual({str(folder): file_hashes(folder) for folder in folders}, before)
