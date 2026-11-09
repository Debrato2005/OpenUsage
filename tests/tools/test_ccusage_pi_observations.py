"""Pi observation checks use explicit candidate digests and private synthetic data."""
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, PIN, ROOT, file_hashes, make_source
from test_ccusage_pi import TIME, header, message
import test_ccusage_pi_snapshot as snapshot


def observation_cases():
    cases = {'empty': ([], 0, 0), 'header_only': ([header()], 0, 0)}
    for kind in ['message', 'usage', 'compaction', 'branch_summary']:
        for variant, usage, total, rows in [
            ('zero', {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0, 'totalTokens': 0}, 0, 1),
            ('zero_total_only', {'totalTokens': 0}, 0, 1),
            ('total_only', {'totalTokens': 162}, 162, 1),
            ('partial', {'input': 100, 'totalTokens': 162}, 162, 1),
            ('absent', 'absent', None if kind in ['message', 'usage'] else 0, 0),
            ('null', None, None, 0),
            ('empty_usage', {}, None, 0),
            ('missing_total', {'input': 100, 'output': 20, 'cacheRead': 30, 'cacheWrite': 5}, None, 0),
            ('partial_no_total', {'input': 100}, None, 0),
            ('zero_no_total', {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}, None, 0),
            ('inconsistent_zero', {'input': 1, 'totalTokens': 0}, None, 0),
        ]:
            row = message()
            row['type'] = kind
            if kind != 'message':
                row.update(kind='synthetic-operation', summary='Synthetic.', fromId='aaaaaaaa',
                           firstKeptEntryId='aaaaaaaa', model='gpt-5', provider='openai')
                del row['message']
            fields = row['message'] if kind == 'message' else row
            if usage == 'absent':
                fields.pop('usage', None)
            else:
                fields['usage'] = usage
            cases[kind + '_' + variant] = ([header(), row], total, rows)

    for target in ['header', 'entry']:
        for name, value in [('absent', None), ('null', None), ('empty', ''), ('number', 1780000000000),
                            ('invalid', 'private-marker'), ('calendar', '2026-02-30T12:00:00Z'),
                            ('no_zone', '2026-05-28T20:26:40')]:
            records = [header(), message()]
            record = records[0 if target == 'header' else 1]
            if name == 'absent':
                del record['timestamp']
            else:
                record['timestamp'] = value
            # A nested request time cannot replace the required entry time.
            records[1]['message']['timestamp'] = 1780000000000
            cases[target + '_time_' + name] = (records, None, 0)

    row = message()
    row['message']['timestamp'] = 1
    cases['nested_time_is_not_entry_time'] = ([header(), row], 162, 1)
    cases['nested_time_absent'] = ([header(), message()], 162, 1)
    row = message()
    row['timestamp'] = '2026-05-29T01:56:40.000+05:30'
    cases['entry_offset'] = ([header(), row], 162, 1)
    for reason in ['stop', 'error', 'aborted']:
        records = copy.deepcopy(cases['message_zero'][0])
        records[1]['message']['stopReason'] = reason
        cases['reported_zero_' + reason] = (records, 0, 1)
    row = message()
    row['message']['role'] = 'user'
    del row['message']['usage']
    cases['non_usage_message'] = ([header(), row], 0, 0)
    records = copy.deepcopy(cases['message_zero'][0])
    del records[1]['message']['model']
    cases['zero_unattributed'] = (records, 0, 1)
    records = copy.deepcopy(cases['message_zero'][0])
    records.append(copy.deepcopy(records[1]))
    cases['zero_replay'] = (records, 0, 1)
    records = copy.deepcopy(cases['message_zero'][0])
    extra = message('bbbbbbbb', 'aaaaaaaa')
    extra['timestamp'] = '2026-05-29T20:26:40.000Z'
    records.append(extra)
    cases['zero_then_positive'] = (records, 162, 2)
    return cases


def collect_observations(test):
    reports = {}
    for case, (records, total, days) in observation_cases().items():
        with test.subTest(case=case), tempfile.TemporaryDirectory(prefix='openusage-pi-observations-') as name:
            root = Path(name)
            make_source(root / 'not-selected', test.inputs['zcode'])
            folder = root / 'sessions'
            folder.mkdir(mode=0o700)
            if records:
                (folder / 'misleading-1999-01-01.jsonl').write_text('\n'.join(map(json.dumps, records)) + '\n')
            before = file_hashes(root)
            report = test.snapshot_report(root, [folder], total)
            test.assertEqual(file_hashes(root), before)
            reports[case] = report
            if total is None:
                continue
            test.assertEqual(len(report['daily']['daily']), days)
            test.assertEqual(len(report['session']['sessions']), int(days > 0))
            if days:
                test.assertEqual(report['daily']['daily'][0]['period'], '2026-05-28')
                test.assertEqual(report['session']['sessions'][0]['firstActivity'], TIME)
                last = '2026-05-29T20:26:40.000Z' if case == 'zero_then_positive' else TIME
                test.assertEqual(report['session']['sessions'][0]['lastActivity'], last)
    return reports


class PiObservationCandidateTests(snapshot.PiSnapshotCandidateTests):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_PI_OBSERVATION_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CCUSAGE_PI_OBSERVATION_CANDIDATE for this local candidate.')
        cls.binary = Path(selected).resolve(strict=True)
        digest = os.environ.get('OPENUSAGE_CCUSAGE_PI_OBSERVATION_CANDIDATE_SHA256')
        if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != digest:
            raise ValueError('Observation candidate digest does not match the explicit expected SHA-256.')
        cls.reference = ROOT / 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu'
        if hashlib.sha256(cls.reference.read_bytes()).hexdigest() != json.loads(PIN.read_text())['executableSha256']:
            raise ValueError('The reference does not match the production pin.')
        cls.inputs = json.loads((FIXTURES / 'inputs.json').read_text())
        cls.expected = json.loads((FIXTURES / 'expected.json').read_text())

    def test_observations_match_independent_expectations_and_recorded_reports(self):
        self.assertEqual(collect_observations(self), json.loads((FIXTURES / 'pi-observation-candidate.json').read_text()))


if __name__ == '__main__':
    unittest.main()
