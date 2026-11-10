"""Qualification of the pinned Pi writer with the unchanged observation candidate."""
import json
import os
from pathlib import Path
import platform
import shutil
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, file_hashes, make_source
import test_ccusage_pi_observations as observations
import pi_producer_support as producer
from pi_scan_schedule import scan_during_write


CASES = {
    'setup_only': (['setup_only'], 0, 0),
    'user_only': (['user_only'], 0, 0),
    'assistant': (['seed'], 120, 1),
    'append_after_reopen': (['seed', 'append'], 137, 1),
    'summaries_and_usage': (['seed', 'compact'], 203, 1),
    'optional_summary_usage': (['seed', 'optional_summary'], 120, 1),
    'inactive_branch': (['seed', 'branch'], 160, 1),
    'branched_session': (['seed', 'branched'], 191, 2),
    'fork_new_usage': (['seed', 'fork'], 151, 2),
    'reported_zero': (['zero'], 0, 1),
    'absent_categories': (['total_only'], 162, 1),
    'buffered_flush': (['flush'], 137, 1),
    'copied_producer_file': (['seed'], 120, 1),
}


def collect_producer(test):
    reports = {}
    for case, (actions, total, sessions) in CASES.items():
        with test.subTest(case=case), tempfile.TemporaryDirectory(prefix='openusage-pi-producer-') as temporary:
            root = Path(temporary)
            make_source(root / 'not-selected', test.inputs['zcode'])
            for action in actions:
                state = producer.produce(test.source, root, action)
            if case == 'copied_producer_file':
                shutil.copyfile(state['file'], root / 'sessions/renamed.jsonl')
            before = file_hashes(root)
            report = test.snapshot_report(root, [root / 'sessions'], total)
            test.assertEqual(file_hashes(root), before)
            test.assertEqual(len(report['session']['sessions']), sessions)
            test.assertEqual(len(report['daily']['daily']), int(sessions > 0))
            if sessions:
                test.assertEqual(report['daily']['daily'][0]['period'], '2026-05-28')
            reports[case] = producer.normalize_report(report, root, state)
    return reports


def collect_schedules(test):
    reports = {}
    for stage in ['before_open', 'after_read']:
        for action, total in [('append', 137), ('compact', 203), ('branch', 160),
                              ('branched', 191), ('fork', 151), ('optional_summary', 120)]:
            with test.subTest(stage=stage, action=action), tempfile.TemporaryDirectory(prefix='openusage-pi-read-') as name:
                root = Path(name)
                make_source(root / 'not-selected', test.inputs['zcode'])
                state = producer.produce(test.source, root, 'seed')
                after_write = {}

                def write():
                    after_write['state'] = producer.produce(test.source, root, action)
                    after_write['hashes'] = file_hashes(root)

                code, stdout, stderr = scan_during_write(test.binary, root, Path(state['file']), stage, write)
                test.assertNotEqual(code, 0)
                test.assertEqual(stdout, b'')
                test.assertIn(b'Pi source identity', stderr)
                test.assertNotIn(str(root).encode(), stderr)
                test.assertEqual(file_hashes(root), after_write['hashes'])
                stable = test.snapshot_report(root, [root / 'sessions'], total)
                test.assertEqual(file_hashes(root), after_write['hashes'])
                reports[stage + '_' + action] = {
                    'duringWrite': {'unavailable': True},
                    'completed': producer.normalize_report(stable, root, after_write['state']),
                }
    for stage, total in [('header', 0), ('usage', 17)]:
        with test.subTest(stage=stage), tempfile.TemporaryDirectory(prefix='openusage-pi-prefix-') as name:
            root = Path(name)
            with producer.PausedProducer(test.source, root, 'flush', stage) as child:
                before = file_hashes(root)
                prefix = test.snapshot_report(root, [root / 'sessions'], total)
                test.assertEqual(file_hashes(root), before)
                state = child.resume()
            before = file_hashes(root)
            stable = test.snapshot_report(root, [root / 'sessions'], 137)
            test.assertEqual(file_hashes(root), before)
            reports['paused_writer_' + stage] = {
                'qualified': False,
                'requiredOutcome': 'unavailable until the producer write completes',
                'observedPrefix': producer.normalize_report(prefix, root, state),
                'completed': producer.normalize_report(stable, root, state),
            }
    return reports


class PiProducerCandidateTests(observations.PiObservationCandidateTests):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_PI_PRODUCER_SOURCE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_PI_PRODUCER_SOURCE for the pinned writer checks.')
        if platform.system() != 'Linux' or not shutil.which('bwrap'):
            raise unittest.SkipTest('The producer fixture needs Linux and bubblewrap.')
        super().setUpClass()
        cls.source = producer.verify_source(selected)

    def test_pinned_writer_reports_match_independent_counts(self):
        self.assertEqual(collect_producer(self), json.loads((FIXTURES / 'pi-producer-candidate.json').read_text()))

    @unittest.skipUnless(platform.machine() == 'x86_64', 'The syscall schedule is Linux x86_64 only.')
    def test_concurrent_writes_and_known_incomplete_publication_gap(self):
        self.assertEqual(collect_schedules(self), json.loads((FIXTURES / 'pi-producer-schedules.json').read_text()))


if __name__ == '__main__':
    unittest.main()
