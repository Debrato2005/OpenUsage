import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from test_ccusage_codex_candidate import run_report
from test_ccusage_formats import file_hashes
from test_ccusage_shared_candidate import (COMMANDS, EXPECTED, FIXTURES, PIN, TOOLS,
                                          expectation, fixture_pairs,
                                          matches_expectation, prepare)


ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/tools/fixtures/claude-json-candidate'
VARIANTS = ('compact', 'spaces', 'tabs', 'outer_whitespace', 'crlf',
            'escaped_usage', 'escaped_message', 'escaped_input', 'reordered')


def render(row, variant):
    if variant == 'reordered':
        def reverse(value):
            if isinstance(value, dict):
                return {key: reverse(item) for key, item in reversed(list(value.items()))}
            if isinstance(value, list):
                return [reverse(item) for item in value]
            return value
        row = reverse(row)
    separators = {'spaces': (', ', ': '), 'tabs': (',\t', '\t:\t')}.get(variant, (',', ':'))
    line = json.dumps(row, separators=separators)
    key = {'escaped_usage': 'usage', 'escaped_message': 'message',
           'escaped_input': 'input_tokens'}.get(variant)
    if key:
        escaped = '\\u%04x' % ord(key[0]) + key[1:]
        line = line.replace('"' + key + '":', '"' + escaped + '":')
    if variant == 'outer_whitespace':
        line = ' \t' + line + '\t '
    return line + ('\r\n' if variant == 'crlf' else '\n')


def prepare_variant(root, variant, *, decoys=False):
    for original in sorted((FIXTURES / 'home/.claude').rglob('*.jsonl')):
        destination = root / original.relative_to(FIXTURES / 'home/.claude')
        destination.parent.mkdir(parents=True, exist_ok=True)
        rows = [json.loads(line) for line in original.read_text().splitlines() if line.strip()]
        if decoys:
            rows = [{'timestamp': '2026-11-05T15:00:00Z', 'type': 'user',
                     'message': {'role': 'user', 'content': [
                         {'type': 'text', 'text': '"usage":{"input_tokens":999999}'}]}},
                    {'timestamp': '2026-11-05T15:00:00Z',
                     'message': {'role': 'assistant', 'content': [],
                                 'metadata': {'usage': {'input_tokens': 999999}}}}]
        destination.write_bytes(''.join(render(row, variant) for row in rows).encode())


class ClaudeJsonHarnessTests(unittest.TestCase):
    def test_variants_preserve_the_json_record(self):
        row = {'message': {'usage': {'input_tokens': 7}, 'model': 'synthetic'}}
        for variant in VARIANTS:
            with self.subTest(variant=variant):
                self.assertEqual(json.loads(render(row, variant)), row)


class ClaudeJsonPinTests(unittest.TestCase):
    def test_patch_chain_and_source_manifest_match_the_pin(self):
        pin = json.loads((FIXTURE / 'pin.json').read_text())
        base = pin['baseCandidatePin']
        base_path = ROOT / base['path']
        self.assertEqual(hashlib.sha256(base_path.read_bytes()).hexdigest(), base['sha256'])
        previous = json.loads(base_path.read_text())
        self.assertEqual(pin['sourceCommit'], previous['sourceCommit'])
        self.assertEqual(pin['patches'][:-1], previous['patches'])
        for patch in pin['patches']:
            self.assertEqual(hashlib.sha256((ROOT / patch['path']).read_bytes()).hexdigest(),
                             patch['sha256'])
        source_file = FIXTURE / 'source-files.json'
        self.assertEqual(hashlib.sha256(source_file.read_bytes()).hexdigest(), pin['sourceFilesSha256'])
        records = json.loads(source_file.read_text())
        names = []
        for record in records:
            self.assertEqual(set(record), {'path', 'sha256'})
            path = Path(record['path'])
            self.assertFalse(path.is_absolute())
            self.assertNotIn('..', path.parts)
            self.assertRegex(record['sha256'], r'\A[0-9a-f]{64}\Z')
            names.append(record['path'])
            source = ROOT / pin['sourceDirectory']
            if source.exists():
                self.assertEqual(hashlib.sha256((source / path).read_bytes()).hexdigest(),
                                 record['sha256'])
        self.assertEqual(names, sorted(set(names)))
        compact = json.dumps(records, separators=(',', ':')).encode()
        self.assertEqual(hashlib.sha256(compact).hexdigest(), pin['sourceManifestSha256'])

    def test_original_scanners_remain_separate_from_the_candidate(self):
        pin = json.loads((FIXTURE / 'pin.json').read_text())
        previous = json.loads((ROOT / pin['baseCandidatePin']['path']).read_text())
        self.assertEqual(pin['productionSha256'], json.loads(PIN.read_text())['executableSha256'])
        self.assertEqual(pin['priorCodexOracleSha256'], previous['executableSha256'])
        candidate = ROOT / pin['oraclePath']
        prior = ROOT / previous['oraclePath']
        if candidate.exists():
            self.assertFalse(candidate.is_symlink())
            self.assertEqual(hashlib.sha256(candidate.read_bytes()).hexdigest(), pin['executableSha256'])
            self.assertEqual(candidate.stat().st_mode & 0o777, 0o555)
            if prior.exists():
                self.assertNotEqual((candidate.stat().st_dev, candidate.stat().st_ino),
                                    (prior.stat().st_dev, prior.stat().st_ino))


class ClaudeJsonCandidateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CLAUDE_JSON_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CLAUDE_JSON_CANDIDATE for candidate execution.')
        cls.binary = Path(selected).resolve(strict=True)
        expected = os.environ.get('OPENUSAGE_CLAUDE_JSON_SHA256')
        if not expected:
            expected = json.loads((FIXTURE / 'pin.json').read_text())['executableSha256']
        if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != expected:
            raise ValueError('The scanner digest differs from its selected pin.')

    def test_jsonl_formatting_preserves_independent_counts_in_every_report(self):
        controls = {}
        for variant in VARIANTS:
            with tempfile.TemporaryDirectory(prefix='openusage-claude-json-') as directory:
                root = Path(directory)
                prepare_variant(root, variant)
                before = file_hashes(root)
                for kind, command in COMMANDS['claude']:
                    with self.subTest(variant=variant, report=kind):
                        results = [run_report(self.binary, root, kind,
                                              variable='CLAUDE_CONFIG_DIR', command=command)
                                   for _ in range(2)]
                        for result in results:
                            self.assertEqual(result['exitCode'], 0, result['stderr'])
                            self.assertEqual(result['measurement']['state'], 'observations')
                            self.assertFalse(result['measurement']['invariantErrors'])
                            for field, count in EXPECTED['claude'].items():
                                self.assertEqual(result['measurement']['tokenCounts'][field], count)
                        self.assertEqual(results[0]['report'], results[1]['report'])
                        if variant == 'compact':
                            controls[kind] = results[0]['report']
                        else:
                            self.assertEqual(results[0]['report'], controls[kind])
                self.assertEqual(file_hashes(root), before)

    def test_usage_text_and_wrong_path_objects_create_no_observations(self):
        for variant in ('compact', 'spaces', 'escaped_usage'):
            with tempfile.TemporaryDirectory(prefix='openusage-claude-decoys-') as directory:
                root = Path(directory)
                prepare_variant(root, variant, decoys=True)
                before = file_hashes(root)
                for kind, command in COMMANDS['claude']:
                    with self.subTest(variant=variant, report=kind):
                        result = run_report(self.binary, root, kind,
                                            variable='CLAUDE_CONFIG_DIR', command=command)
                        self.assertEqual(result['exitCode'], 0, result['stderr'])
                        self.assertEqual(result['measurement']['state'], 'no_observations')
                        self.assertIsNone(result['measurement']['totalTokens'])
                self.assertEqual(file_hashes(root), before)

    def test_non_object_usage_creates_no_observations(self):
        original = sorted((FIXTURES / 'home/.claude').rglob('*.jsonl'))[0]
        template = json.loads(original.read_text().splitlines()[0])
        for usage in (None, False, 17, '{}', [], [0] * 6, [1] * 6,
                      [11, 3, 7, 13, 'standard', {}], [0, 0, 0, 0, 'standard', {}]):
            for variant in ('compact', 'spaces', 'escaped_usage'):
                with tempfile.TemporaryDirectory(prefix='openusage-claude-shape-') as directory:
                    root = Path(directory)
                    destination = root / 'projects/synthetic/session.jsonl'
                    destination.parent.mkdir(parents=True)
                    row = json.loads(json.dumps(template))
                    row['message']['usage'] = usage
                    destination.write_text(render(row, variant))
                    before = file_hashes(root)
                    for kind, command in COMMANDS['claude']:
                        with self.subTest(usage=usage, variant=variant, report=kind):
                            result = run_report(self.binary, root, kind,
                                                variable='CLAUDE_CONFIG_DIR', command=command)
                            self.assertEqual(result['exitCode'], 0, result['stderr'])
                            self.assertEqual(result['measurement']['state'], 'no_observations')
                            self.assertIsNone(result['measurement']['totalTokens'])
                    self.assertEqual(file_hashes(root), before)

    def test_agent_progress_keeps_its_existing_daily_only_scope(self):
        original = sorted((FIXTURES / 'home/.claude').rglob('*.jsonl'))[0]
        template = json.loads(original.read_text().splitlines()[0])
        row = {'sessionId': template['sessionId'], 'data': {'message': {
            'timestamp': template['timestamp'], 'message': template['message']}}}
        for variant in ('compact', 'spaces', 'escaped_usage'):
            with tempfile.TemporaryDirectory(prefix='openusage-claude-progress-') as directory:
                root = Path(directory)
                destination = root / 'projects/synthetic/session.jsonl'
                destination.parent.mkdir(parents=True)
                destination.write_text(render(row, variant))
                before = file_hashes(root)
                for kind, command in COMMANDS['claude']:
                    with self.subTest(variant=variant, report=kind):
                        result = run_report(self.binary, root, kind,
                                            variable='CLAUDE_CONFIG_DIR', command=command)
                        self.assertEqual(result['exitCode'], 0, result['stderr'])
                        if kind == 'daily':
                            self.assertEqual(result['measurement']['totalTokens'], 4700)
                        else:
                            self.assertEqual(result['measurement']['state'], 'no_observations')
                            self.assertIsNone(result['measurement']['totalTokens'])
                self.assertEqual(file_hashes(root), before)

    def test_ignored_metadata_preserves_existing_session_and_block_reports(self):
        pin = json.loads((FIXTURE / 'pin.json').read_text())
        base = json.loads((ROOT / pin['baseCandidatePin']['path']).read_text())
        reference = ROOT / base['oraclePath']
        self.assertEqual(hashlib.sha256(reference.read_bytes()).hexdigest(), base['executableSha256'])
        for name, literal in [('large-number', '1e400'),
                              ('deep-array', '[' * 140 + '0' + ']' * 140)]:
            with tempfile.TemporaryDirectory(prefix='openusage-claude-metadata-') as directory:
                root = Path(directory)
                for original in sorted((FIXTURES / 'home/.claude').rglob('*.jsonl')):
                    destination = root / original.relative_to(FIXTURES / 'home/.claude')
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    rows = [json.loads(line) for line in original.read_text().splitlines() if line.strip()]
                    rows = [json.dumps({key: value for key, value in row.items() if key != 'parentUuid'},
                                       separators=(',', ':')) for row in rows]
                    destination.write_text(''.join(
                        row[:-1] + ',"unrelated":' + literal + '}\n' for row in rows))
                before = file_hashes(root)
                for kind, command in COMMANDS['claude']:
                    if kind == 'daily':
                        continue
                    with self.subTest(metadata=name, report=kind):
                        prior = run_report(reference, root, kind,
                                           variable='CLAUDE_CONFIG_DIR', command=command)
                        current = run_report(self.binary, root, kind,
                                             variable='CLAUDE_CONFIG_DIR', command=command)
                        self.assertEqual(prior['exitCode'], 0, prior['stderr'])
                        self.assertEqual(prior['measurement']['totalTokens'], 14950)
                        self.assertEqual(current['exitCode'], 0, current['stderr'])
                        self.assertEqual(current['measurement']['totalTokens'], 14950)
                        self.assertEqual(current['report'], prior['report'])
                self.assertEqual(file_hashes(root), before)

    def test_legacy_matrix_preserves_reports_except_corrected_claude_whitespace(self):
        reference = ROOT / 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu'
        reference_hash = json.loads(PIN.read_text())['executableSha256']
        self.assertEqual(hashlib.sha256(reference.read_bytes()).hexdigest(), reference_hash)
        candidate_hash = hashlib.sha256(self.binary.read_bytes()).hexdigest()
        output = os.environ.get('OPENUSAGE_CLAUDE_JSON_RESULTS')
        if output and Path(output).exists():
            raise ValueError('Select a new private result path.')
        receipt = {'scope': 'Synthetic Linux scanner commands only.',
                   'comparisons': [], 'complete': False, 'sourcesUnchanged': True,
                   'productionQualificationPassed': False}
        try:
            with tempfile.TemporaryDirectory(prefix='openusage-claude-matrix-') as directory:
                for case, tool in fixture_pairs():
                    root = Path(directory) / case / tool
                    root.mkdir(parents=True)
                    selected = prepare(root, tool, case)
                    before = file_hashes(root)
                    for kind, command in COMMANDS[tool]:
                        with self.subTest(case=case, tool=tool, report=kind):
                            previous = run_report(reference, selected, kind,
                                                  variable=TOOLS[tool][0], command=command)
                            current = [run_report(self.binary, selected, kind,
                                                  variable=TOOLS[tool][0], command=command)
                                       for _ in range(2)]
                            expected = expectation(tool, case, kind)
                            corrected = tool == 'claude' and case == 'spaced_json'
                            self.assertEqual(current[0]['report'], current[1]['report'])
                            for result in current:
                                self.assertFalse(result['measurement']['invariantErrors'])
                                if corrected:
                                    self.assertTrue(matches_expectation(result, result['measurement'], expected))
                                else:
                                    self.assertEqual(result['exitCode'], previous['exitCode'])
                                    self.assertEqual(result['report'], previous['report'])
                            receipt['comparisons'].append({
                                'case': case, 'tool': tool, 'report': kind,
                                'intentionalCorrection': corrected,
                                'referenceExpectationMatched': matches_expectation(
                                    previous, previous['measurement'], expected),
                                'candidateExpectationMatched': matches_expectation(
                                    current[0], current[0]['measurement'], expected)})
                    unchanged = before == file_hashes(root)
                    receipt['sourcesUnchanged'] &= unchanged
                    self.assertTrue(unchanged)
            self.assertEqual(len(receipt['comparisons']), 47)
            receipt['complete'] = True
        finally:
            receipt['executableHashesUnchanged'] = (
                hashlib.sha256(reference.read_bytes()).hexdigest() == reference_hash
                and hashlib.sha256(self.binary.read_bytes()).hexdigest() == candidate_hash)
            receipt['intentionalCorrections'] = sum(row['intentionalCorrection'] for row in receipt['comparisons'])
            receipt['remainingIndependentFailures'] = sum(
                not row['candidateExpectationMatched'] for row in receipt['comparisons'])
            if output:
                with Path(output).open('x') as stream:
                    json.dump(receipt, stream, indent=2)
                    stream.write('\n')
        self.assertTrue(receipt['executableHashesUnchanged'])
