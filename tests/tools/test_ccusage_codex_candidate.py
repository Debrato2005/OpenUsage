"""Qualify the isolated Codex record candidate with private synthetic input."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from test_codex_producer import CAPTURE, CASES, ROOT, prepare_case, raw_captured, source_hashes
from test_ccusage_shared_candidate import measure, fixture_pairs, prepare, TOOLS, COMMANDS
from test_ccusage_formats import file_hashes

FIXTURE = ROOT / 'tests/tools/fixtures/codex-records-candidate'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def selected_hashes(root):
    return {str(path.relative_to(root)): ('link:' + os.readlink(path) if path.is_symlink() else digest(path))
            for path in sorted(root.rglob('*.jsonl'))}


def run_report(binary, selected, kind, extra=(), *, variable="CODEX_HOME", command=None):
    with tempfile.TemporaryDirectory(prefix='openusage-codex-private-home-') as home:
        command = command or (['daily', '--by-agent'] if kind == 'daily' else ['codex', 'session'])
        environment = {'HOME': home, 'CODEX_HOME': home + '/codex',
                       'XDG_CONFIG_HOME': home + '/config', 'XDG_CACHE_HOME': home + '/cache',
                       'XDG_DATA_HOME': home + '/data', 'TZ': 'UTC', 'NO_COLOR': '1', 'LOG_LEVEL': '0'}
        environment[variable] = str(selected)
        result = subprocess.run([str(binary), *command, '--json', '--offline', '--mode',
                                 'calculate', '--timezone', 'UTC', '--no-color', *extra],
                                cwd=home, env=environment,
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=20)
        if any(Path(home).iterdir()):
            raise AssertionError('The scanner changed its private home.')
    try:
        report = json.loads(result.stdout)
    except ValueError:
        report = None
    return {'exitCode': result.returncode, 'report': report,
            'measurement': measure(report, kind), 'stdoutBytes': len(result.stdout), 'stdoutSha256': hashlib.sha256(result.stdout).hexdigest(),
            'stderr': result.stderr.decode(errors='replace')}


def expected_result(run, total):
    if total is None:
        return run['exitCode'] != 0 and run['report'] is None and run['stdoutBytes'] == 0
    return (run['exitCode'] == 0 and run['measurement']['totalTokens'] == total
            and run['measurement']['state'] == 'observations'
            and not run['measurement']['invariantErrors'])


def synthetic_rows(*, thread='thread-a', session=None, response='response-a', total=17, model='gpt-5'):
    usage = {'input_tokens': total, 'cached_input_tokens': 0, 'cache_write_input_tokens': 0,
             'output_tokens': 0, 'reasoning_output_tokens': 0, 'total_tokens': total}
    payloads = [
        ('session_meta', {'id': thread, 'session_id': session or thread,
                          'cli_version': '0.163.0', 'history_mode': 'paginated'}),
        ('event_msg', {'type': 'task_started', 'turn_id': 'turn-a', 'root_turn_id': 'turn-a'}),
        ('turn_context', {'turn_id': 'turn-a', 'root_turn_id': 'turn-a', 'model': model}),
        ('token_usage_record', {'thread_id': thread, 'session_id': session or thread,
                                'turn_id': 'turn-a', 'root_turn_id': 'turn-a',
                                'response_id': response, 'usage': dict(usage),
                                'turn_token_usage': dict(usage), 'thread_token_usage': dict(usage)}),
        ('event_msg', {'type': 'token_count', 'info': {'last_token_usage': dict(usage),
                                                     'total_token_usage': dict(usage)}})]
    return [{'type': kind, 'ordinal': i, 'timestamp': '2026-11-11T12:00:00.000Z', 'payload': payload}
            for i, (kind, payload) in enumerate(payloads)]


def write_rows(path, rows):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(''.join(json.dumps(row, separators=(',', ':')) + '\n' for row in rows))


class CodexRecordCandidateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CODEX_RECORD_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CODEX_RECORD_CANDIDATE for candidate execution.')
        cls.binary = Path(selected).resolve(strict=True)
        expected = os.environ.get('OPENUSAGE_CODEX_RECORD_SHA256')
        if not expected:
            expected = json.loads((FIXTURE / 'pin.json').read_text())['executableSha256']
        if digest(cls.binary) != expected:
            raise ValueError('The candidate does not match its selected digest.')
        cls.receipt = None
        selected_receipt = os.environ.get('OPENUSAGE_CODEX_CAPTURE_RECEIPT')
        if selected_receipt:
            path = Path(selected_receipt)
            if digest(path) != CAPTURE['receiptSha256']:
                raise ValueError('The producer receipt does not match its capture pin.')
            cls.receipt = json.loads(path.read_text())
            for role in CAPTURE['files']:
                raw_captured(role, cls.receipt)

    def test_captured_matrix_meets_independent_expectations(self):
        if self.receipt is None:
            self.skipTest('The exact private producer receipt is required for raw comparisons.')
        evidence = {'candidateSha256': digest(self.binary), 'captureSha256': CAPTURE['receiptSha256'],
                    'cases': [], 'complete': False}
        try:
            with tempfile.TemporaryDirectory(prefix='openusage-codex-records-') as directory:
                root = Path(directory)
                for case, total in CASES.items():
                    selected = root / case
                    prepare_case(selected, case, self.receipt)
                    before = selected_hashes(selected)
                    for kind in ('daily', 'sessions'):
                        runs = [run_report(self.binary, selected, kind) for _ in range(2)]
                        evidence['cases'].append({'case': case, 'kind': kind, 'expected': total,
                                                  'runs': runs,
                                                  'matches': [expected_result(run, total) for run in runs]})
                    self.assertEqual(selected_hashes(selected), before)
            evidence['complete'] = True
        finally:
            output = os.environ.get('OPENUSAGE_CODEX_RECORD_RESULTS')
            if output:
                with Path(output).open('x') as handle:
                    json.dump(evidence, handle, indent=2)
                    handle.write('\n')
        for case in evidence['cases']:
            with self.subTest(case=case['case'], kind=case['kind']):
                self.assertTrue(all(case['matches']), case)
                self.assertEqual(case['runs'][0], case['runs'][1])

    def test_legacy_reports_match_the_unchanged_pi_oracle(self):
        reference = ROOT / '.tools/qualification-oracles/ccusage-pi-bounded-release-linux-x86_64'
        pin = json.loads((ROOT / 'tests/tools/fixtures/pi-producer/pin.json').read_text())
        if not reference.exists():
            self.skipTest('The unchanged Pi oracle is required for the legacy comparison.')
        self.assertEqual(digest(reference), pin['boundedReleaseScannerSha256'])
        evidence = {'candidateSha256': digest(self.binary), 'referenceSha256': digest(reference),
                    'comparisons': [], 'complete': False}
        try:
            with tempfile.TemporaryDirectory(prefix='openusage-codex-legacy-') as directory:
                for case, tool in fixture_pairs():
                    root = Path(directory) / case / tool
                    root.mkdir(parents=True)
                    folders = prepare(root, tool, case)
                    before = file_hashes(root)
                    for kind, command in COMMANDS[tool]:
                        runs = [run_report(binary, folders, kind, variable=TOOLS[tool][0], command=command)
                                for binary in (reference, self.binary) for _ in range(2)]
                        evidence['comparisons'].append({'case': case, 'tool': tool, 'kind': kind,
                                                        'match': all(run == runs[0] for run in runs),
                                                        'stdoutSha256': runs[0]['stdoutSha256']})
                        self.assertTrue(all(run == runs[0] for run in runs), (case, tool, kind, runs))
                    self.assertEqual(file_hashes(root), before)
            evidence['complete'] = True
        finally:
            output = os.environ.get('OPENUSAGE_CODEX_COMPAT_RESULTS')
            if output:
                with Path(output).open('x') as handle:
                    json.dump(evidence, handle, indent=2)
                    handle.write('\n')

    def check_rows(self, rows, total, *, extra=(), transform=None):
        with tempfile.TemporaryDirectory(prefix='openusage-codex-rows-') as directory:
            root = Path(directory)
            selected = root / 'selected'
            path = selected / 'sessions/source.jsonl'
            write_rows(path, rows)
            if transform:
                transform(path)
            before = selected_hashes(selected)
            results = {}
            for kind in ('daily', 'sessions'):
                result = run_report(self.binary, selected, kind, extra)
                self.assertTrue(expected_result(result, total), (kind, result))
                results[kind] = result
            self.assertEqual(selected_hashes(selected), before)
            return results

    def test_measured_zero_differs_from_no_observation(self):
        reports = self.check_rows(synthetic_rows(total=0), 0)
        self.assertTrue(all(run['measurement']['rowCount'] == 1 for run in reports.values()))
        for rows in [synthetic_rows()[:1], synthetic_rows()[:3]]:
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                write_rows(root / 'sessions/empty.jsonl', rows)
                for kind in ('daily', 'sessions'):
                    result = run_report(self.binary, root, kind)
                    self.assertEqual(result['exitCode'], 0, result)
                    self.assertEqual(result['measurement']['state'], 'no_observations')

    def test_complete_usage_prefix_needs_no_counter_or_completion(self):
        self.check_rows(synthetic_rows()[:4], 17)

    def test_missing_model_keeps_tokens_without_a_model_or_estimate(self):
        results = self.check_rows(synthetic_rows(model=None), 17)
        for kind, result in results.items():
            row = result['report'][kind][0]
            self.assertEqual(row.get('models', row.get('modelsUsed')), {} if kind == 'sessions' else [])
            cost = 'costUSD' if kind == 'sessions' else 'totalCost'
            self.assertIsNone(row[cost], row)
            self.assertIsNone(result['report']['totals'][cost])

    def test_unknown_model_has_no_fabricated_price(self):
        results = self.check_rows(synthetic_rows(model='openusage-unknown-codex-model'), 17)
        for result in results.values():
            encoded = json.dumps(result['report'])
            self.assertIn('openusage-unknown-codex-model', encoded)
            self.assertIn('missingPricing', encoded)

    def test_renamed_copies_dedupe_but_equal_distinct_threads_survive(self):
        for distinct, total in [(False, 17), (True, 34)]:
            rows = synthetic_rows(thread='thread-b' if distinct else 'thread-a')
            self.check_rows(synthetic_rows(), total,
                            transform=lambda path, rows=rows: write_rows(path.with_name('copy.jsonl'), rows))

    def test_record_conflicts_and_partial_formats_refuse(self):
        cases = {}
        for field in ('thread_id', 'session_id', 'root_turn_id'):
            rows = synthetic_rows()
            rows[3]['payload'][field] = 'contradictory-id'
            cases[field] = rows
        for field in ('usage', 'turn_token_usage', 'thread_token_usage'):
            rows = synthetic_rows()
            rows[3]['payload'][field]['total_tokens'] += 1
            cases[field] = rows
        rows = synthetic_rows(); del rows[3]['payload']['usage']['cached_input_tokens']
        cases['missing-category'] = rows
        rows = synthetic_rows(); rows[3]['payload']['usage']['input_tokens'] = -1
        cases['negative-count'] = rows
        rows = synthetic_rows(); rows[3]['payload']['usage']['input_tokens'] = 17.0
        cases['fractional-json-number'] = rows
        rows = synthetic_rows(); del rows[0]['payload']['history_mode']
        cases['missing-mode'] = rows
        rows = synthetic_rows(); rows[0]['payload']['history_mode'] = 'legacy'
        cases['mixed-mode'] = rows
        rows = synthetic_rows(); rows[0]['payload']['cli_version'] = 'future'
        cases['unknown-producer'] = rows
        rows = synthetic_rows(); rows[2]['ordinal'] += 1
        cases['ordinal-gap'] = rows
        rows = synthetic_rows(); rows[2]['type'] = 'compacted'
        cases['unsupported-compaction'] = rows
        rows = synthetic_rows(); rows[1]['payload']['type'] = 'thread_rolled_back'
        cases['unsupported-revert'] = rows
        for name, rows in cases.items():
            with self.subTest(case=name):
                self.check_rows(rows, None)
        self.check_rows(synthetic_rows(), None,
                        transform=lambda path: path.write_bytes(path.read_bytes()[:-2]))
        self.check_rows(synthetic_rows(), None, transform=lambda path:
                        path.write_bytes(path.read_bytes().replace(b'"ordinal":3', b'"ordinal":3,"ordinal":3')))

    def test_counter_disagreement_refuses_and_counter_never_adds_usage(self):
        rows = synthetic_rows()
        rows[4]['payload']['info']['last_token_usage']['input_tokens'] = 18
        rows[4]['payload']['info']['last_token_usage']['total_tokens'] = 18
        self.check_rows(rows, None)
        rows = synthetic_rows()
        repeat = json.loads(json.dumps(rows[4]))
        repeat['ordinal'] = 5
        self.check_rows(rows + [repeat], 17)

    def test_orphan_counter_cannot_claim_more_than_its_total(self):
        rows = synthetic_rows()
        rows.pop(3)
        rows[3]['ordinal'] = 3
        for field in rows[3]['payload']['info']['total_token_usage']:
            rows[3]['payload']['info']['total_token_usage'][field] = 0
        self.check_rows(rows, None)
        zero = synthetic_rows(total=0)
        zero.pop(3)
        zero[3]['ordinal'] = 3
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_rows(root / 'sessions/no-observation.jsonl', zero)
            for kind in ('daily', 'sessions'):
                result = run_report(self.binary, root, kind)
                self.assertEqual(result['exitCode'], 0, result)
                self.assertEqual(result['measurement']['state'], 'no_observations')

    def test_reused_response_identity_at_a_new_ordinal_refuses(self):
        rows = synthetic_rows()
        duplicate = json.loads(json.dumps(rows[3]))
        duplicate['ordinal'] = 5
        duplicate['payload']['turn_token_usage']['input_tokens'] = 34
        duplicate['payload']['turn_token_usage']['total_tokens'] = 34
        duplicate['payload']['thread_token_usage']['input_tokens'] = 34
        duplicate['payload']['thread_token_usage']['total_tokens'] = 34
        self.check_rows(rows + [duplicate], None)

    def test_selected_copies_across_roots_and_archives_are_checked(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ('first', 'second'):
                write_rows(root / name / 'sessions/source.jsonl', synthetic_rows())
            selected = ','.join(str(root / name) for name in ('second', 'first'))
            for kind in ('daily', 'sessions'):
                self.assertTrue(expected_result(run_report(self.binary, selected, kind), 17))
            write_rows(root / 'first/archived_sessions/source.jsonl', synthetic_rows(total=18))
            for kind in ('daily', 'sessions'):
                self.assertTrue(expected_result(run_report(self.binary, selected, kind), None))

    def test_date_filters_use_record_time_and_validate_the_selected_scope(self):
        self.check_rows(synthetic_rows(), 17, extra=('--since', '20261111', '--until', '20261111'))
        self.check_rows(synthetic_rows(), None, extra=('--since', '20261111', '--until', '20261111'),
                        transform=lambda path: write_rows(path.with_name('copy.jsonl'), synthetic_rows(total=18)))

    def test_line_and_file_and_record_limits_refuse(self):
        self.check_rows(synthetic_rows(), None, transform=lambda path:
                        path.write_bytes(path.read_bytes() + b' ' * (1 << 20) + b'\n'))
        def oversized(path):
            with path.open('ab') as handle:
                handle.truncate((32 << 20) + 1)
        self.check_rows(synthetic_rows(), None, transform=oversized)
        def many_records(path):
            row = path.read_bytes().splitlines(keepends=True)[0]
            with path.open('ab') as handle:
                handle.write(row * 100001)
        self.check_rows(synthetic_rows(), None, transform=many_records)
        self.check_rows(synthetic_rows(total=(1 << 53) - 1), (1 << 53) - 1)
        self.check_rows(synthetic_rows(total=1 << 53), None)

    def test_symlink_peer_is_refused_without_following_it(self):
        self.check_rows(synthetic_rows(), None,
                        transform=lambda path: path.with_name('link.jsonl').symlink_to('/unselected/missing.jsonl'))

    def test_declared_lineage_needs_the_selected_exact_parent_prefix(self):
        for case in ('complete', 'missing', 'wrong-byte', 'wrong-ordinal', 'wrong-cumulative'):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                parent = root / 'sessions/renamed-parent.jsonl'
                write_rows(parent, synthetic_rows())
                child = synthetic_rows(thread='thread-child', response='response-child', total=3)
                for row in child:
                    row['ordinal'] += 5
                meta = child[0]['payload']
                meta.update(forked_from_id='thread-a', forked_from_ordinal_exclusive=5,
                            history_base={'thread_id': 'thread-a', 'end_ordinal_exclusive': 5,
                                          'end_byte_offset': parent.stat().st_size,
                                          'path': '/unselected/parent.jsonl'})
                for usage in (child[3]['payload']['thread_token_usage'],
                              child[4]['payload']['info']['total_token_usage']):
                    usage['input_tokens'] = 20
                    usage['total_tokens'] = 20
                if case == 'missing':
                    parent.unlink()
                elif case == 'wrong-byte':
                    meta['history_base']['end_byte_offset'] += 1
                elif case == 'wrong-ordinal':
                    meta['history_base']['end_ordinal_exclusive'] -= 1
                elif case == 'wrong-cumulative':
                    for usage in (child[3]['payload']['thread_token_usage'],
                                  child[4]['payload']['info']['total_token_usage']):
                        usage['input_tokens'] = 21
                        usage['total_tokens'] = 21
                write_rows(root / 'sessions/renamed-child.jsonl', child)
                before = selected_hashes(root)
                for kind in ('daily', 'sessions'):
                    result = run_report(self.binary, root, kind)
                    self.assertTrue(expected_result(result, 20 if case == 'complete' else None), result)
                self.assertEqual(selected_hashes(root), before)

    def test_output_session_identity_cannot_have_two_thread_owners(self):
        self.check_rows(synthetic_rows(), None, transform=lambda path:
                        write_rows(path.with_name('other.jsonl'),
                                   synthetic_rows(thread='thread-b', session='thread-a')))

    def test_assertions_detect_a_guarded_output_mutation(self):
        result = self.check_rows(synthetic_rows(), 17)['daily']
        original = json.dumps(result, sort_keys=True)
        try:
            result['report']['daily'][0]['totalTokens'] += 1
            result['measurement'] = measure(result['report'], 'daily')
            with self.assertRaises(AssertionError):
                self.assertTrue(expected_result(result, 17))
        finally:
            result = json.loads(original)
        self.assertTrue(expected_result(result, 17))
        self.assertEqual(json.dumps(result, sort_keys=True), original)


class CodexCandidatePinTests(unittest.TestCase):
    def test_patch_and_source_recipe_match_the_pin(self):
        pin = json.loads((FIXTURE / 'pin.json').read_text())
        for patch in pin['patches']:
            self.assertEqual(digest(ROOT / patch['path']), patch['sha256'])
        self.assertEqual(digest(FIXTURE / 'source-files.json'), pin['sourceFilesSha256'])
        self.assertEqual(pin['captureReceiptSha256'], CAPTURE['receiptSha256'])
        records = json.loads((FIXTURE / 'source-files.json').read_text())
        self.assertIsInstance(records, list)
        expected = {}
        for record in records:
            self.assertEqual(set(record), {'path', 'sha256'})
            self.assertIsInstance(record['path'], str)
            self.assertRegex(record['sha256'], r'\A[0-9a-f]{64}\Z')
            self.assertNotIn(record['path'], expected)
            expected[record['path']] = record['sha256']
        self.assertEqual(list(expected), sorted(expected))
        source = ROOT / pin['sourceDirectory']
        if source.exists():
            self.assertEqual({name: digest(source / name) for name in expected}, expected)

    def test_original_pins_and_separate_oracle_are_preserved(self):
        pin = json.loads((FIXTURE / 'pin.json').read_text())
        production = json.loads((ROOT / 'crates/openusage-locallogs/sources/ccusage-20.0.48-linux-x86_64.json').read_text())
        pi = json.loads((ROOT / 'tests/tools/fixtures/pi-producer/pin.json').read_text())
        self.assertEqual(pin['productionSha256'], production['executableSha256'])
        self.assertEqual(pin['piOracleSha256'], pi['boundedReleaseScannerSha256'])
        oracle = ROOT / pin['oraclePath']
        if oracle.exists():
            self.assertEqual(digest(oracle), pin['executableSha256'])
            self.assertEqual(oracle.stat().st_mode & 0o222, 0)
            target = ROOT / '.tools/build-target/release/ccusage'
            if target.exists():
                self.assertNotEqual((oracle.stat().st_dev, oracle.stat().st_ino),
                                    (target.stat().st_dev, target.stat().st_ino))


if __name__ == '__main__':
    unittest.main()
