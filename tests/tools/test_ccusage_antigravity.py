"""Run the pinned Antigravity scanner on private synthetic databases."""
from contextlib import closing
import json
import os
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest

from local_log_qualification import KINDS, collect, pinned_binary, report
from test_ccusage_formats import blob, file_hashes, number


GOLDEN = Path(__file__).parent / 'fixtures/antigravity/qualification.json'
DATABASE = 'conversations/session-a.db'
TIME = 1780000000
FIELDS = {2: 100, 3: 25, 4: 5, 5: 10, 9: 5, 10: 20}
SCHEMA = ('CREATE TABLE gen_metadata(idx INTEGER PRIMARY KEY, data BLOB NOT NULL);'
          'CREATE TABLE steps(idx INTEGER PRIMARY KEY, metadata BLOB);'
          'CREATE TABLE trajectory_metadata_blob(data BLOB);')


def usage(fields=None, *, response='response-a', message=None, provider_message=None):
    result = b''.join(number(key, value) for key, value in (FIELDS if fields is None else fields).items())
    for key, value in [(11, response), (7, message), (12, provider_message)]:
        if value is not None:
            result += blob(key, value)
    return result


def timestamp(seconds=TIME):
    return number(1, seconds) + number(2, 0)


def generation(value, *, model='gemini-2.5-pro', timed=True, retries=()):
    chat = blob(4, value) if value is not None else b''
    if timed:
        chat += blob(9, blob(4, timestamp()))
    if model:
        chat += blob(19, model)
    for retry in retries:
        chat += blob(17, blob(2, retry))
    return blob(1, chat)


def step(value, *, model='gemini-2.5-pro', timed=True, retries=()):
    result = blob(9, value) if value is not None else b''
    if timed:
        result += blob(8, timestamp())
    if model:
        result += blob(24, blob(12, model))
    for retry in retries:
        result += blob(28, blob(2, retry))
    return result


def database(generations=(), steps=(), trajectory=None):
    with closing(sqlite3.connect(':memory:')) as connection:
        connection.executescript(SCHEMA)
        for table, column, values in [('gen_metadata', 'data', generations), ('steps', 'metadata', steps)]:
            for index, value in enumerate(values):
                connection.execute(f'INSERT INTO {table}(idx,{column}) VALUES(?,?)', (index, value))
        if trajectory is not None:
            connection.execute('INSERT INTO trajectory_metadata_blob VALUES(?)', (trajectory,))
        connection.commit()
        return connection.serialize()


def cases():
    base = generation(usage())
    distinct = generation(usage(response='response-b'))
    conflict = generation(usage(FIELDS | {2: 117}))
    extra = generation(usage({2: 12, 3: 5, 4: 0, 5: 0, 9: 0, 10: 5}, response='response-new'))
    no_id = generation(usage(response=None))

    def single(*events, steps=(), trajectory=None):
        return {'first': {DATABASE: database(events, steps, trajectory)}}

    def peers(first, second, *, renamed=False):
        path = DATABASE.replace('session-a', 'renamed') if renamed else DATABASE
        return {'first': {DATABASE: database(first)}, 'second': {path: database(second)}}

    def case(roots, total, *, dated=False, rows=None, fields=None):
        return {'roots': roots, 'required': {'daily': total if dated else None, 'session': total},
                'rows': rows or {}, 'fields': fields or {}}

    with closing(sqlite3.connect(':memory:')) as connection:
        connection.execute('CREATE TABLE unrelated(value TEXT)')
        wrong_schema = connection.serialize()
    bridge = generation(usage(response='response-a', message='message-b'))
    return {
        'generation_single': case(single(base), 140),
        'step_single': case(single(steps=[step(usage())]), 140),
        'copied_database': case(peers([base], [base]), 140),
        'renamed_database_copy': case(peers([base], [base], renamed=True), 140),
        'replay': case(single(base, base), 140),
        'generation_step_copy': case(single(base, steps=[step(usage())]), 140),
        'distinct_response_ids': case(single(base, distinct), 280),
        'distinct_response_ids_across_databases': case(peers([base], [distinct]), 280),
        'inherited_copy_with_new_call': case(peers([base], [base, extra], renamed=True), 157),
        'same_id_conflict': case(single(base, conflict), None),
        'same_id_conflict_reversed': case(single(conflict, base), None),
        'database_conflict': case(peers([base], [conflict]), None),
        'database_conflict_reversed': case(peers([conflict], [base]), None),
        'generation_step_conflict': case(single(base, steps=[step(usage(FIELDS | {3: 45, 10: 40}))]), None),
        'component_max_fabricates_total': case(single(conflict,
            generation(usage(FIELDS | {3: 45, 10: 40}))), None),
        'bridged_identity_conflict': case(single(base, generation(usage(FIELDS | {2: 117},
            response=None, message='message-b')), bridge), None),
        'missing_id_single': case(single(no_id), 140),
        'missing_id_replay': case(single(no_id, no_id), 140),
        'missing_id_copy': case(peers([no_id], [no_id], renamed=True), 140),
        'retry_same_call': case(single(generation(usage(), retries=[usage()])), 140),
        'retry_distinct_call': case(single(generation(usage(), retries=[usage(response='response-b')])), 280),
        'missing_time': case(single(generation(usage(), timed=False)), 140),
        'trajectory_time': case(single(generation(usage(), timed=False), trajectory=blob(2, timestamp())), 140),
        'missing_model': case(single(generation(usage(), model=None)), 140, fields={'totalCost': None}),
        'missing_input': case(single(generation(usage({k: v for k, v in FIELDS.items() if k != 2}))), None),
        'missing_cache_read': case(single(generation(usage({k: v for k, v in FIELDS.items() if k != 5}))), None),
        'measured_zero': case(single(generation(usage(dict.fromkeys(FIELDS, 0)))), 0, rows=dict.fromkeys(KINDS, 1)),
        'reasoning_exceeds_total_output': case(single(generation(usage(FIELDS | {9: 30}))), None),
        'integer_overflow': case(single(generation(usage(FIELDS | {2: 2**64 - 1}))), None),
        'malformed_proto': case(single(base, b'\xff'), None),
        'malformed_proto_peer': case({'first': {DATABASE: database([base])},
                                     'second': {DATABASE: database([b'\xff'])}}, None),
        'invalid_sqlite_peer': case({'first': {DATABASE: database([base])}, 'second': {DATABASE: b'not SQLite'}}, None),
        'wrong_schema_peer': case({'first': {DATABASE: database([base])}, 'second': {DATABASE: wrong_schema}}, None),
        'empty_sqlite': case({'first': {DATABASE: database()}}, 0, dated=True, rows=dict.fromkeys(KINDS, 0)),
        'uninitialized_sqlite': case({'first': {DATABASE: b''}}, 0, dated=True, rows=dict.fromkeys(KINDS, 0)),
        'header_only': case(single(generation(None)), 0, dated=True, rows=dict.fromkeys(KINDS, 0)),
        'missing_root': case({'first': None}, None),
    }


class AntigravityQualificationTests(unittest.TestCase):
    def setUp(self):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected or sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            self.skipTest('Set OPENUSAGE_CCUSAGE_BIN on Linux with Bubblewrap.')
        self.binary = pinned_binary(selected)

    def test_pinned_outcomes_and_limits(self):
        observed = collect(self.binary, cases(), 'antigravity', 'ANTIGRAVITY_DATA_DIR', DATABASE)
        self.assertEqual(observed, json.loads(GOLDEN.read_text()))
        self.assertTrue(observed['inherited_copy_with_new_call']['reports']['session']['matchesRequired'])
        self.assertFalse(observed['same_id_conflict']['reports']['session']['matchesRequired'])
        self.assertFalse(observed['invalid_sqlite_peer']['reports']['session']['matchesRequired'])
        self.assertEqual(observed['component_max_fabricates_total']['reports']['session']['totals']['totalTokens'], 177)

    def test_committed_wal_and_uncommitted_write(self):
        with tempfile.TemporaryDirectory(prefix='openusage-antigravity-wal-') as temporary:
            root = Path(temporary)
            path = root / 'first' / DATABASE
            path.parent.mkdir(parents=True)
            with closing(sqlite3.connect(path)) as connection:
                connection.execute('PRAGMA journal_mode=WAL')
                connection.execute('PRAGMA wal_autocheckpoint=0')
                connection.executescript(SCHEMA)
                connection.execute('INSERT INTO gen_metadata VALUES(?,?)', (0, generation(usage())))
                connection.commit()
                self.assertGreater(path.with_name(path.name + '-wal').stat().st_size, 32)
                connection.execute('UPDATE gen_metadata SET data=?', (generation(usage(FIELDS | {2: 117})),))
                before = file_hashes(root)
                for kind in KINDS:
                    result = report(self.binary, root, ['first'], kind, 'antigravity', 'ANTIGRAVITY_DATA_DIR')
                    self.assertEqual(result.returncode, 0)
                    self.assertEqual(json.loads(result.stdout)['totals']['totalTokens'], 140)
                self.assertEqual(file_hashes(root), before)


if __name__ == '__main__':
    unittest.main()
