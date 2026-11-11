"""Compare pinned scanners with private synthetic Claude Code and Codex fixtures."""
import copy
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import time
import unittest
from unittest import mock

from test_ccusage_formats import PIN, ROOT, file_hashes


FIXTURES = ROOT / 'crates/openusage-locallogs/tests/fixtures'
CANDIDATE_PIN = ROOT / 'tests/tools/fixtures/pi-producer/pin.json'
CASES = ('baseline', 'copied_roots', 'repeated_root', 'renamed_copy',
         'stored_charges', 'unknown_models', 'spaced_json', 'empty',
         'malformed_peer', 'distinct_equal_usage')
TOOLS = {'claude': ('CLAUDE_CONFIG_DIR', 'home/.claude'),
         'codex': ('CODEX_HOME', 'codex')}
COMMANDS = {'claude': (('daily', ['daily', '--by-agent']),
                       ('sessions', ['claude', 'session']),
                       ('blocks', ['blocks', '-n', '1'])),
            'codex': (('daily', ['daily', '--by-agent']),
                      ('sessions', ['codex', 'session']))}
# These counts come from the synthetic source records, not scanner output.
# Claude cache counts are separate from input. Codex input includes cached input.
EXPECTED = {'claude': {'inputTokens': 1000 + 2000 + 1500 + 100,
                       'outputTokens': 500 + 800 + 700 + 50,
                       'cacheCreationTokens': 200 + 100, 'cacheReadTokens': 3000 + 5000,
                       'totalTokens': 12500 + 2300 + 150},
            'codex': {'inputTokens': 4000 - 1000, 'outputTokens': 600,
                      'cacheCreationTokens': 0, 'cacheReadTokens': 1000,
                      'totalTokens': 4600}}
COUNT_FIELDS = (*EXPECTED['claude'], 'reasoningOutputTokens')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def change_models(value):
    if isinstance(value, dict):
        return {key: 'openusage-unpriced-model' if key == 'model' and isinstance(item, str)
                else change_models(item) for key, item in value.items()}
    if isinstance(value, list):
        return [change_models(item) for item in value]
    return value


def write_rows(path, rows, *, compact=True):
    separators = (',', ':') if compact else (', ', ': ')
    path.write_text(''.join(json.dumps(row, separators=separators) + '\n' for row in rows))


def prepare(root, tool, case):
    first = root / 'usage logs café'
    if case == 'empty':
        (first / ('projects' if tool == 'claude' else 'sessions')).mkdir(parents=True)
        return str(first)
    shutil.copytree(FIXTURES / TOOLS[tool][1], first)
    if case in ('copied_roots', 'renamed_copy'):
        second = root / 'copied logs'
        shutil.copytree(first, second)
        if case == 'renamed_copy':
            for path in sorted(second.rglob('*.jsonl')):
                path.rename(path.with_stem(path.stem + '-copy'))
        return f'{first},{second}'
    if case == 'repeated_root':
        return f'{first},{first}'
    if case in ('unknown_models', 'stored_charges', 'spaced_json'):
        for path in sorted(first.rglob('*.jsonl')):
            rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
            if case == 'unknown_models':
                rows = [change_models(row) for row in rows]
            elif case == 'stored_charges' and tool == 'claude':
                for index, row in enumerate(rows):
                    row['costUSD'] = 0 if index == 0 else 999
            write_rows(path, rows, compact=case != 'spaced_json')
    if case == 'malformed_peer':
        # Keep the malformed peer separate from every valid file.
        source = sorted(first.rglob('*.jsonl'))[0]
        peer = source.with_name('44444444-dddd-4ddd-8ddd-444444444444.jsonl')
        with peer.open('x') as stream:
            stream.write('{"type":\n')
    if case == 'distinct_equal_usage':
        if tool != 'codex':
            raise ValueError('distinct_equal_usage: codex only')
        source = sorted(first.rglob('*.jsonl'))[0]
        rows = [json.loads(line) for line in source.read_text().splitlines()]
        session_id = '55555555-eeee-4eee-8eee-555555555555'
        for row in rows:
            if row.get('type') == 'session_meta':
                row['payload']['id'] = session_id
        peer = source.with_name(f'rollout-2026-11-05T12-00-00-{session_id}.jsonl')
        write_rows(peer, rows)
    return str(first)


def fixture_pairs():
    return [(case, tool) for case in CASES for tool in TOOLS
            if case != 'distinct_equal_usage' or tool == 'codex']


def field_state(mapping, key):
    if key not in mapping:
        return 'missing'
    value = mapping[key]
    if value is None:
        return 'null'
    if type(value) is not int or value < 0:
        return 'invalid'
    return 'zero' if value == 0 else 'value'


def details(value, path='$', found=None):
    if found is None:
        found = {'nullPaths': [], 'zeroPaths': [], 'pricingFields': {}}
    if value is None:
        found['nullPaths'].append(path)
    elif type(value) in (int, float) and value == 0:
        found['zeroPaths'].append(path)
    elif isinstance(value, dict):
        for key, item in value.items():
            child = f'{path}.{key}'
            if key in ('cost', 'costUSD', 'totalCost', 'missingPricing', 'unpricedModels'):
                found['pricingFields'][child] = item
            details(item, child, found)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            details(item, f'{path}[{index}]', found)
    return found


def measure(report, kind):
    summary = {'state': 'invalid_report', 'totalTokens': None, 'rowCount': None,
               'tokenCounts': {}, 'metricStates': {}, 'invariantErrors': []}
    if not isinstance(report, dict):
        return summary
    summary.update(details(report))
    totals = report.get('totals', {})
    if not isinstance(totals, dict):
        summary['invariantErrors'].append('invalid_totals')
        totals = {}
    summary['reportedTotalTokens'] = totals.get('totalTokens')
    rows = report.get(kind)
    if not isinstance(rows, list) or any(not isinstance(row, dict) for row in rows):
        summary['invariantErrors'].append('invalid_rows')
        return summary
    rows = [row for row in rows if not (kind == 'blocks' and row.get('isGap'))]
    summary['rowCount'] = len(rows)
    if not rows:
        summary['state'] = 'no_observations'
        summary['metricStates'] = {key: field_state(totals, key) for key in COUNT_FIELDS}
        if totals.get('totalTokens') not in (None, 0):
            summary['invariantErrors'].append('nonzero_total_without_rows')
        return summary
    summary['state'] = 'observations'
    row_counts = []
    for row in rows:
        counts = dict(row.get('tokenCounts', {})) if kind == 'blocks' else dict(row)
        if kind == 'blocks':
            for old, new in (('cacheCreationInputTokens', 'cacheCreationTokens'),
                             ('cacheReadInputTokens', 'cacheReadTokens')):
                if old in counts:
                    counts[new] = counts.pop(old)
            counts['totalTokens'] = row.get('totalTokens')
        row_counts.append(counts)
        if field_state(counts, 'totalTokens') not in ('zero', 'value'):
            summary['invariantErrors'].append('invalid_row_total')
        components = [counts.get(key) for key in EXPECTED['claude'] if key != 'totalTokens']
        if all(type(value) is int and value >= 0 for value in components):
            if sum(components) != counts.get('totalTokens'):
                summary['invariantErrors'].append('row_components_differ')
    for key in COUNT_FIELDS:
        states = [field_state(counts, key) for counts in row_counts]
        if all(state in ('zero', 'value') for state in states):
            value = sum(counts[key] for counts in row_counts)
            summary['tokenCounts'][key] = value
            summary['metricStates'][key] = 'zero' if value == 0 else 'value'
            if kind != 'blocks' and totals.get(key) != value:
                summary['invariantErrors'].append(f'{key}:rows_differ_from_totals')
        else:
            summary['tokenCounts'][key] = None
            summary['metricStates'][key] = states[0] if len(set(states)) == 1 else 'mixed'
    summary['totalTokens'] = summary['tokenCounts']['totalTokens']
    return summary


def expectation(tool, case, kind):
    if case == 'malformed_peer':
        return {'outcome': 'refusal', 'tokenCounts': None}
    if case == 'empty':
        return {'outcome': 'no_observations', 'tokenCounts': None}
    counts = dict(EXPECTED[tool])
    if tool == 'codex' and kind == 'sessions':
        counts['reasoningOutputTokens'] = 200
    if case == 'distinct_equal_usage':
        counts = {key: value * 2 for key, value in counts.items()}
    return {'outcome': 'observations', 'tokenCounts': counts}


def matches_expectation(output, measured, expected):
    if expected['outcome'] == 'refusal':
        return output['exitCode'] != 0
    if output['exitCode'] != 0 or measured['state'] != expected['outcome']:
        return False
    return expected['tokenCounts'] is None or all(
        measured['tokenCounts'].get(key) == value
        for key, value in expected['tokenCounts'].items())


def finish_results(results):
    comparisons = results['comparisons']
    checks = results['sourceChecks']
    results['comparisonCount'] = len(comparisons)
    results['commandCount'] = len(results['runs'])
    results['matchingComparisons'] = sum(row['referenceCompatible'] for row in comparisons)
    results['independentMatches'] = sum(all(row['sourceExpectationMatched']) for row in comparisons)
    results['inheritedDisagreements'] = sum(row['referenceCompatible'] and not all(
        row['sourceExpectationMatched']) for row in comparisons)
    results['matrixComplete'] = (results['iterationComplete']
                                 and len(comparisons) == results['expectedComparisonCount']
                                 and len(results['runs']) == results['expectedCommandCount']
                                 and all(run['executionComplete'] for run in results['runs']))
    results['sourceHashesUnchanged'] = None
    if any(check['unchanged'] is False for check in checks):
        results['sourceHashesUnchanged'] = False
    elif results['matrixComplete'] and len(checks) == results['expectedSourceCount']:
        results['sourceHashesUnchanged'] = all(check['unchanged'] is True for check in checks)
    results['comparisonCheckPassed'] = bool(
        results['matrixComplete'] and results['sourceHashesUnchanged']
        and results['inputHashesUnchanged'] and results['executableHashesUnchanged']
        and all(run['privateHomeUnchanged'] for run in results['runs'])
        and all(row['referenceCompatible'] and all(row['repeatMatched'])
                and not any(item['invariantErrors'] for item in row['measurements'])
                for row in comparisons))
    # A complete comparison can expose inherited failures without qualifying either scanner.
    results['productionQualificationPassed'] = (results['comparisonCheckPassed']
                                               and results['independentMatches'] == len(comparisons))


class SharedCandidateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_SHARED_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_SHARED_CANDIDATE to run the pinned comparison.')
        cls.candidate = Path(selected).resolve(strict=True)
        cls.reference = ROOT / 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu'
        cls.production_pin = json.loads(PIN.read_text())
        cls.candidate_pin = json.loads(CANDIDATE_PIN.read_text())
        for binary, expected in ((cls.reference, cls.production_pin['executableSha256']),
                                 (cls.candidate, cls.candidate_pin['boundedReleaseScannerSha256'])):
            if digest(binary) != expected:
                raise ValueError('The executable digest differs from its pin.')

    def run_report(self, root, binary, variable, folders, command):
        with tempfile.TemporaryDirectory(prefix='empty-home-', dir=root) as empty:
            argv = [str(binary), *command, '--json', '--offline', '--mode', 'calculate',
                    '--timezone', 'UTC', '--no-color']
            environment = {'HOME': empty, 'TZ': 'UTC', 'NO_COLOR': '1',
                           'LOG_LEVEL': '0', 'COLUMNS': '200', variable: folders}
            receipt = {'argv': argv, 'cwd': empty, 'environment': environment,
                       'executionComplete': False, 'privateHomeUnchanged': None}
            self.results['runs'].append(receipt)
            started = time.monotonic()
            try:
                result = subprocess.run(argv, cwd=empty, env=environment,
                                        stdin=subprocess.DEVNULL, capture_output=True, timeout=20)
                receipt.update({'executionComplete': True, 'exitCode': result.returncode,
                                'stdoutBytes': len(result.stdout), 'stderrBytes': len(result.stderr),
                                'stdoutSha256': hashlib.sha256(result.stdout).hexdigest(),
                                'stderrSha256': hashlib.sha256(result.stderr).hexdigest()})
                self.assertLess(len(result.stdout), 128 * 1024)
                try:
                    report = json.loads(result.stdout)
                except (ValueError, UnicodeDecodeError):
                    report = None
                return {'exitCode': result.returncode, 'report': report,
                        'stdoutSha256': receipt['stdoutSha256']}
            finally:
                receipt['elapsedSeconds'] = round(time.monotonic() - started, 6)
                receipt['privateHomeUnchanged'] = not any(Path(empty).iterdir())

    def test_enabled_reports_match_the_reference_across_private_cases(self):
        pairs = fixture_pairs()
        inputs = [Path(__file__), PIN, CANDIDATE_PIN]
        inputs += [path for tool in TOOLS for path in (FIXTURES / TOOLS[tool][1]).rglob('*')
                   if path.is_file()]
        before_inputs = {str(path.relative_to(ROOT)): digest(path) for path in inputs}
        self.results = results = {
            'scope': 'synthetic Linux', 'platform': platform.platform(),
            'productionVersion': self.production_pin['version'],
            'candidateVersion': self.candidate_pin['boundedReleaseScannerVersion'],
            'productionSha256': digest(self.reference), 'candidateSha256': digest(self.candidate),
            'productionPin': str(PIN.relative_to(ROOT)),
            'candidatePin': str(CANDIDATE_PIN.relative_to(ROOT)),
            'candidateProducerSourceCommit': self.candidate_pin['sourceCommit'],
            'inputHashesBefore': before_inputs, 'inputHashesUnchanged': None,
            'expectedSourceCount': len(pairs),
            'expectedComparisonCount': sum(len(COMMANDS[tool]) for _, tool in pairs),
            'expectedCommandCount': 4 * sum(len(COMMANDS[tool]) for _, tool in pairs),
            'comparisons': [], 'runs': [], 'sourceChecks': [], 'iterationComplete': False,
            'sourceHashesUnchanged': None, 'executableHashesUnchanged': None,
        }
        try:
            with tempfile.TemporaryDirectory(prefix='openusage-shared-candidate-') as temporary:
                for case, tool in pairs:
                    variable = TOOLS[tool][0]
                    root = Path(temporary) / case / tool
                    root.mkdir(parents=True)
                    folders = prepare(root, tool, case)
                    check = {'case': case, 'tool': tool, 'folders': folders,
                             'before': file_hashes(root), 'after': None, 'unchanged': None}
                    results['sourceChecks'].append(check)
                    try:
                        for kind, command in COMMANDS[tool]:
                            outputs, repeats = [], []
                            run_start = len(results['runs'])
                            for binary in (self.reference, self.candidate):
                                first = self.run_report(root, binary, variable, folders, command)
                                repeated = self.run_report(root, binary, variable, folders, command)
                                repeats.append(first == repeated)
                                outputs.append(first)
                            # Compare complete output bytes, including fields outside the measured totals.
                            measured = [measure(output['report'], kind) for output in outputs]
                            expected = expectation(tool, case, kind)
                            source_matches = [matches_expectation(output, measurement, expected)
                                              for output, measurement in zip(outputs, measured)]
                            results['comparisons'].append({
                                'case': case, 'tool': tool, 'report': kind,
                                'referenceCompatible': outputs[0] == outputs[1],
                                'repeatMatched': repeats, 'expectation': expected,
                                'observedTotalTokens': [item['totalTokens'] for item in measured],
                                'sourceExpectationMatched': source_matches, 'measurements': measured,
                                'runIndexes': list(range(run_start, len(results['runs']))),
                            })
                    finally:
                        check['after'] = file_hashes(root)
                        check['unchanged'] = check['before'] == check['after']
                results['iterationComplete'] = True
        except Exception as error:
            results['executionError'] = {'type': type(error).__name__, 'message': str(error)}
            raise
        finally:
            results['inputHashesAfter'] = {str(path.relative_to(ROOT)): digest(path) for path in inputs}
            results['inputHashesUnchanged'] = before_inputs == results['inputHashesAfter']
            results['executableHashesAfter'] = {'production': digest(self.reference),
                                                'candidate': digest(self.candidate)}
            results['executableHashesUnchanged'] = (
                results['productionSha256'] == results['executableHashesAfter']['production']
                and results['candidateSha256'] == results['executableHashesAfter']['candidate'])
            finish_results(results)
            destination = os.environ.get('OPENUSAGE_SHARED_CANDIDATE_RESULTS')
            if destination:
                with Path(destination).open('x') as stream:
                    stream.write(json.dumps(results, indent=2) + '\n')
        self.assertTrue(results['matrixComplete'] and results['sourceHashesUnchanged'],
                        'The comparison did not finish or preserve its inputs.')
        self.assertTrue(results['comparisonCheckPassed'], 'The candidate differs from the reference.')


class SharedHarnessTests(unittest.TestCase):
    def test_malformed_peer_preserves_every_valid_source(self):
        for tool in TOOLS:
            with self.subTest(tool=tool), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                original = file_hashes(FIXTURES / TOOLS[tool][1])
                folder = Path(prepare(root, tool, 'malformed_peer'))
                after = file_hashes(folder)
                self.assertEqual({path: after[path] for path in original}, original)
                self.assertEqual(len(after), len(original) + 1)
                peer, = set(after) - set(original)
                with self.assertRaises(json.JSONDecodeError):
                    json.loads((folder / peer).read_text())

    def test_model_and_charge_cases_keep_compact_usage_markers(self):
        for case in ('unknown_models', 'stored_charges', 'spaced_json'):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                folder = Path(prepare(Path(temporary), 'claude', case))
                rows = []
                for path in folder.rglob('*.jsonl'):
                    contents = path.read_text()
                    self.assertEqual('"usage":{' in contents, case != 'spaced_json')
                    rows += [json.loads(line) for line in contents.splitlines()]
                self.assertEqual(sum(sum(row['message']['usage'].values()) for row in rows), 14950)
                if case == 'stored_charges':
                    self.assertEqual({row['costUSD'] for row in rows}, {0, 999})
                if case == 'unknown_models':
                    self.assertEqual({row['message']['model'] for row in rows}, {'openusage-unpriced-model'})

    def test_distinct_sessions_have_separate_identity_and_equal_usage(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(prepare(Path(temporary), 'codex', 'distinct_equal_usage'))
            files = sorted(folder.rglob('*.jsonl'))
            self.assertEqual(len(files), 2)
            rows = [[json.loads(line) for line in path.read_text().splitlines()] for path in files]
            self.assertNotEqual(rows[0][0]['payload']['id'], rows[1][0]['payload']['id'])
            self.assertEqual(rows[0][1:], rows[1][1:])
            self.assertEqual(sum(data[-1]['payload']['info']['total_token_usage']['total_tokens']
                                 for data in rows), 9200)

    def test_empty_null_missing_and_zero_have_separate_states(self):
        empty = measure({'daily': [], 'totals': {'totalTokens': 0}}, 'daily')
        self.assertEqual(empty['state'], 'no_observations')
        self.assertIsNone(empty['totalTokens'])
        self.assertEqual(empty['reportedTotalTokens'], 0)
        for value, state in ((None, 'null'), (0, 'zero'), (True, 'invalid')):
            report = {'daily': [{'totalTokens': value}], 'totals': {'totalTokens': value}}
            self.assertEqual(measure(report, 'daily')['metricStates']['totalTokens'], state)
        self.assertEqual(measure({'daily': [{}]}, 'daily')['metricStates']['totalTokens'], 'missing')
        self.assertEqual(measure({'daily': [{'totalTokens': 0}], 'totals': {'totalTokens': 0}},
                                 'daily')['totalTokens'], 0)

    def run_fake_matrix(self, *, fail=False, change=False):
        test = SharedCandidateTests('test_enabled_reports_match_the_reference_across_private_cases')
        test.reference = test.candidate = Path(__file__)
        test.production_pin = json.loads(PIN.read_text())
        test.candidate_pin = json.loads(CANDIDATE_PIN.read_text())
        report = json.loads((FIXTURES / 'ccusage-20.0.48-daily-by-agent-claude.json').read_text())
        calls = 0

        def fake_run(*args):
            nonlocal calls
            calls += 1
            if fail:
                raise RuntimeError('synthetic_interruption')
            output = copy.deepcopy(report)
            if change and calls > 2:
                output['unexpectedField'] = 1
            encoded = json.dumps(output).encode()
            test.results['runs'].append({'executionComplete': True, 'privateHomeUnchanged': True})
            return {'exitCode': 0, 'report': output,
                    'stdoutSha256': hashlib.sha256(encoded).hexdigest()}

        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / 'result.json'
            with mock.patch.dict(os.environ, {'OPENUSAGE_SHARED_CANDIDATE_RESULTS': str(destination)}), \
                    mock.patch(__name__ + '.fixture_pairs', return_value=[('baseline', 'claude')]), \
                    mock.patch.dict(COMMANDS, {'claude': (('daily', ['daily', '--by-agent']),)}), \
                    mock.patch.object(test, 'run_report', side_effect=fake_run):
                if fail or change:
                    with self.assertRaises(RuntimeError if fail else AssertionError):
                        test.test_enabled_reports_match_the_reference_across_private_cases()
                else:
                    test.test_enabled_reports_match_the_reference_across_private_cases()
            return json.loads(destination.read_text())

    def test_interrupted_matrix_cannot_claim_source_or_qualification_success(self):
        results = self.run_fake_matrix(fail=True)
        self.assertFalse(results['matrixComplete'])
        self.assertIsNone(results['sourceHashesUnchanged'])
        self.assertTrue(results['sourceChecks'][0]['unchanged'])
        self.assertFalse(results['productionQualificationPassed'])
        self.assertTrue(results['inputHashesUnchanged'])

    def test_changed_candidate_report_fails_complete_comparison(self):
        results = self.run_fake_matrix(change=True)
        self.assertTrue(results['matrixComplete'])
        self.assertTrue(results['sourceHashesUnchanged'])
        self.assertTrue(results['inputHashesUnchanged'])
        self.assertEqual(results['matchingComparisons'], 0)
        self.assertEqual(results['independentMatches'], 1)
        self.assertFalse(results['comparisonCheckPassed'])
        self.assertFalse(results['productionQualificationPassed'])

    def test_unchanged_reports_pass_complete_comparison(self):
        results = self.run_fake_matrix()
        self.assertTrue(results['comparisonCheckPassed'])
        self.assertTrue(results['productionQualificationPassed'])


if __name__ == '__main__':
    unittest.main()
