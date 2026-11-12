"""Record only candidate reports and synthetic source fingerprints."""
import argparse
import hashlib
import json
from pathlib import Path
import tempfile

from test_ccusage_codex_candidate import digest, synthetic_rows, write_rows
from test_ccusage_codex_snapshot import run_command
from test_codex_producer import CAPTURE, ROOT, prepare_case, raw_captured, source_hashes


def record_fixture(capture_receipt):
    pin_path = ROOT / 'tests/tools/fixtures/codex-snapshot-candidate/pin.json'
    pin = json.loads(pin_path.read_text())
    binary = ROOT / pin['oraclePath']
    if digest(binary) != pin['executableSha256']:
        raise ValueError('The candidate artifact does not match its pin.')
    if digest(capture_receipt) != CAPTURE['receiptSha256']:
        raise ValueError('The producer receipt does not match its capture pin.')
    receipt = json.loads(capture_receipt.read_text())
    for role in CAPTURE['files']:
        raw_captured(role, receipt)
    cases = {}
    for name, rows, expected in [
        ('known', synthetic_rows(), 17),
        ('unattributed', synthetic_rows(model=None), 17),
        ('unknown_model', synthetic_rows(model='openusage-unknown-codex-model'), 17),
        ('zero', synthetic_rows(total=0), 0),
        ('zero_unattributed', synthetic_rows(total=0, model=None), 0),
        ('empty', [], 0),
        ('prefix', synthetic_rows()[:4], 17),
        ('mixed_attribution', None, 40),
        ('two_days', None, 40),
        ('rounding_partition', None, 802),
        ('lineage', None, 12650),
    ]:
        with tempfile.TemporaryDirectory(prefix='openusage-codex-contract-') as directory:
            selected = Path(directory)
            if rows:
                write_rows(selected / 'sessions/source.jsonl', rows)
            elif name in ('mixed_attribution', 'two_days'):
                first = synthetic_rows(model=None if name == 'mixed_attribution' else 'gpt-5')
                second = synthetic_rows(thread='thread-b', response='response-b', total=23)
                if name == 'two_days':
                    for row in second:
                        row['timestamp'] = '2026-11-12T12:00:00.000Z'
                write_rows(selected / 'sessions/first.jsonl', first)
                write_rows(selected / 'sessions/second.jsonl', second)
            elif name == 'rounding_partition':
                for index, total in enumerate((182, 265, 46, 205, 104)):
                    write_rows(selected / f'sessions/source-{index}.jsonl',
                               synthetic_rows(thread=f'thread-{index}',
                                              response=f'response-{index}', total=total))
            elif name == 'lineage':
                prepare_case(selected, name, receipt)
            before = source_hashes(selected)
            result = run_command(binary, selected)
            if result.returncode != 0:
                raise ValueError('The candidate report did not complete.')
            report = json.loads(result.stdout)
            for section in ('daily', 'session'):
                if report[section]['totals']['totalTokens'] != expected:
                    raise ValueError('The candidate report does not match the independent total.')
            if source_hashes(selected) != before:
                raise ValueError('The synthetic source records changed during capture.')
            cases[name] = {
                'expectedTotalTokens': expected,
                'sourceFiles': [{'path': path, 'sha256': sha} for path, sha in before.items()],
                'stdoutSha256': hashlib.sha256(result.stdout).hexdigest(),
                'snapshot': report,
            }
    if digest(binary) != pin['executableSha256']:
        raise ValueError('The candidate artifact does not match its pin.')
    return {
        'scannerVersion': pin['version'],
        'scannerSha256': pin['executableSha256'],
        'producerCaptureSha256': CAPTURE['receiptSha256'],
        'generatorSha256': digest(Path(__file__)),
        'cases': cases,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--capture-receipt', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    reports = record_fixture(args.capture_receipt)
    with args.output.open('x') as handle:
        json.dump(reports, handle, indent=2, sort_keys=True)
        handle.write('\n')
    print(json.dumps({'cases': len(reports['cases']), 'fixtureSha256': digest(args.output),
                      'scannerSha256': reports['scannerSha256']}))


if __name__ == '__main__':
    main()
