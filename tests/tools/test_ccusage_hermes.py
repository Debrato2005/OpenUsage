"""Record the pinned Hermes scanner outcomes on private SQLite fixtures."""
from contextlib import closing
import json
import os
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest

from local_log_qualification import KINDS, collect, pinned_binary, report
from test_ccusage_formats import FIXTURES, file_hashes


GOLDEN = Path(__file__).parent / 'fixtures/hermes/qualification.json'
STATE = 'state.db'
ESTIMATE = 0.0009958500000000002


def cases():
    inputs = json.loads((FIXTURES / 'inputs.json').read_text())
    script = inputs['hermes']['databases'][STATE]['sql']

    def database(*changes):
        with closing(sqlite3.connect(':memory:')) as connection:
            connection.executescript(script)
            for change in changes:
                connection.execute(change)
            connection.commit()
            return connection.serialize()

    def case(roots, tokens, *, daily=None, rows=None, fields=None):
        return {'roots': roots, 'required': {'daily': daily, 'session': tokens},
                'rows': rows or {}, 'fields': fields or {}}

    def single(contents):
        return {'first': {STATE: contents}}

    def peers(first, second):
        return {'first': {STATE: first}, 'second': {STATE: second}}

    base = database()
    changed = database('UPDATE sessions SET input_tokens = input_tokens + 17')
    rekeyed = database("UPDATE sessions SET id = 'session-other'")
    fork = database('ALTER TABLE sessions ADD COLUMN parent_session_id TEXT',
                    "UPDATE sessions SET id = 'session-fork', parent_session_id = 'session-z', input_tokens = input_tokens + 17")
    child_delta = database('ALTER TABLE sessions ADD COLUMN parent_session_id TEXT',
                           "UPDATE sessions SET id = 'session-child', parent_session_id = 'session-z', input_tokens = 12, output_tokens = 5, cache_read_tokens = 0, cache_write_tokens = 0, reasoning_tokens = 0")
    zero = database('UPDATE sessions SET input_tokens = 0, output_tokens = 0, cache_read_tokens = 0, cache_write_tokens = 0, reasoning_tokens = 0, estimated_cost_usd = 0, actual_cost_usd = 0')
    return {
        'single': case(single(base), 173),
        'current_producer_reasoning_inside_output': case(single(base), 162),
        'copy': case(peers(base, base), 173),
        'replay': case(peers(base, base) | {'third': {STATE: base}}, 173),
        'distinct_ids_equal_counts': case(peers(base, rekeyed), 346),
        'copied_usage_with_new_id': case(peers(base, rekeyed), 173),
        'conflicting_copy': case(peers(base, changed), None),
        'conflicting_copy_reversed': case(peers(changed, base), None),
        'synthetic_inherited_fork_with_new_usage': case(peers(base, fork), 190),
        'synthetic_child_with_local_usage': case(peers(base, child_delta), 190),
        'invalid_database_peer': case(peers(base, b'not a SQLite database\n'), None),
        'missing_column_peer': case(peers(base, database('ALTER TABLE sessions DROP COLUMN reasoning_tokens')), None),
        'missing_root': case(single(base) | {'missing': None}, None),
        'invalid_session_id': case(single(database("UPDATE sessions SET id = ''")), None),
        'missing_model': case(single(database('UPDATE sessions SET model = NULL')), 173,
                              fields={'totalCost': None}),
        'missing_time': case(single(database('UPDATE sessions SET started_at = NULL')), 173,
                             fields={'totalCost': None}),
        'missing_reasoning': case(single(database('UPDATE sessions SET reasoning_tokens = NULL')), None),
        'negative_count': case(single(database('UPDATE sessions SET input_tokens = -1')), None),
        'fractional_count': case(single(database('UPDATE sessions SET input_tokens = 120.5')), None),
        'text_count': case(single(database("UPDATE sessions SET input_tokens = 'private-marker'")), None),
        'empty_database': case(single(database('DELETE FROM sessions')), 0, daily=0,
                               rows=dict.fromkeys(KINDS, 0)),
        'unqualified_zero_row': case(single(zero), None),
        'stored_actual_and_estimate': case(single(base), 173, fields={'totalCost': ESTIMATE}),
        'stored_estimate_only': case(single(database('UPDATE sessions SET actual_cost_usd = NULL')), 173,
                                    fields={'totalCost': ESTIMATE}),
        'stored_zero_actual': case(single(database('UPDATE sessions SET actual_cost_usd = 0')), 173,
                                   fields={'totalCost': ESTIMATE}),
        'no_stored_cost': case(single(database('UPDATE sessions SET actual_cost_usd = NULL, estimated_cost_usd = NULL')), 173,
                               fields={'totalCost': ESTIMATE}),
        'unknown_model_with_stored_actual': case(single(database("UPDATE sessions SET model = 'synthetic-model-not-priced'")), 173,
                                                 fields={'totalCost': None}),
        'unknown_model_without_cost': case(single(database("UPDATE sessions SET model = 'synthetic-model-not-priced', actual_cost_usd = NULL, estimated_cost_usd = NULL")), 173,
                                            fields={'totalCost': None}),
    }


class HermesQualificationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected or sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            raise unittest.SkipTest('Set OPENUSAGE_CCUSAGE_BIN on Linux with Bubblewrap.')
        cls.binary = pinned_binary(selected)

    def test_pinned_outcomes_and_limits(self):
        observed = collect(self.binary, cases(), 'hermes', 'HERMES_HOME', STATE)
        self.assertEqual(observed, json.loads(GOLDEN.read_text()))
        self.assertEqual(observed['conflicting_copy']['reports']['session']['totals']['totalTokens'], 173)
        self.assertEqual(observed['conflicting_copy_reversed']['reports']['session']['totals']['totalTokens'], 190)
        self.assertEqual(observed['current_producer_reasoning_inside_output']['reports']['session']['totals']['totalTokens'], 173)
        self.assertEqual(observed['distinct_ids_equal_counts']['reports']['session']['sha256'],
                         observed['copied_usage_with_new_id']['reports']['session']['sha256'])

    def test_pinned_sqlite_reader_sees_committed_wal_only(self):
        with tempfile.TemporaryDirectory(prefix='openusage-hermes-wal-') as temporary:
            root = Path(temporary)
            folder = root / 'selected'
            folder.mkdir()
            path = folder / STATE
            path.write_bytes(cases()['single']['roots']['first'][STATE])
            with closing(sqlite3.connect(path, isolation_level=None)) as writer:
                self.assertEqual(writer.execute('PRAGMA journal_mode=WAL').fetchone(), ('wal',))
                writer.execute('PRAGMA wal_autocheckpoint=0')
                writer.execute('UPDATE sessions SET input_tokens=137')
                self.assertGreater((folder / 'state.db-wal').stat().st_size, 32)
                writer.execute('BEGIN IMMEDIATE')
                writer.execute('UPDATE sessions SET input_tokens=999')
                before = file_hashes(folder)
                for kind in KINDS:
                    result = report(self.binary, root, ['selected'], kind, 'hermes', 'HERMES_HOME')
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(json.loads(result.stdout)['totals']['totalTokens'], 190)
                self.assertEqual(file_hashes(folder), before)
                writer.execute('ROLLBACK')


if __name__ == '__main__':
    unittest.main()
