"""Check isolated v2 usage and whole-scan refusal of unsupported OpenCode sources."""
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import tempfile
import unittest

import test_ccusage_opencode_capture as capture_checks
import test_ccusage_opencode_forks as fork_checks
import test_ccusage_opencode_records as record_checks
import test_ccusage_opencode_runtime as runtime_checks
import test_ccusage_pi_candidate as reference_checks
from test_ccusage_formats import FormatComparisons, file_hashes, make_source


CANDIDATE_ENV = 'OPENUSAGE_CCUSAGE_OPENCODE_V2_CANDIDATE'
VERSION = '20.0.48-openusage.opencode-v2-candidate.1'


def candidate(cls):
    selected = os.environ.get(CANDIDATE_ENV)
    if not selected:
        raise unittest.SkipTest(f'Set {CANDIDATE_ENV} for the v2 experiment.')
    cls.binary = Path(selected).resolve(strict=True)
    if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != os.environ.get(CANDIDATE_ENV + '_SHA256'):
        raise ValueError('The v2 candidate digest does not match.')
    result = runtime_checks.checked_run(runtime_checks.sandbox() + [
        '--ro-bind', str(cls.binary), '/runtime/ccusage', '/runtime/ccusage', '--version'])
    if result.returncode != 0 or result.stdout.decode().strip() != 'ccusage ' + VERSION:
        raise ValueError('The v2 candidate version does not match.')


class OpenCodeV2RuntimeTests(record_checks.OpenCodeRecordsRuntimeTests):
    @classmethod
    def setUpClass(cls):
        candidate(cls)
        runtime_checks.OpenCodeRuntimeTests.setUpClass.__func__(cls)

    def refused(self, folders, kind, root):
        with tempfile.TemporaryDirectory(dir=root, prefix='empty-home-') as home:
            result = self.report(self.binary, folders, kind, Path(home))
            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertEqual(result.stdout, b'')
            self.assertEqual(result.stderr, b'Error: CliError("OpenCode source capture is unavailable")\n')
            self.assertEqual(list(Path(home).iterdir()), [])

    def test_unsupported_sources_refuse_all_roots_in_both_orders(self):
        faults = ('legacy-message', 'legacy-part', 'aggregate-input', 'aggregate-reasoning',
                  'aggregate-cache', 'aggregate-cost', 'aggregate-negative', 'aggregate-text',
                  'aggregate-only', 'session-v2', 'extra-column', 'missing-sequence',
                  'empty-id', 'negative-sequence', 'missing-session', 'duplicate-sequence', 'json',
                  'payload-total', 'payload-missing-reasoning', 'payload-missing-cache',
                  'payload-model', 'payload-role', 'payload-unknown')
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-v2-boundary-') as temporary:
            root = Path(temporary)
            valid = self.produce(root) / 'completed'
            for fault in faults:
                with self.subTest(fault=fault):
                    selected = root / fault
                    shutil.copytree(valid, selected)
                    with closing(sqlite3.connect(selected / 'opencode.db', isolation_level=None)) as database:
                        session = database.execute('SELECT id FROM session').fetchone()[0]
                        if fault == 'legacy-message':
                            database.execute('INSERT INTO message(id,session_id,time_created,time_updated,data) VALUES(?,?,?,?,?)',
                                             ('msg_legacy', session, 1, 1, '{"role":"user"}'))
                        elif fault == 'legacy-part':
                            database.execute('INSERT INTO part(id,message_id,session_id,time_created,time_updated,data) VALUES(?,?,?,?,?,?)',
                                             ('prt_legacy', 'msg_missing', session, 1, 1, '{"type":"text","text":"private-marker"}'))
                        elif fault.startswith('aggregate-'):
                            if fault == 'aggregate-only':
                                database.execute('DELETE FROM session_message')
                            column, value = {
                                'aggregate-input': ('tokens_input', 17),
                                'aggregate-reasoning': ('tokens_reasoning', 17),
                                'aggregate-cache': ('tokens_cache_read', 17),
                                'aggregate-cost': ('cost', 0.1),
                                'aggregate-negative': ('tokens_input', -1),
                                'aggregate-text': ('tokens_input', 'private-marker'),
                                'aggregate-only': ('tokens_input', 173),
                            }[fault]
                            database.execute(f'UPDATE session SET {column}=?', (value,))
                        elif fault == 'session-v2':
                            database.execute('CREATE TABLE session_v2(id TEXT)')
                        elif fault == 'extra-column':
                            database.execute('ALTER TABLE session_message ADD COLUMN copy_origin TEXT')
                        elif fault in ('missing-sequence', 'duplicate-sequence'):
                            indexes = database.execute("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='session_message' AND sql IS NOT NULL").fetchall()
                            for (name,) in indexes:
                                database.execute('DROP INDEX "' + name.replace('"', '""') + '"')
                            if fault == 'missing-sequence':
                                database.execute('ALTER TABLE session_message DROP COLUMN seq')
                            else:
                                database.execute("INSERT INTO session_message(id,session_id,type,seq,time_created,time_updated,data) SELECT 'msg_duplicate',session_id,type,seq,time_created,time_updated,data FROM session_message")
                        elif fault == 'empty-id':
                            database.execute("UPDATE session_message SET id=''")
                        elif fault == 'negative-sequence':
                            database.execute('UPDATE session_message SET seq=-1')
                        elif fault == 'missing-session':
                            database.execute("UPDATE session_message SET session_id='ses_missing'")
                        elif fault.startswith('payload-'):
                            identifier, raw = database.execute("SELECT id,data FROM session_message WHERE type='assistant'").fetchone()
                            payload = json.loads(raw)
                            if fault in ('payload-total', 'payload-missing-reasoning', 'payload-missing-cache'):
                                payload['tokens']['total'] = 173
                                if fault == 'payload-missing-reasoning':
                                    del payload['tokens']['reasoning']
                                elif fault == 'payload-missing-cache':
                                    del payload['tokens']['cache']['read']
                            elif fault == 'payload-model':
                                model = payload.pop('model')
                                payload.update(modelID=model['id'], providerID=model['providerID'])
                            elif fault == 'payload-role':
                                payload['role'] = 'assistant'
                            else:
                                payload['forkParentID'] = 'ses_unsupported'
                            database.execute('UPDATE session_message SET data=? WHERE id=?', (json.dumps(payload), identifier))
                        database.execute('PRAGMA wal_checkpoint(TRUNCATE)').fetchall()
                    if fault == 'json':
                        target = selected / 'storage/message/ses_fixture/message.json'
                        target.parent.mkdir(parents=True)
                        target.write_text('{}\n')
                    before = {str(folder): file_hashes(folder) for folder in (valid, selected)}
                    for folders in ([selected], [valid, selected], [selected, valid]):
                        for kind in ('daily', 'session'):
                            self.refused(folders, kind, root)
                    self.assertEqual({str(folder): file_hashes(folder) for folder in (valid, selected)}, before)

    def test_empty_schema_has_no_measured_zero_row(self):
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-v2-empty-') as temporary:
            root = Path(temporary)
            selected = self.produce(root) / 'completed'
            with closing(sqlite3.connect(selected / 'opencode.db', isolation_level=None)) as database:
                database.execute('DELETE FROM session_message')
                database.execute('PRAGMA wal_checkpoint(TRUNCATE)').fetchall()
            before = file_hashes(selected)
            for kind in ('daily', 'session'):
                with tempfile.TemporaryDirectory(dir=root) as home:
                    result = self.report(self.binary, [selected], kind, Path(home))
                    self.assertEqual(result.returncode, 0, result.stderr)
                    report = json.loads(result.stdout)
                    self.assertEqual(report['totals']['totalTokens'], 0)
                    self.assertEqual(report['daily' if kind == 'daily' else 'sessions'], [])
                    self.assertEqual(list(Path(home).iterdir()), [])
            self.assertEqual(file_hashes(selected), before)

    def test_v2_committed_wal_and_open_transaction(self):
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-v2-wal-') as temporary:
            root = Path(temporary)
            selected = self.produce(root) / 'completed'
            with closing(sqlite3.connect(selected / 'opencode.db', isolation_level=None)) as writer:
                self.assertEqual(writer.execute('PRAGMA journal_mode=WAL').fetchone(), ('wal',))
                writer.execute('PRAGMA wal_autocheckpoint=0')
                identifier, raw = writer.execute("SELECT id,data FROM session_message WHERE type='assistant'").fetchone()
                payload = json.loads(raw)
                payload['tokens']['input'] += 17
                writer.execute('UPDATE session_message SET data=? WHERE id=?', (json.dumps(payload), identifier))
                self.assertGreater((selected / 'opencode.db-wal').stat().st_size, 32)
                for uncommitted in (False, True):
                    if uncommitted:
                        writer.execute('BEGIN IMMEDIATE')
                        payload['tokens']['input'] = 999
                        writer.execute('UPDATE session_message SET data=? WHERE id=?', (json.dumps(payload), identifier))
                    before = file_hashes(selected)
                    for kind in ('daily', 'session'):
                        with tempfile.TemporaryDirectory(dir=root) as home:
                            result = self.report(self.binary, [selected], kind, Path(home))
                            self.assertEqual(result.returncode, 0, result.stderr)
                            self.assertEqual(json.loads(result.stdout)['totals']['totalTokens'], 190)
                            self.assertEqual(list(Path(home).iterdir()), [])
                    self.assertEqual(file_hashes(selected), before)
                writer.execute('ROLLBACK')

    def test_linked_database_and_invalid_wal_refuse(self):
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-v2-files-') as temporary:
            root = Path(temporary)
            valid = self.produce(root) / 'completed'
            for fault in ('symlink', 'hardlink', 'invalid-wal'):
                selected = root / fault
                if fault == 'invalid-wal':
                    shutil.copytree(valid, selected)
                    (selected / 'opencode.db-wal').write_bytes(b'private-marker invalid WAL\n')
                else:
                    selected.mkdir()
                    target = selected / 'opencode.db'
                    if fault == 'symlink':
                        target.symlink_to(valid / 'opencode.db')
                    else:
                        os.link(valid / 'opencode.db', target)
                before = {str(folder): file_hashes(folder) for folder in (valid, selected)}
                for kind in ('daily', 'session'):
                    self.refused([selected], kind, root)
                self.assertEqual({str(folder): file_hashes(folder) for folder in (valid, selected)}, before)


class OpenCodeV2ForkTests(fork_checks.OpenCodeForkTests):
    @classmethod
    def setUpClass(cls):
        candidate(cls)
        super().setUpClass()

    def test_candidate_fork_outcomes_repeat(self):
        observed = self.collect(self.binary)
        self.assertEqual(len(observed), 11)
        for name, record in observed.items():
            with self.subTest(case=name):
                self.assertTrue(all(report == {'status': 'refused', 'exitCode': 1}
                                    for report in record['reports'].values()))


class OpenCodeV2OtherToolsTests(FormatComparisons, unittest.TestCase):
    CANDIDATE_ENV = CANDIDATE_ENV
    run_cli = reference_checks.PiUsageCandidateTests.run_cli
    test_enabled_tools_match_the_reference = reference_checks.PiUsageCandidateTests.test_enabled_tools_still_match_the_reference

    @classmethod
    def setUpClass(cls):
        candidate(cls)
        capture_checks.OpenCodeCaptureCandidateTests.setUpClass.__func__(cls)
        cls.inputs = {tool: value for tool, value in cls.inputs.items() if tool != 'opencode'}
        cls.expected = {tool: value for tool, value in cls.expected.items() if tool != 'opencode'}

    def test_daily_and_session_contracts_repeat_without_source_changes(self):
        self.assertEqual(set(self.inputs), set(self.expected))
        self.assertEqual(len(self.inputs), 15)
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-v2-regression-') as temporary:
            root = Path(temporary)
            folders = {tool: make_source(root, fixture) for tool, fixture in self.inputs.items()}
            before = file_hashes(root)
            for tool, fixture in self.inputs.items():
                for kind in ('daily', 'session'):
                    with self.subTest(tool=tool, report=kind):
                        first = self.run_report(root, tool, fixture, folders[tool], kind)
                        self.assertEqual(first, self.expected[tool][kind])
                        self.assertEqual(self.run_report(root, tool, fixture, folders[tool], kind), first)
            self.assertEqual(file_hashes(root), before)


if __name__ == '__main__':
    unittest.main()
