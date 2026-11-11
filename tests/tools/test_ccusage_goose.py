"""Record pinned Goose reports and database path limits on synthetic Linux data."""
from contextlib import closing
import json
import os
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest

from local_log_qualification import KINDS, collect, pinned_binary
from test_ccusage_formats import FIXTURES, file_hashes
from test_ccusage_opencode_runtime import checked_run, sandbox


GOLDEN = Path(__file__).parent / 'fixtures/goose/qualification.json'
DATABASE = 'data/sessions/sessions.db'
DEFAULT_DATABASES = ('.local/share/goose/sessions/sessions.db',
                     '.local/share/Block/goose/sessions/sessions.db')


def database(*changes):
    inputs = json.loads((FIXTURES / 'inputs.json').read_text())
    script = inputs['goose']['databases'][DATABASE]['sql']
    with closing(sqlite3.connect(':memory:')) as connection:
        connection.executescript(script)
        for change in changes:
            connection.execute(change)
        connection.commit()
        return connection.serialize()


def cases():
    def case(contents, tokens, *, daily=None, rows=None, fields=None):
        return {'roots': {'first': {DATABASE: contents}}, 'required': {'daily': daily, 'session': tokens},
                'rows': rows or {}, 'fields': fields or {}}

    duplicate = 'INSERT INTO sessions SELECT * FROM sessions'
    base = database()
    return {
        'single': case(base, 161),
        'unmeasured_cache': case(base, 161, fields={'cacheReadTokens': None, 'cacheCreationTokens': None}),
        'duplicate_row': case(database(duplicate), 161),
        'conflicting_rows': case(database(duplicate, 'UPDATE sessions SET accumulated_total_tokens = 179 WHERE rowid = 2'), None),
        'conflicting_rows_reversed': case(database(duplicate, 'UPDATE sessions SET accumulated_total_tokens = 179 WHERE rowid = 1'), None),
        'distinct_calls': case(database(duplicate, "UPDATE sessions SET id = 'independent' WHERE rowid = 2"), 322),
        'synthetic_inherited_fork': case(database(duplicate, "UPDATE sessions SET id = 'fork', accumulated_total_tokens = 178 WHERE rowid = 2"), 178),
        'missing_model': case(database('UPDATE sessions SET model_config_json = NULL'), 161, fields={'totalCost': None}),
        'missing_time': case(database('UPDATE sessions SET created_at = NULL'), 161),
        'bad_model_json': case(database(duplicate, "UPDATE sessions SET id = 'invalid', model_config_json = '{' WHERE rowid = 2"), None),
        'unknown_model': case(database("UPDATE sessions SET model_config_json = '{\"model_name\":\"synthetic-model-not-priced\"}'"), 161,
                              fields={'totalCost': None}),
        'independent_total_only': case(database('UPDATE sessions SET input_tokens = NULL, output_tokens = NULL, accumulated_input_tokens = NULL, accumulated_output_tokens = NULL'), 161,
                                       fields={'inputTokens': None, 'outputTokens': None}),
        'contradictory_total': case(database('UPDATE sessions SET accumulated_total_tokens = 1'), None),
        'negative_bucket': case(database('UPDATE sessions SET accumulated_input_tokens = -1'), None),
        'fractional_bucket': case(database('UPDATE sessions SET accumulated_input_tokens = 120.5'), None),
        'text_bucket': case(database("UPDATE sessions SET accumulated_input_tokens = 'private-marker'"), None),
        'absent_accumulator': case(database('UPDATE sessions SET accumulated_input_tokens = NULL, accumulated_output_tokens = NULL, accumulated_total_tokens = NULL'), 150),
        'ambiguous_zero_accumulator': case(database('UPDATE sessions SET accumulated_input_tokens = 0, accumulated_output_tokens = 0, accumulated_total_tokens = 0'), None),
        'unqualified_zero_row': case(database('UPDATE sessions SET input_tokens = 0, output_tokens = 0, total_tokens = 0, accumulated_input_tokens = 0, accumulated_output_tokens = 0, accumulated_total_tokens = 0'), None),
        'missing_column': case(database('ALTER TABLE sessions DROP COLUMN accumulated_output_tokens'), None),
        'invalid_database': case(b'not a SQLite database\n', None),
        'empty_database': case(database('DELETE FROM sessions'), 0, daily=0, rows=dict.fromkeys(KINDS, 0)),
    }


class GooseQualificationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected or sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            raise unittest.SkipTest('Set OPENUSAGE_CCUSAGE_BIN on Linux with Bubblewrap.')
        cls.binary = pinned_binary(selected)

    def test_pinned_outcomes_and_limits(self):
        observed = collect(self.binary, cases(), 'goose', 'GOOSE_PATH_ROOT', DATABASE)
        self.assertEqual(observed, json.loads(GOLDEN.read_text()))
        self.assertEqual(observed['conflicting_rows']['reports']['session']['totals']['totalTokens'], 161)
        self.assertEqual(observed['conflicting_rows_reversed']['reports']['session']['totals']['totalTokens'], 179)

    def run_report(self, mounts, kind, selected=None):
        command = sandbox() + ['--ro-bind', str(self.binary), '/runtime/ccusage']
        for source, target in mounts:
            command.extend(['--ro-bind', str(source), target])
        if selected is not None:
            command.extend(['--setenv', 'GOOSE_PATH_ROOT', selected])
        command.extend(['--setenv', 'HOME', '/fixture/home', '--chdir', '/fixture/home',
                        '/runtime/ccusage', 'goose', kind, '--json', '--offline', '--mode',
                        'calculate', '--timezone', 'UTC', '--no-color'])
        result = checked_run(command, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)

    def test_pinned_default_paths_count_copies_again(self):
        with tempfile.TemporaryDirectory(prefix='openusage-goose-defaults-') as temporary:
            home = Path(temporary)
            content = database()
            for relative in DEFAULT_DATABASES:
                path = home / relative
                path.parent.mkdir(parents=True)
                path.write_bytes(content)
            before = file_hashes(home)
            for kind in KINDS:
                report = self.run_report([(home, '/fixture/home')], kind)
                self.assertEqual(report['totals']['totalTokens'], 322)
            self.assertEqual(file_hashes(home), before)

    def test_pinned_database_symlink_reaches_unselected_decoy(self):
        with tempfile.TemporaryDirectory(prefix='openusage-goose-symlink-') as temporary:
            root = Path(temporary)
            home = root / 'home'
            home.mkdir()
            selected = root / 'selected'
            link = selected / DATABASE
            link.parent.mkdir(parents=True)
            decoy = root / 'decoy'
            decoy.mkdir()
            (decoy / 'sessions.db').write_bytes(database())
            link.symlink_to('/synthetic-decoy/sessions.db')
            before = file_hashes(decoy)
            for kind in KINDS:
                report = self.run_report([(home, '/fixture/home'), (selected, '/selected'),
                                          (decoy, '/synthetic-decoy')], kind, '/selected')
                self.assertEqual(report['totals']['totalTokens'], 161)
            self.assertEqual(file_hashes(decoy), before)
            self.assertEqual(os.readlink(link), '/synthetic-decoy/sessions.db')
            self.assertEqual(list(home.iterdir()), [])


if __name__ == '__main__':
    unittest.main()
