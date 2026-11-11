"""Qualify the pinned OpenCode reports with private JSON and SQLite fixtures."""
import copy
from contextlib import closing
import hashlib
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, FormatComparisons, file_hashes, make_source


def cases():
    fixture = json.loads((FIXTURES / 'inputs.json').read_text())['opencode']
    base = copy.deepcopy(next(iter(fixture['files'].values())))

    def changed(**fields):
        return dict(copy.deepcopy(base), **fields)

    def value(total):
        return {'input': total - 5, 'output': 5, 'cache': {'read': 0, 'write': 0}, 'total': total}

    def case(roots, required):
        return {'roots': roots, 'required': required}

    conflict = changed(tokens=value(190))
    no_id = {key: item for key, item in base.items() if key != 'id'}
    no_time = {key: item for key, item in base.items() if key != 'time'}
    no_model = {key: item for key, item in base.items() if key != 'modelID'}
    no_provider = {key: item for key, item in base.items() if key != 'providerID'}
    summary = changed(id='summary', summary=True, tokens=value(49))
    extra = changed(id='new-message', tokens=value(17))
    return {
        'json': case([{'json': [base]}], 173),
        'json_copy': case([{'json': [base]}, {'json': [base]}], 173),
        'json_replay': case([{'json': [base, base]}], 173),
        'distinct_calls': case([{'json': [base, changed(id='another-message')]}], 346),
        'conflict_first': case([{'json': [base]}, {'json': [conflict]}], None),
        'conflict_reversed': case([{'json': [conflict]}, {'json': [base]}], None),
        'missing_id_copy': case([{'json': [no_id]}, {'json': [no_id]}], None),
        'missing_time': case([{'json': [no_time]}], 173),
        'missing_model': case([{'json': [no_model]}], 173),
        'missing_provider': case([{'json': [no_provider]}], 173),
        'missing_total': case([{'json': [changed(tokens={'input': 120})]}], None),
        'reported_zero': case([{'json': [changed(tokens=value(5) | {'input': 0, 'output': 0, 'total': 0})]}], 0),
        'summary_usage': case([{'json': [base, summary]}], 222),
        'malformed_json_peer': case([{'json': [base, '@malformed']}], None),
        'invalid_utf8_peer': case([{'json': [base, '@utf8']}], None),
        'database': case([{'db': [base]}], 173),
        'database_copy': case([{'db': [base]}, {'db': [base]}], 173),
        'database_and_json_copy': case([{'db': [base], 'json': [base]}], 173),
        'database_and_json_conflict': case([{'db': [base], 'json': [conflict]}], None),
        'database_replay': case([{'db': [base, base]}], 173),
        'malformed_database_row': case([{'db': [base, '@malformed']}], None),
        'invalid_database_peer': case([{'json': [base]}, {'invalidDb': True}], None),
        'missing_explicit_root': case([{'json': [base]}, {'missing': True}], None),
        'fork_through': case([{'v2': [base, extra], 'boundary': 'through'}], 190),
        'fork_missing_boundary': case([{'v2': [base, extra], 'boundary': 'missing'}], None),
        'empty': case([{}], 0),
    }


def populate(folder, spec):
    for index, record in enumerate(spec.get('json', [])):
        path = folder / 'storage/message/session-z' / f'copy-{index}.json'
        path.parent.mkdir(parents=True, exist_ok=True)
        if record == '@malformed':
            path.write_bytes(b'{"private-marker":')
        elif record == '@utf8':
            path.write_bytes(b'\xff\xfe')
        else:
            path.write_text(json.dumps(record) + '\n')
    database = folder / 'opencode.db'
    if spec.get('invalidDb'):
        database.write_bytes(b'Synthetic invalid database.\n')
    if 'db' in spec:
        with closing(sqlite3.connect(database)) as db, db:
            db.execute('CREATE TABLE message (id TEXT, session_id TEXT, data TEXT)')
            for index, record in enumerate(spec['db']):
                identifier = record.get('id', f'bad-{index}') if isinstance(record, dict) else f'bad-{index}'
                payload = json.dumps(record) if isinstance(record, dict) else '{'
                db.execute('INSERT INTO message VALUES (?,?,?)', (identifier, 'session-z', payload))
    if 'v2' in spec:
        base, extra = spec['v2']
        with closing(sqlite3.connect(database)) as db, db:
            db.execute('CREATE TABLE session_message (id TEXT PRIMARY KEY, session_id TEXT, type TEXT, seq INTEGER, time_created INTEGER, data TEXT)')
            db.execute('CREATE TABLE session_v2 (id TEXT PRIMARY KEY, fork_session_id TEXT, fork_boundary TEXT)')
            for identifier, session, seq, record in [('parent-message', 'parent', 0, base),
                                                      ('fork-copy', 'fork', 0, base),
                                                      ('fork-own', 'fork', 1, extra)]:
                payload = {key: item for key, item in record.items()
                           if key not in ('id', 'sessionID', 'type', 'role')}
                db.execute('INSERT INTO session_message VALUES (?,?,?,?,?,?)',
                           (identifier, session, 'assistant', seq, base['time']['created'], json.dumps(payload)))
            boundary = {'type': 'through', 'messageID': 'parent-message' if spec['boundary'] == 'through' else 'missing'}
            db.execute('INSERT INTO session_v2 VALUES (?,?,?)', ('fork', 'parent', json.dumps(boundary)))


def collect(test):
    observations = {}
    for name, spec in cases().items():
        with test.subTest(case=name), tempfile.TemporaryDirectory(prefix='openusage-opencode-') as temporary:
            root = Path(temporary)
            make_source(root / 'not-selected', test.inputs['pi'])
            folders = []
            for index, source in enumerate(spec['roots']):
                folder = root / f'root-{index}'
                if not source.get('missing'):
                    folder.mkdir(mode=0o700)
                    populate(folder, source)
                folders.append(folder)
            before = file_hashes(root)
            results = {}
            for kind in ('daily', 'session'):
                report = FormatComparisons.run_report(test, root, 'opencode', {'variable': 'OPENCODE_DATA_DIR'},
                                                     ','.join(map(str, folders)), kind)
                rows = report['daily' if kind == 'daily' else 'sessions']
                key = 'period' if kind == 'daily' else 'sessionId'
                results[kind] = {'tokens': report['totals']['totalTokens'],
                                 'rows': [[row[key], row['totalTokens']] for row in rows],
                                 'sha256': hashlib.sha256(json.dumps(report, sort_keys=True).encode()).hexdigest()}
            test.assertEqual(file_hashes(root), before)
            qualified = spec['required'] is not None and all(
                item['tokens'] == spec['required'] for item in results.values())
            if name == 'reported_zero':
                qualified = qualified and all(len(item['rows']) == 1 for item in results.values())
            observations[name] = {'requiredTokens': spec['required'], 'reports': results,
                                  'qualified': qualified}
    return observations


class OpenCodeQualificationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        FormatComparisons.setUpClass.__func__(cls)

    def test_recorded_reports_and_qualification_limits(self):
        result = collect(self)
        self.assertEqual(result, json.loads((FIXTURES / 'opencode-qualification.json').read_text()))
        self.assertTrue(result['json_copy']['qualified'])
        self.assertFalse(result['conflict_first']['qualified'])
        self.assertFalse(result['malformed_json_peer']['qualified'])
        self.assertFalse(result['reported_zero']['qualified'])
        self.assertTrue(result['missing_time']['qualified'])


if __name__ == '__main__':
    unittest.main()
