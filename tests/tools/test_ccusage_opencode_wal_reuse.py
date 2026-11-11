"""Check ordinary reused WAL files and coherent committed frontiers."""
from contextlib import closing
import json
from pathlib import Path
import sqlite3
import tempfile

import test_ccusage_opencode_wal as wal_checks
from test_ccusage_formats import file_hashes


class OpenCodeWalReuseCandidateTests(wal_checks.OpenCodeWalCandidateTests):
    CANDIDATE_ENV = 'OPENUSAGE_CCUSAGE_OPENCODE_WAL_REUSE_CANDIDATE'

    def reused(self, selected, size=4096, rounds=1):
        database = self.database(selected, size)
        self.addCleanup(database.close)
        self.insert(database, self.base())
        database.execute('CREATE TABLE synthetic_padding (data BLOB)')
        database.execute('INSERT INTO synthetic_padding VALUES (?)', (b'x' * (32 * size),))
        wal = selected / 'opencode.db-wal'
        original_length = wal.stat().st_size
        for index in range(rounds):
            checkpoint = database.execute('PRAGMA wal_checkpoint(PASSIVE)').fetchone()
            self.assertEqual(checkpoint[0], 0)
            self.assertEqual(checkpoint[1], checkpoint[2])
            record = self.base()
            total = 190 + index * 17
            record['tokens'] = {'input': total - 5, 'output': 5, 'cache': {'read': 0, 'write': 0}, 'total': total}
            database.execute('UPDATE message SET data=?', (json.dumps(record),))
            self.assertEqual(wal.stat().st_size, original_length)
            content = wal.read_bytes()
            frame_size = size + 24
            self.assertTrue(any(frame[8:16] != content[16:24]
                                for frame in (content[offset:offset + frame_size]
                                              for offset in range(32, len(content), frame_size))))
        with closing(sqlite3.connect(f'file:{selected / "opencode.db"}?mode=ro', uri=True)) as oracle:
            stored = json.loads(oracle.execute('SELECT data FROM message').fetchone()[0])
            self.assertEqual(stored['tokens']['total'], total)
        return database, total

    def test_repeated_reuse_matches_the_sqlite_reader(self):
        for size in (512, 4096, 65536):
            with self.subTest(page_size=size), tempfile.TemporaryDirectory(prefix='openusage-opencode-reuse-') as temporary:
                root = Path(temporary)
                selected = root / 'selected'
                selected.mkdir()
                database, total = self.reused(selected, size, rounds=4)
                try:
                    self.exact(root, [selected], total)
                finally:
                    database.close()

    def test_reuse_with_uncommitted_spill_keeps_only_committed_usage(self):
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-reuse-spill-') as temporary:
            root = Path(temporary)
            selected = root / 'selected'
            selected.mkdir()
            database, total = self.reused(selected, 512)
            try:
                before_spill = (selected / 'opencode.db-wal').read_bytes()
                database.execute('PRAGMA cache_size=1')
                database.execute('PRAGMA cache_spill=ON')
                database.execute('BEGIN IMMEDIATE')
                for index in range(6):
                    database.execute('INSERT INTO message VALUES (?,?,?)',
                                     (f'uncommitted-{index}', 'session-z', '{"private-marker":' + 'x' * 2048))
                self.assertEqual(database.execute('SELECT COUNT(*) FROM message').fetchone()[0], 7)
                spilled = (selected / 'opencode.db-wal').read_bytes()
                self.assertNotEqual(spilled, before_spill)
                self.assertTrue(any(spilled[offset + 8:offset + 16] != spilled[16:24]
                                    for offset in range(32, len(spilled), 512 + 24)))
                self.exact(root, [selected], total)
                database.execute('ROLLBACK')
                self.exact(root, [selected], total)
            finally:
                database.close()

    def test_missing_or_inconsistent_reuse_frontier_refuses(self):
        for fault in ('missing-shm', 'one-header', 'stale-headers', 'current-frame', 'torn-tail'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory(prefix='openusage-opencode-reuse-frontier-') as temporary:
                root = Path(temporary)
                selected = root / 'selected'
                selected.mkdir()
                database, _ = self.reused(selected)
                try:
                    shm = selected / 'opencode.db-shm'
                    wal = selected / 'opencode.db-wal'
                    if fault == 'missing-shm':
                        shm.unlink()
                    elif fault == 'one-header':
                        content = bytearray(shm.read_bytes())
                        content[16] ^= 1
                        shm.write_bytes(content)
                    elif fault == 'stale-headers':
                        previous = shm.read_bytes()[:96]
                        record = self.base()
                        record['tokens'] = {'input': 202, 'output': 5, 'cache': {'read': 0, 'write': 0}, 'total': 207}
                        database.execute('UPDATE message SET data=?', (json.dumps(record),))
                        content = shm.read_bytes()
                        shm.write_bytes(previous + content[96:])
                    elif fault == 'current-frame':
                        content = bytearray(wal.read_bytes())
                        content[32 + 24 + 40] ^= 0x40
                        wal.write_bytes(content)
                    else:
                        with wal.open('ab') as stream:
                            stream.write(b'\x01')
                    before = file_hashes(root)
                    for kind in ('daily', 'session'):
                        self.capture(root, [selected], kind, unavailable=True)
                    self.assertEqual(file_hashes(root), before)
                finally:
                    database.close()
