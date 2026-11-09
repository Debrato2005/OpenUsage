"""Explicit local identity candidate; private synthetic sources only."""
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
from test_ccusage_pi import TIME, cases, header, message
import test_ccusage_pi_candidate as usage_candidate


UNAVAILABLE = {'legacy_copy', 'conflicting_entry', 'malformed_lineage', 'missing_parent',
               'missing_time', 'malformed_peer', 'invalid_utf8_peer'}


def identity_cases():
    result = cases()
    for name in UNAVAILABLE:
        result[name]['expected'] = None
    root = 'first/sessions/project-a/root.jsonl'
    for name, rows, total in [
        ('headerless', [message()], None),
        ('version_one', [header() | {'version': 1}, message()], None),
        ('version_two', [header() | {'version': 2}, message()], 162),
        ('unknown_version', [header() | {'version': 4}, message()], None),
        ('unattributed', [header(), {'type': 'usage', 'id': 'aaaaaaaa', 'parentId': None,
                                   'timestamp': TIME, 'kind': 'synthetic', 'usage': {'totalTokens': 17}}], 17),
        ('absent_project', [{key: value for key, value in header().items() if key != 'cwd'}, message()], 162),
    ]:
        result[name] = {'files': {root: rows}, 'expected': total}
    for field in ('provider', 'model', 'cost', 'missing_category', 'zero'):
        changed = copy.deepcopy(message())
        if field in ('provider', 'model'):
            changed['message'][field] = 'synthetic-conflict'
        elif field == 'cost':
            changed['message']['usage']['cost'] = {'total': 0}
        elif field == 'zero':
            changed['message']['usage'] = {'totalTokens': 0}
        else:
            del changed['message']['usage']['cacheRead']
        result['conflict_' + field] = {'files': {root: [header(), message(), changed]}, 'expected': None}
    return result


def collect_identity(test):
    reports = {}
    for name, case in identity_cases().items():
        with test.subTest(case=name), tempfile.TemporaryDirectory(prefix='openusage-pi-identity-') as temporary:
            root = Path(temporary)
            make_source(root / 'not-selected', test.inputs['zcode'])
            for relative, rows in case['files'].items():
                path = root / relative
                path.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
                if rows == ['@invalid-utf8']:
                    path.write_bytes(b'\xff\xfe\n')
                elif rows == ['@malformed']:
                    path.write_text('{"private-marker":')
                else:
                    path.write_text(('\n'.join(json.dumps(row) for row in rows)
                                     .replace('@parent', str(root / 'first/sessions/project-a/root.jsonl'))) + '\n')
            if target := case.get('copy_to'):
                target = root / target
                target.parent.mkdir(parents=True, mode=0o700)
                shutil.copyfile(root / 'first/sessions/project-a/root.jsonl', target)
            roots = [root / 'first' / ('custom' if case.get('custom') else 'sessions')]
            if (root / 'second').exists():
                roots.append(root / 'second/sessions')
            before = file_hashes(root)
            reports[name] = {}
            for kind, command in [('daily', ['daily', '--by-agent']), ('session', ['pi', 'session'])]:
                report = test.candidate_report(root, command, roots, case['expected'])
                reports[name][kind] = report
                if len(roots) > 1:
                    test.assertEqual(test.candidate_report(root, command, list(reversed(roots)), case['expected']), report)
            test.assertEqual(file_hashes(root), before)
    return reports


class PiIdentityCandidateTests(unittest.TestCase):
    run_cli = usage_candidate.PiUsageCandidateTests.run_cli
    run_report = FormatComparisons.run_report
    test_enabled_tools_still_match_the_reference = usage_candidate.PiUsageCandidateTests.test_enabled_tools_still_match_the_reference
    test_empty_roots_have_no_rows_and_qwen_has_null_session_totals = FormatComparisons.test_empty_roots_have_no_rows_and_qwen_has_null_session_totals

    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_PI_IDENTITY_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CCUSAGE_PI_IDENTITY_CANDIDATE for the local identity candidate.')
        cls.binary = Path(selected).resolve(strict=True)
        if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != os.environ.get('OPENUSAGE_CCUSAGE_PI_IDENTITY_CANDIDATE_SHA256'):
            raise ValueError('Identity candidate digest does not match the explicit expected SHA-256.')
        cls.reference = ROOT / 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu'
        if hashlib.sha256(cls.reference.read_bytes()).hexdigest() != json.loads(PIN.read_text())['executableSha256']:
            raise ValueError('The reference does not match the production pin.')
        cls.inputs = json.loads((FIXTURES / 'inputs.json').read_text())
        cls.expected = json.loads((FIXTURES / 'expected.json').read_text())

    def candidate_report(self, root, command, roots, expected):
        with tempfile.TemporaryDirectory(prefix='empty-home-', dir=root) as empty:
            result = subprocess.run([str(self.binary), *command, '--json', '--offline', '--mode', 'calculate',
                                     '--timezone', 'UTC', '--no-color'], cwd=empty,
                                    env={'HOME': empty, 'TZ': 'UTC', 'NO_COLOR': '1', 'LOG_LEVEL': '0',
                                         'PI_AGENT_DIR': ','.join(map(str, roots))}, stdin=subprocess.DEVNULL,
                                    capture_output=True, timeout=20)
            self.assertEqual(list(Path(empty).iterdir()), [])
            self.assertLess(len(result.stdout) + len(result.stderr), 128 * 1024)
            if expected is None:
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, b'')
                self.assertIn(b'Pi source identity', result.stderr)
                self.assertNotIn(b'private-marker', result.stderr)
                self.assertNotIn(str(root).encode(), result.stderr)
                return {'unavailable': True}
            self.assertEqual(result.returncode, 0, result.stderr[:1000])
            report = json.loads(result.stdout)
            self.assertEqual(report['totals']['totalTokens'], expected)
            return report

    def test_identity_reports_and_reversed_roots(self):
        self.assertEqual(collect_identity(self), json.loads((FIXTURES / 'pi-identity-candidate.json').read_text()))

    def test_other_fifteen_formats_match_the_production_reference(self):
        with tempfile.TemporaryDirectory(prefix='openusage-pi-other-formats-') as temporary:
            root = Path(temporary)
            folders = {tool: make_source(root, fixture) for tool, fixture in self.inputs.items()}
            before = file_hashes(root)
            for tool, fixture in self.inputs.items():
                if tool == 'pi':
                    continue
                for kind in ('daily', 'session'):
                    with self.subTest(tool=tool, report=kind):
                        self.assertEqual(self.run_report(root, tool, fixture, folders[tool], kind), self.expected[tool][kind])
            self.assertEqual(file_hashes(root), before)

    def test_unified_and_pi_views_count_standalone_usage(self):
        with tempfile.TemporaryDirectory(prefix='openusage-pi-identity-views-') as temporary:
            root = Path(temporary)
            folder = root / 'sessions'
            folder.mkdir(mode=0o700)
            rows = identity_cases()['unattributed']['files']['first/sessions/project-a/root.jsonl']
            (folder / 'root.jsonl').write_text('\n'.join(map(json.dumps, rows)) + '\n')
            before = file_hashes(root)
            for command in usage_candidate.COMMANDS:
                self.assertEqual(self.run_cli(root, command, 'PI_AGENT_DIR', folder)['totals']['totalTokens'], 17)
                self.assertIn('17', self.run_cli(root, command, 'PI_AGENT_DIR', folder, json_output=False))
            self.assertEqual(file_hashes(root), before)


if __name__ == '__main__':
    unittest.main()
