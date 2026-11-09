"""Local Pi format correction, layered after usage and identity candidates."""
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, PIN, ROOT, file_hashes, make_source
from test_ccusage_pi import TIME, header, message
import test_ccusage_pi_identity as identity_candidate


def format_cases():
    def source(rows):
        return '\n'.join(json.dumps(row, separators=(',', ':')) for row in rows) + '\n'
    baseline = source([header(), message()])
    cases = {
        'escaped_usage_key': (baseline.replace('"usage"', '"\\u0075sage"'), 162),
        'unrelated_header_field': (source([header() | {'message': 42}, message()]), 162),
        'duplicate_header_key': (baseline.replace('"version":3', '"version":2,"version":3'), None),
        'duplicate_usage_key': (baseline.replace('"input":100', '"input":0,"input":100'), None),
        'duplicate_escaped_key': (baseline.replace('"input":100', '"input":100,"\\u0069nput":100'), None),
        'unknown_record': (source([header(), message(), {'type': 'future_record', 'id': 'bbbbbbbb',
                                                       'parentId': 'aaaaaaaa', 'timestamp': TIME}]), None),
        'context_edit': (source([header(), message(), {'type': 'context_edit', 'id': 'bbbbbbbb',
                                                     'parentId': 'aaaaaaaa', 'timestamp': TIME,
                                                     'targetId': 'aaaaaaaa', 'replacement': None}]), 162),
    }
    for field in ('model', 'provider', 'role', 'message', 'id'):
        row = copy.deepcopy(message())
        if field == 'message':
            row['message'] = 42
        elif field == 'id':
            row['id'] = ' aaaaaaaa '
        else:
            row['message'][field] = 42
        cases['invalid_' + field] = (source([header(), row]), None)
    for name, count in [('float', 1.5), ('negative', -1), ('null', None), ('string', '100'), ('overflow', 2**64)]:
        row = copy.deepcopy(message())
        row['message']['usage']['input'] = count
        cases['invalid_count_' + name] = (source([header(), row]), None)
    for name, usage in [('assistant_total_only', {'totalTokens': 162}),
                        ('assistant_partial_details', {'input': 100, 'totalTokens': 162})]:
        row = copy.deepcopy(message())
        row['message']['usage'] = usage
        cases[name] = (source([header(), row]), 162)
    return cases


def collect_formats(test):
    reports = {}
    for name, (source, total) in format_cases().items():
        with test.subTest(case=name), tempfile.TemporaryDirectory(prefix='openusage-pi-format-') as temporary:
            root = Path(temporary)
            make_source(root / 'not-selected', test.inputs['zcode'])
            folder = root / 'sessions'
            folder.mkdir(mode=0o700)
            (folder / 'renamed.jsonl').write_text(source)
            before = file_hashes(root)
            reports[name] = {kind: test.candidate_report(root, command, [folder], total)
                             for kind, command in [('daily', ['daily', '--by-agent']), ('session', ['pi', 'session'])]}
            test.assertEqual(file_hashes(root), before)
    return reports


class PiFormatCandidateTests(identity_candidate.PiIdentityCandidateTests):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_PI_COMPLETENESS_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CCUSAGE_PI_COMPLETENESS_CANDIDATE for the local format candidate.')
        cls.binary = Path(selected).resolve(strict=True)
        if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != os.environ.get('OPENUSAGE_CCUSAGE_PI_COMPLETENESS_CANDIDATE_SHA256'):
            raise ValueError('Format candidate digest does not match the explicit expected SHA-256.')
        cls.reference = ROOT / 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu'
        if hashlib.sha256(cls.reference.read_bytes()).hexdigest() != json.loads(PIN.read_text())['executableSha256']:
            raise ValueError('The reference does not match the production pin.')
        cls.inputs = json.loads((FIXTURES / 'inputs.json').read_text())
        cls.expected = json.loads((FIXTURES / 'expected.json').read_text())

    def test_identity_reports_and_reversed_roots(self):
        reports = identity_candidate.collect_identity(self)
        previous = json.loads((FIXTURES / 'pi-identity-candidate.json').read_text())
        expected = json.loads((FIXTURES / 'pi-format-candidate.json').read_text())
        previous['missing_details'] = expected['assistant_total_only']
        self.assertEqual(reports, previous)

    def test_supported_formats_and_explicit_unavailable_results(self):
        self.assertEqual(collect_formats(self), json.loads((FIXTURES / 'pi-format-candidate.json').read_text()))


if __name__ == '__main__':
    unittest.main()
