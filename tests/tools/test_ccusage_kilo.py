"""Kilo qualification evidence; known scanner gaps must not enable the tool."""
import copy
from contextlib import closing
import json
from pathlib import Path
import shutil
import sqlite3
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, FormatComparisons, file_hashes, make_source


def cases():
    base = json.loads((FIXTURES / 'inputs.json').read_text())['kilo']['databases']['kilo.db']['message']
    legacy = copy.deepcopy(base)
    legacy.pop('id')
    legacy.pop('session_id')

    def row(value=legacy, identifier='message-z', session='session-z'):
        return [identifier, session, copy.deepcopy(value)]

    fork = copy.deepcopy(legacy)
    fork['cost'] = 0
    extra = copy.deepcopy(legacy)
    extra['tokens'] = {'input': 10, 'output': 3, 'cache': {'read': 2, 'write': 1},
                       'reasoning': 1, 'total': 17}
    compact = copy.deepcopy(legacy)
    compact.update(summary=True, mode='compaction', agent='compaction')
    compact['tokens'] = {'input': 30, 'output': 10, 'cache': {'read': 2, 'write': 3},
                         'reasoning': 4, 'total': 49}
    user = {'role': 'user', 'time': {'created': 1794107045006}}
    conflict = copy.deepcopy(base)
    conflict['tokens']['input'] = 137
    conflict['tokens']['total'] = 190
    unknown = copy.deepcopy(legacy)
    unknown['modelID'] = 'openusage-synthetic-unpriced-model'
    unknown_no_charge = copy.deepcopy(unknown)
    unknown_no_charge.pop('cost')
    no_time = copy.deepcopy(legacy)
    no_time.pop('time')
    no_details = copy.deepcopy(legacy)
    no_details['tokens'] = {'total': 173}
    # Each case has an independent token oracle and the pinned scanner's observed total.
    # None means conflicting payloads cannot establish an accepted total.
    return {
        'legacy': {'roots': [[row()]], 'unique': 173, 'observed': 173},
        'copied_legacy': {'roots': [[row()], [row()]], 'unique': 173, 'observed': 346},
        'copied_embedded_id': {'roots': [[row(base)], [row(base)]], 'unique': 173, 'observed': 173},
        'forked_legacy': {'roots': [[row(), row(fork, 'fork-copy', 'fork-session')]],
                          'unique': 173, 'observed': 346},
        'fork_with_new_usage': {'roots': [[row(), row(fork, 'fork-copy', 'fork-session'),
                                          row(extra, 'new-message', 'fork-session')]],
                                'unique': 190, 'observed': 363},
        'replayed_update': {'roots': [[row(), row()]], 'unique': 173, 'observed': 173},
        'compacted': {'roots': [[row(), row(compact, 'summary'), row(user, 'compaction-marker'),
                                 row(user, 'replayed-user')]], 'unique': 222, 'observed': 222},
        'conflicting_id_first': {'roots': [[row(base)], [row(conflict)]],
                                 'unique': None, 'observed': 173},
        'conflicting_id_reversed': {'roots': [[row(conflict)], [row(base)]],
                                    'unique': None, 'observed': 190},
        'unknown_price_stored_charge': {'roots': [[row(unknown)]], 'unique': 173, 'observed': 173},
        'unknown_price_no_charge': {'roots': [[row(unknown_no_charge)]], 'unique': 173, 'observed': 173},
        'missing_time': {'roots': [[row(no_time)]], 'unique': 173, 'observed': 0},
        'missing_details': {'roots': [[row(no_details)]], 'unique': 173, 'observed': 173},
        'malformed_peer': {'roots': [[row(), ['bad-message', 'session-z', '{']]],
                           'unique': None, 'observed': 173},
        'invalid_database_peer': {'roots': [[row()], None], 'unique': None, 'observed': 173},
        'empty': {'roots': [[]], 'unique': 0, 'observed': 0},
    }


def collect(test):
    reports = {}
    for name, case in cases().items():
        with test.subTest(case=name), tempfile.TemporaryDirectory(prefix='openusage-kilo-') as temporary:
            root = Path(temporary)
            # A populated tool outside the selected roots must not enter either report.
            make_source(root / 'not-selected', test.inputs['pi'])
            folders = []
            for index, rows in enumerate(case['roots']):
                folder = root / str(index)
                folder.mkdir(mode=0o700)
                folders.append(folder)
                if index == 1 and name in ('copied_legacy', 'copied_embedded_id'):
                    shutil.copyfile(folders[0] / 'kilo.db', folder / 'kilo.db')
                    continue
                if rows is None:
                    (folder / 'kilo.db').write_bytes(b'Synthetic invalid database.\n')
                    continue
                with closing(sqlite3.connect(folder / 'kilo.db')) as db, db:
                    db.execute('CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, data TEXT)')
                    for identifier, session, data in rows:
                        db.execute('INSERT INTO message VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data',
                                   (identifier, session, json.dumps(data) if isinstance(data, dict) else data))
            before = file_hashes(root)
            reports[name] = {}
            for kind in ('daily', 'session'):
                report = FormatComparisons.run_report(
                    test, root, 'kilo', {'variable': 'KILO_DATA_DIR'},
                    ','.join(map(str, folders)), kind)
                test.assertEqual(report['totals']['totalTokens'], case['observed'])
                reports[name][kind] = report
            test.assertEqual(file_hashes(root), before)
    return reports


class KiloQualificationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        FormatComparisons.setUpClass.__func__(cls)

    def test_recorded_reports_and_known_qualification_gaps(self):
        actual = collect(self)
        expected = json.loads((FIXTURES / 'kilo-qualification.json').read_text())
        self.assertEqual(actual, expected)
        for name in ('copied_legacy', 'forked_legacy', 'fork_with_new_usage', 'missing_time'):
            self.assertNotEqual(cases()[name]['unique'], cases()[name]['observed'])


if __name__ == '__main__':
    unittest.main()
