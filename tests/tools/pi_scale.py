"""Measure the pinned Pi scanner and grouped SQLCipher path with private synthetic input."""
from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import time

from pi_producer_support import PIN, produce, verify_source
from test_ccusage_formats import ROOT

sys.path.insert(0, str(ROOT))
from scripts.storage import diagnostic_run

MIB = 1 << 20
SAMPLES = 3
WORKLOADS = [
    ('year', 4, 1000, 10),
    ('record-cap', 16, 4000, 25),
    ('file-and-entry-caps', 64, 8192, 1),
    ('file-and-depth-caps', 1, 1, 1),
    ('total-byte-cap', 4, 5, 1),
]


def row(value):
    return (json.dumps(value, separators=(',', ':')) + '\n').encode()


def session(path, header, message, index, records):
    stamp = datetime(2025, 1, 1, tzinfo=timezone.utc) + timedelta(days=index % 365)
    iso = stamp.isoformat(timespec='milliseconds').replace('+00:00', 'Z')
    first = dict(header, id=f's-{index:05}', timestamp=iso, cwd='/synthetic/project')
    body = dict(message['message'], timestamp=int(stamp.timestamp()) * 1000,
                content=[{'type': 'text', 'text': 'x' * 512}])
    with path.open('xb') as stream:
        stream.write(row(first))
        parent = None
        for index in range(records):
            key = f'r-{index:05}'
            stream.write(row(dict(message, id=key, parentId=parent, timestamp=iso, message=body)))
            parent = key


def pad(path, size):
    # Blank space after a complete JSON line isolates the file-size check.
    remaining = size - path.stat().st_size
    assert remaining >= 0
    with path.open('ab') as stream:
        block = b' ' * MIB
        while remaining:
            count = min(remaining, MIB)
            stream.write(block[:count])
            remaining -= count


def shape(roots, records):
    entries = [path for root in roots for path in root.rglob('*')]
    sources = [path for path in entries if path.is_file() and path.suffix == '.jsonl']
    return {'roots': len(roots), 'files': len(sources), 'entries': len(entries),
            'records': records, 'bytes': sum(path.stat().st_size for path in sources)}


def measure_scanner(binary, root, roots, timeout, output_bound):
    home = root / 'empty-home'
    env = {'HOME': str(home), 'PI_AGENT_DIR': ','.join(map(str, roots)),
           'NO_COLOR': '1', 'TZ': 'UTC'}
    argv = [binary, 'pi', 'snapshot', '--json', '--offline', '--no-color',
            '--mode', 'calculate', '--timezone', 'UTC']
    started = time.monotonic()
    with tempfile.TemporaryFile(dir=root) as output, subprocess.Popen(
            argv, cwd=home, env=env, stdin=subprocess.DEVNULL, stdout=output,
            stderr=subprocess.DEVNULL) as child:
        while True:
            pid, status, usage = os.wait4(child.pid, os.WNOHANG)
            if pid:
                child.returncode = os.waitstatus_to_exitcode(status)
                break
            if time.monotonic() - started >= timeout or os.fstat(output.fileno()).st_size > output_bound:
                child.kill()
                raise RuntimeError('The direct scanner exceeded its deadline or output bound')
            time.sleep(0.005)
        elapsed = time.monotonic() - started
        size = os.fstat(output.fileno()).st_size
        if child.returncode or not 0 < size <= output_bound:
            raise RuntimeError(f'The direct scanner returned {child.returncode} and {size} bytes')
    return {'wallSeconds': elapsed, 'userSeconds': usage.ru_utime, 'systemSeconds': usage.ru_stime,
            'peakRssKiB': usage.ru_maxrss, 'outputBytes': size}


def grouped(case):
    command = ['cargo', 'test', '--offline', '--locked', '-q', '-p', 'openusage-storage',
               '--features', 'pi-qualification', '--test', 'pi_scale', '--',
               '--ignored', '--nocapture']
    env = dict(os.environ, OPENUSAGE_PI_SCALE_CASE=json.dumps(case))
    result = subprocess.run(command, cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                            capture_output=True, text=True, timeout=180, check=False)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    report, = [line.removeprefix('PI_SCALE ') for line in result.stdout.splitlines()
               if line.startswith('PI_SCALE ')]
    return json.loads(report)


def run_workload(binary, root, header, message, workload):
    name, root_count, file_count, records_per_file = workload
    roots = [root / 'selected' / f'root-{i:02}' for i in range(root_count)]
    for path in roots:
        path.mkdir(parents=True)
    (root / 'empty-home').mkdir()
    first_folder = roots[0]
    if name == 'file-and-depth-caps':
        first_folder = first_folder.joinpath(*(['nested'] * 64))
        first_folder.mkdir(parents=True)

    paths = []
    for index in range(file_count):
        parent = first_folder if index == 0 else roots[index % root_count]
        path = parent / f's-{index:05}.jsonl'
        session(path, header, message, index, records_per_file)
        paths.append(path)
    if name == 'file-and-entry-caps':
        for index in range(32768 - file_count):
            (roots[index % root_count] / f'e-{index:05}.txt').touch(exist_ok=False)
    if name == 'file-and-depth-caps':
        pad(paths[0], 32 * MIB)
    if name == 'total-byte-cap':
        for path, size in zip(paths, [32, 32, 32, 16, 16], strict=True):
            pad(path, size * MIB)

    records = file_count * records_per_file
    case = {'root': str(root), 'outcome': 'accepted', 'days': min(file_count, 365),
            'sessions': file_count, 'tokens': records * message['message']['usage']['totalTokens']}
    result = {'name': name, 'shape': shape(roots, records), 'tokens': case['tokens'],
              'samples': [], 'refusals': []}
    print(f'Measuring {name}', flush=True)
    for i in range(SAMPLES):
        case['database'] = f'sample-{i}.db'
        scan = grouped(case)
        # Run the matching child command separately to measure raw bytes and child resources.
        profile = measure_scanner(binary, root, roots, scan['timeoutSeconds'], scan['outputBound'])
        result['samples'].append({'grouped': scan, 'scanner': profile})
    case['scanId'] = scan['scanId']

    def refuse(label, outcome='candidate', seen_roots=roots, seen_records=records, **bounds):
        metrics = grouped(dict(case, outcome=outcome, **bounds))
        result['refusals'].append(dict(metrics, label=label, shape=shape(seen_roots, seen_records)))

    if name == 'record-cap':
        refuse('output-minus-one', 'output', maxOutput=profile['outputBytes'] - 1)
        refuse('one-millisecond-deadline', 'deadline', timeoutMillis=1)
        # Replace one private fixture file with a version that holds one more complete record.
        extra = root / 'extra.jsonl'
        session(extra, header, message, 0, records_per_file + 1)
        extra.replace(paths[0])
        refuse('one-extra-record', seen_records=records + 1)
    elif name == 'file-and-entry-caps':
        extra = root / 'selected/root-64'
        extra.mkdir()
        refuse('one-extra-root', 'roots', seen_roots=[*roots, extra])
        extra.rmdir()
        extra = roots[0] / 'extra.txt'
        extra.touch(exist_ok=False)
        refuse('one-extra-entry')
        extra.unlink()
        (roots[0] / 'e-00000.txt').unlink()
        session(roots[0] / 'extra.jsonl', header, message, file_count, 1)
        refuse('one-extra-file', seen_records=records + 1)
    elif name == 'file-and-depth-caps':
        pad(paths[0], 32 * MIB + 1)
        refuse('one-extra-file-byte')
        with paths[0].open('r+b') as stream:
            stream.truncate(32 * MIB)
        (first_folder / 'extra').mkdir()
        refuse('one-extra-level')
    elif name == 'total-byte-cap':
        lower = root / 'lower.jsonl'
        session(lower, header, message, file_count - 1, 0)
        pad(lower, 16 * MIB + 1)
        lower.replace(paths[-1])
        refuse('one-extra-aggregate-byte', seen_records=records - 1)
    return result


def main():
    if sys.platform != 'linux':
        raise SystemExit('The Pi scale fixture needs Linux')
    binary = Path(os.environ['OPENUSAGE_PI_QUALIFICATION_BIN']).resolve(strict=True)
    with binary.open('rb') as executable:
        if hashlib.file_digest(executable, 'sha256').hexdigest() != PIN['scannerSha256']:
            raise ValueError('The selected candidate does not match the fixture pin')
    os.environ['OPENUSAGE_PI_QUALIFICATION_BIN'] = str(binary)
    source = verify_source(os.environ['OPENUSAGE_PI_PRODUCER_SOURCE'])
    with diagnostic_run(ROOT, 'pi-scale') as run:
        with tempfile.TemporaryDirectory(dir=run) as name:
            path = Path(name)
            state = produce(source, path, 'seed')
            entries = [json.loads(line) for line in Path(state['file']).read_text().splitlines()]
            header = entries[0]
            message, = [entry for entry in entries if entry.get('message', {}).get('role') == 'assistant']
        result = {'scannerSha256': PIN['scannerSha256'], 'producerCommit': PIN['sourceCommit'],
                  'kernel': platform.release(), 'machine': platform.machine(), 'samples': SAMPLES,
                  'workloads': []}
        output = run / 'results.json'
        print(f'Metadata: {output.relative_to(ROOT)}', flush=True)
        for workload in WORKLOADS:
            with tempfile.TemporaryDirectory(dir=run) as name:
                result['workloads'].append(run_workload(binary, Path(name), header, message, workload))
            output.write_text(json.dumps(result, indent=2) + '\n')
        print('All Pi scale workloads and refusals passed.', flush=True)


if __name__ == '__main__':
    main()
