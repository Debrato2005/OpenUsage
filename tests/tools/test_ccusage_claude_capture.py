"""Check whole-report refusal for invalid legacy JSONL captures."""
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path
import tempfile
import test_ccusage_claude_json as formats
import unittest

from test_ccusage_codex_candidate import run_report
from test_ccusage_formats import file_hashes
from test_ccusage_shared_candidate import (COMMANDS, TOOLS, expectation, fixture_pairs,
                                          matches_expectation, prepare)

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/tools/fixtures/claude-capture-candidate'


class ClaudeCaptureCandidateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CLAUDE_CAPTURE_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CLAUDE_CAPTURE_CANDIDATE for scanner execution.')
        cls.binary = Path(selected).resolve(strict=True)
        expected = os.environ.get('OPENUSAGE_CLAUDE_CAPTURE_SHA256')
        if not expected:
            expected = json.loads((FIXTURE / 'pin.json').read_text())['executableSha256']
        if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != expected:
            raise ValueError('The scanner digest differs from its selected pin.')

    def test_malformed_peer_refuses_every_legacy_report(self):
        for tool in TOOLS:
            with tempfile.TemporaryDirectory(prefix='openusage-capture-peer-') as directory:
                root = Path(directory)
                selected = prepare(root, tool, 'malformed_peer')
                before = file_hashes(root)
                for kind, command in COMMANDS[tool]:
                    with self.subTest(tool=tool, report=kind):
                        result = run_report(self.binary, selected, kind,
                                            variable=TOOLS[tool][0], command=command)
                        self.assertNotEqual(result['exitCode'], 0)
                        self.assertIsNone(result['report'])
                        self.assertEqual(result['stdoutBytes'], 0)
                        self.assertNotIn('44444444', result['stderr'])
                        self.assertNotIn(str(root), result['stderr'])
                self.assertEqual(file_hashes(root), before)

    def test_invalid_records_refuse_before_between_and_after_valid_records(self):
        invalid = (b'{"private-marker":', b'{"private-marker":}\n',
                   b'{} {}\n', b'{"private-marker":"\xff"}\n',
                   b'{"ignored":[1,]}\n')
        for tool in TOOLS:
            for index, bad in enumerate(invalid):
                for location in ('before', 'between', 'after'):
                    with tempfile.TemporaryDirectory(prefix='openusage-capture-line-') as directory:
                        root = Path(directory)
                        selected = prepare(root, tool, 'baseline')
                        source = sorted(Path(selected).rglob('*.jsonl'))[0]
                        lines = source.read_bytes().splitlines(keepends=True)
                        offset = {'before': 0, 'between': 1, 'after': len(lines)}[location]
                        lines.insert(offset, bad if location == 'after' else bad.rstrip(b'\n') + b'\n')
                        source.write_bytes(b''.join(lines))
                        before = file_hashes(root)
                        for kind, command in COMMANDS[tool]:
                            with self.subTest(tool=tool, record=index, location=location, report=kind):
                                result = run_report(self.binary, selected, kind,
                                                    variable=TOOLS[tool][0], command=command)
                                self.assertNotEqual(result['exitCode'], 0)
                                self.assertIsNone(result['report'])
                                self.assertEqual(result['stdoutBytes'], 0)
                                self.assertNotIn('private-marker', result['stderr'])
                                self.assertNotIn(str(root), result['stderr'])
                        self.assertEqual(file_hashes(root), before)

    def test_valid_blank_lines_and_final_record_without_newline_preserve_reports(self):
        for tool in TOOLS:
            with tempfile.TemporaryDirectory(prefix='openusage-capture-valid-') as directory:
                root = Path(directory)
                selected = prepare(root, tool, 'baseline')
                controls = {kind: run_report(self.binary, selected, kind,
                                            variable=TOOLS[tool][0], command=command)
                            for kind, command in COMMANDS[tool]}
                for source in Path(selected).rglob('*.jsonl'):
                    source.write_bytes(b' \t\n\r\n' + b'\n'.join(source.read_bytes().splitlines()))
                before = file_hashes(root)
                for kind, command in COMMANDS[tool]:
                    result = run_report(self.binary, selected, kind,
                                        variable=TOOLS[tool][0], command=command)
                    self.assertEqual(result['exitCode'], 0, result['stderr'])
                    self.assertEqual(result['report'], controls[kind]['report'])
                    self.assertTrue(matches_expectation(result, result['measurement'],
                                                        expectation(tool, 'baseline', kind)))
                self.assertEqual(file_hashes(root), before)

    def test_prior_claude_json_and_schema_commands_remain_valid(self):
        for name in ('test_jsonl_formatting_preserves_independent_counts_in_every_report',
                     'test_usage_text_and_wrong_path_objects_create_no_observations',
                     'test_non_object_usage_creates_no_observations',
                     'test_agent_progress_keeps_its_existing_daily_only_scope',
                     'test_ignored_metadata_preserves_existing_session_and_block_reports'):
            with self.subTest(prior=name):
                case = formats.ClaudeJsonCandidateTests(name)
                case.binary = self.binary
                getattr(case, name)()

    def test_legacy_matrix_changes_only_malformed_peer_refusal(self):
        base = json.loads((formats.FIXTURE / 'pin.json').read_text())
        reference = ROOT / base['oraclePath']
        reference_hash = hashlib.sha256(reference.read_bytes()).hexdigest()
        self.assertEqual(reference_hash, base['executableSha256'])
        current_hash = hashlib.sha256(self.binary.read_bytes()).hexdigest()
        output = os.environ.get('OPENUSAGE_CLAUDE_CAPTURE_RESULTS')
        if output and Path(output).exists():
            raise ValueError('Select a new private result path.')
        receipt = {'scope': 'Synthetic Linux legacy JSONL commands only.',
                   'candidateSha256': current_hash, 'parentSha256': reference_hash,
                   'comparisons': [], 'complete': False, 'sourcesUnchanged': True,
                   'productionQualificationPassed': False}
        try:
            with tempfile.TemporaryDirectory(prefix='openusage-capture-matrix-') as directory:
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
                            corrected = case == 'malformed_peer'
                            self.assertEqual(current[0]['report'], current[1]['report'])
                            for result in current:
                                self.assertEqual(result['exitCode'], current[0]['exitCode'])
                                self.assertFalse(result['measurement']['invariantErrors'])
                                if corrected:
                                    self.assertTrue(matches_expectation(result, result['measurement'], expected))
                                    self.assertEqual(result['stdoutBytes'], 0)
                                else:
                                    self.assertEqual(result['exitCode'], previous['exitCode'])
                                    self.assertEqual(result['report'], previous['report'])
                            receipt['comparisons'].append({
                                'case': case, 'tool': tool, 'report': kind,
                                'intentionalCorrection': corrected,
                                'parentExpectationMatched': matches_expectation(
                                    previous, previous['measurement'], expected),
                                'candidateExpectationMatched': matches_expectation(
                                    current[0], current[0]['measurement'], expected)})
                    receipt['sourcesUnchanged'] &= before == file_hashes(root)
                    self.assertTrue(receipt['sourcesUnchanged'])
            self.assertEqual(len(receipt['comparisons']), 47)
            receipt['complete'] = True
        finally:
            receipt['executableHashesUnchanged'] = (
                hashlib.sha256(reference.read_bytes()).hexdigest() == reference_hash
                and hashlib.sha256(self.binary.read_bytes()).hexdigest() == current_hash)
            receipt['intentionalCorrections'] = sum(row['intentionalCorrection'] for row in receipt['comparisons'])
            receipt['remainingIndependentFailures'] = sum(
                not row['candidateExpectationMatched'] for row in receipt['comparisons'])
            if output:
                with Path(output).open('x') as stream:
                    json.dump(receipt, stream, indent=2)
                    stream.write('\n')
        self.assertTrue(receipt['executableHashesUnchanged'])
        self.assertEqual(receipt['intentionalCorrections'], 5)
        self.assertEqual(receipt['remainingIndependentFailures'], 2)

    def test_single_thread_controls_preserve_totals_and_refuse_malformed_peers(self):
        for case in ('baseline', 'malformed_peer'):
            for tool in TOOLS:
                with tempfile.TemporaryDirectory(prefix='openusage-capture-serial-') as directory:
                    root = Path(directory)
                    selected = prepare(root, tool, case)
                    before = file_hashes(root)
                    for kind, command in COMMANDS[tool]:
                        result = run_report(self.binary, selected, kind, extra=('--single-thread',),
                                            variable=TOOLS[tool][0], command=command)
                        self.assertTrue(matches_expectation(result, result['measurement'],
                                                            expectation(tool, case, kind)))
                        if case == 'malformed_peer':
                            self.assertEqual(result['stdoutBytes'], 0)
                    self.assertEqual(file_hashes(root), before)

    @unittest.skipIf(not hasattr(os, 'geteuid') or os.geteuid() == 0,
                     'Unreadable-file controls require an unprivileged Unix user.')
    def test_unreadable_peer_refuses_without_partial_output(self):
        for tool in TOOLS:
            with tempfile.TemporaryDirectory(prefix='openusage-capture-unreadable-') as directory:
                root = Path(directory)
                selected = prepare(root, tool, 'baseline')
                source = sorted(Path(selected).rglob('*.jsonl'))[0]
                before = file_hashes(root)
                mode = source.stat().st_mode & 0o777
                try:
                    source.chmod(0)
                    for extra in ((), ('--single-thread',)):
                        for kind, command in COMMANDS[tool]:
                            with self.subTest(tool=tool, report=kind, extra=extra):
                                result = run_report(self.binary, selected, kind, extra=extra,
                                                    variable=TOOLS[tool][0], command=command)
                                self.assertNotEqual(result['exitCode'], 0)
                                self.assertIsNone(result['report'])
                                self.assertEqual(result['stdoutBytes'], 0)
                                self.assertNotIn(str(root), result['stderr'])
                finally:
                    source.chmod(mode)
                self.assertEqual(file_hashes(root), before)

    def test_corrupt_compressed_codex_peer_refuses(self):
        with tempfile.TemporaryDirectory(prefix='openusage-capture-compressed-') as directory:
            root = Path(directory)
            selected = prepare(root, 'codex', 'baseline')
            source = sorted(Path(selected).rglob('*.jsonl'))[0]
            peer = source.with_name('private-marker.jsonl.zst')
            peer.write_bytes(b'not a Zstandard stream')
            before = file_hashes(root)
            for extra in ((), ('--single-thread',)):
                for kind, command in COMMANDS['codex']:
                    result = run_report(self.binary, selected, kind, extra=extra,
                                        variable=TOOLS['codex'][0], command=command)
                    self.assertNotEqual(result['exitCode'], 0)
                    self.assertIsNone(result['report'])
                    self.assertEqual(result['stdoutBytes'], 0)
                    self.assertNotIn('private-marker', result['stderr'])
                    self.assertNotIn(str(root), result['stderr'])
            self.assertEqual(file_hashes(root), before)

    def test_truncated_compressed_codex_capture_refuses_after_a_valid_frame(self):
        compressor = shutil.which('zstd')
        if not compressor:
            self.skipTest('The public compressed-frame control needs the zstd command.')
        with tempfile.TemporaryDirectory(prefix='openusage-capture-truncated-') as directory:
            root = Path(directory)
            selected = prepare(root, 'codex', 'baseline')
            source = sorted(Path(selected).rglob('*.jsonl'))[0]
            content = source.read_bytes().rstrip(b'\n') + b'\n'
            encoded = subprocess.run([compressor, '-q', '-c'], input=content,
                                     capture_output=True, timeout=10,
                                     env={'PATH': '/usr/bin:/bin', 'HOME': str(root)})
            self.assertEqual(encoded.returncode, 0, encoded.stderr)
            compressed = source.with_suffix('.jsonl.zst')
            compressed.write_bytes(encoded.stdout)
            source.unlink()
            for kind, command in COMMANDS['codex']:
                result = run_report(self.binary, selected, kind,
                                    variable=TOOLS['codex'][0], command=command)
                self.assertTrue(matches_expectation(result, result['measurement'],
                                                    expectation('codex', 'baseline', kind)))
            compressed.write_bytes(encoded.stdout + encoded.stdout[:len(encoded.stdout) // 2])
            before = file_hashes(root)
            for extra in ((), ('--single-thread',)):
                for kind, command in COMMANDS['codex']:
                    result = run_report(self.binary, selected, kind, extra=extra,
                                        variable=TOOLS['codex'][0], command=command)
                    self.assertNotEqual(result['exitCode'], 0)
                    self.assertIsNone(result['report'])
                    self.assertEqual(result['stdoutBytes'], 0)
                    self.assertNotIn(str(root), result['stderr'])
            self.assertEqual(file_hashes(root), before)


class ClaudeCapturePinTests(unittest.TestCase):
    def test_patch_manifest_and_separate_oracles_match_the_pin(self):
        pin = json.loads((FIXTURE / 'pin.json').read_text())
        parent_file = ROOT / pin['baseCandidatePin']['path']
        self.assertEqual(hashlib.sha256(parent_file.read_bytes()).hexdigest(),
                         pin['baseCandidatePin']['sha256'])
        parent = json.loads(parent_file.read_text())
        self.assertEqual(pin['patches'][:-1], parent['patches'])
        self.assertEqual(pin['sourceCommit'], parent['sourceCommit'])
        for patch in pin['patches']:
            self.assertEqual(hashlib.sha256((ROOT / patch['path']).read_bytes()).hexdigest(),
                             patch['sha256'])
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
                               (ROOT / parent['oraclePath'], pin['priorClaudeOracleSha256'])):
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
        with tempfile.TemporaryDirectory(prefix='openusage-capture-reconstruct-') as directory:
            root = Path(directory)
            for record in records:
                original = base_source / record['path']
                self.assertEqual(hashlib.sha256(original.read_bytes()).hexdigest(), record['sha256'])
                target = root / record['path']
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(original, target)
            patch = ROOT / pin['patches'][-1]['path']
            result = subprocess.run(['patch', '--batch', '--fuzz=0', '-p1', '-i', str(patch)],
                                    cwd=root, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            for record in json.loads((FIXTURE / 'source-files.json').read_text()):
                self.assertEqual(hashlib.sha256((root / record['path']).read_bytes()).hexdigest(),
                                 record['sha256'])
