import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('storage', Path(__file__).parents[2] / 'scripts/storage.py')
storage = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(storage)


class StorageTests(unittest.TestCase):
    def setUp(self):
        parent = storage.REPOSITORY_ROOT
        for name in ('.recovery', 'tool-test-tmp'):
            parent = parent / name
            if storage.linked(parent):
                self.fail('Test temporary directories must not follow links')
            parent.mkdir(mode=0o700, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix='storage-', dir=parent)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        (self.root / 'AGENTS.md').write_text('Synthetic workspace')
        (self.root / '.git').mkdir()

    def run_directory(self, name, finished=100, payload=12):
        directory = storage.diagnostics(self.root, create=True) / name
        directory.mkdir()
        (directory / 'output.log').write_bytes(b'x' * payload)
        (directory / storage.MARKER).write_text(json.dumps({
            'version': 1, 'state': 'finished',
            'finishedAt': storage.datetime.fromtimestamp(finished, storage.timezone.utc).isoformat()}))
        return directory

    def test_three_lifecycle_cycles_stay_bounded_and_active_run_is_preserved(self):
        for index in range(3):
            with storage.diagnostic_run(self.root, 'fixture', count=1) as directory:
                (directory / 'output.log').write_text('Synthetic log')
                outcome = storage.clean(self.root, apply=True, count=0)
                self.assertTrue(directory.exists())
                self.assertIn(directory.name, [row['run'] for row in outcome['preserved']])
            remaining = [p for p in storage.diagnostics(self.root).iterdir() if p.is_dir()]
            self.assertEqual(len(remaining), 1)

    def test_dry_run_then_apply_removes_oldest_only(self):
        old = self.run_directory('old', 100)
        new = self.run_directory('new', 200)
        report = storage.clean(self.root, now=250, days=7, count=1)
        self.assertEqual([row['run'] for row in report['selected']], ['old'])
        self.assertTrue(old.exists())
        storage.clean(self.root, now=250, days=7, count=1, apply=True)
        self.assertFalse(old.exists())
        self.assertTrue(new.exists())

    def test_age_and_allocated_size_limits(self):
        old = self.run_directory('old', 100)
        new = self.run_directory('new', 200)
        report = storage.clean(self.root, now=250, days=100 / 86400, count=10)
        self.assertEqual([row['run'] for row in report['selected']], ['old'])
        report = storage.clean(self.root, now=250, max_bytes=0)
        self.assertEqual([row['run'] for row in report['selected']], ['old', 'new'])
        self.assertTrue(old.exists() and new.exists())

    def test_protected_content_keep_and_pinned_marker_are_never_removed(self):
        protected = ['records.db', 'secret.key', 'auth.json', 'copy.oubackup', 'source.rs',
                     '.keep', '.git', 'records.db-journal', 'old.db3', 'old.sqlite-wal',
                     'old.sqlite3-shm', 'old.sqlite3-journal', 'worker.mjs', 'component.jsx',
                     'build.cjs', 'source.c', 'source.cpp', 'source.h', 'config.yml',
                     'config.yaml', 'notes.md', 'session.json', 'theme.css', 'index.html', 'icon.svg']
        for index, name in enumerate(protected):
            directory = self.run_directory('protected-' + str(index))
            (directory / name).write_text('Synthetic protected data')
        pinned = self.run_directory('pinned')
        marker = json.loads((pinned / storage.MARKER).read_text())
        marker['pinned'] = True
        (pinned / storage.MARKER).write_text(json.dumps(marker))
        result = storage.clean(self.root, apply=True, count=0, days=0, max_bytes=0)
        self.assertEqual(result['selected'], [])
        self.assertEqual(len(result['preserved']), len(protected) + 1)

    def test_missing_invalid_and_future_markers_are_preserved(self):
        for name in ['missing', 'invalid', 'future']:
            directory = self.run_directory(name, 99999)
            if name == 'missing':
                (directory / storage.MARKER).unlink()
            elif name == 'invalid':
                (directory / storage.MARKER).write_text('{}')
        result = storage.clean(self.root, apply=True, now=300, count=0)
        self.assertEqual(result['selected'], [])
        self.assertEqual(len(result['preserved']), 3)

    def test_concurrent_cleanup_fails_without_changes(self):
        directory = self.run_directory('owned')
        lock = storage.diagnostics(self.root) / '.cleanup.lock'
        lock.mkdir()
        with self.assertRaises(RuntimeError):
            storage.clean(self.root, apply=True, count=0)
        self.assertTrue(directory.exists())
        self.assertTrue(lock.exists())

    def test_raced_run_stops_before_delete(self):
        directory = self.run_directory('raced')
        original = storage.scan
        calls = 0
        def raced(path, **kwargs):
            nonlocal calls
            calls += 1
            if calls == 2:
                (directory / '.keep').write_text('Preserve')
            return original(path, **kwargs)
        with patch.object(storage, 'scan', side_effect=raced), self.assertRaises((RuntimeError, ValueError)):
            storage.clean(self.root, apply=True, count=0)
        self.assertTrue((directory / 'output.log').exists())
        self.assertFalse((storage.diagnostics(self.root) / '.cleanup.lock').exists())

    def test_symlink_run_and_descendant_are_preserved_without_traversal(self):
        outside = self.root / 'preserved'
        outside.mkdir()
        (outside / 'important.txt').write_text('Preserve')
        parent = storage.diagnostics(self.root, create=True)
        try:
            (parent / 'linked').symlink_to(outside, target_is_directory=True)
        except (OSError, NotImplementedError):
            self.skipTest('Symlink creation is unavailable')
        directory = self.run_directory('nested-link')
        (directory / 'link').symlink_to(outside, target_is_directory=True)
        result = storage.clean(self.root, apply=True, count=0)
        self.assertEqual(result['selected'], [])
        self.assertTrue((outside / 'important.txt').exists())
        self.assertTrue(directory.exists())

    def test_unsafe_roots_and_linked_diagnostics_are_refused(self):
        with self.assertRaises(ValueError):
            storage.clean(self.root / 'absent', apply=True)
        outside = self.root / 'preserved'
        outside.mkdir()
        try:
            (self.root / '.recovery').symlink_to(outside, target_is_directory=True)
        except (OSError, NotImplementedError):
            self.skipTest('Symlink creation is unavailable')
        with self.assertRaises(ValueError):
            storage.clean(self.root, apply=True)

    def test_roots_outside_own_repository_are_refused_before_metadata_resolution(self):
        with patch.object(Path, 'resolve', side_effect=AssertionError('Must not resolve outside root')):
            with self.assertRaises(ValueError):
                storage.workspace(storage.REPOSITORY_ROOT.parent)
            with self.assertRaises(ValueError):
                storage.workspace(storage.REPOSITORY_ROOT / '..' / 'another-project')

    def test_root_symlink_is_refused_before_resolving_target(self):
        alias = self.root / 'alias'
        try:
            alias.symlink_to(self.root, target_is_directory=True)
        except (OSError, NotImplementedError):
            self.skipTest('Symlink creation is unavailable')
        with patch.object(Path, 'resolve', side_effect=AssertionError('Must not resolve linked target')):
            with self.assertRaises(ValueError):
                storage.workspace(alias)

    def test_check_reports_allocated_bytes_and_warns_without_cleaning(self):
        directory = self.run_directory('owned')
        with patch.object(storage.shutil, 'disk_usage', return_value=storage.shutil._ntuple_diskusage(100, 99, 1)):
            report = storage.check(self.root, warn_gib=10)
        self.assertFalse(report['warning'])
        self.assertEqual(report['freeBytes'], 1)
        self.assertGreater(report['totalAllocatedBytes'], 0)
        self.assertTrue(storage.check(self.root, warn_gib=0)['warning'])
        self.assertTrue(any(row['path'] == '.recovery' and row['allocatedBytes'] > 0
                            for row in report['topDirectories']))
        self.assertTrue(directory.exists())
        for value in [-1, float('nan')]:
            with self.assertRaises(ValueError):
                storage.check(self.root, value)

    def test_nonobject_markers_are_preserved(self):
        for index, value in enumerate([[], None, 1, 'finished', True]):
            directory = self.run_directory('invalid-' + str(index))
            (directory / storage.MARKER).write_text(json.dumps(value))
        result = storage.clean(self.root, apply=True, count=0)
        self.assertEqual(result['selected'], [])
        self.assertEqual(len(result['preserved']), 5)

    def test_parent_keep_pins_all_runs(self):
        directory = self.run_directory('owned')
        (storage.diagnostics(self.root) / '.keep').write_text('Preserve')
        result = storage.clean(self.root, apply=True, count=0)
        self.assertEqual(result['selected'], [])
        self.assertTrue(directory.exists())

    def test_check_deduplicates_hardlinks_between_top_directories(self):
        a = self.root / 'a'
        b = self.root / 'b'
        a.mkdir()
        b.mkdir()
        (a / 'data').write_bytes(b'x' * 8192)
        try:
            os.link(a / 'data', b / 'data')
        except OSError:
            self.skipTest('Hardlinks are unavailable')
        report = storage.check(self.root)
        sizes = {item['path']: item['allocatedBytes'] for item in report['topDirectories']}
        self.assertEqual(sizes['b'], storage.allocated(b.stat()))

    def test_automatic_retention_preserves_completed_run_with_protected_content(self):
        with storage.diagnostic_run(self.root, 'protected', count=0) as directory:
            (directory / 'records.db').write_bytes(b'Synthetic protected bytes')
        self.assertTrue((directory / 'records.db').exists())


if __name__ == '__main__':
    unittest.main()
