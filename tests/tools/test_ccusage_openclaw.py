"""Record pinned OpenClaw reports on private JSONL and SQLite fixtures."""
from contextlib import closing
import json
import os
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest

from local_log_qualification import KINDS, collect, pinned_binary, report
from test_ccusage_formats import file_hashes


GOLDEN = Path(__file__).parent / 'fixtures/openclaw/qualification.json'
CHAT = 'agents/main/sessions/session-a.jsonl'
DATABASE = 'agents/main/agent/openclaw-agent.sqlite'
TIME = 1780000000000
USAGE = {'input': 100, 'output': 20, 'cacheRead': 10, 'cacheWrite': 5, 'totalTokens': 135}
BASE = {'id': 'message-a', 'parentId': None, 'timestamp': '2026-05-28T20:26:40.000Z',
        'type': 'message', 'message': {'role': 'assistant', 'api': 'openai-responses',
        'content': [{'type': 'text', 'text': 'Synthetic reply'}], 'stopReason': 'stop',
        'model': 'gpt-5.2', 'provider': 'openai', 'timestamp': TIME, 'usage': USAGE}}
SCHEMA = ('CREATE TABLE transcript_events(session_id TEXT NOT NULL, seq INTEGER NOT NULL, '
          'event_json TEXT NOT NULL, created_at INTEGER NOT NULL, PRIMARY KEY(session_id,seq))')


def database_rows(rows):
    with closing(sqlite3.connect(':memory:')) as connection:
        connection.execute(SCHEMA)
        for session, seq, event, created in rows:
            connection.execute('INSERT INTO transcript_events VALUES(?,?,?,?)',
                               (session, seq, json.dumps(event) if isinstance(event, dict) else event, created))
        connection.commit()
        return connection.serialize()


def database(*events, session='session-a'):
    return database_rows([(session, seq, event, TIME) for seq, event in enumerate(events)])


def cases():
    def message(**values):
        return BASE | {'message': BASE['message'] | values}

    def usage(**values):
        return message(usage=USAGE | values)

    def lines(*events):
        return ''.join(json.dumps(event) + '\n' for event in events).encode()

    def single(*events, path=CHAT):
        return {'first': {path: lines(*events)}}

    def peers(first, second, path=CHAT):
        return {'first': {path: lines(*first)}, 'second': {path: lines(*second)}}

    def sql(*events):
        return {'first': {DATABASE: database(*events)}}

    def case(roots, total, *, undated=False, rows=None, fields=None):
        return {'roots': roots, 'required': {'daily': None if undated else total, 'session': total},
                'rows': rows or {}, 'fields': fields or {}}

    distinct = BASE | {'id': 'message-b', 'parentId': BASE['id']}
    conflict = usage(input=117, totalTokens=152)
    extra = usage(input=12, output=5, cacheRead=0, cacheWrite=0, totalTokens=17) | {'id': 'message-new', 'parentId': BASE['id']}
    no_id = {k: v for k, v in BASE.items() if k != 'id'}
    no_model = message()
    no_model['message'].pop('model')
    no_time = message()
    no_time['message'].pop('timestamp')
    no_time.pop('timestamp')
    mixed = {CHAT: lines(BASE), DATABASE: database(BASE)}
    with closing(sqlite3.connect(':memory:')) as connection:
        connection.execute('CREATE TABLE unrelated(value TEXT)')
        wrong_schema = connection.serialize()
    return {
        'json_single': case(single(BASE), 135),
        'json_copied_root': case(peers([BASE], [BASE]), 135),
        'json_replay': case(single(BASE, BASE), 135),
        'json_distinct_ids_equal_values': case(single(BASE, distinct), 270),
        'json_distinct_later_time': case(single(BASE, distinct | {'message': distinct['message'] | {'timestamp': TIME + 1}}), 270),
        'json_renamed_copy': case({'first': {CHAT: lines(BASE)},
                                  'second': {CHAT.replace('session-a', 'renamed'): lines(BASE)}}, 135),
        'json_fork_with_new_call': case({'first': {CHAT: lines(BASE)},
                                       'second': {CHAT.replace('session-a', 'fork'): lines(BASE, extra)}}, 152),
        'json_conflict': case(peers([BASE], [conflict]), None),
        'json_conflict_reversed': case(peers([conflict], [BASE]), None),
        'json_reset_archive': case({'first': {CHAT: lines(BASE), CHAT + '.reset.2026-05-28': lines(BASE)}}, 135),
        'json_compaction_notice': case(single(BASE, {'type': 'compaction', 'id': 'summary',
            'parentId': BASE['id'], 'timestamp': '2026-05-28T20:26:41.000Z',
            'summary': 'Synthetic summary', 'firstKeptEntryId': BASE['id'], 'tokensBefore': 135}), 135),
        'json_explicit_zero': case(single(usage(**dict.fromkeys(USAGE, 0))), 0, rows=dict.fromkeys(KINDS, 1)),
        'json_total_only': case(single(message(usage={'totalTokens': 135})), 135, fields={'outputTokens': None}),
        'json_missing_model': case(single(no_model), 135, fields={'totalCost': None}),
        'json_missing_time': case(single(no_time), 135, undated=True),
        'json_unpriced_remainder': case(single(usage(totalTokens=145)), 145, fields={'totalCost': None}),
        'json_available_usage': case(single(usage(usageReport={'state': 'available'})), 135),
        'json_unavailable_usage': case(single(usage(usageReport={'state': 'unavailable'})), None),
        'json_unavailable_cache': case(single(usage(input=115, cacheRead=0, cacheWrite=0,
            usageReport={'state': 'available'}, cacheTelemetry={'state': 'unavailable'})), 135,
            fields={'inputTokens': None, 'cacheReadTokens': None, 'cacheCreationTokens': None}),
        'json_missing_id_ambiguous_copy': case(peers([no_id], [no_id]), None),
        'json_negative_count': case(single(usage(input=-1)), None),
        'json_fractional_count': case(single(usage(input=100.5)), None),
        'json_text_count': case(single(usage(input='100')), None),
        'json_contradictory_total': case(single(usage(totalTokens=1)), None),
        'json_malformed_peer': case({'first': {CHAT: lines(BASE)}, 'second': {CHAT: b'{\n'}}, None),
        'json_malformed_line': case({'first': {CHAT: lines(BASE) + b'{\n'}}, None),
        'json_invalid_utf8_peer': case({'first': {CHAT: lines(BASE)}, 'second': {CHAT: b'\xff\xfe'}}, None),
        'missing_root': case(single(BASE) | {'missing': None}, None),
        'empty_json': case(single(), 0, rows=dict.fromkeys(KINDS, 0)),
        'sqlite_single': case(sql(BASE), 135),
        'sqlite_copied_root': case({'first': {DATABASE: database(BASE)}, 'second': {DATABASE: database(BASE)}}, 135),
        'sqlite_replay': case(sql(BASE, BASE), 135),
        'sqlite_distinct_ids_equal_values': case(sql(BASE, distinct), 270),
        'sqlite_conflict': case(sql(BASE, conflict), None),
        'sqlite_conflict_reversed': case(sql(conflict, BASE), None),
        'sqlite_root_conflict': case({'first': {DATABASE: database(BASE)}, 'second': {DATABASE: database(conflict)}}, None),
        'sqlite_root_conflict_reversed': case({'first': {DATABASE: database(conflict)}, 'second': {DATABASE: database(BASE)}}, None),
        'sqlite_missing_id': case(sql(no_id), None),
        'sqlite_fork_with_new_call': case({'first': {DATABASE: database_rows([
            ('session-a', 0, BASE, TIME), ('fork', 0, BASE, TIME), ('fork', 1, extra, TIME)])}}, 152),
        'sqlite_distinct_agents': case({'first': {DATABASE: database(BASE),
                            DATABASE.replace('main', 'independent'): database(distinct)}}, 270),
        'sqlite_distinct_sessions': case({'first': {DATABASE: database_rows([
            ('session-a', 0, BASE, TIME), ('independent', 0, distinct, TIME)])}}, 270),
        'sqlite_malformed_row': case(sql(BASE, '{'), None),
        'sqlite_invalid_peer': case({'first': {CHAT: lines(BASE), DATABASE: b'not SQLite'}}, None),
        'sqlite_missing_table_peer': case({'first': {CHAT: lines(BASE), DATABASE: wrong_schema}}, None),
        'sqlite_missing_usage_time': case({'first': {DATABASE: database_rows([('session-a', 0, no_time, 0)])}}, 135, undated=True),
        'sqlite_explicit_zero': case(sql(usage(**dict.fromkeys(USAGE, 0))), 0, rows=dict.fromkeys(KINDS, 1)),
        'sqlite_total_only': case(sql(message(usage={'totalTokens': 135})), 135, fields={'outputTokens': None}),
        'sqlite_unavailable_usage': case(sql(usage(usageReport={'state': 'unavailable'})), None),
        'empty_sqlite': case(sql(), 0, rows=dict.fromkeys(KINDS, 0)),
        'mixed_migrated_copy': case({'first': mixed}, 135),
        'mixed_distinct_ids_equal_values': case({'first': mixed | {DATABASE: database(distinct)}}, 270),
        'mixed_same_id_conflict': case({'first': mixed | {DATABASE: database(conflict)}}, None),
    }


class OpenClawQualificationTests(unittest.TestCase):
    def setUp(self):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected or sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            self.skipTest('Set OPENUSAGE_CCUSAGE_BIN on Linux with Bubblewrap.')
        self.binary = pinned_binary(selected)

    def test_pinned_outcomes_and_limits(self):
        observed = collect(self.binary, cases(), 'openclaw', 'OPENCLAW_DIR', CHAT)
        self.assertEqual(observed, json.loads(GOLDEN.read_text()))
        self.assertEqual(observed['json_fork_with_new_call']['reports']['session']['totals']['totalTokens'], 287)
        self.assertEqual(observed['sqlite_distinct_ids_equal_values']['reports']['session']['totals']['totalTokens'], 135)

    def test_committed_wal_and_uncommitted_write(self):
        with tempfile.TemporaryDirectory(prefix='openusage-openclaw-wal-') as temporary:
            root = Path(temporary)
            path = root / 'first' / DATABASE
            path.parent.mkdir(parents=True)
            with closing(sqlite3.connect(path)) as connection:
                connection.execute('PRAGMA journal_mode=WAL')
                connection.execute('PRAGMA wal_autocheckpoint=0')
                connection.execute(SCHEMA)
                connection.execute('INSERT INTO transcript_events VALUES(?,?,?,?)',
                                   ('session-a', 0, json.dumps(BASE), TIME))
                connection.commit()
                self.assertGreater(path.with_name(path.name + '-wal').stat().st_size, 32)
                connection.execute('UPDATE transcript_events SET event_json=?',
                                   (json.dumps(BASE | {'message': BASE['message'] | {'usage': USAGE | {'input': 117, 'totalTokens': 152}}}),))
                before = file_hashes(root)
                for kind in KINDS:
                    result = report(self.binary, root, ['first'], kind, 'openclaw', 'OPENCLAW_DIR')
                    self.assertEqual(result.returncode, 0)
                    self.assertEqual(json.loads(result.stdout)['totals']['totalTokens'], 135)
                self.assertEqual(file_hashes(root), before)


if __name__ == '__main__':
    unittest.main()
