"""ZCode ledger qualification with private synthetic databases only."""
from contextlib import closing
import json
from pathlib import Path
import shutil
import sqlite3
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, FormatComparisons, file_hashes, make_source


BASE = {
    'id': 'usage-a', 'session_id': 'session-a', 'started_at': 1780000000000,
    'model_id': 'GLM-5.3', 'provider_id': 'builtin:zai-coding-plan',
    'status': 'completed', 'input_tokens': 100, 'output_tokens': 20,
    'cache_creation_input_tokens': 5, 'cache_read_input_tokens': 40,
    'computed_total_tokens': 130,
}
OPTIONAL = {'provider_id', 'cache_creation_input_tokens', 'cache_read_input_tokens',
            'computed_total_tokens'}


def cases():
    extra = BASE | {'id': 'usage-new', 'session_id': 'fork-session', 'input_tokens': 12,
                    'output_tokens': 5, 'cache_creation_input_tokens': 0,
                    'cache_read_input_tokens': 0, 'computed_total_tokens': 17}
    compact = extra | {'id': 'usage-compact', 'session_id': 'session-a', 'input_tokens': 39,
                       'output_tokens': 10, 'computed_total_tokens': 49}
    conflict = BASE | {'input_tokens': 130, 'computed_total_tokens': 160}
    return {
        'baseline': {'roots': [[BASE]], 'expected': 130, 'observed': 130},
        'copied': {'roots': [[BASE], [BASE]], 'expected': 130, 'observed': 130},
        'copied_with_new_usage': {'roots': [[BASE], [BASE, extra]], 'expected': 147, 'observed': 147},
        'forked_transcript': {'roots': [[BASE]], 'transcript': True, 'expected': 130, 'observed': 130},
        'fork_with_new_usage': {'roots': [[BASE, extra]], 'transcript': True, 'expected': 147, 'observed': 147},
        'replayed_update': {'roots': [[BASE, BASE]], 'expected': 130, 'observed': 130},
        'compacted': {'roots': [[BASE, compact]], 'transcript': True, 'expected': 179, 'observed': 179},
        'conflicting_id_first': {'roots': [[BASE], [conflict]], 'expected': None, 'observed': 130},
        'conflicting_id_reversed': {'roots': [[conflict], [BASE]], 'expected': None, 'observed': 160},
        'unknown_price': {'roots': [[BASE | {'model_id': 'openusage-unpriced-model'}]],
                          'expected': 130, 'observed': 130},
        'custom_provider': {'roots': [[BASE | {'provider_id': 'synthetic-custom-provider'}]],
                            'expected': 130, 'observed': 130},
        'legacy_columns': {'roots': [[BASE]], 'legacy': True, 'expected': None, 'observed': 120},
        'missing_time': {'roots': [[BASE | {'started_at': None}]], 'expected': 130, 'observed': 0},
        'missing_project': {'roots': [[BASE]], 'no_session': True, 'expected': 130, 'observed': 130},
        'empty_project': {'roots': [[BASE]], 'directory': '', 'expected': 130, 'observed': 130},
        'invalid_database_peer': {'roots': [[BASE], None], 'expected': None, 'observed': 130},
        'unsupported_schema_peer': {'roots': [[BASE], 'unsupported'], 'expected': None, 'observed': 130},
        'noncompleted': {'roots': [[BASE | {'status': 'running'}]], 'expected': None, 'observed': 0},
        'empty': {'roots': [[]], 'expected': 0, 'observed': 0},
    }


def database(path, rows, case):
    with closing(sqlite3.connect(path)) as db, db:
        if rows == 'unsupported':
            db.execute('CREATE TABLE unrelated (id TEXT)')
            return
        columns = [name for name in BASE if not case.get('legacy') or name not in OPTIONAL]
        text_columns = {'id', 'session_id', 'model_id', 'provider_id', 'status'}
        definitions = [f'{name} {"TEXT" if name in text_columns else "INTEGER"}'
                       + (' PRIMARY KEY' if name == 'id' else '') for name in columns]
        db.execute(f'CREATE TABLE model_usage ({",".join(definitions)})')
        db.execute('CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT)')
        if not case.get('legacy'):
            db.execute('ALTER TABLE session ADD COLUMN version TEXT')
        if not case.get('no_session'):
            for session in {'session-a', 'fork-session'}:
                db.execute('INSERT INTO session(id,directory) VALUES(?,?)',
                           (session, case.get('directory', '/synthetic/project')))
        if case.get('transcript'):
            # Fork/compaction transcript records are separate from usage ledger facts.
            db.execute('CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, data TEXT)')
            for identifier, session in [('original', 'session-a'), ('fork-copy', 'fork-session'),
                                        ('summary', 'session-a'), ('replayed-user', 'session-a')]:
                db.execute('INSERT INTO message VALUES(?,?,?)',
                           (identifier, session, json.dumps({'synthetic': True})))
        updates = ','.join(f'{name}=excluded.{name}' for name in columns if name != 'id')
        for row in rows:
            db.execute(f'INSERT INTO model_usage VALUES({",".join("?" for _ in columns)}) '
                       f'ON CONFLICT(id) DO UPDATE SET {updates}', [row[name] for name in columns])


def collect(test):
    reports = {}
    for name, case in cases().items():
        with test.subTest(case=name), tempfile.TemporaryDirectory(prefix='openusage-zcode-') as temporary:
            root = Path(temporary)
            make_source(root / 'not-selected', test.inputs['pi'])
            folders = []
            for index, rows in enumerate(case['roots']):
                folder = root / str(index)
                (folder / 'cli/db').mkdir(parents=True, mode=0o700)
                folders.append(folder)
                path = folder / 'cli/db/db.sqlite'
                if name == 'copied' and index == 1:
                    shutil.copyfile(folders[0] / 'cli/db/db.sqlite', path)
                elif rows is None:
                    path.write_bytes(b'Synthetic invalid database.\n')
                else:
                    database(path, rows, case)
            before = file_hashes(root)
            reports[name] = {}
            for kind in ('daily', 'session'):
                report = FormatComparisons.run_report(
                    test, root, 'zcode', {'variable': 'ZCODE_HOME'}, ','.join(map(str, folders)), kind)
                test.assertEqual(report['totals']['totalTokens'], case['observed'])
                reports[name][kind] = report
            test.assertEqual(file_hashes(root), before)
    return reports


class ZcodeQualificationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        FormatComparisons.setUpClass.__func__(cls)

    def test_recorded_reports_and_known_qualification_gaps(self):
        self.assertEqual(collect(self), json.loads((FIXTURES / 'zcode-qualification.json').read_text()))


if __name__ == '__main__':
    unittest.main()
