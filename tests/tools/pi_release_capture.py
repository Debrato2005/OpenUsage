"""Measure the release Pi capture and encrypted save with private synthetic input."""
import json
import os
from pathlib import Path
import platform
import sys
import tempfile

from pi_bounded_capture import dense_history, profile, scanner
from pi_large_history import check_totals, compare_batches, padded_copies
from pi_producer_support import PIN, ROOT, verify_source
from scripts.storage import diagnostic_run


def main():
    if sys.platform != 'linux':
        raise SystemExit('The Pi release capture test needs Linux')
    binary = scanner('OPENUSAGE_PI_QUALIFICATION_BIN', 'boundedReleaseScannerSha256')
    source = verify_source(os.environ['OPENUSAGE_PI_PRODUCER_SOURCE'])
    allowed = sorted(os.sched_getaffinity(0))
    os.sched_setaffinity(0, allowed[:4])
    os.environ.update(OPENUSAGE_PI_QUALIFICATION_BIN=str(binary),
                      OPENUSAGE_PI_QUALIFICATION_VARIANT='bounded-release')
    with diagnostic_run(ROOT, 'pi-release-capture') as run:
        with tempfile.TemporaryDirectory(dir=run) as name:
            root = Path(name)
            original, home, identity = compare_batches(binary, source, root)
            records = [json.loads(line) for line in original.read_bytes().splitlines()]
            header = records[0]
            message, = [row for row in records if row.get('message', {}).get('role') == 'assistant']
            usage = message['message']['usage']['totalTokens']
            roots, _ = padded_copies(root, original)
            copies = [profile(binary, root, roots,
                              lambda report: check_totals(report, {'root': 120})) for _ in range(3)]
            assert not list(home.iterdir())
        samples = []
        for index in range(3):
            print(f'Measuring release sample {index + 1}', flush=True)
            with tempfile.TemporaryDirectory(dir=run) as name:
                samples.append(dense_history(binary, Path(name), header, message, usage, release=True))
        result = {'scannerSha256': PIN['boundedReleaseScannerSha256'],
                  'producerCommit': PIN['sourceCommit'], 'kernel': platform.release(),
                  'machine': platform.machine(), 'affinity': sorted(os.sched_getaffinity(0)),
                  'identity': identity, 'copies': copies, 'dense': samples}
        output = run / 'results.json'
        output.write_text(json.dumps(result, indent=2) + '\n')
        print(f'Pi release capture passed. Metadata: {output.relative_to(ROOT)}')


if __name__ == '__main__':
    main()
