"""Run pinned local-log reports against private read-only fixtures."""
import hashlib
import json
import os
from pathlib import Path
import tempfile

from test_ccusage_formats import PIN, file_hashes
from test_ccusage_opencode_runtime import checked_run, sandbox


KINDS = ('daily', 'session')


def report(binary, root, names, kind, tool, variable):
    with tempfile.TemporaryDirectory(dir=root, prefix='empty-home-') as temporary:
        home = Path(temporary)
        command = sandbox() + ['--ro-bind', str(binary), '/runtime/ccusage',
                               '--bind', str(home), '/home/fixture', '--setenv', 'HOME', '/home/fixture']
        selected = []
        for name in names:
            mount = '/selected/' + name
            if (root / name).exists():
                command.extend(['--ro-bind', str(root / name), mount])
            selected.append(mount)
        command.extend(['--setenv', variable, ','.join(selected), '--chdir', '/home/fixture',
                        '/runtime/ccusage', tool, kind, '--json', '--offline', '--mode',
                        'calculate', '--timezone', 'UTC', '--no-color'])
        result = checked_run(command, timeout=15)
        if list(home.iterdir()):
            raise ValueError('The scanner changed the private home.')
        return result


def collect(binary, cases, tool, variable, canary_path):
    observations = {}
    for name, spec in cases.items():
        with tempfile.TemporaryDirectory(prefix=f'openusage-{tool}-') as temporary:
            root = Path(temporary)
            canary = root / 'not-selected' / canary_path
            canary.parent.mkdir(parents=True)
            canary.write_text('[{"id":"private-marker"}]\n')
            for selected, files in spec['roots'].items():
                if files is None:
                    continue
                (root / selected).mkdir(parents=True)
                for relative, value in files.items():
                    path = root / selected / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(value if isinstance(value, bytes) else json.dumps(value).encode() + b'\n')
                    os.utime(path, (1767312000, 1767312000))
            before = file_hashes(root)
            reports = {}
            for kind in KINDS:
                first, second = [report(binary, root, spec['roots'], kind, tool, variable) for _ in range(2)]
                if (first.returncode, first.stdout, first.stderr) != (second.returncode, second.stdout, second.stderr):
                    raise ValueError(f'Nonrepeatable report: {name}, {kind}.')
                if first.returncode:
                    if first.stdout or b'private-marker' in first.stderr:
                        raise ValueError(f'Refusal exposed source data: {name}, {kind}.')
                    reports[kind] = {'exitCode': first.returncode, 'matchesRequired': spec['required'][kind] is None}
                    continue
                output = json.loads(first.stdout)
                totals = output['totals']
                rows = output['daily' if kind == 'daily' else 'sessions']
                key = 'date' if kind == 'daily' else 'sessionId'
                required = spec['required'][kind]
                matches = required is not None and totals['totalTokens'] == required
                matches &= len(rows) == spec['rows'].get(kind, len(rows))
                matches &= all(totals.get(field) == value for field, value in spec['fields'].items())
                reports[kind] = {'totals': totals, 'rows': [[row[key], row['totalTokens']] for row in rows],
                                 'sha256': hashlib.sha256(json.dumps(output, sort_keys=True).encode()).hexdigest(),
                                 'matchesRequired': matches}
            if file_hashes(root) != before:
                raise ValueError(f'The scanner changed source files: {name}.')
            observations[name] = {'requiredTokens': spec['required'], 'requiredRows': spec['rows'],
                                  'requiredFields': spec['fields'], 'reports': reports}
    return observations


def pinned_binary(path):
    binary = Path(path).resolve(strict=True)
    pin = json.loads(PIN.read_text())
    if hashlib.sha256(binary.read_bytes()).hexdigest() != pin['executableSha256']:
        raise ValueError('The scanner digest does not match the production pin.')
    return binary
