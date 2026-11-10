"""Test whether reports from bounded Pi batches can be merged safely."""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

from pi_scale import MIB, pad, row
from pi_producer_support import PIN, ROOT, produce, verify_source
from scripts.storage import diagnostic_run


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def snapshot(binary, home, roots, *, unavailable=False):
    result = subprocess.run(
        [binary, 'pi', 'snapshot', '--json', '--offline', '--no-color',
         '--mode', 'calculate', '--timezone', 'UTC'],
        cwd=home, env={'HOME': str(home), 'PI_AGENT_DIR': ','.join(map(str, roots)),
                       'NO_COLOR': '1', 'TZ': 'UTC'},
        stdin=subprocess.DEVNULL, capture_output=True, timeout=20,
    )
    assert not list(home.iterdir())
    assert len(result.stdout) + len(result.stderr) < 128 * 1024
    if unavailable:
        assert result.returncode > 0 and not result.stdout
        assert b'Pi source identity' in result.stderr
        assert str(home.parent).encode() not in result.stderr
        assert b'changed private content' not in result.stderr
        return None
    assert result.returncode == 0, f'Snapshot exited with {result.returncode}'
    report = json.loads(result.stdout)
    assert set(report) == {'schemaVersion', 'tool', 'daily', 'session'}
    assert report['schemaVersion'] == 1 and report['tool'] == 'pi'
    return report


def check_totals(report, sessions):
    expected = sum(sessions.values())
    daily = report['daily']
    session = report['session']
    assert [item['totalTokens'] for item in daily['daily']] == [expected]
    assert {item['sessionId']: item['totalTokens'] for item in session['sessions']} == sessions
    assert len(session['sessions']) == len(sessions)
    assert daily['totals']['totalTokens'] == session['totals']['totalTokens'] == expected
    return expected


def compare_batches(binary, source, root):
    home = root / 'empty-home'
    home.mkdir()
    original = Path(produce(source, root, 'seed')['file'])
    fork = Path(produce(source, root, 'fork')['file'])
    fork_root = root / 'fork-only'
    fork_root.mkdir()
    fork.rename(fork_root / fork.name)

    copy_root, conflict_root, independent_root = [root / name for name in
                                                  ('copy', 'not-selected', 'independent')]
    for folder in (copy_root, conflict_root, independent_root):
        folder.mkdir()
    shutil.copyfile(original, copy_root / 'renamed.jsonl')
    records = [json.loads(line) for line in original.read_bytes().splitlines()]
    (independent_root / 'session.jsonl').write_bytes(
        row(records[0] | {'id': 'independent'}) + b''.join(map(row, records[1:])))
    assistant, = [record for record in records if record.get('message', {}).get('role') == 'assistant']
    assistant['message']['content'][0]['text'] = 'changed private content'
    (conflict_root / 'session.jsonl').write_bytes(b''.join(map(row, records)))

    selected = [root / 'sessions']
    before = {path: digest(path) for path in root.rglob('*.jsonl')}
    baseline = snapshot(binary, home, selected)
    one = check_totals(baseline, {'root': 120})
    assert baseline == snapshot(binary, home, [copy_root]) == snapshot(binary, home, [conflict_root])
    assert baseline == snapshot(binary, home, [*selected, copy_root])
    assert baseline == snapshot(binary, home, [copy_root, *selected])
    snapshot(binary, home, [*selected, conflict_root], unavailable=True)
    independent = check_totals(snapshot(binary, home, [*selected, independent_root]),
                               {'root': 120, 'independent': 120})
    snapshot(binary, home, [fork_root], unavailable=True)
    forked = check_totals(snapshot(binary, home, [fork_root, copy_root, *selected]),
                          {'root': 120, 'fork': 31})
    assert before == {path: digest(path) for path in root.rglob('*.jsonl')}
    return original, home, {'singleOrCopiedTokens': one, 'isolatedCopyAndConflictReportsEqual': True,
                            'jointConflictRefused': True, 'independentTokens': independent,
                            'isolatedForkRefused': True, 'jointForkAndCopyTokens': forked}


def check_byte_limit(binary, root, original, home):
    # Keep file, record and directory counts below their caps. Only source bytes grow.
    batches = [root / f'batch-{index}' for index in range(8)]
    files = []
    for batch in batches:
        batch.mkdir()
        for index in range(4):
            path = batch / f'copy-{index}.jsonl'
            shutil.copyfile(original, path)
            pad(path, 32 * MIB)
            files.append(path)
    size = sum(path.stat().st_size for path in files)
    assert size == 1024 * MIB
    before = [digest(path) for path in files]
    snapshot(binary, home, batches, unavailable=True)
    totals = [check_totals(snapshot(binary, home, [batch]), {'root': 120}) for batch in batches]
    assert before == [digest(path) for path in files]
    return {'sourceBytes': size, 'files': len(files), 'jointScanRefused': True,
            'batchTokens': totals, 'naiveSumTokens': sum(totals)}


def main():
    if sys.platform != 'linux':
        raise SystemExit('The Pi history experiment needs Linux')
    binary = Path(os.environ['OPENUSAGE_PI_QUALIFICATION_BIN']).resolve(strict=True)
    if digest(binary) != PIN['scannerSha256']:
        raise ValueError('The selected candidate does not match the fixture pin')
    source = verify_source(os.environ['OPENUSAGE_PI_PRODUCER_SOURCE'])
    with diagnostic_run(ROOT, 'pi-large-history') as run:
        with tempfile.TemporaryDirectory(dir=run) as temporary:
            root = Path(temporary)
            original, home, identity = compare_batches(binary, source, root)
            limits = check_byte_limit(binary, root, original, home)
        result = {'scannerSha256': PIN['scannerSha256'], 'producerCommit': PIN['sourceCommit'],
                  'kernel': platform.release(), 'machine': platform.machine(),
                  'identity': identity, 'limits': limits}
        output = run / 'results.json'
        output.write_text(json.dumps(result, indent=2) + '\n')
        print(f'Pi large history experiment passed. Metadata: {output.relative_to(ROOT)}')


if __name__ == '__main__':
    main()
