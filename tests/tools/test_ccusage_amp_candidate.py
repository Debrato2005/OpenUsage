"""Candidate checks use synthetic logs and an explicit binary hash."""
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, PIN, ROOT, FormatComparisons, file_hashes, make_source


class AmpCandidateTests(FormatComparisons, unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CCUSAGE_CANDIDATE to test a local candidate.')
        cls.binary = Path(selected).resolve(strict=True)
        digest = hashlib.sha256(cls.binary.read_bytes()).hexdigest()
        if digest != os.environ.get('OPENUSAGE_CCUSAGE_CANDIDATE_SHA256'):
            raise ValueError('The candidate must match OPENUSAGE_CCUSAGE_CANDIDATE_SHA256.')
        cls.reference = ROOT / 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu'
        pin = json.loads(PIN.read_text())
        if hashlib.sha256(cls.reference.read_bytes()).hexdigest() != pin['executableSha256']:
            raise ValueError('The reference binary must match the production pin.')
        cls.inputs = json.loads((FIXTURES / 'inputs.json').read_text())
        cls.expected = json.loads((FIXTURES / 'expected.json').read_text())
        cls.invocations = 0

    @classmethod
    def tearDownClass(cls):
        print(f'Candidate comparison: {cls.invocations} scanner invocations.', flush=True)

    def run_report(self, *args):
        type(self).invocations += 1
        return super().run_report(*args)

    def run_cli(self, root, arguments, variable, folders, *, binary=None, json_output=True, single=False, fail=False):
        with tempfile.TemporaryDirectory(prefix='empty-home-', dir=root) as empty:
            command = [str(binary or self.binary), *arguments, '--offline', '--mode', 'calculate',
                       '--timezone', 'UTC', '--no-color']
            if json_output:
                command.append('--json')
            if single:
                command.append('--single-thread')
            type(self).invocations += 1
            result = subprocess.run(command, cwd=empty,
                                    env={'HOME': empty, 'TZ': 'UTC', 'NO_COLOR': '1',
                                         variable: ','.join(str(folder) for folder in folders)},
                                    stdin=subprocess.DEVNULL, capture_output=True, timeout=20)
            self.assertLess(len(result.stdout), 128 * 1024)
            self.assertEqual(list(Path(empty).iterdir()), [])
            if fail:
                self.assertNotEqual(result.returncode, 0, arguments)
                self.assertEqual(result.stdout.strip(), b'')
                self.assertIn(b'Amp', result.stderr)
                self.assertNotIn(b'private-content', result.stderr)
                return None
            self.assertEqual(result.returncode, 0, (arguments, result.stderr[:1000]))
            return json.loads(result.stdout) if json_output else result.stdout

    def test_existing_enabled_tools_match_the_reference_binary(self):
        with tempfile.TemporaryDirectory(prefix='openusage-candidate-enabled-') as temporary:
            root = Path(temporary)
            logs = root / 'logs'
            fixtures = ROOT / 'crates/openusage-locallogs/tests/fixtures'
            shutil.copytree(fixtures / 'home/.claude', logs / '.claude')
            shutil.copytree(fixtures / 'codex', logs / '.codex')
            before = file_hashes(logs)
            for tool, variable in [('claude', 'CLAUDE_CONFIG_DIR'), ('codex', 'CODEX_HOME')]:
                commands = [['daily', '--by-agent'], [tool, 'session']]
                if tool == 'claude':
                    commands.append(['blocks', '-n', '1'])
                for arguments in commands:
                    expected = self.run_cli(root, arguments, variable, [logs / f'.{tool}'], binary=self.reference)
                    self.assertEqual(self.run_cli(root, arguments, variable, [logs / f'.{tool}']), expected)
            self.assertEqual(file_hashes(logs), before)

    def test_amp_copies_overlap_and_conflicts_through_the_cli(self):
        commands = [['daily', '--by-agent'], ['amp', 'daily'],
                    ['amp', 'monthly'], ['amp', 'session']]
        with tempfile.TemporaryDirectory(prefix='openusage-candidate-amp-') as temporary:
            root = Path(temporary)
            folder = make_source(root, self.inputs['amp'])
            source = folder / 'threads/thread-z.json'
            raw = json.loads(source.read_text())
            originals = {}
            for arguments in commands:
                for json_output in [False, True]:
                    expected = self.run_cli(root, arguments, 'AMP_DATA_DIR', [folder],
                                            binary=self.reference, json_output=json_output)
                    self.assertEqual(self.run_cli(root, arguments, 'AMP_DATA_DIR', [folder],
                                                  json_output=json_output), expected)
                    originals[(tuple(arguments), json_output)] = expected
            duplicate = folder / 'threads/copy.json'
            shutil.copyfile(source, duplicate)
            other = root / 'other'
            shutil.copytree(folder, other)
            before = file_hashes(root)
            for arguments in commands:
                for single in [False, True]:
                    actual = self.run_cli(root, arguments, 'AMP_DATA_DIR', [folder, other], single=single)
                    self.assertEqual(actual, originals[(tuple(arguments), True)])
                self.assertEqual(self.run_cli(root, arguments, 'AMP_DATA_DIR', [folder, other], json_output=False),
                                 originals[(tuple(arguments), False)])
            self.assertEqual(file_hashes(root), before)

            newer = copy.deepcopy(raw)
            extra = copy.deepcopy(newer['usageLedger']['events'][0])
            extra['id'] = 'event-next'
            newer['usageLedger']['events'].append(extra)
            duplicate.write_text(json.dumps(newer))
            before = file_hashes(root)
            for arguments in commands:
                actual = self.run_cli(root, arguments, 'AMP_DATA_DIR', [folder, other])
                self.assertEqual(actual['totals']['totalTokens'], 324)
            self.assertEqual(file_hashes(root), before)

            conflict = copy.deepcopy(raw)
            conflict['usageLedger']['events'][0]['credits'] = 3
            duplicate.write_text(json.dumps(conflict))
            before = file_hashes(root)
            for arguments in commands:
                for folders in [[folder, other], [other, folder]]:
                    self.run_cli(root, arguments, 'AMP_DATA_DIR', folders, fail=True)
            self.assertEqual(file_hashes(root), before)

            anonymous = copy.deepcopy(raw)
            del anonymous['usageLedger']['events'][0]['id']
            source.write_text(json.dumps(anonymous))
            duplicate.write_text(json.dumps(anonymous))
            before = file_hashes(root)
            for arguments in commands:
                self.run_cli(root, arguments, 'AMP_DATA_DIR', [folder], fail=True)
            self.assertEqual(file_hashes(root), before)

    def test_amp_incomplete_sources_fail_without_partial_output(self):
        commands = [['daily', '--by-agent'], ['amp', 'daily'],
                    ['amp', 'monthly'], ['amp', 'session']]
        with tempfile.TemporaryDirectory(prefix='openusage-candidate-errors-') as temporary:
            root = Path(temporary)
            folder = make_source(root, self.inputs['amp'])
            valid = folder / 'threads/thread-z.json'
            unknown = json.loads(valid.read_text())
            del unknown['id']
            bad = folder / 'threads/incomplete.json'
            for content in ['{', '[', '"private-content"', json.dumps(unknown)]:
                bad.write_text(content)
                before = file_hashes(root)
                for arguments in commands:
                    for single in [False, True]:
                        self.run_cli(root, arguments, 'AMP_DATA_DIR', [folder], single=single, fail=True)
                self.assertEqual(file_hashes(root), before)
            bad.write_text('{"title":"metadata"}')
            for arguments in commands:
                actual = self.run_cli(root, arguments, 'AMP_DATA_DIR', [folder])
                self.assertEqual(actual['totals']['totalTokens'], 162)
            bad.unlink()

    def test_amp_invalid_explicit_roots_fail_without_partial_output(self):
        with tempfile.TemporaryDirectory(prefix='openusage-candidate-roots-') as temporary:
            root = Path(temporary)
            folder = make_source(root, self.inputs['amp'])
            ordinary_file = root / 'ordinary-file'
            ordinary_file.write_text('not a directory')
            before = file_hashes(root)
            for invalid in [root / 'missing', ordinary_file]:
                for arguments in [['daily', '--by-agent'], ['amp', 'session']]:
                    for folders in [[folder, invalid], [invalid, folder]]:
                        self.run_cli(root, arguments, 'AMP_DATA_DIR', folders, fail=True)
            self.assertEqual(file_hashes(root), before)

    @unittest.skipUnless(os.name == 'posix' and os.geteuid() != 0, 'Requires Unix permission denial.')
    def test_amp_unreadable_sources_fail_without_partial_output(self):
        commands = [['daily', '--by-agent'], ['amp', 'session']]
        with tempfile.TemporaryDirectory(prefix='openusage-candidate-permissions-') as temporary:
            root = Path(temporary)
            folder = make_source(root, self.inputs['amp'])
            valid = folder / 'threads/thread-z.json'
            nested = folder / 'threads/archive'
            nested.mkdir()
            newer = nested / 'newer.json'
            shutil.copyfile(valid, newer)
            before = file_hashes(root)
            for denied in [newer, nested]:
                old_mode = denied.stat().st_mode
                denied.chmod(0)
                try:
                    self.assertFalse(os.access(denied, os.R_OK))
                    for arguments in commands:
                        for single in [False, True]:
                            for json_output in [False, True]:
                                self.run_cli(root, arguments, 'AMP_DATA_DIR', [folder], single=single,
                                             json_output=json_output, fail=True)
                finally:
                    denied.chmod(old_mode)
                self.assertEqual(file_hashes(root), before)


if __name__ == '__main__':
    unittest.main()
