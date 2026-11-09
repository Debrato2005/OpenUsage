"""A separate local Pi candidate, checked by digest with synthetic data only."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, PIN, ROOT, FormatComparisons, file_hashes
from test_ccusage_pi import TIME, collect


CHANGED = {'compaction_usage': 211, 'branch_summary_usage': 211,
           'cache_warming_usage': 179, 'unknown_usage_kind': 179}
COMMANDS = [['daily', '--by-agent'], ['weekly', '--by-agent'],
            ['pi', 'daily'], ['pi', 'monthly'], ['pi', 'session']]


def unattributed_reports(test):
    with tempfile.TemporaryDirectory(prefix='openusage-pi-total-') as temporary:
        root = Path(temporary)
        folder = root / 'sessions'
        (folder / 'project-a').mkdir(parents=True)
        record = {'type': 'usage', 'id': 'aaaaaaaa', 'parentId': None, 'timestamp': TIME,
                  'kind': 'synthetic-operation', 'usage': {'totalTokens': 17}}
        (folder / 'project-a/root.jsonl').write_text(json.dumps(record) + '\n')
        before = file_hashes(root)
        reports = {kind: test.run_report(root, 'pi', {'variable': 'PI_AGENT_DIR'}, folder, kind)
                   for kind in ('daily', 'session')}
        test.assertEqual(file_hashes(root), before)
        return reports


def report_views(test):
    with tempfile.TemporaryDirectory(prefix='openusage-pi-views-') as temporary:
        root = Path(temporary)
        folder = root / 'sessions'
        (folder / 'project-a').mkdir(parents=True)
        record = {'type': 'usage', 'id': 'aaaaaaaa', 'parentId': None, 'timestamp': TIME,
                  'kind': 'cache_warm', 'model': 'gpt-5', 'provider': 'openai',
                  'usage': {'input': 12, 'output': 5, 'cacheRead': 0, 'cacheWrite': 0, 'totalTokens': 17}}
        (folder / 'project-a/root.jsonl').write_text(json.dumps(record) + '\n')
        before = file_hashes(root)
        views = {}
        for command in COMMANDS:
            report = test.run_cli(root, command, 'PI_AGENT_DIR', folder)
            test.assertEqual(report['totals']['totalTokens'], 17)
            views[' '.join(command)] = {
                'json': report,
                'table': test.run_cli(root, command, 'PI_AGENT_DIR', folder, json_output=False)}
        test.assertEqual(file_hashes(root), before)
        return views


class PiUsageCandidateTests(FormatComparisons, unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_PI_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CCUSAGE_PI_CANDIDATE for the local Pi candidate.')
        cls.binary = Path(selected).resolve(strict=True)
        if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != os.environ.get('OPENUSAGE_CCUSAGE_PI_CANDIDATE_SHA256'):
            raise ValueError('Pi candidate digest does not match the explicit expected SHA-256.')
        cls.reference = ROOT / 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu'
        if hashlib.sha256(cls.reference.read_bytes()).hexdigest() != json.loads(PIN.read_text())['executableSha256']:
            raise ValueError('The reference binary does not match the production pin.')
        cls.inputs = json.loads((FIXTURES / 'inputs.json').read_text())
        cls.expected = json.loads((FIXTURES / 'expected.json').read_text())

    def run_cli(self, root, arguments, variable, folder, *, binary=None, json_output=True):
        with tempfile.TemporaryDirectory(prefix='empty-home-', dir=root) as empty:
            command = [str(binary or self.binary), *arguments, '--offline', '--mode', 'calculate',
                       '--timezone', 'UTC', '--no-color']
            if json_output:
                command.append('--json')
            result = subprocess.run(command, cwd=empty,
                                    env={'HOME': empty, 'TZ': 'UTC', 'NO_COLOR': '1',
                                         'LOG_LEVEL': '0', 'COLUMNS': '200', variable: str(folder)},
                                    stdin=subprocess.DEVNULL, capture_output=True, timeout=20)
            self.assertEqual(result.returncode, 0, (arguments, result.stderr[:1000]))
            self.assertLess(len(result.stdout), 128 * 1024)
            self.assertEqual(list(Path(empty).iterdir()), [])
            return json.loads(result.stdout) if json_output else result.stdout.decode()

    def test_only_the_qualified_usage_cases_change(self):
        reports = collect(self)
        baseline = json.loads((FIXTURES / 'pi-qualification.json').read_text())
        candidate = json.loads((FIXTURES / 'pi-usage-candidate.json').read_text())
        for name, report in reports.items():
            if name in CHANGED:
                self.assertEqual(report, candidate[name])
                for kind in ('daily', 'session'):
                    self.assertEqual(report[kind]['totals']['totalTokens'], CHANGED[name])
            else:
                self.assertEqual(report, baseline[name], name)
        self.assertEqual(unattributed_reports(self), candidate['unattributed'])
        candidate_binary = self.binary
        try:
            self.binary = self.reference
            reference = unattributed_reports(self)
        finally:
            self.binary = candidate_binary
        self.assertEqual(reference['daily']['daily'], [])
        self.assertEqual(reference['session']['sessions'], [])

    def test_json_and_table_views_preserve_the_new_usage(self):
        self.assertEqual(report_views(self), json.loads((FIXTURES / 'pi-usage-candidate-cli.json').read_text()))

    def test_enabled_tools_still_match_the_reference(self):
        with tempfile.TemporaryDirectory(prefix='openusage-pi-enabled-') as temporary:
            root = Path(temporary)
            fixtures = ROOT / 'crates/openusage-locallogs/tests/fixtures'
            shutil.copytree(fixtures / 'home/.claude', root / '.claude')
            shutil.copytree(fixtures / 'codex', root / '.codex')
            before = file_hashes(root)
            for tool, variable in [('claude', 'CLAUDE_CONFIG_DIR'), ('codex', 'CODEX_HOME')]:
                commands = [['daily', '--by-agent'], [tool, 'session']]
                if tool == 'claude':
                    commands.append(['blocks', '-n', '1'])
                for command in commands:
                    expected = self.run_cli(root, command, variable, root / f'.{tool}', binary=self.reference)
                    self.assertEqual(self.run_cli(root, command, variable, root / f'.{tool}'), expected)
            self.assertEqual(file_hashes(root), before)


if __name__ == '__main__':
    unittest.main()
