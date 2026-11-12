"""Public legacy identity controls with private synthetic input."""
import copy
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path
import tempfile
import unittest
import test_ccusage_claude_capture as capture

from test_ccusage_codex_candidate import run_report
from test_ccusage_formats import file_hashes
from test_ccusage_shared_candidate import (COMMANDS, TOOLS, expectation, fixture_pairs,
                                          matches_expectation, prepare)

ROOT = Path(__file__).resolve().parents[2]
PARENT_PIN = ROOT / 'tests/tools/fixtures/claude-capture-candidate/pin.json'
FIXTURE = ROOT / 'tests/tools/fixtures/codex-legacy-identity-candidate'


class CodexLegacyIdentityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CODEX_LEGACY_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CODEX_LEGACY_CANDIDATE for scanner execution.')
        cls.binary = Path(selected).resolve(strict=True)
        expected = os.environ.get('OPENUSAGE_CODEX_LEGACY_SHA256')
        if not expected:
            expected = json.loads((FIXTURE / 'pin.json').read_text())['executableSha256']
        if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != expected:
            raise ValueError('The scanner digest differs from its selected pin.')

    def test_copied_and_distinct_threads_meet_independent_totals(self):
        for case in ('baseline', 'copied_roots', 'repeated_root', 'renamed_copy', 'distinct_equal_usage'):
            with tempfile.TemporaryDirectory(prefix='openusage-legacy-identity-') as directory:
                root = Path(directory)
                selected = prepare(root, 'codex', case)
                before = file_hashes(root)
                for kind, command in COMMANDS['codex']:
                    with self.subTest(case=case, report=kind):
                        result = run_report(self.binary, selected, kind, command=command)
                        self.assertTrue(matches_expectation(result, result['measurement'],
                                                            expectation('codex', case, kind)))
                self.assertEqual(file_hashes(root), before)

    def test_unproven_or_conflicting_legacy_identity_refuses(self):
        for case in ('missing-id', 'missing-header', 'duplicate-id', 'escaped-duplicate-id',
                     'fork', 'subagent', 'foreign-header', 'conflict', 'decrease',
                     'replay-marker', 'compaction', 'revert'):
            with tempfile.TemporaryDirectory(prefix='openusage-legacy-ambiguous-') as directory:
                root = Path(directory)
                selected = prepare(root, 'codex', 'baseline')
                source = sorted(Path(selected).rglob('*.jsonl'))[0]
                rows = [json.loads(line) for line in source.read_text().splitlines()]
                if case == 'missing-id':
                    del rows[0]['payload']['id']
                elif case == 'missing-header':
                    del rows[0]
                elif case == 'fork':
                    rows[0]['payload']['forked_from_id'] = 'unselected-parent'
                elif case == 'subagent':
                    rows[0]['payload']['source'] = {'subagent': {'thread_spawn': {
                        'parent_thread_id': 'unselected-parent'}}}
                elif case == 'foreign-header':
                    foreign = json.loads(json.dumps(rows[0]))
                    foreign['payload']['id'] = 'foreign-thread'
                    rows.insert(1, foreign)
                elif case == 'replay-marker':
                    rows.append({'type': 'inter_agent_communication_metadata',
                                 'payload': {'trigger_turn': True}})
                elif case == 'compaction':
                    rows.append({'type': 'compacted', 'payload': {'compaction_response_id': 'unproven'}})
                elif case == 'revert':
                    rows.append({'type': 'event_msg', 'payload': {'type': 'thread_rolled_back'}})
                elif case == 'conflict':
                    peer = source.with_name('renamed-conflicting-copy.jsonl')
                    changed = json.loads(json.dumps(rows))
                    changed[1]['payload']['model'] = 'gpt-5'
                    peer.write_text(''.join(json.dumps(row) + '\n' for row in changed))
                elif case == 'decrease':
                    decreased = json.loads(json.dumps(rows[-1]))
                    for category in ('total_token_usage', 'last_token_usage'):
                        for key in decreased['payload']['info'][category]:
                            decreased['payload']['info'][category][key] //= 2
                    rows.append(decreased)
                source.write_text(''.join(json.dumps(row) + '\n' for row in rows))
                if case in ('duplicate-id', 'escaped-duplicate-id'):
                    raw = source.read_text()
                    key = 'id' if case == 'duplicate-id' else 'i\\u0064'
                    source.write_text(raw.replace('"payload": {', '"payload": {"' + key + '": "other",', 1))
                before = file_hashes(root)
                for kind, command in COMMANDS['codex']:
                    with self.subTest(case=case, report=kind):
                        result = run_report(self.binary, selected, kind, command=command)
                        self.assertNotEqual(result['exitCode'], 0)
                        self.assertIsNone(result['report'])
                        self.assertEqual(result['stdoutBytes'], 0)
                        self.assertNotIn(str(root), result['stderr'])
                        self.assertNotIn('unselected-parent', result['stderr'])
                self.assertEqual(file_hashes(root), before)

    def test_advancing_equal_usage_counters_and_exact_prefix_copies_keep_usage(self):
        for copied in (False, True):
            with tempfile.TemporaryDirectory(prefix='openusage-legacy-prefix-') as directory:
                root = Path(directory)
                selected = prepare(root, 'codex', 'baseline')
                source = sorted(Path(selected).rglob('*.jsonl'))[0]
                rows = [json.loads(line) for line in source.read_text().splitlines()]
                advanced = json.loads(json.dumps(rows[-1]))
                for key, value in advanced['payload']['info']['total_token_usage'].items():
                    advanced['payload']['info']['total_token_usage'][key] = value * 2
                target = source.with_name('renamed-extended-copy.jsonl') if copied else source
                target.write_text(''.join(json.dumps(row) + '\n' for row in rows + [advanced]))
                before = file_hashes(root)
                for kind, command in COMMANDS['codex']:
                    with self.subTest(copied=copied, report=kind):
                        result = run_report(self.binary, selected, kind, command=command)
                        self.assertEqual(result['exitCode'], 0, result['stderr'])
                        self.assertEqual(result['measurement']['totalTokens'], 9200)
                self.assertEqual(file_hashes(root), before)

    def test_prior_capture_and_claude_json_controls_remain_valid(self):
        names = unittest.defaultTestLoader.getTestCaseNames(capture.ClaudeCaptureCandidateTests)
        for name in names:
            if name == 'test_legacy_matrix_changes_only_malformed_peer_refusal':
                continue
            with self.subTest(parent=name):
                case = capture.ClaudeCaptureCandidateTests(name)
                case.binary = self.binary
                getattr(case, name)()

    def test_legacy_matrix_names_numeric_and_session_metadata_corrections(self):
        parent_pin = json.loads(PARENT_PIN.read_text())
        parent = ROOT / parent_pin['oraclePath']
        parent_hash = hashlib.sha256(parent.read_bytes()).hexdigest()
        self.assertEqual(parent_hash, parent_pin['executableSha256'])
        candidate_hash = hashlib.sha256(self.binary.read_bytes()).hexdigest()
        output = os.environ.get('OPENUSAGE_CODEX_LEGACY_RESULTS')
        if output and Path(output).exists():
            raise ValueError('Select a new private result path.')
        receipt = {'scope': 'Synthetic Linux legacy identity and metadata only.',
                   'candidateSha256': candidate_hash, 'parentSha256': parent_hash,
                   'comparisons': [], 'complete': False, 'sourcesUnchanged': True,
                   'productionQualificationPassed': False}
        def without_path_metadata(report):
            value = copy.deepcopy(report)
            for row in value.get('sessions', []):
                for key in ('sessionId', 'sessionFile', 'directory'):
                    row.pop(key, None)
            if 'sessions' in value:
                value['sessions'].sort(key=lambda row: json.dumps(row, sort_keys=True))
            return value
        controls = {}
        try:
            with tempfile.TemporaryDirectory(prefix='openusage-legacy-matrix-') as directory:
                for case, tool in fixture_pairs():
                    root = Path(directory) / case / tool
                    root.mkdir(parents=True)
                    selected = prepare(root, tool, case)
                    before = file_hashes(root)
                    for kind, command in COMMANDS[tool]:
                        with self.subTest(case=case, tool=tool, report=kind):
                            previous = run_report(parent, selected, kind,
                                                  variable=TOOLS[tool][0], command=command)
                            current = [run_report(self.binary, selected, kind,
                                                  variable=TOOLS[tool][0], command=command)
                                       for _ in range(2)]
                            expected = expectation(tool, case, kind)
                            numeric = tool == 'codex' and (case, kind) in (
                                ('renamed_copy', 'sessions'), ('distinct_equal_usage', 'daily'))
                            metadata = tool == 'codex' and kind == 'sessions' and expected['outcome'] == 'observations'
                            if case == 'baseline':
                                controls[tool, kind] = previous['report']
                            self.assertEqual(current[0]['report'], current[1]['report'])
                            self.assertEqual(current[0]['exitCode'], current[1]['exitCode'])
                            for result in current:
                                self.assertTrue(matches_expectation(result, result['measurement'], expected))
                                self.assertFalse(result['measurement']['invariantErrors'])
                                if expected['outcome'] == 'refusal':
                                    self.assertEqual(result['stdoutBytes'], 0)
                            if metadata:
                                report = current[0]['report']
                                ids = {'22222222-bbbb-4bbb-8bbb-222222222222'}
                                if case == 'distinct_equal_usage':
                                    ids.add('55555555-eeee-4eee-8eee-555555555555')
                                self.assertEqual({row['sessionId'] for row in report['sessions']}, ids)
                                for row in report['sessions']:
                                    self.assertEqual(row['sessionFile'], row['sessionId'])
                                    self.assertEqual(row['directory'], '')
                                reference = controls[tool, kind] if case == 'renamed_copy' else previous['report']
                                self.assertEqual(without_path_metadata(report), without_path_metadata(reference))
                            elif not numeric:
                                self.assertEqual(current[0]['exitCode'], previous['exitCode'])
                                self.assertEqual(current[0]['report'], previous['report'])
                            receipt['comparisons'].append({'case': case, 'tool': tool, 'report': kind,
                                'numericCorrection': numeric, 'sessionMetadataCorrection': metadata,
                                'independentExpectationMatched': matches_expectation(
                                    current[0], current[0]['measurement'], expected)})
                    receipt['sourcesUnchanged'] &= before == file_hashes(root)
                    self.assertTrue(receipt['sourcesUnchanged'])
            self.assertEqual(len(receipt['comparisons']), 47)
            receipt['complete'] = True
        finally:
            receipt['executableHashesUnchanged'] = (
                hashlib.sha256(parent.read_bytes()).hexdigest() == parent_hash
                and hashlib.sha256(self.binary.read_bytes()).hexdigest() == candidate_hash)
            receipt['numericCorrections'] = sum(row['numericCorrection'] for row in receipt['comparisons'])
            receipt['sessionMetadataCorrections'] = sum(row['sessionMetadataCorrection'] for row in receipt['comparisons'])
            receipt['remainingIndependentFailures'] = sum(
                not row['independentExpectationMatched'] for row in receipt['comparisons'])
            if output:
                with Path(output).open('x') as stream:
                    json.dump(receipt, stream, indent=2)
                    stream.write('\n')
        self.assertTrue(receipt['executableHashesUnchanged'])
        self.assertEqual(receipt['numericCorrections'], 2)
        self.assertEqual(receipt['sessionMetadataCorrections'], 8)
        self.assertEqual(receipt['remainingIndependentFailures'], 0)

    def test_distinct_threads_with_the_same_relative_path_stay_separate(self):
        with tempfile.TemporaryDirectory(prefix='openusage-legacy-homes-') as directory:
            root = Path(directory)
            first = Path(prepare(root / 'first', 'codex', 'baseline'))
            second = root / 'second'
            shutil.copytree(first, second)
            source = sorted(second.rglob('*.jsonl'))[0]
            rows = [json.loads(line) for line in source.read_text().splitlines()]
            rows[0]['payload']['id'] = '55555555-eeee-4eee-8eee-555555555555'
            source.write_text(''.join(json.dumps(row) + '\n' for row in rows))
            before = file_hashes(root)
            controls = {}
            for roots in ((first, second), (second, first)):
                for kind, command in COMMANDS['codex']:
                    result = run_report(self.binary, ','.join(map(str, roots)), kind, command=command)
                    self.assertEqual(result['exitCode'], 0, result['stderr'])
                    self.assertEqual(result['measurement']['totalTokens'], 9200)
                    if kind == 'sessions':
                        self.assertEqual(len(result['report']['sessions']), 2)
                    if kind in controls:
                        self.assertEqual(result['report'], controls[kind])
                    controls[kind] = result['report']
            self.assertEqual(file_hashes(root), before)

    def test_nonusage_record_conflicts_cannot_be_hidden_by_equal_counts(self):
        with tempfile.TemporaryDirectory(prefix='openusage-legacy-content-') as directory:
            root = Path(directory)
            selected = prepare(root, 'codex', 'baseline')
            source = sorted(Path(selected).rglob('*.jsonl'))[0]
            rows = [json.loads(line) for line in source.read_text().splitlines()]
            rows[1]['payload']['private_note'] = 'private-marker'
            peer = source.with_name('renamed-copy.jsonl')
            peer.write_text(''.join(json.dumps(row) + '\n' for row in rows))
            before = file_hashes(root)
            for kind, command in COMMANDS['codex']:
                result = run_report(self.binary, selected, kind, command=command)
                self.assertNotEqual(result['exitCode'], 0)
                self.assertEqual(result['stdoutBytes'], 0)
                self.assertNotIn('private-marker', result['stderr'])
                self.assertNotIn(str(root), result['stderr'])
            self.assertEqual(file_hashes(root), before)

    def test_decoded_record_copies_and_compressed_copies_count_once(self):
        compressor = shutil.which('zstd')
        if not compressor:
            self.skipTest('Compressed controls need the zstd command.')
        for compressed in (False, True):
            with tempfile.TemporaryDirectory(prefix='openusage-legacy-encoding-') as directory:
                root = Path(directory)
                selected = prepare(root, 'codex', 'baseline')
                source = sorted(Path(selected).rglob('*.jsonl'))[0]
                rows = [json.loads(line) for line in source.read_text().splitlines()]
                content = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\r\n'
                                  for row in rows).encode()
                if compressed:
                    result = subprocess.run([compressor, '-q', '-c'], input=content,
                                            capture_output=True, timeout=10,
                                            env={'PATH': '/usr/bin:/bin', 'HOME': str(root)})
                    self.assertEqual(result.returncode, 0)
                    content = result.stdout
                source.with_name('renamed-copy.jsonl' + ('.zst' if compressed else '')).write_bytes(content)
                before = file_hashes(root)
                for extra in ((), ('--single-thread',)):
                    for kind, command in COMMANDS['codex']:
                        result = run_report(self.binary, selected, kind, command=command, extra=extra)
                        self.assertEqual(result['exitCode'], 0, result['stderr'])
                        self.assertEqual(result['measurement']['totalTokens'], 4600)
                self.assertEqual(file_hashes(root), before)

    def test_repeated_counter_snapshot_is_no_second_usage(self):
        with tempfile.TemporaryDirectory(prefix='openusage-legacy-counter-') as directory:
            root = Path(directory)
            selected = prepare(root, 'codex', 'baseline')
            source = sorted(Path(selected).rglob('*.jsonl'))[0]
            lines = source.read_bytes().splitlines(keepends=True)
            source.write_bytes(b''.join(lines + [lines[-1]]))
            before = file_hashes(root)
            for kind, command in COMMANDS['codex']:
                result = run_report(self.binary, selected, kind, command=command)
                self.assertEqual(result['exitCode'], 0, result['stderr'])
                self.assertEqual(result['measurement']['totalTokens'], 4600)
            self.assertEqual(file_hashes(root), before)

    def test_ambiguous_counter_categories_refuse_instead_of_becoming_zero(self):
        for case in ('negative', 'fraction', 'string', 'null', 'missing',
                     'cached-over-input', 'reason-over-output', 'last-total-disagreement', 'alias-conflict'):
            with tempfile.TemporaryDirectory(prefix='openusage-legacy-count-') as directory:
                root = Path(directory)
                selected = prepare(root, 'codex', 'baseline')
                source = sorted(Path(selected).rglob('*.jsonl'))[0]
                rows = [json.loads(line) for line in source.read_text().splitlines()]
                info = rows[-1]['payload']['info']
                for name in ('last_token_usage', 'total_token_usage'):
                    value = info[name]
                    if case in ('negative', 'fraction', 'string', 'null'):
                        value['input_tokens'] = {'negative': -1, 'fraction': 0.5,
                                                 'string': '4000', 'null': None}[case]
                    elif case == 'missing':
                        del value['input_tokens']
                    elif case == 'cached-over-input':
                        value['cached_input_tokens'] = 4001
                    elif case == 'reason-over-output':
                        value['reasoning_output_tokens'] = 601
                    elif case == 'alias-conflict':
                        value['cache_write_input_tokens'] = 0
                        value['cache_creation_input_tokens'] = 0
                if case == 'last-total-disagreement':
                    info['last_token_usage']['input_tokens'] = 3999
                    info['last_token_usage']['total_tokens'] = 4599
                source.write_text(''.join(json.dumps(row) + '\n' for row in rows))
                before = file_hashes(root)
                for kind, command in COMMANDS['codex']:
                    result = run_report(self.binary, selected, kind, command=command)
                    self.assertNotEqual(result['exitCode'], 0, case)
                    self.assertIsNone(result['report'])
                    self.assertEqual(result['stdoutBytes'], 0)
                self.assertEqual(file_hashes(root), before)

    def test_joint_exact_count_limit_refuses_before_aggregation(self):
        with tempfile.TemporaryDirectory(prefix='openusage-legacy-exact-') as directory:
            root = Path(directory)
            selected = prepare(root, 'codex', 'baseline')
            source = sorted(Path(selected).rglob('*.jsonl'))[0]
            rows = [json.loads(line) for line in source.read_text().splitlines()]
            usage = {'input_tokens': 1 << 52, 'cached_input_tokens': 0,
                     'output_tokens': 0, 'reasoning_output_tokens': 0, 'total_tokens': 1 << 52}
            for name in ('last_token_usage', 'total_token_usage'):
                rows[-1]['payload']['info'][name] = dict(usage)
            source.write_text(''.join(json.dumps(row) + '\n' for row in rows))
            rows[0]['payload']['id'] = '55555555-eeee-4eee-8eee-555555555555'
            source.with_name('distinct-thread.jsonl').write_text(
                ''.join(json.dumps(row) + '\n' for row in rows))
            before = file_hashes(root)
            for kind, command in COMMANDS['codex']:
                result = run_report(self.binary, selected, kind, command=command)
                self.assertNotEqual(result['exitCode'], 0)
                self.assertIsNone(result['report'])
                self.assertEqual(result['stdoutBytes'], 0)
            self.assertEqual(file_hashes(root), before)


class LegacyIdentitySourceTests(unittest.TestCase):
    def test_fixture_version_stays_distinct_from_the_qualified_producer(self):
        from test_ccusage_shared_candidate import FIXTURES
        source = sorted((FIXTURES / 'codex').rglob('*.jsonl'))[0]
        fixture = json.loads(source.read_text().splitlines()[0])
        producer = json.loads((ROOT / 'tests/tools/fixtures/codex-producer/capture.json').read_text())
        self.assertEqual(fixture['payload']['cli_version'], '0.166.0')
        self.assertEqual(producer['producerVersion'], '0.163.0')
        self.assertNotEqual(fixture['payload']['cli_version'], producer['producerVersion'])


class LegacyIdentityPinTests(unittest.TestCase):
    def test_patch_manifest_and_separate_oracles_match_the_pin(self):
        pin = json.loads((FIXTURE / 'pin.json').read_text())
        parent_file = ROOT / pin['baseCandidatePin']['path']
        self.assertEqual(hashlib.sha256(parent_file.read_bytes()).hexdigest(),
                         pin['baseCandidatePin']['sha256'])
        parent = json.loads(parent_file.read_text())
        self.assertEqual(pin['patches'][:-1], parent['patches'])
        self.assertEqual(pin['sourceCommit'], parent['sourceCommit'])
        for patch in pin['patches']:
            self.assertEqual(hashlib.sha256((ROOT / patch['path']).read_bytes()).hexdigest(), patch['sha256'])
        manifest = FIXTURE / 'source-files.json'
        self.assertEqual(hashlib.sha256(manifest.read_bytes()).hexdigest(), pin['sourceFilesSha256'])
        records = json.loads(manifest.read_text())
        self.assertEqual(hashlib.sha256(json.dumps(records, separators=(',', ':')).encode()).hexdigest(),
                         pin['sourceManifestSha256'])
        names = []
        source = ROOT / pin['sourceDirectory']
        for record in records:
            self.assertEqual(set(record), {'path', 'sha256'})
            path = Path(record['path'])
            self.assertFalse(path.is_absolute())
            self.assertNotIn('..', path.parts)
            self.assertRegex(record['sha256'], r'\A[0-9a-f]{64}\Z')
            names.append(record['path'])
            if source.exists():
                self.assertEqual(hashlib.sha256((source / path).read_bytes()).hexdigest(), record['sha256'])
        self.assertEqual(names, sorted(set(names)))
        identities = set()
        for path, expected in ((ROOT / pin['oraclePath'], pin['executableSha256']),
                               (ROOT / parent['oraclePath'], pin['priorCaptureOracleSha256'])):
            if path.exists():
                self.assertFalse(path.is_symlink())
                self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), expected)
                self.assertEqual(path.stat().st_mode & 0o777, 0o555)
                identities.add((path.stat().st_dev, path.stat().st_ino))
        if (ROOT / pin['oraclePath']).exists() and (ROOT / parent['oraclePath']).exists():
            self.assertEqual(len(identities), 2)

    def test_patch_reconstructs_the_exact_parent_source(self):
        pin = json.loads((FIXTURE / 'pin.json').read_text())
        parent = json.loads((ROOT / pin['baseCandidatePin']['path']).read_text())
        base_source = ROOT / parent['sourceDirectory']
        if not base_source.exists():
            self.skipTest('The private pinned parent source is required for reconstruction.')
        parent_manifest = Path(pin['baseCandidatePin']['path']).parent / 'source-files.json'
        records = json.loads((ROOT / parent_manifest).read_text())
        with tempfile.TemporaryDirectory(prefix='openusage-legacy-reconstruct-') as directory:
            root = Path(directory)
            for record in records:
                original = base_source / record['path']
                self.assertEqual(hashlib.sha256(original.read_bytes()).hexdigest(), record['sha256'])
                target = root / record['path']
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(original, target)
            patch = ROOT / pin['patches'][-1]['path']
            result = subprocess.run(['patch', '--batch', '--fuzz=0', '-p1', '-i', str(patch)],
                                    cwd=root, capture_output=True, text=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stderr)
            for record in json.loads((FIXTURE / 'source-files.json').read_text()):
                self.assertEqual(hashlib.sha256((root / record['path']).read_bytes()).hexdigest(), record['sha256'])


if __name__ == '__main__':
    unittest.main(verbosity=2)
