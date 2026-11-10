"""Compare the Pi bounded capture prototype on private synthetic histories."""
import json
import os
from pathlib import Path
import platform
import sys
import tempfile

from pi_large_history import check_totals, compare_batches, digest, padded_copies, snapshot
from pi_producer_support import PIN, ROOT, verify_source
from pi_scale import MIB, grouped, measure_scanner, pad, session, shape
from scripts.storage import diagnostic_run


def scanner(variable, pin):
    binary = Path(os.environ[variable]).resolve(strict=True)
    if digest(binary) != PIN[pin]:
        raise ValueError(f'{variable} does not match the scanner pin')
    return binary


def profile(binary, root, roots, inspect):
    return measure_scanner(binary, root, roots, 60, 64 * MIB, inspect_report=inspect)


def replayed_copies(baseline, prototype, root, original, home):
    roots, files = padded_copies(root, original)
    size = sum(path.stat().st_size for path in files)
    assert size == 1024 * MIB
    before = [digest(path) for path in files]
    snapshot(baseline, home, roots, unavailable=True)
    measured = profile(prototype, root, roots, lambda report: check_totals(report, {'root': 120}))
    assert before == [digest(path) for path in files]
    assert not list(home.iterdir())
    return {'bytes': size, 'files': len(files), 'tokens': 120, 'prototype': measured}


def history(root, header, message, files, records_per_file, message_bytes, *, padded_size=None):
    roots = [root / 'selected' / f'root-{index:02}' for index in range(8)]
    for folder in roots:
        folder.mkdir(parents=True)
    (root / 'empty-home').mkdir()
    for index in range(files):
        path = roots[index % len(roots)] / f's-{index:05}.jsonl'
        session(path, header, message, index, records_per_file, message_bytes=message_bytes)
        if padded_size is not None:
            pad(path, padded_size)
    return roots


def check_history(report, sessions, records_per_file, usage):
    assert report['schemaVersion'] == 1 and report['tool'] == 'pi'
    per_session = records_per_file * usage
    daily = report['daily']['daily']
    session_rows = report['session']['sessions']
    assert len(daily) == min(sessions, 365)
    assert len(session_rows) == sessions
    assert {row['sessionId']: row['totalTokens'] for row in session_rows} == {
        f's-{index:05}': per_session for index in range(sessions)
    }
    for index, row in enumerate(daily):
        assert row['totalTokens'] == len(range(index, sessions, 365)) * per_session
    expected = sessions * per_session
    assert report['daily']['totals']['totalTokens'] == expected
    assert report['session']['totals']['totalTokens'] == expected


def matched_history(baseline, prototype, root, header, message, usage):
    files, records_per_file = 4000, 25
    roots = history(root, header, message, files, records_per_file, 512)
    expected_report = None

    def inspect(report):
        nonlocal expected_report
        check_history(report, files, records_per_file, usage)
        if expected_report is None:
            expected_report = report
        else:
            assert report == expected_report

    result = {'shape': shape(roots, files * records_per_file), 'samples': []}
    for _ in range(3):
        result['samples'].append({'baseline': profile(baseline, root, roots, inspect),
                                  'prototype': profile(prototype, root, roots, inspect)})
    return result


def dense_history(prototype, root, header, message, usage):
    files, records_per_file = 64, 1024
    roots = history(root, header, message, files, records_per_file, 15500, padded_size=16 * MIB)
    counts = shape(roots, files * records_per_file)
    assert counts['bytes'] == 1024 * MIB
    measured = profile(prototype, root, roots,
                       lambda report: check_history(report, files, records_per_file, usage))
    tokens = files * records_per_file * usage
    case = {'root': str(root), 'database': 'history.db', 'outcome': 'accepted',
            'days': files, 'sessions': files, 'tokens': tokens}
    accepted = grouped(case)
    with (roots[0] / 's-00000.jsonl').open('ab') as stream:
        stream.write(b' ')
    refused = grouped(dict(case, outcome='candidate', scanId=accepted['scanId']))
    return {'shape': counts, 'tokens': tokens, 'prototype': measured,
            'grouped': accepted, 'overflowRefusal': refused}


def main():
    if sys.platform != 'linux':
        raise SystemExit('The Pi bounded capture test needs Linux')
    baseline = scanner('OPENUSAGE_PI_OBSERVATION_BIN', 'scannerSha256')
    prototype = scanner('OPENUSAGE_PI_QUALIFICATION_BIN', 'boundedScannerSha256')
    source = verify_source(os.environ['OPENUSAGE_PI_PRODUCER_SOURCE'])
    os.environ.update(OPENUSAGE_PI_QUALIFICATION_BIN=str(prototype),
                      OPENUSAGE_PI_QUALIFICATION_VARIANT='bounded')
    with diagnostic_run(ROOT, 'pi-bounded-capture') as run:
        with tempfile.TemporaryDirectory(dir=run) as name:
            root = Path(name)
            original, home, identity = compare_batches(prototype, source, root)
            records = [json.loads(line) for line in original.read_bytes().splitlines()]
            header = records[0]
            message, = [record for record in records if record.get('message', {}).get('role') == 'assistant']
            usage = message['message']['usage']['totalTokens']
            print('Checking 1 GiB of copied sessions', flush=True)
            copies = replayed_copies(baseline, prototype, root, original, home)
        with tempfile.TemporaryDirectory(dir=run) as name:
            print('Comparing 100,000 records', flush=True)
            matched = matched_history(baseline, prototype, Path(name), header, message, usage)
        with tempfile.TemporaryDirectory(dir=run) as name:
            print('Scanning and saving 1 GiB of distinct records', flush=True)
            dense = dense_history(prototype, Path(name), header, message, usage)
        result = {'scannerSha256': PIN['boundedScannerSha256'], 'baselineSha256': PIN['scannerSha256'],
                  'producerCommit': PIN['sourceCommit'], 'kernel': platform.release(),
                  'machine': platform.machine(), 'identity': identity, 'copies': copies,
                  'matched': matched, 'dense': dense}
        output = run / 'results.json'
        output.write_text(json.dumps(result, indent=2) + '\n')
        print(f'Pi bounded capture passed. Metadata: {output.relative_to(ROOT)}')


if __name__ == '__main__':
    main()
