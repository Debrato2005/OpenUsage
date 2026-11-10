"""Reference completion protocol checks. No producer or scanner is enabled here."""
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import unittest

import pi_producer_support as producer
from pi_publication_probe import Publication, Unavailable
import test_ccusage_pi_observations as observations
import test_ccusage_pi_snapshot as snapshots


class PiPublicationTests(unittest.TestCase):
    snapshot_report = snapshots.PiSnapshotCandidateTests.snapshot_report

    @classmethod
    def setUpClass(cls):
        source = os.environ.get('OPENUSAGE_PI_PRODUCER_SOURCE')
        if not source or platform.system() != 'Linux' or not shutil.which('bwrap'):
            raise unittest.SkipTest('The publication probe needs the explicit pinned producer and Linux sandbox.')
        observations.PiObservationCandidateTests.setUpClass.__func__(cls)
        cls.source = producer.verify_source(source)

    def test_uncommitted_prefix_is_rejected_even_when_no_writer_takes_the_lock(self):
        for pause, total in [('header', 0), ('usage', 17)]:
            with self.subTest(pause=pause), tempfile.TemporaryDirectory(prefix='pi-publication-') as temporary:
                root = Path(temporary)
                gate = Publication(root / 'published')
                with producer.PausedProducer(self.source, gate.root, 'flush', pause) as child:
                    with self.assertRaises(Unavailable):
                        gate.scan(lambda: self.snapshot_report(gate.root, [gate.sessions], total))
                    child.resume()

    def prepared(self, root, action, base=None):
        stage = root / 'stage'
        stage.mkdir(mode=0o700)
        if base is not None:
            (stage / 'sessions').mkdir(mode=0o700)
            source = stage / 'sessions/root.jsonl'
            shutil.copyfile(base, source)
            (stage / 'writer-state.json').write_text(json.dumps({'file': str(source)}))
        state = producer.produce(self.source, stage, action)
        return Path(state['file'])

    def seed(self, root, gate):
        source = self.prepared(root, 'seed')
        with gate.lease(True) as lease:
            gate.publish(lease, source, gate.current())

    def read_total(self, gate, total):
        return gate.scan(lambda: self.snapshot_report(gate.root, [gate.sessions], total))

    def test_coordinated_publication_covers_both_paused_writer_failures(self):
        for pause in ['header', 'usage']:
            with self.subTest(pause=pause), tempfile.TemporaryDirectory(prefix='pi-publication-') as name:
                root = Path(name)
                gate = Publication(root / 'published')
                stage = root / 'private-stage'
                stage.mkdir(mode=0o700)
                with gate.lease(True) as lease:
                    expected = gate.current()
                    with producer.PausedProducer(self.source, stage, 'flush', pause) as child:
                        self.assertEqual(list(gate.sessions.iterdir()), [])
                        with self.assertRaises(Unavailable):
                            self.read_total(gate, 0)
                        state = child.resume()
                    gate.publish(lease, Path(state['file']), expected)
                self.read_total(gate, 137)

    def test_raw_reader_lock_does_not_make_the_unmodified_producer_cooperate(self):
        for pause, total in [('header', 0), ('usage', 17)]:
            with self.subTest(pause=pause), tempfile.TemporaryDirectory(prefix='pi-publication-') as name:
                gate = Publication(Path(name) / 'published')
                with gate.lease(False):
                    with producer.PausedProducer(self.source, gate.root, 'flush', pause) as child:
                        self.snapshot_report(gate.root, [gate.sessions], total)
                        child.resume()
                with self.assertRaises(Unavailable):
                    self.read_total(gate, 137)

    def test_terminated_private_writer_keeps_the_prior_publication(self):
        for pause in ['header', 'usage']:
            with self.subTest(pause=pause), tempfile.TemporaryDirectory(prefix='pi-publication-') as name:
                root = Path(name)
                gate = Publication(root / 'published')
                self.seed(root, gate)
                original = gate.target.read_bytes()
                stage = root / 'private-stage'
                stage.mkdir(mode=0o700)
                with gate.lease(True):
                    with producer.PausedProducer(self.source, stage, 'flush', pause) as child:
                        with self.assertRaises(Unavailable):
                            self.read_total(gate, 120)
                    self.assertNotEqual(child.process.returncode, 0)
                self.assertEqual(gate.target.read_bytes(), original)
                self.read_total(gate, 120)

    def test_failure_before_data_replacement_keeps_the_prior_complete_file(self):
        with tempfile.TemporaryDirectory(prefix='pi-publication-') as name:
            root = Path(name)
            gate = Publication(root / 'published')
            self.seed(root, gate)
            before = gate.target.read_bytes()
            update_root = root / 'next'
            update_root.mkdir(mode=0o700)
            update = self.prepared(update_root, 'append', gate.target)
            with gate.lease(True) as lease:
                with self.assertRaisesRegex(RuntimeError, 'injected'):
                    gate.publish(lease, update, gate.current(),
                                 lambda stage: (_ for _ in ()).throw(RuntimeError('injected')) if stage == 'before_data' else None)
            self.assertEqual(gate.target.read_bytes(), before)
            self.read_total(gate, 120)

    def test_failure_after_data_replacement_requires_a_new_completion_record(self):
        for stop in ['after_data', 'after_commit']:
            with self.subTest(stop=stop), tempfile.TemporaryDirectory(prefix='pi-publication-') as name:
                root = Path(name)
                gate = Publication(root / 'published')
                self.seed(root, gate)
                update_root = root / 'next'
                update_root.mkdir(mode=0o700)
                update = self.prepared(update_root, 'append', gate.target)
                complete_bytes = update.read_bytes()
                expected = gate.current()
                code = '''import json, os, sys
from pathlib import Path
sys.path.insert(0, sys.argv[5])
from pi_publication_probe import Publication
gate = Publication.attach(sys.argv[1], json.loads(sys.argv[4]))
with gate.lease(True) as lease:
    gate.publish(lease, Path(sys.argv[2]), gate.current(), lambda phase: os._exit(87) if phase == sys.argv[3] else None)
'''
                child_home = root / 'child-home'
                child_home.mkdir(mode=0o700)
                result = subprocess.run([sys.executable, '-I', '-S', '-c', code, str(gate.root), str(update), stop,
                                         json.dumps(gate.lock_identity), str(Path(__file__).parent)],
                                        env={'PATH': os.environ['PATH'], 'HOME': str(child_home)},
                                        capture_output=True, timeout=10)
                self.assertEqual(result.returncode, 87, result.stderr)
                self.assertEqual(list(child_home.iterdir()), [])
                self.assertEqual(gate.target.read_bytes(), complete_bytes)
                if stop == 'after_data':
                    with self.assertRaises(Unavailable):
                        self.read_total(gate, 137)
                    self.assertNotEqual(gate.target.stat().st_ino, expected[1][1])
                else:
                    self.read_total(gate, 137)

    def test_empty_zero_and_unavailable_categories_remain_distinct(self):
        for action, total in [('zero', 0), ('total_only', 162)]:
            with self.subTest(action=action), tempfile.TemporaryDirectory(prefix='pi-publication-') as name:
                root = Path(name)
                gate = Publication(root / 'published')
                self.assertEqual(self.read_total(gate, 0)['session']['sessions'], [])
                source = self.prepared(root, action)
                original = source.read_bytes()
                with gate.lease(True) as lease:
                    gate.publish(lease, source, gate.current())
                self.assertEqual(gate.target.read_bytes(), original)
                report = self.read_total(gate, total)
                self.assertEqual(len(report['session']['sessions']), 1)
                if action == 'total_only':
                    self.assertEqual(report['session']['sessions'][0]['outputTokens'], 0)
                    self.assertEqual(report['session']['sessions'][0]['totalTokens'], 162)

    def test_stale_preparation_cannot_replace_a_newer_committed_update(self):
        with tempfile.TemporaryDirectory(prefix='pi-publication-') as name:
            root = Path(name)
            gate = Publication(root / 'published')
            self.seed(root, gate)
            stale = gate.current()
            first, second = root / 'first', root / 'second'
            first.mkdir(mode=0o700)
            second.mkdir(mode=0o700)
            old_update = self.prepared(first, 'append', gate.target)
            new_update = self.prepared(second, 'compact', gate.target)
            with gate.lease(True) as lease:
                gate.publish(lease, new_update, stale)
            accepted = gate.target.read_bytes()
            with gate.lease(True) as lease, self.assertRaises(Unavailable):
                gate.publish(lease, old_update, stale)
            self.assertEqual(gate.target.read_bytes(), accepted)
            self.read_total(gate, 203)

    def test_reader_lease_excludes_writer_and_unsealed_changes_remain_unavailable(self):
        with tempfile.TemporaryDirectory(prefix='pi-publication-') as name:
            root = Path(name)
            gate = Publication(root / 'published')
            self.seed(root, gate)
            with gate.lease(False):
                with self.assertRaises(Unavailable):
                    with gate.lease(True):
                        self.fail('A writer acquired the reader lease')
            original = gate.target.read_bytes()

            def mutate_after_report():
                report = self.snapshot_report(gate.root, [gate.sessions], 120)
                gate.target.write_bytes(original.splitlines(keepends=True)[0])
                return report

            with self.assertRaises(Unavailable):
                gate.scan(mutate_after_report)
            with self.assertRaises(Unavailable):
                self.read_total(gate, 0)

    def test_missing_ambiguous_extra_or_linked_publication_data_is_unavailable(self):
        for change in ['missing_seal', 'duplicate_seal', 'unknown_version', 'extra_file', 'linked_data', 'replaced_lock']:
            with self.subTest(change=change), tempfile.TemporaryDirectory(prefix='pi-publication-') as name:
                root = Path(name)
                gate = Publication(root / 'published')
                self.seed(root, gate)
                if change == 'missing_seal':
                    gate.seal.unlink()
                elif change == 'duplicate_seal':
                    gate.seal.write_text(gate.seal.read_text().replace('"schemaVersion": 1', '"schemaVersion": 1, "schemaVersion": 1'))
                elif change == 'unknown_version':
                    value = json.loads(gate.seal.read_text())
                    value['schemaVersion'] = 2
                    gate.seal.write_text(json.dumps(value))
                elif change == 'extra_file':
                    shutil.copyfile(gate.target, gate.sessions / 'other.jsonl')
                elif change == 'linked_data':
                    outside = root / 'outside.jsonl'
                    gate.target.rename(outside)
                    gate.target.symlink_to(outside)
                else:
                    gate.lock.rename(gate.control / 'old-lease')
                    gate.lock.write_bytes(b'')
                with self.assertRaises(Unavailable):
                    self.read_total(gate, 120)


if __name__ == '__main__':
    unittest.main()
