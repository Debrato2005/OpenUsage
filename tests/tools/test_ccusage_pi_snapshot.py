"""Pi snapshot candidate checks with explicit digests and synthetic sources."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, PIN, ROOT, file_hashes, make_source
from test_ccusage_pi import header, message
import test_ccusage_pi_identity as identity
import test_ccusage_pi_format as formats
import test_ccusage_pi_filesystem as filesystem


class SnapshotViews:
    def __init__(self, test):
        self.test = test

    def __getattr__(self, name):
        return getattr(self.test, name)

    def candidate_report(self, root, command, roots, expected):
        report = self.test.snapshot_report(root, roots, expected)
        if expected is None:
            return report
        return report['daily' if command[0] == 'daily' else 'session']


def capture_snapshots(test):
    reports = {}
    for case in ['baseline', 'total_only', 'unattributed', 'empty', 'malformed']:
        with tempfile.TemporaryDirectory(prefix='openusage-pi-snapshot-') as name:
            root = Path(name)
            make_source(root / 'not-selected', test.inputs['zcode'])
            folder = root / 'sessions'
            folder.mkdir(mode=0o700)
            record = message()
            expected = 162
            if case == 'total_only':
                record['message']['usage'] = {'totalTokens': 162}
            elif case == 'unattributed':
                del record['message']['model']
            if case == 'empty':
                expected = 0
            else:
                (folder / 'root.jsonl').write_text('\n'.join(map(json.dumps, [header(), record])) + '\n')
            if case == 'malformed':
                (folder / 'broken.jsonl').write_text('{"private-marker":')
                expected = None
            before = file_hashes(root)
            reports[case] = test.snapshot_report(root, [folder], expected)
            test.assertEqual(file_hashes(root), before)
    return reports


class PiSnapshotCandidateTests(filesystem.PiFilesystemCandidateTests):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_PI_SNAPSHOT_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CCUSAGE_PI_SNAPSHOT_CANDIDATE for this local candidate.')
        cls.binary = Path(selected).resolve(strict=True)
        if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != os.environ.get('OPENUSAGE_CCUSAGE_PI_SNAPSHOT_CANDIDATE_SHA256'):
            raise ValueError('Snapshot candidate digest does not match the explicit expected SHA-256.')
        cls.reference = ROOT / 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu'
        if hashlib.sha256(cls.reference.read_bytes()).hexdigest() != json.loads(PIN.read_text())['executableSha256']:
            raise ValueError('The reference does not match the production pin.')
        cls.inputs = json.loads((FIXTURES / 'inputs.json').read_text())
        cls.expected = json.loads((FIXTURES / 'expected.json').read_text())

    def snapshot_report(self, root, roots, expected):
        with tempfile.TemporaryDirectory(prefix='empty-home-', dir=root) as home:
            result = subprocess.run([str(self.binary), 'pi', 'snapshot', '--json', '--offline',
                                     '--mode', 'calculate', '--timezone', 'UTC', '--no-color'],
                                    cwd=home, env={'HOME': home, 'TZ': 'UTC', 'NO_COLOR': '1', 'LOG_LEVEL': '0',
                                                   'PI_AGENT_DIR': ','.join(map(str, roots))},
                                    stdin=subprocess.DEVNULL, capture_output=True, timeout=20)
            self.assertEqual(list(Path(home).iterdir()), [])
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
            self.assertEqual(set(report), {'schemaVersion', 'tool', 'daily', 'session'})
            self.assertEqual(report['schemaVersion'], 1)
            self.assertEqual(report['tool'], 'pi')
            for kind in ['daily', 'session']:
                self.assertEqual(report[kind]['totals']['totalTokens'], expected)
            return report

    def test_recorded_snapshot_envelopes(self):
        self.assertEqual(capture_snapshots(self), json.loads((FIXTURES / 'pi-snapshot-candidate.json').read_text()))

    def test_snapshot_keeps_prior_identity_format_and_filesystem_outcomes(self):
        view = SnapshotViews(self)
        expected = json.loads((FIXTURES / 'pi-identity-candidate.json').read_text())
        expected['missing_details'] = json.loads((FIXTURES / 'pi-format-candidate.json').read_text())['assistant_total_only']
        self.assertEqual(identity.collect_identity(view), expected)
        self.assertEqual(formats.collect_formats(view), json.loads((FIXTURES / 'pi-format-candidate.json').read_text()))
        self.assertEqual(filesystem.collect_filesystem(view), json.loads((FIXTURES / 'pi-filesystem-candidate.json').read_text()))


if __name__ == '__main__':
    unittest.main()
