"""Behavioral checks for the offline engineering commands; all trace data is synthetic."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).parents[2] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
SPEC = importlib.util.spec_from_file_location('engineering', SCRIPTS / 'engineering.py')
engineering = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(engineering)


class EngineeringTests(unittest.TestCase):
    def setUp(self):
        parent = engineering.storage.diagnostics(engineering.ROOT, create=True).parent / 'tool-test-tmp'
        if engineering.storage.linked(parent):
            self.fail('Test temporary root must not be linked')
        parent.mkdir(exist_ok=True, mode=0o700)
        temporary = tempfile.TemporaryDirectory(prefix='engineering-', dir=parent)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / 'AGENTS.md').write_text('Synthetic workspace')

    def repository(self):
        def git(*args):
            return subprocess.run(['git', *args], cwd=self.root, check=True,
                                  capture_output=True, timeout=10).stdout.decode().strip()
        git('init', '--initial-branch=main')
        git('config', 'user.name', 'Synthetic Tool Test')
        git('config', 'user.email', 'tool@example.invalid')
        git('add', 'AGENTS.md')
        git('-c', 'core.hooksPath=', 'commit', '-m', 'Synthetic initial source')
        return git

    def test_docs_and_tooling_avoid_application_builds_but_storage_keeps_native_gate(self):
        docs = engineering.plan(['docs/guide.md'])
        self.assertEqual([c['id'] for c in docs['commands']], ['whitespace'])
        tooling = engineering.plan(['scripts/engineering.py'])
        self.assertEqual([c['id'] for c in tooling['commands']], ['whitespace', 'python_tools'])
        readme = engineering.plan(['crates/openusage-storage/README.md'])
        self.assertEqual([c['id'] for c in readme['commands']], ['whitespace'])
        rust = engineering.plan(['crates/openusage-storage/src/backup.rs'])
        command = next(c['argv'] for c in rust['commands'] if c['id'] == 'rust_tests')
        self.assertIn('openusage-storage', command)
        self.assertNotIn('--workspace', command)
        self.assertTrue(rust['manualGates'])
        renderer = engineering.plan(['apps/desktop/src/main.tsx'], ['tests/renderer/overview.spec.ts'])
        self.assertIn('renderer_targeted', [c['id'] for c in renderer['commands']])
        self.assertNotIn('renderer_milestone', [c['id'] for c in renderer['commands']])
        full = engineering.plan(['apps/desktop/src/main.tsx'], milestone=True)
        self.assertEqual([c['id'] for c in full['commands']], ['whitespace', 'renderer_milestone'])

    def test_real_git_includes_staged_unstaged_untracked_deleted_and_base_changes(self):
        git = self.repository()
        base = git('rev-parse', 'HEAD')
        (self.root / 'tracked.md').write_text('Original')
        git('add', 'tracked.md')
        git('-c', 'core.hooksPath=', 'commit', '-m', 'Synthetic tracked source')
        (self.root / 'AGENTS.md').write_text('Changed source')
        git('add', 'AGENTS.md')
        (self.root / 'tracked.md').unlink()
        (self.root / 'untracked.md').write_text('Synthetic new source')
        self.assertEqual(engineering.changed_paths(self.root), ['AGENTS.md', 'tracked.md', 'untracked.md'])
        self.assertEqual(engineering.changed_paths(self.root, staged=True), ['AGENTS.md'])
        self.assertEqual(engineering.changed_paths(self.root, base), ['AGENTS.md', 'untracked.md'])

    def test_trace_never_returns_content_and_does_not_sum_cumulative_tokens(self):
        secret = 'SYNTHETIC-PRIVATE-MESSAGE-AND-TOKEN'
        path = self.root / 'fixture.jsonl'
        records = [
            {'type': 'response_item', 'timestamp': '2026-10-22T00:00:00Z', 'payload': {'type': 'function_call', 'name': 'exec', 'arguments': 'cargo test --workspace ' + secret}},
            {'type': 'response_item', 'timestamp': '2026-10-22T00:00:04Z', 'payload': {'type': 'function_call', 'name': 'exec', 'arguments': 'cargo test --workspace ' + secret}},
            {'type': 'event_msg', 'payload': {'type': 'token_count', 'info': {'total_token_usage': {'total_tokens': 100}}}},
            {'type': 'token_usage_record', 'payload': {'thread_token_usage': {'total_tokens': 150}}},
            {'type': 'response_item', 'payload': {'type': 'function_call_output', 'output': {'exit_code': 1, 'secret': secret}}},
        ]
        path.write_text('\n'.join(json.dumps(r) for r in records) + '\nmalformed\n')
        result = engineering.trace(path)
        self.assertNotIn(secret, json.dumps(result))
        self.assertNotIn(str(path), json.dumps(result))
        self.assertEqual(result['metrics']['repeatedExactCalls'], 1)
        self.assertEqual(result['metrics']['failureSignalOutputs'], 1)
        self.assertEqual(result['metrics']['malformedRecords'], 1)
        self.assertEqual(result['reportedTokenHighWatermark']['total_tokens'], 150)
        self.assertEqual(result['elapsedSpanSeconds'], 4)

    def test_trace_refuses_link_and_oversized_record(self):
        path = self.root / 'fixture.jsonl'
        path.write_bytes(b'x' * 512)
        with patch.object(engineering, 'MAX_LINE', 128), self.assertRaises(ValueError):
            engineering.trace(path)
        link = self.root / 'linked.jsonl'
        try:
            link.symlink_to(path)
        except OSError:
            self.skipTest('Symlink privilege unavailable')
        with self.assertRaises(ValueError):
            engineering.trace(link)
        (self.root / '.tools').symlink_to(self.root / 'elsewhere', target_is_directory=True)
        with self.assertRaises(ValueError):
            engineering.environment(self.root)

    def test_inherited_agent_context_does_not_become_child_token_usage(self):
        path = self.root / 'replayed.jsonl'
        records = [
            {'type': 'session_meta', 'payload': {'id': 'synthetic-child'}},
            {'type': 'session_meta', 'payload': {'id': 'synthetic-parent'}},
            {'type': 'token_usage_record', 'payload': {'thread_id': 'synthetic-parent', 'thread_token_usage': {'total_tokens': 900}}},
            {'type': 'event_msg', 'payload': {'type': 'token_count', 'info': {'total_token_usage': {'total_tokens': 999}}}},
            {'type': 'token_usage_record', 'payload': {'thread_id': 'synthetic-child', 'thread_token_usage': {'total_tokens': 150}}},
        ]
        path.write_text('\n'.join(json.dumps(r) for r in records))
        result = engineering.trace(path)
        self.assertTrue(result['containsInheritedHistory'])
        self.assertEqual(result['reportedTokenHighWatermark'], {'total_tokens': 150})

    def test_command_output_is_discarded_and_timeout_is_bounded(self):
        private = 'SYNTHETIC-RAW-CHILD-OUTPUT'
        result = engineering.execute([sys.executable, '-c', f'print({private!r})'], self.root, os.environ.copy(), 5)
        self.assertEqual(result['status'], 'passed')
        self.assertNotIn(private, json.dumps(result))
        self.assertGreater(result['outputBytes'], 0)
        result = engineering.execute([sys.executable, '-c', 'import time; time.sleep(30)'], self.root, os.environ.copy(), .1)
        self.assertEqual(result['status'], 'timeout')
        self.assertLess(result['elapsedSeconds'], 8)

    @unittest.skipIf(os.name == 'nt', 'Linux/macOS process-group assertion; Windows tree termination needs CI')
    def test_timeout_stops_descendants_without_waiting_indefinitely(self):
        marker = self.root / 'child-survived.txt'
        child = 'import time; from pathlib import Path; time.sleep(1); Path(' + repr(str(marker)) + ').write_text("survived")'
        parent = 'import subprocess, sys, time; subprocess.Popen([sys.executable,"-c",' + repr(child) + ']); time.sleep(30)'
        result = engineering.execute([sys.executable, '-c', parent], self.root, os.environ.copy(), .2)
        self.assertEqual(result['status'], 'timeout')
        time.sleep(1.1)
        self.assertFalse(marker.exists())

    def test_failed_run_stops_after_first_check_and_retention_stays_bounded(self):
        self.repository()
        args = type('Args', (), {'base': None, 'staged': False, 'renderer_test': [], 'milestone': False, 'run': True, 'timeout': 10})()
        (self.root / 'scripts').mkdir()
        (self.root / 'scripts/test.py').write_text('# Synthetic tooling source\n')
        for _ in range(5):
            with patch.object(engineering, 'execute', return_value={'status': 'failed', 'elapsedSeconds': 0}) as call:
                result = engineering.verify(self.root, args)
                self.assertEqual(call.call_count, 1)
                self.assertEqual(result['status'], 'failed')
        directory = engineering.storage.diagnostics(self.root)
        runs = [p for p in directory.iterdir() if p.is_dir()]
        self.assertEqual(len(runs), 3)
        self.assertLess(sum(p.stat().st_size for d in runs for p in d.iterdir()), 16384)

    def test_missing_renderer_selection_and_tracked_private_files_fail_before_execution(self):
        git = self.repository()
        args = type('Args', (), {'base': None, 'staged': False, 'renderer_test': [], 'milestone': False, 'run': True, 'timeout': 10})()
        renderer = self.root / 'apps/desktop/src'
        renderer.mkdir(parents=True)
        (renderer / 'main.tsx').write_text('// Synthetic source')
        with patch.object(engineering, 'execute') as call, self.assertRaises(ValueError):
            engineering.verify(self.root, args)
        call.assert_not_called()
        (self.root / '.gitignore').write_text('auth.json\n')
        (self.root / 'auth.json').write_text('Synthetic forbidden artifact without credentials')
        git('add', '--force', 'auth.json')
        args.run = False
        with self.assertRaises(ValueError):
            engineering.verify(self.root, args)


if __name__ == '__main__':
    unittest.main()
