"""Compare the OpenCode observation candidate with the pinned production scanner."""
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, PIN, ROOT, FormatComparisons
from test_ccusage_opencode import cases, collect, populate
import test_ccusage_pi_candidate as reference_checks


CHANGED = {'missing_model': 173, 'missing_provider': 173, 'reported_zero': 0}


class OpenCodeObservationCandidateTests(FormatComparisons, unittest.TestCase):
    run_cli = reference_checks.PiUsageCandidateTests.run_cli
    test_enabled_tools_match_the_reference = reference_checks.PiUsageCandidateTests.test_enabled_tools_still_match_the_reference

    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_OPENCODE_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CCUSAGE_OPENCODE_CANDIDATE for this local candidate.')
        cls.binary = Path(selected).resolve(strict=True)
        actual = hashlib.sha256(cls.binary.read_bytes()).hexdigest()
        if actual != os.environ.get('OPENUSAGE_CCUSAGE_OPENCODE_CANDIDATE_SHA256'):
            raise ValueError('The OpenCode candidate digest does not match the selected binary.')
        cls.reference = ROOT / 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu'
        if hashlib.sha256(cls.reference.read_bytes()).hexdigest() != json.loads(PIN.read_text())['executableSha256']:
            raise ValueError('The production scanner does not match its pin.')
        cls.inputs = json.loads((FIXTURES / 'inputs.json').read_text())
        cls.expected = json.loads((FIXTURES / 'expected.json').read_text())

    def test_observations_change_only_the_three_loss_cases(self):
        baseline = json.loads((FIXTURES / 'opencode-qualification.json').read_text())
        corrected = json.loads((FIXTURES / 'opencode-observation-candidate.json').read_text())
        actual = collect(self)
        for name, report in actual.items():
            if name not in CHANGED:
                self.assertEqual(report, baseline[name], name)
                continue
            self.assertEqual(report, corrected[name], name)
            self.assertTrue(report['qualified'], name)
            for view in report['reports'].values():
                self.assertEqual(view['tokens'], CHANGED[name])
                self.assertEqual(len(view['rows']), 1)

    def test_missing_labels_keep_the_independent_total_separate(self):
        for name in ('missing_model', 'missing_provider'):
            with self.subTest(case=name), tempfile.TemporaryDirectory(prefix='openusage-opencode-counts-') as temporary:
                root = Path(temporary)
                folder = root / 'selected'
                folder.mkdir()
                populate(folder, cases()[name]['roots'][0])
                report = self.run_report(root, 'opencode', {'variable': 'OPENCODE_DATA_DIR'}, folder, 'daily')
                self.assertEqual(report['totals']['totalTokens'], 173)
                self.assertEqual(report['totals']['inputTokens'], 120)
                self.assertEqual(report['totals']['outputTokens'], 30)
                self.assertEqual(report['totals']['cacheReadTokens'], 7)
                self.assertEqual(report['totals']['cacheCreationTokens'], 5)
                if name == 'missing_model':
                    self.assertEqual(report['daily'][0]['modelsUsed'], [])


if __name__ == '__main__':
    unittest.main()
