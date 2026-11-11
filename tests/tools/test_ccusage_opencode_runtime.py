"""Run the pinned OpenCode writer against private synthetic input on Linux."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import sqlite3
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
FIXTURE = Path(__file__).resolve().parent / 'fixtures/opencode-runtime'
PIN = json.loads((FIXTURE / 'pin.json').read_text())
SOURCE_FILES = {record['path']: record['sha256'] for record in PIN['sourceFiles']}
EXPECTED = json.loads((FIXTURE / 'producer-expected.json').read_text())
METADATA = ('agent_switched', 'model_switched', 'system', 'synthetic', 'shell', 'compaction')
REQUIRED = {
    'completed': 173, 'user_peer': 173, 'repeated_save': 173, 'changed_save': 190,
    'distinct_calls': 346, 'reported_zero': 0, 'unfinished': None, 'failed': None,
    **dict.fromkeys(METADATA, 173), 'replayed': 173, 'copied_roots': 173, 'replayed_roots': 173,
}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tree_digest(root):
    digest = hashlib.sha256()
    for path in sorted(root.rglob('*')):
        if path.is_symlink():
            raise ValueError('A registry package contains a symlink.')
        if path.is_file():
            digest.update((path.relative_to(root).as_posix() + '\0' + sha256(path) + '\n').encode())
    return digest.hexdigest()


def file_hashes(root):
    return {path.relative_to(root).as_posix(): sha256(path)
            for path in sorted(root.rglob('*')) if path.is_file()}


def checked_run(command, timeout=30):
    with subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, env={'PATH': '/usr/bin'},
                          start_new_session=True) as process:
        try:
            stdout, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate(timeout=5)
            raise
    if len(stdout) > 128 * 1024 or len(stderr) > 128 * 1024:
        raise ValueError('The fixture output exceeds its limit.')
    return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def sandbox():
    command = ['/usr/bin/bwrap', '--unshare-all', '--die-with-parent', '--new-session', '--clearenv']
    for path in ('/usr', '/lib', '/lib64'):
        if Path(path).exists():
            command.extend(['--ro-bind', path, path])
    return command + ['--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                      '--setenv', 'PATH', '/usr/bin', '--setenv', 'TZ', 'UTC']


class OpenCodeRuntimeFixturePins(unittest.TestCase):
    def test_fixture_boundary_matches_the_pin(self):
        self.assertTrue(PIN['fixtureFiles'])
        for name, digest in PIN['fixtureFiles'].items():
            self.assertEqual(sha256(FIXTURE / name), digest, name)


class OpenCodeRuntimeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        variables = ('OPENUSAGE_OPENCODE_RUNTIME_SOURCE', 'OPENUSAGE_OPENCODE_RUNTIME_DEPENDENCIES',
                     'OPENUSAGE_OPENCODE_RUNTIME_BUN')
        if not all(os.environ.get(name) for name in variables):
            raise unittest.SkipTest('Set the three OPENUSAGE_OPENCODE_RUNTIME paths for this optional fixture.')
        if sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            raise unittest.SkipTest('This fixture requires Linux and Bubblewrap.')
        cls.source, cls.dependencies, cls.bun = [Path(os.environ[name]).resolve(strict=True) for name in variables]
        link = cls.source / 'node_modules'
        if not link.is_symlink() or os.readlink(link) != '../opencode-runtime-deps/node_modules':
            raise ValueError('The producer dependency link does not match the private runtime layout.')
        for name, digest in SOURCE_FILES.items():
            path = cls.source / name
            if path.is_symlink() or not path.resolve(strict=True).is_relative_to(cls.source) or sha256(path) != digest:
                raise ValueError('An OpenCode source file does not match its pin: ' + name)
        if sha256(cls.bun) != PIN['bun']['executableSha256']:
            raise ValueError('The Bun executable does not match its pin.')
        packages = set()
        for path in (cls.dependencies / 'node_modules').iterdir():
            if path.name.startswith('@'):
                if path.is_symlink() or not path.is_dir():
                    raise ValueError('A dependency scope is not a regular directory.')
                packages.update(path.name + '/' + child.name for child in path.iterdir())
            else:
                packages.add(path.name)
        if packages != PIN['registryPackages'].keys() | PIN['workspacePackages'].keys():
            raise ValueError('The dependency directory contains unpinned packages.')
        for name, record in PIN['registryPackages'].items():
            package = cls.dependencies / 'node_modules' / name
            if not package.is_dir() or package.is_symlink() or tree_digest(package) != record['extractedTreeSha256']:
                raise ValueError('A registry package does not match its pin: ' + name)
        for name, target in PIN['workspacePackages'].items():
            link = cls.dependencies / 'node_modules' / name
            if not link.is_symlink() or os.readlink(link) != target:
                raise ValueError('An OpenCode workspace link does not match its pin: ' + name)
        cls.source_hashes = {name: sha256(cls.source / name) for name in SOURCE_FILES}

    def produce(self, root):
        fixture = root / 'fixture'
        fixture.mkdir()
        for name in ('home', 'data', 'config', 'cache', 'state', 'project'):
            (fixture / name).mkdir()
        canary = root / 'outside-canary.txt'
        canary.write_text('Synthetic outside fixture.\n')
        command = sandbox() + [
            '--ro-bind', str(self.source), '/upstream/opencode-v1.19.5',
            '--ro-bind', str(self.dependencies / 'node_modules'), '/upstream/opencode-runtime-deps/node_modules',
            '--ro-bind', str(FIXTURE), '/upstream/opencode-runtime-deps/fixture',
            '--ro-bind', str(self.bun), '/runtime/bun', '--bind', str(fixture), '/fixture',
            '--setenv', 'HOME', '/fixture/home', '--setenv', 'OPENCODE_DISABLE_CHANNEL_DB', '1',
            '--setenv', 'OPENCODE_FIXTURE_CANARY_PATH', str(canary),
            '--setenv', 'OPENCODE_FIXTURE_PARENT_NETWORK', os.readlink('/proc/self/ns/net'),
        ]
        for variable, name in [('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config'),
                               ('XDG_CACHE_HOME', 'cache'), ('XDG_STATE_HOME', 'state')]:
            command.extend(['--setenv', variable, '/fixture/' + name])
        result = checked_run(command + ['--chdir', '/fixture', '/runtime/bun', '--no-install',
                                        '/upstream/opencode-runtime-deps/fixture/bootstrap.ts'])
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace')[:4000])
        self.assertEqual(result.stdout, b'')
        self.assertEqual(json.loads((fixture / 'loaded-source.json').read_text()), PIN['executedSourceFiles'])
        self.assertEqual(json.loads((fixture / 'producer-result.json').read_text()), EXPECTED)
        self.assertEqual(json.loads((fixture / 'isolation.json').read_text()),
                         {'canaryHidden': True, 'privateNetwork': True, 'privateHome': True})
        self.assertEqual({name: sha256(self.source / name) for name in SOURCE_FILES}, self.source_hashes)
        return fixture

    def test_real_writer_and_durable_replay(self):
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-runtime-') as temporary:
            fixture = self.produce(Path(temporary))
            for name, expected in EXPECTED.items():
                with self.subTest(case=name):
                    uri = (fixture / name / 'opencode.db').as_uri() + '?mode=ro&immutable=1'
                    connection = sqlite3.connect(uri, uri=True)
                    try:
                        self.assertEqual(connection.execute('PRAGMA integrity_check').fetchall(), [('ok',)])
                        self.assertEqual(connection.execute('SELECT COUNT(*) FROM session_message').fetchone()[0],
                                         expected['messageCount'])
                        self.assertEqual(connection.execute('SELECT COUNT(*) FROM event').fetchone()[0],
                                         expected['eventCount'])
                        rows = connection.execute("SELECT data FROM session_message WHERE type = 'assistant'").fetchall()
                        values = [json.loads(row[0]).get('tokens') for row in rows]
                        self.assertEqual(values, expected['assistantTokens'])
                        if REQUIRED[name] is not None:
                            total = sum(value['input'] + value['output'] + value['reasoning']
                                        + value['cache']['read'] + value['cache']['write'] for value in values)
                            self.assertEqual(total, REQUIRED[name])
                    finally:
                        connection.close()

    def report(self, binary, folders, kind, home):
        command = sandbox() + ['--ro-bind', str(binary), '/runtime/ccusage',
                               '--bind', str(home), '/home/fixture', '--setenv', 'HOME', '/home/fixture',
                               '--setenv', 'NO_COLOR', '1']
        selected = []
        for index, folder in enumerate(folders):
            selected.append(f'/sources/{index}')
            command.extend(['--ro-bind', str(folder), selected[-1]])
        command.extend(['--setenv', 'OPENCODE_DATA_DIR', ','.join(selected), '--chdir', '/home/fixture',
                        '/runtime/ccusage'])
        command.extend(['daily', '--by-agent'] if kind == 'daily' else ['opencode', 'session'])
        return checked_run(command + ['--json', '--offline', '--mode', 'calculate',
                                      '--timezone', 'UTC', '--no-color'], timeout=20)

    def test_candidate_reports_and_refusals_repeat(self):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_OPENCODE_WAL_REUSE_CANDIDATE')
        if not selected:
            self.skipTest('Set OPENUSAGE_CCUSAGE_OPENCODE_WAL_REUSE_CANDIDATE for the scanner comparison.')
        binary = Path(selected).resolve(strict=True)
        self.assertEqual(sha256(binary), PIN['candidate']['executableSha256'])
        observations = self.collect(binary)
        self.assertEqual(observations, json.loads((FIXTURE / 'qualification.json').read_text()))

    def collect(self, binary):
        observations = {}
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-runtime-scan-') as temporary:
            root = Path(temporary)
            fixture = self.produce(root)
            shutil.copytree(fixture / 'completed', fixture / 'copied')
            selected_roots = {name: [fixture / name] for name in EXPECTED}
            selected_roots.update(copied_roots=[fixture / 'completed', fixture / 'copied'],
                                  replayed_roots=[fixture / 'completed', fixture / 'replayed'])
            before = file_hashes(fixture)
            for name, folders in selected_roots.items():
                observations[name] = {'requiredTokens': REQUIRED[name], 'reports': {}}
                for kind in ('daily', 'session'):
                    with self.subTest(case=name, report=kind), tempfile.TemporaryDirectory(dir=root) as empty:
                        first = self.report(binary, folders, kind, Path(empty))
                        second = self.report(binary, folders, kind, Path(empty))
                        self.assertEqual((first.returncode, first.stdout, first.stderr),
                                         (second.returncode, second.stdout, second.stderr))
                        self.assertEqual(list(Path(empty).iterdir()), [])
                        if first.returncode == 0:
                            report = json.loads(first.stdout)
                            observations[name]['reports'][kind] = {
                                'status': 'reported', 'totalTokens': report['totals']['totalTokens'],
                                'rowCount': len(report['daily' if kind == 'daily' else 'sessions']),
                                'totals': report['totals'],
                            }
                            self.assertEqual(report['totals']['totalTokens'], REQUIRED[name])
                            if name == 'reported_zero':
                                self.assertEqual(len(report['daily' if kind == 'daily' else 'sessions']), 1)
                        else:
                            self.assertEqual(first.returncode, 1)
                            self.assertEqual(first.stdout, b'')
                            self.assertEqual(first.stderr, b'Error: CliError("OpenCode source capture is unavailable")\n')
                            observations[name]['reports'][kind] = {'status': 'refused', 'exitCode': 1}
            self.assertEqual(file_hashes(fixture), before)
        for name, record in observations.items():
            record['qualification'] = ('usage-unavailable' if REQUIRED[name] is None else
                                       'exact' if all(report['status'] == 'reported'
                                                      for report in record['reports'].values()) else
                                       'known-total-refused')
        return observations


if __name__ == '__main__':
    unittest.main()
