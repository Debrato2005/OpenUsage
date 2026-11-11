"""Check committed SQLite WAL history through the explicit Linux candidate."""
from contextlib import closing
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import sqlite3
import tempfile

import test_ccusage_opencode_capture as capture_checks
from test_ccusage_formats import file_hashes


class OpenCodeWalCandidateTests(capture_checks.OpenCodeCaptureCandidateTests):
    CANDIDATE_ENV = 'OPENUSAGE_CCUSAGE_OPENCODE_WAL_CANDIDATE'
    # WAL acceptance replaces the previous layer's deliberate refusal.
    test_committed_wal_is_explicitly_unavailable = None

    def base(self):
        return copy.deepcopy(next(iter(self.inputs['opencode']['files'].values())))

    def database(self, selected, page_size=4096, name='opencode.db'):
        database = sqlite3.connect(selected / name, isolation_level=None)
        database.execute(f'PRAGMA page_size={page_size}')
        self.assertEqual(database.execute('PRAGMA journal_mode=WAL').fetchone()[0], 'wal')
        database.execute('PRAGMA wal_autocheckpoint=0')
        database.execute('CREATE TABLE message (id TEXT, session_id TEXT, data TEXT)')
        database.execute('PRAGMA wal_checkpoint(TRUNCATE)')
        return database

    def insert(self, database, record):
        database.execute('INSERT INTO message VALUES (?,?,?)',
                         (record['id'], record['sessionID'], json.dumps(record)))

    def exact(self, root, roots, tokens):
        before = file_hashes(root)
        for kind in ('daily', 'session'):
            report = self.capture(root, roots, kind)
            self.assertEqual(report['totals']['totalTokens'], tokens)
        self.assertEqual(file_hashes(root), before)

    def test_committed_wal_and_checkpointed_mode_preserve_exact_history(self):
        for size in (512, 4096, 65536):
            with self.subTest(page_size=size), tempfile.TemporaryDirectory(prefix='openusage-opencode-wal-') as temporary:
                root = Path(temporary)
                selected = root / 'selected'
                selected.mkdir()
                with closing(self.database(selected, size)) as database:
                    self.insert(database, self.base())
                    self.assertGreater((selected / 'opencode.db-wal').stat().st_size, 32)
                    # The captured main image alone has no message. The committed WAL owns it.
                    main = sqlite3.connect(f'file:{selected / "opencode.db"}?immutable=1', uri=True)
                    with closing(main):
                        self.assertEqual(main.execute('SELECT COUNT(*) FROM message').fetchone()[0], 0)
                    self.exact(root, [selected], 173)
                    database.execute('PRAGMA wal_checkpoint(TRUNCATE)')
                    self.exact(root, [selected], 173)
                self.assertFalse((selected / 'opencode.db-wal').exists())
                self.assertEqual((selected / 'opencode.db').read_bytes()[18:20], b'\x02\x02')
                self.exact(root, [selected], 173)

    def test_latest_commit_and_copied_roots_count_once(self):
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-wal-copies-') as temporary:
            root = Path(temporary)
            first, second = root / 'first', root / 'second'
            first.mkdir()
            second.mkdir()
            with closing(self.database(first)) as left, closing(self.database(second, name='opencode-beta.1.db')) as right:
                for database in (left, right):
                    record = self.base()
                    self.insert(database, record)
                    record['tokens'] = {'input': 185, 'output': 5, 'cache': {'read': 0, 'write': 0}, 'total': 190}
                    database.execute('UPDATE message SET data=?', (json.dumps(record),))
                self.exact(root, [first, second], 190)
                self.exact(root, [second, first], 190)

    def test_uncommitted_spilled_frames_never_become_usage(self):
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-wal-uncommitted-') as temporary:
            root = Path(temporary)
            selected = root / 'selected'
            selected.mkdir()
            with closing(self.database(selected, 512)) as database:
                self.insert(database, self.base())
                wal = selected / 'opencode.db-wal'
                committed_size = wal.stat().st_size
                database.execute('PRAGMA cache_size=1')
                database.execute('PRAGMA cache_spill=ON')
                database.execute('BEGIN IMMEDIATE')
                for index in range(48):
                    database.execute('INSERT INTO message VALUES (?,?,?)',
                                     (f'uncommitted-{index}', 'session-z', '{"private-marker":' + 'x' * 2048))
                self.assertGreater(wal.stat().st_size, committed_size)
                self.assertEqual(database.execute('SELECT COUNT(*) FROM message').fetchone()[0], 49)
                self.exact(root, [selected], 173)
                database.execute('ROLLBACK')
                self.exact(root, [selected], 173)

    def test_damaged_wal_refuses_whole_capture_without_source_changes(self):
        for fault in ('header', 'header-checksum', 'frame', 'torn-tail'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory(prefix='openusage-opencode-wal-damage-') as temporary:
                root = Path(temporary)
                selected = root / 'selected'
                selected.mkdir()
                with closing(self.database(selected)) as database:
                    self.insert(database, self.base())
                    wal = selected / 'opencode.db-wal'
                    content = bytearray(wal.read_bytes())
                    if fault == 'torn-tail':
                        content += b'\x01'
                    else:
                        offset = {'header': 0, 'header-checksum': 24, 'frame': 32 + 24 + 40}[fault]
                        content[offset] ^= 0x40
                    wal.write_bytes(content)
                    before = file_hashes(root)
                    for kind in ('daily', 'session'):
                        self.capture(root, [selected], kind, unavailable=True)
                    self.assertEqual(file_hashes(root), before)

    def test_pinned_producer_schema_and_message_shapes_record_limits(self):
        selected_source = os.environ.get('OPENUSAGE_OPENCODE_PRODUCER_SOURCE')
        if not selected_source:
            self.skipTest('Set OPENUSAGE_OPENCODE_PRODUCER_SOURCE for the source-schema fixture.')
        source = Path(selected_source).resolve(strict=True)
        pin = json.loads((Path(__file__).parent / 'fixtures/opencode-producer/pin.json').read_text())
        for relative, expected in pin['files'].items():
            self.assertEqual(hashlib.sha256((source / relative).read_bytes()).hexdigest(), expected, relative)
        schema = (source / 'packages/core/src/database/schema.gen.ts').read_text()
        statements = [sql.replace('\\`', '`') for sql in re.findall(r'yield\* tx\.run\(`((?:\\.|[^`])*)`\)', schema, re.S)]
        chosen = [sql for sql in statements if re.match(r'\s*CREATE TABLE `(message|session_message|session)` \(', sql)]
        self.assertEqual(len(chosen), 3)
        self.assertTrue(all('${' not in sql for sql in chosen))
        for shape in ('assistant', 'user-and-assistant', 'unfinished-assistant', 'agent-switched'):
            with self.subTest(shape=shape), tempfile.TemporaryDirectory(prefix='openusage-opencode-source-schema-') as temporary:
                root = Path(temporary)
                selected = root / 'selected'
                selected.mkdir()
                with closing(sqlite3.connect(selected / 'opencode.db', isolation_level=None)) as database:
                    self.assertEqual(database.execute('PRAGMA journal_mode=WAL').fetchone()[0], 'wal')
                    for sql in chosen:
                        database.execute(sql)
                    database.execute('INSERT INTO session (id,project_id,slug,directory,title,version,time_created,time_updated) VALUES (?,?,?,?,?,?,?,?)',
                                     ('session-z', 'synthetic-project', 'synthetic', '/synthetic', 'Synthetic', pin['tag'], 1780000000000, 1780000000000))
                    payload = {'agent': 'build', 'model': {'id': 'gpt-5', 'providerID': 'openai'},
                               'content': [], 'time': {'created': 1780000000000, 'completed': 1780000000001},
                               'cost': 0, 'tokens': {'input': 120, 'output': 30, 'reasoning': 11, 'cache': {'read': 7, 'write': 5}}}
                    if shape == 'unfinished-assistant':
                        del payload['tokens']
                        del payload['time']['completed']
                    database.execute('INSERT INTO session_message VALUES (?,?,?,?,?,?,?)',
                                     ('msg_synthetic', 'session-z', 'assistant', 1, 1780000000000, 1780000000001, json.dumps(payload)))
                    if shape in ('user-and-assistant', 'agent-switched'):
                        kind = 'user' if shape == 'user-and-assistant' else shape
                        peer = {'time': {'created': 1780000000000}}
                        peer['text' if kind == 'user' else 'agent'] = 'Synthetic' if kind == 'user' else 'build'
                        database.execute('INSERT INTO session_message VALUES (?,?,?,?,?,?,?)',
                                         ('msg_peer', 'session-z', kind, 0, 1780000000000, 1780000000000, json.dumps(peer)))
                    unavailable = shape in ('unfinished-assistant', 'agent-switched')
                    before = file_hashes(root)
                    for kind in ('daily', 'session'):
                        report = self.capture(root, [selected], kind, unavailable=unavailable)
                        if report is not None:
                            self.assertEqual(report['totals']['totalTokens'], 173)
                    self.assertEqual(file_hashes(root), before)
