import json
from pathlib import PurePosixPath
import unittest

from test_ccusage_codex_candidate import digest
from test_codex_producer import CAPTURE, ROOT

FIXTURE = ROOT / 'crates/openusage-locallogs/tests/fixtures/codex-snapshot-candidate.json'
CONTRACT = json.loads(FIXTURE.read_text())


class CodexSnapshotFixtureTests(unittest.TestCase):
    def test_fixture_matches_the_selected_scanner_capture_and_generator(self):
        pin = json.loads((ROOT / 'tests/tools/fixtures/codex-snapshot-candidate/pin.json').read_text())
        self.assertEqual(CONTRACT['scannerSha256'], pin['executableSha256'])
        self.assertEqual(CONTRACT['scannerVersion'], pin['version'])
        self.assertEqual(CONTRACT['producerCaptureSha256'], CAPTURE['receiptSha256'])
        self.assertEqual(CONTRACT['generatorSha256'], digest(ROOT / 'tests/tools/codex_snapshot_fixture.py'))

    def test_fixture_contains_only_report_projections_and_source_fingerprints(self):
        self.assertEqual(set(CONTRACT), {'scannerSha256', 'scannerVersion', 'producerCaptureSha256',
                                         'generatorSha256', 'cases'})
        encoded = json.dumps(CONTRACT)
        for key in ('base_instructions', 'developer_instructions', 'content', 'text',
                    'last_agent_message', 'preview', 'prompt', 'messages', 'body', 'input'):
            self.assertNotIn('"' + key + '"', encoded)
        for case in CONTRACT['cases'].values():
            self.assertEqual(set(case), {'snapshot', 'sourceFiles', 'expectedTotalTokens', 'stdoutSha256'})
            self.assertEqual(set(case['snapshot']), {'schemaVersion', 'tool', 'daily', 'session'})
            for record in case['sourceFiles']:
                self.assertEqual(set(record), {'path', 'sha256'})
                path = PurePosixPath(record['path'])
                self.assertFalse(path.is_absolute())
                self.assertNotIn('..', path.parts)
                self.assertEqual(path.suffix, '.jsonl')
                self.assertRegex(record['sha256'], r'\A[0-9a-f]{64}\Z')


if __name__ == '__main__':
    unittest.main()
