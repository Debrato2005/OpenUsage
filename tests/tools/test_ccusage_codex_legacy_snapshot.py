"""Exercise ordinary compatibility and explicit strict capture through public commands."""
import copy
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import unittest

from test_ccusage_codex_candidate import run_report, synthetic_rows, write_rows
from test_ccusage_codex_snapshot import run_command
from test_ccusage_formats import file_hashes
from test_ccusage_shared_candidate import COMMANDS, TOOLS, fixture_pairs, prepare

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'tests/tools/fixtures/codex-legacy-snapshot-candidate'
PARENT = ROOT / 'tests/tools/fixtures/claude-capture-candidate/pin.json'
COMMAND = ('codex', 'legacy-snapshot')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_rows(selected):
    source = sorted(Path(selected).rglob('*.jsonl'))[0]
    return source, [json.loads(line) for line in source.read_text().splitlines()]


def traced_legacy_source_change(binary, selected, source, mutate, phase):
    """Pause the private Linux child at its file boundary, without a scanner hook."""
    import ctypes
    import signal
    import time

    class Registers(ctypes.Structure):
        _fields_ = [(name, ctypes.c_ulonglong) for name in (
            'r15 r14 r13 r12 rbp rbx r11 r10 r9 r8 rax rcx rdx rsi rdi '
            'orig_rax rip cs eflags rsp ss fs_base gs_base ds es fs gs').split()]

    libc = ctypes.CDLL(None, use_errno=True)
    libc.ptrace.restype = ctypes.c_long

    def trace(request, pid, address=None, data=None):
        result = libc.ptrace(request, pid, address, data)
        if result == -1:
            raise OSError(ctypes.get_errno(), 'ptrace failed for the private scanner')
        return result

    def child_trace():
        trace(0, 0)

    with tempfile.TemporaryDirectory(prefix='openusage-codex-trace-home-') as home:
        environment = {'HOME': home, 'CODEX_HOME': str(selected),
                       'XDG_CONFIG_HOME': home + '/config', 'XDG_CACHE_HOME': home + '/cache',
                       'XDG_DATA_HOME': home + '/data', 'TZ': 'UTC', 'NO_COLOR': '1', 'LOG_LEVEL': '0'}
        command = [str(binary), *COMMAND, '--json', '--offline', '--mode',
                   'calculate', '--timezone', 'UTC', '--no-color']
        process = subprocess.Popen(command, cwd=home, env=environment,
                                   stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, preexec_fn=child_trace)
        deadline = time.monotonic() + 20
        root_fd = None
        source_fds = set()
        source_opens = 0
        changed = False
        entering = True

        def wait_stop():
            while time.monotonic() < deadline:
                pid, status = os.waitpid(process.pid, os.WNOHANG)
                if pid:
                    return status
                time.sleep(0.0001)
            raise TimeoutError('The private traced scanner exceeded 20 seconds.')

        try:
            status = wait_stop()
            if not os.WIFSTOPPED(status):
                raise AssertionError('The private scanner did not enter the trace stop.')
            trace(0x4200, process.pid, None, ctypes.c_void_p(0x100001))
            while True:
                trace(24, process.pid)
                status = wait_stop()
                if os.WIFEXITED(status) or os.WIFSIGNALED(status):
                    process.returncode = os.waitstatus_to_exitcode(status)
                    break
                if os.WSTOPSIG(status) != signal.SIGTRAP | 0x80:
                    raise AssertionError('Unexpected signal from the private scanner.')
                registers = Registers()
                trace(12, process.pid, None, ctypes.byref(registers))
                syscall = registers.orig_rax
                if not entering and syscall == 437 and registers.rax < (1 << 63):
                    fd = registers.rax
                    path = Path(os.readlink(f'/proc/{process.pid}/fd/{fd}'))
                    if path == source:
                        source_opens += 1
                        source_fds.add(fd)
                    if path == source.parent and root_fd is None:
                        root_fd = fd
                if entering and syscall == 3:
                    fd = registers.rdi
                    during = phase == 'during' and fd in source_fds and source_opens == 1
                    after = phase == 'after' and fd == root_fd and source_opens == 2
                    if not changed and (during or after):
                        mutate()
                        changed = True
                    source_fds.discard(fd)
                entering = not entering
            stdout, stderr = process.communicate(timeout=2)
        finally:
            if process.returncode is None:
                process.kill()
                process.communicate(timeout=2)
        if not changed:
            raise AssertionError('The controlled source change did not execute.')
        if any(Path(home).iterdir()):
            raise AssertionError('The traced scanner changed its private home.')
        return subprocess.CompletedProcess(command, process.returncode, stdout, stderr), source_opens


class LegacySnapshotCommands(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CODEX_LEGACY_SNAPSHOT_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Select OPENUSAGE_CODEX_LEGACY_SNAPSHOT_CANDIDATE for execution.')
        cls.binary = Path(selected).resolve(strict=True)
        expected = os.environ.get('OPENUSAGE_CODEX_LEGACY_SNAPSHOT_SHA256')
        if not expected:
            expected = json.loads((FIXTURE / 'pin.json').read_text())['executableSha256']
        if digest(cls.binary) != expected:
            raise ValueError('The selected executable does not match its digest.')
        parent = json.loads(PARENT.read_text())
        cls.parent = ROOT / parent['oraclePath']
        if digest(cls.parent) != parent['executableSha256']:
            raise ValueError('The parent executable does not match its pin.')

    def assert_snapshot(self, selected, total, sessions=1):
        result = run_command(self.binary, selected, COMMAND)
        self.assertEqual(result.returncode, 0, result.stderr)
        output = json.loads(result.stdout)
        self.assertEqual(set(output), {'schemaVersion', 'tool', 'historyMode', 'daily', 'session'})
        self.assertEqual((output['schemaVersion'], output['tool'], output['historyMode']),
                         (1, 'codex', 'legacy'))
        for kind in ('daily', 'session'):
            self.assertEqual(output[kind]['totals']['totalTokens'], total)
        self.assertEqual(len(output['session']['sessions']), sessions)
        for row in output['session']['sessions']:
            self.assertEqual(row['sessionFile'], row['sessionId'])
            self.assertEqual(row['directory'], '')
        return output

    def assert_refusal(self, selected, extra=()):
        result = run_command(self.binary, selected, COMMAND, extra)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, b'')
        self.assertNotIn(str(selected).encode(), result.stderr)

    def test_ordinary_reports_match_the_parent_matrix(self):
        comparisons = []
        with tempfile.TemporaryDirectory(prefix='openusage-legacy-route-matrix-') as directory:
            for case, tool in fixture_pairs():
                root = Path(directory) / tool / case
                root.mkdir(parents=True)
                selected = prepare(root, tool, case)
                before = file_hashes(root)
                for kind, command in COMMANDS[tool]:
                    with self.subTest(case=case, tool=tool, kind=kind):
                        previous = run_report(self.parent, selected, kind,
                                              variable=TOOLS[tool][0], command=command)
                        current = run_report(self.binary, selected, kind,
                                             variable=TOOLS[tool][0], command=command)
                        self.assertEqual(current, previous)
                        comparisons.append((case, tool, kind))
                self.assertEqual(before, file_hashes(root))
        self.assertEqual(len(comparisons), 47)

    def test_supported_legacy_shapes_and_unused_metadata_keep_parent_reports(self):
        variants = ('complete', 'last-only', 'total-only', 'no-reasoning', 'notes-zst',
                    'unused-large-number', 'unused-deep-array', 'missing-id', 'lineage')
        for variant in variants:
            with self.subTest(variant=variant), tempfile.TemporaryDirectory(
                    prefix='openusage-legacy-route-shape-') as directory:
                root = Path(directory)
                selected = prepare(root, 'codex', 'baseline')
                source, rows = source_rows(selected)
                info = rows[-1]['payload']['info']
                if variant == 'last-only':
                    del info['total_token_usage']
                elif variant == 'total-only':
                    del info['last_token_usage']
                elif variant == 'no-reasoning':
                    for category in ('last_token_usage', 'total_token_usage'):
                        info[category].pop('reasoning_output_tokens', None)
                elif variant == 'notes-zst':
                    source.with_name('notes.zst').write_bytes(b'unrelated notes')
                elif variant == 'missing-id':
                    del rows[0]['payload']['id']
                elif variant == 'lineage':
                    rows[0]['payload']['forked_from_id'] = 'synthetic-missing-parent'
                write_rows(source, rows)
                if variant in ('unused-large-number', 'unused-deep-array'):
                    raw = source.read_text()
                    value = '1e400' if variant == 'unused-large-number' else '[' * 140 + '0' + ']' * 140
                    source.write_text(raw.replace('{', '{"unused":' + value + ',', 1))
                before = file_hashes(root)
                for kind, command in (('daily', ['codex', 'daily']),
                                      ('daily', ['daily', '--by-agent']),
                                      ('sessions', ['codex', 'session'])):
                    previous = run_report(self.parent, selected, kind, command=command)
                    current = run_report(self.binary, selected, kind, command=command)
                    self.assertEqual(previous['exitCode'], 0, previous['stderr'])
                    self.assertEqual(previous['measurement']['totalTokens'], 4600)
                    self.assertEqual(current, previous)
                self.assertEqual(before, file_hashes(root))

    def test_ordinary_path_basename_and_recorded_id_selectors_match_parent(self):
        with tempfile.TemporaryDirectory(prefix='openusage-legacy-aliases-') as directory:
            root = Path(directory)
            selected = prepare(root, 'codex', 'baseline')
            source, rows = source_rows(selected)
            ordinary = run_report(self.parent, selected, 'sessions', command=['codex', 'session'])
            path_id = ordinary['report']['sessions'][0]['sessionId']
            thread = rows[0]['payload']['id']
            selectors = (path_id, Path(path_id).name, source.name, thread, 'codex://threads/' + thread)
            before = file_hashes(root)
            for selector in selectors:
                with self.subTest(selector=selector):
                    extra = ('--id', selector)
                    previous = run_report(self.parent, selected, 'sessions', extra,
                                          command=['codex', 'session'])
                    current = run_report(self.binary, selected, 'sessions', extra,
                                         command=['codex', 'session'])
                    self.assertEqual(previous['exitCode'], 0, previous['stderr'])
                    self.assertEqual(current, previous)
            self.assertEqual(before, file_hashes(root))

    def test_strict_copies_and_distinct_threads_keep_independent_counts(self):
        for case, total, sessions in (('baseline', 4600, 1), ('copied_roots', 4600, 1),
                                     ('repeated_root', 4600, 1), ('renamed_copy', 4600, 1),
                                     ('distinct_equal_usage', 9200, 2), ('empty', 0, 0)):
            with self.subTest(case=case), tempfile.TemporaryDirectory(
                    prefix='openusage-legacy-snapshot-copies-') as directory:
                root = Path(directory)
                selected = prepare(root, 'codex', case)
                before = file_hashes(root)
                output = self.assert_snapshot(selected, total, sessions)
                if sessions:
                    self.assertIn('22222222-bbbb-4bbb-8bbb-222222222222',
                                  {row['sessionId'] for row in output['session']['sessions']})
                self.assertEqual(before, file_hashes(root))

    def test_strict_prefix_copy_and_reordered_roots_share_one_longest_capture(self):
        with tempfile.TemporaryDirectory(prefix='openusage-legacy-snapshot-prefix-') as directory:
            root = Path(directory)
            first = Path(prepare(root, 'codex', 'baseline'))
            second = root / 'copy'
            shutil.copytree(first, second)
            source, rows = source_rows(second)
            advanced = copy.deepcopy(rows[-1])
            for field in advanced['payload']['info']['total_token_usage']:
                advanced['payload']['info']['total_token_usage'][field] *= 2
            write_rows(source, rows + [advanced])
            before = file_hashes(root)
            a = self.assert_snapshot(','.join(map(str, (first, second))), 9200)
            b = self.assert_snapshot(','.join(map(str, (second, first))), 9200)
            self.assertEqual(a, b)
            self.assertEqual(before, file_hashes(root))

    def test_strict_unknown_identity_and_unsupported_histories_refuse(self):
        variants = ('missing-id', 'missing-header', 'duplicate-id', 'escaped-duplicate-id',
                    'foreign-header', 'fork', 'subagent', 'conflict', 'decrease', 'replay',
                    'compaction', 'revert', 'paginated', 'last-only', 'total-only',
                    'no-reasoning', 'notes-zst', 'unused-large-number', 'unused-deep-array')
        for variant in variants:
            with self.subTest(variant=variant), tempfile.TemporaryDirectory(
                    prefix='openusage-legacy-snapshot-refusal-') as directory:
                root = Path(directory)
                selected = prepare(root, 'codex', 'baseline')
                source, rows = source_rows(selected)
                if variant == 'missing-id':
                    del rows[0]['payload']['id']
                elif variant == 'missing-header':
                    del rows[0]
                elif variant == 'foreign-header':
                    foreign = copy.deepcopy(rows[0])
                    foreign['payload']['id'] = 'other-thread'
                    rows.insert(1, foreign)
                elif variant == 'fork':
                    rows[0]['payload']['forked_from_id'] = 'synthetic-parent'
                elif variant == 'subagent':
                    rows[0]['payload']['source'] = {'subagent': {'thread_spawn': {
                        'parent_thread_id': 'synthetic-parent'}}}
                elif variant == 'conflict':
                    changed = copy.deepcopy(rows)
                    changed[1]['payload']['model'] = 'gpt-5'
                    write_rows(source.with_name('conflict.jsonl'), changed)
                elif variant == 'decrease':
                    smaller = copy.deepcopy(rows[-1])
                    for category in ('last_token_usage', 'total_token_usage'):
                        counts = smaller['payload']['info'][category]
                        for key in counts:
                            counts[key] //= 2
                    rows.append(smaller)
                elif variant == 'replay':
                    rows.append({'type': 'inter_agent_communication_metadata', 'payload': {}})
                elif variant == 'compaction':
                    rows.append({'type': 'compacted', 'payload': {}})
                elif variant == 'revert':
                    rows.append({'type': 'event_msg', 'payload': {'type': 'thread_rolled_back'}})
                elif variant == 'paginated':
                    rows = synthetic_rows()
                elif variant in ('last-only', 'total-only'):
                    del rows[-1]['payload']['info'][
                        'total_token_usage' if variant == 'last-only' else 'last_token_usage']
                elif variant == 'no-reasoning':
                    for category in ('last_token_usage', 'total_token_usage'):
                        rows[-1]['payload']['info'][category].pop('reasoning_output_tokens', None)
                elif variant == 'notes-zst':
                    source.with_name('notes.zst').write_bytes(b'unrelated notes')
                write_rows(source, rows)
                if variant in ('duplicate-id', 'escaped-duplicate-id'):
                    key = 'id' if variant == 'duplicate-id' else 'i\\u0064'
                    source.write_text(source.read_text().replace('"payload":{',
                                      '"payload":{"' + key + '":"other",', 1))
                elif variant in ('unused-large-number', 'unused-deep-array'):
                    value = '1e400' if variant == 'unused-large-number' else '[' * 140 + '0' + ']' * 140
                    source.write_text(source.read_text().replace('{', '{"unused":' + value + ',', 1))
                before = file_hashes(root)
                self.assert_refusal(selected)
                self.assertEqual(before, file_hashes(root))

    def test_strict_options_and_missing_or_linked_roots_refuse(self):
        with tempfile.TemporaryDirectory(prefix='openusage-legacy-snapshot-boundary-') as directory:
            root = Path(directory)
            selected = prepare(root, 'codex', 'baseline')
            for extra in (('--since', '20260101'), ('--until', '20270101'), ('--last', '1'),
                          ('--jq', '.'), ('--no-cost',), ('--no-offline',), ('--id', 'thread'),
                          ('--sections', 'daily')):
                with self.subTest(extra=extra):
                    self.assert_refusal(selected, extra)
            self.assert_refusal(root / 'missing')
            link = root / 'linked'
            link.symlink_to(Path(selected), target_is_directory=True)
            self.assert_refusal(link)
            self.assert_refusal(str(selected) + ',' + str(root / 'missing'))

    def test_strict_decoded_compressed_copies_and_truncated_frames(self):
        compressor = shutil.which('zstd')
        if not compressor:
            self.skipTest('The compressed controls require zstd.')
        with tempfile.TemporaryDirectory(prefix='openusage-legacy-snapshot-compressed-') as directory:
            root = Path(directory)
            selected = prepare(root, 'codex', 'baseline')
            source, rows = source_rows(selected)
            content = ''.join(json.dumps(row, sort_keys=True) + '\r\n' for row in rows).encode()
            encoded = subprocess.run([compressor, '-q', '-c'], input=content, capture_output=True,
                                     timeout=10, env={'PATH': '/usr/bin:/bin', 'HOME': str(root)})
            self.assertEqual(encoded.returncode, 0, encoded.stderr)
            peer = source.with_name('renamed-copy.jsonl.zst')
            peer.write_bytes(encoded.stdout)
            before = file_hashes(root)
            self.assert_snapshot(selected, 4600)
            self.assertEqual(before, file_hashes(root))
            peer.write_bytes(encoded.stdout + encoded.stdout[:len(encoded.stdout) // 2])
            before = file_hashes(root)
            self.assert_refusal(selected)
            self.assertEqual(before, file_hashes(root))

    def test_strict_counter_values_and_joint_exact_limit_refuse(self):
        for variant in ('negative', 'fraction', 'string', 'null', 'missing-input',
                        'cached-over-input', 'reason-over-output', 'last-total-disagreement',
                        'alias-conflict', 'joint-limit'):
            with self.subTest(variant=variant), tempfile.TemporaryDirectory(
                    prefix='openusage-legacy-snapshot-counts-') as directory:
                root = Path(directory)
                selected = prepare(root, 'codex', 'baseline')
                source, rows = source_rows(selected)
                info = rows[-1]['payload']['info']
                for category in ('last_token_usage', 'total_token_usage'):
                    value = info[category]
                    if variant in ('negative', 'fraction', 'string', 'null'):
                        value['input_tokens'] = {'negative': -1, 'fraction': 0.5,
                                                 'string': '4000', 'null': None}[variant]
                    elif variant == 'missing-input':
                        del value['input_tokens']
                    elif variant == 'cached-over-input':
                        value['cached_input_tokens'] = 4001
                    elif variant == 'reason-over-output':
                        value['reasoning_output_tokens'] = 601
                    elif variant == 'alias-conflict':
                        value['cache_write_input_tokens'] = 0
                        value['cache_creation_input_tokens'] = 0
                    elif variant == 'joint-limit':
                        value.update(input_tokens=1 << 52, cached_input_tokens=0,
                                     output_tokens=0, reasoning_output_tokens=0, total_tokens=1 << 52)
                if variant == 'last-total-disagreement':
                    info['last_token_usage']['input_tokens'] = 3999
                    info['last_token_usage']['total_tokens'] = 4599
                write_rows(source, rows)
                if variant == 'joint-limit':
                    rows[0]['payload']['id'] = 'another-thread'
                    write_rows(source.with_name('another.jsonl'), rows)
                before = file_hashes(root)
                self.assert_refusal(selected)
                self.assertEqual(before, file_hashes(root))

    def test_strict_repeated_counters_and_nonusage_conflicts(self):
        with tempfile.TemporaryDirectory(prefix='openusage-legacy-snapshot-repeat-') as directory:
            root = Path(directory)
            selected = prepare(root, 'codex', 'baseline')
            source, rows = source_rows(selected)
            write_rows(source, rows + [rows[-1]])
            self.assert_snapshot(selected, 4600)
            rows[1]['payload']['private_note'] = 'synthetic-private-marker'
            write_rows(source.with_name('same-count-conflict.jsonl'), rows)
            before = file_hashes(root)
            self.assert_refusal(selected)
            self.assertEqual(before, file_hashes(root))

    @unittest.skipUnless(platform.system() == 'Linux' and platform.machine() == 'x86_64',
                         'The controlled trace requires Linux x86_64.')
    def test_strict_joint_capture_refuses_mid_read_change_and_uses_captured_bytes(self):
        for phase in ('during', 'after'):
            with self.subTest(phase=phase), tempfile.TemporaryDirectory(
                    prefix='openusage-legacy-snapshot-trace-') as directory:
                root = Path(directory)
                selected = prepare(root / 'fixture', 'codex', 'baseline')
                source, rows = source_rows(selected)
                shutil.rmtree(Path(selected) / 'sessions')
                source = Path(selected) / 'sessions/source.jsonl'
                write_rows(source, rows)
                def mutate():
                    source.write_text('{"changed":true}\n')
                result, opens = traced_legacy_source_change(self.binary, Path(selected), source, mutate, phase)
                if phase == 'during':
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(result.stdout, b'')
                else:
                    self.assertEqual(opens, 2)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    report = json.loads(result.stdout)
                    for kind in ('daily', 'session'):
                        self.assertEqual(report[kind]['totals']['totalTokens'], 4600)


class LegacySnapshotPins(unittest.TestCase):
    def test_pin_preserves_parents_and_reconstructs_source(self):
        pin = json.loads((FIXTURE / 'pin.json').read_text())
        parent_path = ROOT / pin['baseCandidatePin']['path']
        self.assertEqual(digest(parent_path), pin['baseCandidatePin']['sha256'])
        parent = json.loads(parent_path.read_text())
        self.assertEqual(pin['patches'][:-1], parent['patches'])
        for patch in pin['patches']:
            self.assertEqual(digest(ROOT / patch['path']), patch['sha256'])
        manifest = FIXTURE / 'source-files.json'
        self.assertEqual(digest(manifest), pin['sourceFilesSha256'])
        records = json.loads(manifest.read_text())
        self.assertEqual(hashlib.sha256(json.dumps(records, separators=(',', ':')).encode()).hexdigest(),
                         pin['sourceManifestSha256'])
        self.assertEqual([r['path'] for r in records], sorted({r['path'] for r in records}))
        for record in records:
            self.assertEqual(set(record), {'path', 'sha256'})
            self.assertFalse(Path(record['path']).is_absolute())
            self.assertNotIn('..', Path(record['path']).parts)
        oracle = ROOT / pin['oraclePath']
        if oracle.exists():
            self.assertFalse(oracle.is_symlink())
            self.assertEqual(digest(oracle), pin['executableSha256'])
            self.assertEqual(oracle.stat().st_mode & 0o777, 0o555)
        base = ROOT / parent['sourceDirectory']
        if not base.exists():
            self.skipTest('The private parent source is required for reconstruction.')
        old = json.loads((parent_path.parent / 'source-files.json').read_text())
        with tempfile.TemporaryDirectory(prefix='openusage-legacy-snapshot-source-') as directory:
            reconstructed = Path(directory)
            for record in old:
                original = base / record['path']
                self.assertEqual(digest(original), record['sha256'])
                target = reconstructed / record['path']
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(original, target)
            result = subprocess.run(['patch', '--batch', '--fuzz=0', '-p1', '-i',
                                     str(ROOT / pin['patches'][-1]['path'])],
                                    cwd=reconstructed, capture_output=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertNotIn(b'offset', result.stdout)
            for record in records:
                self.assertEqual(digest(reconstructed / record['path']), record['sha256'])
                actual = ROOT / pin['sourceDirectory'] / record['path']
                if actual.exists():
                    self.assertEqual(digest(actual), record['sha256'])


if __name__ == '__main__':
    unittest.main(verbosity=2)
