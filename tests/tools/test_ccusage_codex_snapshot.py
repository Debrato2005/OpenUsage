"""Check the joint Codex report through the scanner command interface."""
import hashlib
import json
import os
import platform
from pathlib import Path
import subprocess
import tempfile
import unittest

from test_ccusage_codex_candidate import digest, synthetic_rows, write_rows
from test_codex_producer import ROOT

FIXTURE = ROOT / 'tests/tools/fixtures/codex-snapshot-candidate'


def run_command(binary, selected, command=('codex', 'snapshot'), extra=()):
    with tempfile.TemporaryDirectory(prefix='openusage-codex-snapshot-home-') as home:
        environment = {'HOME': home, 'CODEX_HOME': str(selected),
                       'XDG_CONFIG_HOME': home + '/config', 'XDG_CACHE_HOME': home + '/cache',
                       'XDG_DATA_HOME': home + '/data', 'TZ': 'UTC', 'NO_COLOR': '1', 'LOG_LEVEL': '0'}
        result = subprocess.run([str(binary), *command, '--json', '--offline', '--mode',
                                 'calculate', '--timezone', 'UTC', '--no-color', *extra],
                                cwd=home, env=environment, stdin=subprocess.DEVNULL,
                                capture_output=True, timeout=20)
        if any(Path(home).iterdir()):
            raise AssertionError('The scanner changed its private home.')
    return result


def traced_source_change(binary, selected, source, mutate, phase):
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
        command = [str(binary), 'codex', 'snapshot', '--json', '--offline', '--mode',
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


class CodexSnapshotTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CODEX_SNAPSHOT_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CODEX_SNAPSHOT_CANDIDATE for candidate execution.')
        cls.binary = Path(selected).resolve(strict=True)
        expected = os.environ.get('OPENUSAGE_CODEX_SNAPSHOT_SHA256')
        if not expected:
            expected = json.loads((FIXTURE / 'pin.json').read_text())['executableSha256']
        if digest(cls.binary) != expected:
            raise ValueError('The candidate does not match its selected digest.')

    def test_snapshot_contains_two_complete_reports_from_the_selected_records(self):
        with tempfile.TemporaryDirectory(prefix='openusage-codex-snapshot-') as directory:
            selected = Path(directory)
            write_rows(selected / 'sessions/source.jsonl', synthetic_rows())
            result = run_command(self.binary, selected)
            self.assertEqual(result.returncode, 0, result.stderr)
            report = json.loads(result.stdout)
            self.assertEqual(set(report), {'schemaVersion', 'tool', 'daily', 'session'})
            self.assertEqual(report['schemaVersion'], 1)
            self.assertEqual(report['tool'], 'codex')
            for kind, rows in [('daily', 'daily'), ('session', 'sessions')]:
                self.assertEqual(report[kind]['totals']['totalTokens'], 17)
                self.assertEqual(len(report[kind][rows]), 1)
                control = run_command(self.binary, selected, ('codex', kind))
                self.assertEqual(control.returncode, 0, control.stderr)
                self.assertEqual(report[kind], json.loads(control.stdout))

    def test_snapshot_refuses_an_invalid_session_directory_beside_valid_archived_usage(self):
        with tempfile.TemporaryDirectory(prefix='openusage-codex-snapshot-') as directory:
            selected = Path(directory)
            write_rows(selected / 'archived_sessions/source.jsonl', synthetic_rows())
            (selected / 'sessions').write_text('not a directory')
            result = run_command(self.binary, selected)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, b'')

    @unittest.skipUnless(platform.system() == 'Linux' and platform.machine() == 'x86_64',
                         'The controlled syscall trace requires Linux x86_64.')
    def test_controlled_source_changes_keep_one_capture_or_refuse(self):
        for phase in ('during', 'after'):
            for change in ('append', 'rewrite', 'replace', 'delete'):
                with self.subTest(phase=phase, change=change), tempfile.TemporaryDirectory() as directory:
                    selected = Path(directory)
                    source = selected / 'sessions/source.jsonl'
                    write_rows(source, synthetic_rows())
                    def mutate():
                        if change == 'append':
                            rows = synthetic_rows()
                            extra = synthetic_rows(response='response-b', total=12)[3]
                            extra['ordinal'] = 5
                            for field in ('thread_token_usage', 'turn_token_usage'):
                                extra['payload'][field]['input_tokens'] = 29
                                extra['payload'][field]['total_tokens'] = 29
                            write_rows(source, rows + [extra])
                        elif change == 'rewrite':
                            write_rows(source, synthetic_rows(total=29))
                        elif change == 'replace':
                            replacement = source.with_name('new.tmp')
                            write_rows(replacement, synthetic_rows(total=29))
                            replacement.replace(source)
                        else:
                            source.unlink()
                    result, opens = traced_source_change(self.binary, selected, source, mutate, phase)
                    if phase == 'during':
                        self.assertNotEqual(result.returncode, 0)
                        self.assertEqual(result.stdout, b'')
                    else:
                        self.assertEqual(opens, 2, 'One capture reads and validates the source once.')
                        self.assertEqual(result.returncode, 0, result.stderr)
                        report = json.loads(result.stdout)
                        for kind in ('daily', 'session'):
                            self.assertEqual(report[kind]['totals']['totalTokens'], 17)
                        next_result = run_command(self.binary, selected)
                        self.assertEqual(next_result.returncode, 0, next_result.stderr)
                        next_report = json.loads(next_result.stdout)
                        for kind in ('daily', 'session'):
                            self.assertEqual(next_report[kind]['totals']['totalTokens'],
                                             0 if change == 'delete' else 29)

    def assert_snapshot(self, result, total, rows=None):
        if total is None:
            self.assertNotEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, b'')
            return None
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(set(report), {'schemaVersion', 'tool', 'daily', 'session'})
        self.assertEqual((report['schemaVersion'], report['tool']), (1, 'codex'))
        for kind, key in [('daily', 'daily'), ('session', 'sessions')]:
            section = report[kind]
            self.assertEqual(section['totals']['totalTokens'], total)
            self.assertEqual(sum(row['totalTokens'] for row in section[key]), total)
            if rows is not None:
                self.assertEqual(len(section[key]), rows)
            for row in [*section[key], section['totals']]:
                self.assertEqual(row['inputTokens'] + row['cacheReadTokens']
                                 + row['cacheCreationTokens'] + row['outputTokens'], row['totalTokens'])
                self.assertLessEqual(row['reasoningOutputTokens'], row['outputTokens'])
        return report

    def test_captured_matrix_matches_independent_totals_and_separate_controls(self):
        from test_codex_producer import CAPTURE, CASES, prepare_case, raw_captured, source_hashes
        selected_receipt = os.environ.get('OPENUSAGE_CODEX_CAPTURE_RECEIPT')
        if not selected_receipt:
            self.skipTest('The exact private producer receipt is required for raw comparisons.')
        receipt_path = Path(selected_receipt)
        self.assertEqual(digest(receipt_path), CAPTURE['receiptSha256'])
        receipt = json.loads(receipt_path.read_text())
        for role in CAPTURE['files']:
            raw_captured(role, receipt)
        evidence = {'candidateSha256': digest(self.binary), 'captureSha256': digest(receipt_path),
                    'cases': [], 'complete': False}
        try:
            with tempfile.TemporaryDirectory(prefix='openusage-codex-snapshot-matrix-') as directory:
                for case, total in CASES.items():
                    with self.subTest(case=case):
                        selected = Path(directory) / case
                        prepare_case(selected, case, receipt)
                        before = source_hashes(selected)
                        outputs = [run_command(self.binary, selected) for _ in range(2)]
                        reports = [self.assert_snapshot(output, total) for output in outputs]
                        self.assertEqual(outputs[0].stdout, outputs[1].stdout)
                        controls = {}
                        for kind in ('daily', 'session'):
                            control = run_command(self.binary, selected, ('codex', kind))
                            controls[kind] = {'exitCode': control.returncode,
                                              'stdoutSha256': hashlib.sha256(control.stdout).hexdigest()}
                            if total is None:
                                self.assertNotEqual(control.returncode, 0)
                                self.assertEqual(control.stdout, b'')
                            else:
                                self.assertEqual(control.returncode, 0, control.stderr)
                                self.assertEqual(reports[0][kind], json.loads(control.stdout))
                        self.assertEqual(source_hashes(selected), before)
                        evidence['cases'].append({'case': case, 'expectedTotalTokens': total,
                                                   'exitCodes': [output.returncode for output in outputs],
                                                   'stdoutSha256': hashlib.sha256(outputs[0].stdout).hexdigest(),
                                                   'controls': controls, 'matched': True})
            evidence['complete'] = True
        finally:
            destination = os.environ.get('OPENUSAGE_CODEX_SNAPSHOT_RESULTS')
            if destination:
                with Path(destination).open('x') as handle:
                    json.dump(evidence, handle, indent=2)
                    handle.write('\n')

    def test_empty_prefix_and_measured_zero_keep_distinct_observation_states(self):
        cases = [('empty', [], 0), ('header', synthetic_rows()[:1], 0),
                 ('context', synthetic_rows()[:3], 0), ('zero', synthetic_rows(total=0), 1),
                 ('complete-prefix', synthetic_rows()[:4], 1)]
        for name, rows, count in cases:
            with self.subTest(case=name), tempfile.TemporaryDirectory() as directory:
                selected = Path(directory)
                if rows:
                    write_rows(selected / 'sessions/source.jsonl', rows)
                self.assert_snapshot(run_command(self.binary, selected),
                                     17 if name == 'complete-prefix' else 0, count)

    def test_missing_and_unknown_models_preserve_tokens_without_an_estimate(self):
        for model in (None, 'openusage-unknown-codex-model'):
            with self.subTest(model=model), tempfile.TemporaryDirectory() as directory:
                selected = Path(directory)
                write_rows(selected / 'sessions/source.jsonl', synthetic_rows(model=model))
                report = self.assert_snapshot(run_command(self.binary, selected), 17, 1)
                for kind, key in [('daily', 'daily'), ('session', 'sessions')]:
                    row = report[kind][key][0]
                    if model is None:
                        self.assertEqual(row['models'], {})
                        self.assertIsNone(row['costUSD'])
                        self.assertIsNone(report[kind]['totals']['costUSD'])
                    else:
                        self.assertTrue(row['models'][model]['missingPricing'])

    def test_all_selected_copies_and_distinct_responses_are_preserved(self):
        for distinct, total in [(False, 17), (True, 34)]:
            with self.subTest(distinct=distinct), tempfile.TemporaryDirectory() as directory:
                selected = Path(directory)
                write_rows(selected / 'a/sessions/source.jsonl', synthetic_rows())
                write_rows(selected / 'b/archived_sessions/copy.jsonl',
                           synthetic_rows(thread='thread-b' if distinct else 'thread-a',
                                          response='response-b' if distinct else 'response-a'))
                roots = ','.join(str(selected / part) for part in ('a', 'b'))
                self.assert_snapshot(run_command(self.binary, roots), total)

    def test_unsupported_and_incomplete_records_refuse_without_partial_output(self):
        cases = {}
        rows = synthetic_rows(); del rows[3]['payload']['usage']['cached_input_tokens']
        cases['missing-category'] = rows
        rows = synthetic_rows(); rows[3]['payload']['response_id'] = None
        cases['missing-response'] = rows
        rows = synthetic_rows(); rows[3]['payload']['thread_token_usage']['total_tokens'] += 1
        cases['counter-conflict'] = rows
        rows = synthetic_rows(); rows[0]['payload']['cli_version'] = 'future'
        cases['future-version'] = rows
        rows = synthetic_rows(); rows[2]['type'] = 'compacted'
        cases['compaction'] = rows
        rows = synthetic_rows(); rows[1]['payload']['type'] = 'thread_rolled_back'
        cases['revert'] = rows
        rows = synthetic_rows(); rows[3]['payload']['session_id'] = 'conflict'
        cases['identity-conflict'] = rows
        rows = synthetic_rows(); rows[0]['payload']['history_mode'] = 'legacy'
        cases['legacy'] = [{key: value for key, value in row.items() if key != 'ordinal'} for row in rows]
        cases['over-count-limit'] = synthetic_rows(total=1 << 53)
        for name, rows in cases.items():
            with self.subTest(case=name), tempfile.TemporaryDirectory() as directory:
                selected = Path(directory)
                write_rows(selected / 'sessions/source.jsonl', rows)
                self.assert_snapshot(run_command(self.binary, selected), None)

    def test_required_file_faults_and_limits_refuse(self):
        faults = ('missing-root', 'file-root', 'link-root', 'link-file', 'fifo', 'unreadable-file',
                  'unreadable-root', 'truncated', 'invalid-utf8', 'compressed', 'empty-compressed',
                  'oversize-file', 'oversize-line', 'mixed-legacy', 'conflicting-copy', 'archive-link')
        if os.geteuid() == 0:
            self.skipTest('Permission refusal requires an unprivileged user.')
        for fault in faults:
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as directory:
                base = Path(directory)
                selected = base / 'selected'
                source = selected / 'sessions/source.jsonl'
                write_rows(source, synthetic_rows())
                try:
                    if fault == 'missing-root':
                        selected = base / 'missing'
                    elif fault == 'file-root':
                        selected = source
                    elif fault == 'link-root':
                        (base / 'link').symlink_to(selected, target_is_directory=True)
                        selected = base / 'link'
                    elif fault == 'link-file':
                        source.with_name('link.jsonl').symlink_to('/unselected/missing.jsonl')
                    elif fault == 'fifo':
                        os.mkfifo(source.with_name('pipe.jsonl'))
                    elif fault == 'unreadable-file':
                        source.chmod(0)
                    elif fault == 'unreadable-root':
                        source.parent.chmod(0)
                    elif fault == 'truncated':
                        source.write_bytes(source.read_bytes()[:-2])
                    elif fault == 'invalid-utf8':
                        source.write_bytes(source.read_bytes() + b'\xff\n')
                    elif fault in ('compressed', 'empty-compressed'):
                        source.with_suffix('.jsonl.zst').write_bytes(b'unsupported')
                        if fault == 'empty-compressed':
                            source.unlink()
                    elif fault == 'oversize-file':
                        with source.open('ab') as handle:
                            handle.truncate((32 << 20) + 1)
                    elif fault == 'oversize-line':
                        source.write_bytes(source.read_bytes() + b' ' * (1 << 20) + b'\n')
                    elif fault == 'mixed-legacy':
                        write_rows(source.with_name('legacy.jsonl'),
                                   [{'type': 'session_meta', 'payload': {'id': 'legacy'}}])
                    elif fault == 'conflicting-copy':
                        write_rows(source.with_name('copy.jsonl'), synthetic_rows(total=18))
                    elif fault == 'archive-link':
                        (selected / 'archived_sessions').symlink_to(source.parent, target_is_directory=True)
                    self.assert_snapshot(run_command(self.binary, selected), None)
                finally:
                    if source.exists():
                        source.chmod(0o600)
                    source.parent.chmod(0o700)

    def test_report_modifiers_cannot_hide_required_output(self):
        for extra in [('--since', '20261111'), ('--until', '20261111'), ('--last', '1'),
                      ('--jq', '.daily'), ('--no-cost',), ('--no-offline',),
                      ('--id', 'thread-a'), ('--sections', 'daily')]:
            with self.subTest(extra=extra), tempfile.TemporaryDirectory() as directory:
                selected = Path(directory)
                write_rows(selected / 'sessions/source.jsonl', synthetic_rows())
                self.assert_snapshot(run_command(self.binary, selected, extra=extra), None)

    def test_assertions_detect_a_changed_report(self):
        with tempfile.TemporaryDirectory() as directory:
            selected = Path(directory)
            write_rows(selected / 'sessions/source.jsonl', synthetic_rows())
            result = run_command(self.binary, selected)
            original = result.stdout
            report = json.loads(original)
            report['session']['totals']['totalTokens'] += 1
            result.stdout = json.dumps(report).encode()
            with self.assertRaises(AssertionError):
                self.assert_snapshot(result, 17)
            result.stdout = original
            self.assert_snapshot(result, 17)


class CodexSnapshotPinTests(unittest.TestCase):
    def test_patch_recipe_and_explicit_source_manifest_match_the_pin(self):
        pin = json.loads((FIXTURE / 'pin.json').read_text())
        for patch in pin['patches']:
            self.assertEqual(digest(ROOT / patch['path']), patch['sha256'])
        source_file = FIXTURE / 'source-files.json'
        self.assertEqual(digest(source_file), pin['sourceFilesSha256'])
        records = json.loads(source_file.read_text())
        self.assertIsInstance(records, list)
        names = []
        for record in records:
            self.assertEqual(set(record), {'path', 'sha256'})
            self.assertIsInstance(record['path'], str)
            self.assertRegex(record['sha256'], r'\A[0-9a-f]{64}\Z')
            names.append(record['path'])
            source = ROOT / pin['sourceDirectory'] / record['path']
            if (ROOT / pin['sourceDirectory']).exists():
                self.assertTrue(source.is_file(), record['path'])
                self.assertEqual(digest(source), record['sha256'])
        self.assertEqual(names, sorted(set(names)))
        compact = json.dumps(records, separators=(',', ':')).encode()
        self.assertEqual(hashlib.sha256(compact).hexdigest(), pin['sourceManifestSha256'])

    def test_original_pins_and_physical_readonly_oracles_remain_separate(self):
        pin = json.loads((FIXTURE / 'pin.json').read_text())
        prior = json.loads((ROOT / 'tests/tools/fixtures/codex-records-candidate/pin.json').read_text())
        production = json.loads((ROOT / 'crates/openusage-locallogs/sources/ccusage-20.0.48-linux-x86_64.json').read_text())
        pi = json.loads((ROOT / 'tests/tools/fixtures/pi-producer/pin.json').read_text())
        self.assertEqual(pin['priorCodexOracleSha256'], prior['executableSha256'])
        self.assertEqual(pin['productionSha256'], production['executableSha256'])
        self.assertEqual(pin['piOracleSha256'], pi['boundedReleaseScannerSha256'])
        identities = set()
        for path, sha in [(pin['oraclePath'], pin['executableSha256']),
                          (prior['oraclePath'], prior['executableSha256'])]:
            oracle = ROOT / path
            if oracle.exists():
                self.assertFalse(oracle.is_symlink())
                self.assertEqual(digest(oracle), sha)
                self.assertEqual(oracle.stat().st_mode & 0o777, 0o555)
                identity = (oracle.stat().st_dev, oracle.stat().st_ino)
                self.assertNotIn(identity, identities)
                identities.add(identity)
        target = ROOT / '.tools/build-target/release/ccusage'
        if target.exists():
            self.assertNotIn((target.stat().st_dev, target.stat().st_ino), identities)


if __name__ == '__main__':
    unittest.main()
