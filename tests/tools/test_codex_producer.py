"""Check captured producer records and compare unchanged scanner artifacts."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from codex_producer import ROOT, PIN, sandbox_command, sha256
from test_ccusage_shared_candidate import measure


FIXTURE = ROOT / 'tests/tools/fixtures/codex-producer'
CAPTURE = json.loads((FIXTURE / 'capture.json').read_text())
CASES = {'baseline': 4600, 'renamed_copy': 4600, 'distinct_equal': 9200,
         'distinct_equal_time': 9200, 'replayed_record': 4600,
         'conflicting_copy': None, 'lineage': 12650, 'lineage_prefix': 12650,
         'missing_parent': None}


def raw_captured(role, receipt):
    meta = CAPTURE['files'][role]
    if role == 'beforeResume':
        evidence = json.loads(receipt['evidence.json'])
        text = evidence['checkpoints']['beforeResume'][meta['originalPath']]
    else:
        text = receipt['rollouts'][meta['originalPath']]
    data = text.encode()
    if hashlib.sha256(data).hexdigest() != meta['sha256']:
        raise ValueError('The captured producer file differs from its hash.')
    if len(data) != meta['bytes']:
        raise ValueError('The captured producer file differs from its byte count.')
    return data


def rows(role):
    return CAPTURE['projections'][role]


def usage_records(role):
    return [row for row in rows(role) if row['type'] == 'token_usage_record']


def prepare_case(root, case, receipt=None):
    def data_for(role):
        if receipt is not None:
            return raw_captured(role, receipt)
        return ''.join(json.dumps(row, separators=(',', ':')) + '\n'
                       for row in rows(role)).encode()

    selected = {'beforeResume': data_for('beforeResume')}
    if case in ('distinct_equal', 'distinct_equal_time'):
        selected['second'] = data_for('second')
    elif case in ('lineage', 'lineage_prefix'):
        selected = {role: data_for(role) for role in ('parent', 'second', 'fork')}
        if case == 'lineage_prefix':
            prefix = []
            for line in selected['fork'].splitlines(keepends=True):
                row = json.loads(line)
                if (row['type'] == 'event_msg' and row['payload']['type'] == 'task_started'
                        and row['payload']['turn_id'] == CAPTURE['turns']['missing']):
                    break
                prefix.append(line)
            selected['fork'] = b''.join(prefix)
    elif case == 'missing_parent':
        selected = {'fork': data_for('fork')}
    if case == 'distinct_equal_time':
        # Keep the emitted identities. Set equal times only in this mutated case.
        for role, data in selected.items():
            changed = list(map(json.loads, data.splitlines()))
            for row in changed:
                row['timestamp'] = '2026-11-11T12:00:00.000Z'
            selected[role] = ''.join(json.dumps(row, separators=(',', ':')) + '\n'
                                     for row in changed).encode()
    if case == 'replayed_record':
        data = selected['beforeResume']
        event = next(line for line in data.splitlines(keepends=True)
                     if json.loads(line)['type'] == 'token_usage_record')
        selected['beforeResume'] = data + event
    destinations = []
    for role, data in selected.items():
        relative = CAPTURE['files'][role]['originalPath'].removeprefix('codex/')
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        destinations.append(path)
    if case in ('renamed_copy', 'conflicting_copy'):
        path = destinations[0]
        data = path.read_bytes()
        if case == 'conflicting_copy':
            changed = list(map(json.loads, data.splitlines()))
            for row in changed:
                if row['type'] == 'token_usage_record':
                    row['payload']['usage']['input_tokens'] += 1
                    row['payload']['usage']['total_tokens'] += 1
            data = ''.join(json.dumps(row, separators=(',', ':')) + '\n'
                           for row in changed).encode()
        path.with_stem(path.stem + '-copy').write_bytes(data)


def source_hashes(root):
    return {str(path.relative_to(root)): sha256(path)
            for path in sorted(root.rglob('*.jsonl'))}


class CapturedContractTests(unittest.TestCase):
    def test_tracked_fixture_excludes_prompt_and_message_bodies(self):
        self.assertEqual([path.name for path in FIXTURE.iterdir()], ['capture.json'])
        encoded_capture = json.dumps(CAPTURE)
        for key in ('base_instructions', 'developer_instructions', 'content', 'text', 'last_agent_message', 'preview'):
            self.assertNotIn('"' + key + '"', encoded_capture)
        for request in CAPTURE['requests']:
            self.assertNotIn('body', request)
            self.assertNotIn('input', request)

    def test_capture_binds_the_executed_producer_and_source(self):
        self.assertEqual(CAPTURE['producerSha256'], json.loads(PIN.read_text())['executableSha256'])
        self.assertEqual(CAPTURE['producerSourcePinSha256'], sha256(PIN))
        self.assertEqual(CAPTURE['harnessSha256'], sha256(ROOT / 'tests/tools/codex_producer.py'))
        self.assertEqual(CAPTURE['sourceCommit'], '83086188ae0614bd882818ff1aa23f3194962012')
        for record in CAPTURE['files'].values():
            self.assertEqual(len(record['recordHashes']), len(record['ordinals']))
            self.assertGreater(record['bytes'], 0)
            self.assertEqual(len(record['sha256']), 64)

    def test_sandbox_excludes_host_network_and_environment(self):
        command = sandbox_command(Path('/private-fixture'), 'basic')
        self.assertIn('--unshare-net', command)
        self.assertIn('--unshare-pid', command)
        self.assertIn('--clearenv', command)
        self.assertEqual(CAPTURE['sandbox']['interfaces'], [[1, 'lo']])
        self.assertNotEqual(CAPTURE['sandbox']['networkNamespace'], CAPTURE['hostNetworkNamespace'])
        self.assertEqual(len(CAPTURE['sandbox']['routes'].splitlines()), 1)
        self.assertTrue(all(not row['authorizationPresent'] for row in CAPTURE['requests']))

    def test_one_response_has_usage_and_counter_representations(self):
        data = rows('beforeResume')
        record, = usage_records('beforeResume')
        counter, = [row for row in data if row['type'] == 'event_msg'
                    and row['payload']['type'] == 'token_count']
        self.assertEqual(record['payload']['usage'], counter['payload']['info']['last_token_usage'])
        self.assertEqual(record['payload']['usage']['total_tokens'], 4600)
        self.assertEqual(counter['ordinal'], record['ordinal'] + 1)

    def test_resume_preserves_identity_prefix_and_advances_ordinals(self):
        parent_hashes = CAPTURE['files']['parent']['recordHashes']
        before_hashes = CAPTURE['files']['beforeResume']['recordHashes']
        self.assertEqual(parent_hashes[:len(before_hashes)], before_hashes)
        self.assertEqual(CAPTURE['threads']['first'], CAPTURE['threads']['resumed'])
        ordinals = CAPTURE['files']['parent']['ordinals']
        self.assertEqual(ordinals, list(range(len(ordinals))))
        first, second = [row['payload'] for row in usage_records('parent')]
        self.assertEqual(first['thread_id'], second['thread_id'])
        self.assertEqual(first['session_id'], second['session_id'])
        self.assertNotEqual(first['turn_id'], second['turn_id'])
        self.assertNotEqual(first['response_id'], second['response_id'])
        self.assertEqual(second['usage']['total_tokens'], 2300)
        self.assertEqual(second['thread_token_usage']['total_tokens'], 6900)

    def test_distinct_equal_usage_has_distinct_emitted_identities(self):
        first = usage_records('beforeResume')[0]['payload']
        second = usage_records('second')[0]['payload']
        self.assertEqual(first['usage'], second['usage'])
        for key in ('thread_id', 'session_id', 'turn_id', 'root_turn_id', 'response_id'):
            self.assertNotEqual(first[key], second[key])

    def test_fork_declares_parent_prefix_without_copying_usage(self):
        child = rows('fork')
        meta = child[0]['payload']
        base = meta['history_base']
        parent = rows('parent')
        self.assertEqual(meta['forked_from_id'], CAPTURE['threads']['first'])
        self.assertEqual(base['thread_id'], CAPTURE['threads']['first'])
        self.assertEqual(base['end_byte_offset'], CAPTURE['files']['parent']['bytes'])
        self.assertEqual(base['end_ordinal_exclusive'], parent[-1]['ordinal'] + 1)
        self.assertEqual(meta['forked_from_ordinal_exclusive'], base['end_ordinal_exclusive'])
        self.assertEqual(child[0]['ordinal'], base['end_ordinal_exclusive'])
        self.assertEqual(meta['id'], CAPTURE['threads']['fork'])
        self.assertNotEqual(meta['session_id'], CAPTURE['threads']['first'])
        record = usage_records('fork')[0]['payload']
        self.assertEqual(record['usage']['total_tokens'], 1150)
        self.assertEqual(record['thread_token_usage']['total_tokens'], 8050)
        self.assertTrue(all(row['payload']['thread_id'] == meta['id'] for row in usage_records('fork')))

    def test_missing_usage_and_reported_zero_are_different(self):
        records = [row['payload'] for row in usage_records('fork')]
        self.assertNotIn('response-missing', [record['response_id'] for record in records])
        zero, = [record for record in records if record['response_id'] == 'response-zero']
        self.assertEqual(zero['usage']['total_tokens'], 0)
        self.assertEqual(zero['turn_id'], CAPTURE['turns']['zero'])

    def test_reported_usage_matches_the_local_response_fixture(self):
        records = {row['payload']['response_id']: row['payload']
                   for role in ('parent', 'second', 'fork') for row in usage_records(role)}
        expected = {row['responseId']: row['suppliedUsage'] for row in CAPTURE['requests']}
        self.assertEqual(set(records), {key for key, value in expected.items() if value is not None})
        for response_id, record in records.items():
            supplied = expected[response_id]
            self.assertEqual(record['usage']['total_tokens'], supplied['total_tokens'])
            self.assertEqual(record['usage']['input_tokens'], supplied['input_tokens'])
            self.assertEqual(record['usage']['cached_input_tokens'], supplied['input_tokens_details']['cached_tokens'])
            self.assertEqual(record['usage']['output_tokens'], supplied['output_tokens'])
            self.assertEqual(record['usage']['reasoning_output_tokens'], supplied['output_tokens_details']['reasoning_tokens'])
        self.assertEqual(sum(row['usage']['total_tokens'] for row in records.values()), 12650)

    def test_copy_replay_and_conflict_keep_the_emitted_identity(self):
        for case in ('renamed_copy', 'replayed_record', 'conflicting_copy'):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                prepare_case(root, case)
                records = [json.loads(line)['payload'] for path in root.rglob('*.jsonl')
                           for line in path.read_text().splitlines()
                           if json.loads(line)['type'] == 'token_usage_record']
                self.assertEqual(len(records), 2)
                identities = {(row['thread_id'], row['turn_id'], row['response_id']) for row in records}
                self.assertEqual(len(identities), 1)
                self.assertEqual(records[0] == records[1], case != 'conflicting_copy')


class CapturedScannerComparisons(unittest.TestCase):
    @unittest.skipUnless(os.environ.get('OPENUSAGE_CODEX_CAPTURE_RESULTS'), 'Set the capture result path for scanner execution.')
    def test_unchanged_scanners_with_captured_producer_output(self):
        output = Path(os.environ['OPENUSAGE_CODEX_CAPTURE_RESULTS'])
        self.assertFalse(output.exists())
        raw_path = Path(os.environ['OPENUSAGE_CODEX_CAPTURE_RECEIPT'])
        self.assertEqual(sha256(raw_path), CAPTURE['receiptSha256'])
        receipt = json.loads(raw_path.read_text())
        for role in CAPTURE['files']:
            raw_captured(role, receipt)
        binaries = {
            'production': ROOT / 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu',
            'candidate': ROOT / '.tools/qualification-oracles/ccusage-pi-bounded-release-linux-x86_64'}
        pins = {
            'production': json.loads((ROOT / 'crates/openusage-locallogs/sources/ccusage-20.0.48-linux-x86_64.json').read_text())['executableSha256'],
            'candidate': json.loads((ROOT / 'tests/tools/fixtures/pi-producer/pin.json').read_text())['boundedReleaseScannerSha256']}
        for name, binary in binaries.items():
            self.assertEqual(sha256(binary), pins[name])
        fixture_hashes = {path.name: sha256(path) for path in sorted(FIXTURE.iterdir()) if path.is_file()}
        evidence = {'inputHashes': fixture_hashes, 'binaryHashes': pins,
                    'privateCaptureSha256': sha256(raw_path),
                    'cases': [], 'complete': False}
        try:
            with tempfile.TemporaryDirectory(prefix='openusage-codex-capture-') as directory:
                root = Path(directory)
                for case, expected in CASES.items():
                    selected = root / case
                    prepare_case(selected, case, receipt)
                    before = source_hashes(selected)
                    for kind, args in [('daily', ['daily', '--by-agent']), ('sessions', ['codex', 'session'])]:
                        row = {'case': case, 'kind': kind, 'expectedTotalTokens': expected,
                               'expectedOutcome': 'refusal' if expected is None else 'observations',
                               'runs': []}
                        evidence['cases'].append(row)
                        for name, binary in binaries.items():
                            for repetition in range(2):
                                with tempfile.TemporaryDirectory(dir=root, prefix='empty-home-') as home:
                                    command = [str(binary), *args, '--json', '--offline', '--mode',
                                               'calculate', '--timezone', 'UTC', '--no-color']
                                    result = subprocess.run(command, cwd=home, timeout=20,
                                                            env={'HOME': home, 'CODEX_HOME': str(selected),
                                                                 'TZ': 'UTC', 'NO_COLOR': '1', 'LOG_LEVEL': '0'},
                                                            stdin=subprocess.DEVNULL, capture_output=True)
                                    try:
                                        report = json.loads(result.stdout)
                                    except ValueError:
                                        report = None
                                    observed = measure(report, kind)
                                    row['runs'].append({'binary': name, 'repetition': repetition,
                                                        'exitCode': result.returncode, 'report': report,
                                                        'measurement': observed,
                                                        'stdoutSha256': hashlib.sha256(result.stdout).hexdigest(),
                                                        'stderr': result.stderr.decode(errors='replace'),
                                                        'homeUnchanged': not any(Path(home).iterdir())})
                        runs = row['runs']
                        row['reportsMatch'] = len({(run['exitCode'], run['stdoutSha256']) for run in runs}) == 1
                        row['expectationMatches'] = [run['exitCode'] != 0 if expected is None else
                            run['exitCode'] == 0 and run['measurement']['state'] == 'observations'
                            and run['measurement']['totalTokens'] == expected for run in runs]
                    self.assertEqual(source_hashes(selected), before)
            evidence['complete'] = True
        finally:
            evidence['binaryHashesAfter'] = {name: sha256(path) for name, path in binaries.items()}
            evidence['inputHashesAfter'] = {path.name: sha256(path) for path in sorted(FIXTURE.iterdir()) if path.is_file()}
            output.write_text(json.dumps(evidence, indent=2) + '\n')
        self.assertEqual(evidence['binaryHashesAfter'], pins)
        self.assertEqual(evidence['inputHashesAfter'], evidence['inputHashes'])
        self.assertTrue(all(row['reportsMatch'] for row in evidence['cases']))
        self.assertTrue(all(run['homeUnchanged'] for row in evidence['cases'] for run in row['runs']))


if __name__ == '__main__':
    unittest.main()
