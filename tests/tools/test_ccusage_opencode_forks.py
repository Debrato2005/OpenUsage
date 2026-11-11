"""Run the pinned legacy fork service with private synthetic data on Linux."""
import json
import os
from pathlib import Path
import shutil
import sqlite3
import sys
import tempfile
import unittest

from test_ccusage_opencode_runtime import checked_run, sandbox, sha256, tree_digest, file_hashes


ROOT = Path(__file__).resolve().parents[2]
FIXTURE = Path(__file__).parent / 'fixtures/opencode-forks'
PIN = json.loads((FIXTURE / 'pin.json').read_text())
BASE_FIXTURE = FIXTURE.parent / 'opencode-runtime'
BASE_PIN = json.loads((BASE_FIXTURE / 'pin.json').read_text())
REQUIRED = dict.fromkeys(('parent_only', 'full_fork', 'bounded_fork', 'before_usage',
                          'replayed', 'copied_roots', 'replayed_roots', 'reversed_roots',
                          'parent_and_fork'), 173)
REQUIRED['local_usage'] = 190
REQUIRED['distinct_calls'] = 346


def canonical(result):
    output = {}
    for name, record in result.items():
        aliases = {}
        for kind, plural in (('s', 'sessions'), ('m', 'messages'), ('p', 'parts')):
            aliases.update({row['id']: f'{kind}{index + 1}' for index, row in enumerate(record[plural])})

        def replace(value):
            if isinstance(value, dict):
                return {key: replace(child) for key, child in value.items()}
            if isinstance(value, list):
                return [replace(child) for child in value]
            return aliases.get(value, value) if isinstance(value, str) else value
        output[name] = replace(record)
    return output


class OpenCodeForkFixturePins(unittest.TestCase):
    def test_fixture_files_match(self):
        self.assertTrue(PIN['fixtureFiles'])
        self.assertEqual(sha256(BASE_FIXTURE / 'pin.json'), PIN['runtimePinSha256'])
        for record in PIN.get('fixtureFiles', []):
            self.assertEqual(sha256(FIXTURE / record['path']), record['sha256'], record['path'])


class OpenCodeForkTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        names = ('OPENUSAGE_OPENCODE_RUNTIME_SOURCE', 'OPENUSAGE_OPENCODE_RUNTIME_DEPENDENCIES',
                 'OPENUSAGE_OPENCODE_RUNTIME_BUN', 'OPENUSAGE_OPENCODE_FORKS_DEPENDENCIES')
        if not all(os.environ.get(name) for name in names):
            raise unittest.SkipTest('Set the three runtime paths and OPENUSAGE_OPENCODE_FORKS_DEPENDENCIES.')
        if sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            raise unittest.SkipTest('This fixture requires Linux and Bubblewrap.')
        cls.source, cls.dependencies, cls.bun, cls.extra = [Path(os.environ[name]).resolve(strict=True) for name in names]
        if not (cls.source / 'node_modules').is_symlink() or os.readlink(cls.source / 'node_modules') != '../opencode-runtime-deps/node_modules':
            raise ValueError('The original dependency link changed.')
        for record in PIN['sourceFiles']:
            if sha256(cls.source / record['path']) != record['sha256']:
                raise ValueError('An official source file changed: ' + record['path'])
        for record in PIN['supplementalSource']:
            if sha256(cls.extra / 'source' / record['path']) != record['sha256']:
                raise ValueError('A supplemental source file changed: ' + record['path'])
        for base, records in ((cls.dependencies, BASE_PIN['registryPackages']), (cls.extra, PIN['registryPackages'])):
            for name, record in records.items():
                if tree_digest(base / 'node_modules' / name) != record['extractedTreeSha256']:
                    raise ValueError('A registry tree changed: ' + name)
        for record in PIN['resolutionLinks']:
            link = cls.extra / 'node_modules' / record['path']
            if not link.is_symlink() or os.readlink(link) != record['target']:
                raise ValueError('A resolution link changed: ' + record['path'])
        for record in PIN['resolutionPackages']:
            if (cls.extra / record['path']).read_text() != record['contents']:
                raise ValueError('A resolution manifest changed: ' + record['path'])
        packages = set()
        for path in (cls.extra / 'node_modules').iterdir():
            packages.update(path.name + '/' + child.name for child in path.iterdir()) if path.name.startswith('@') else packages.add(path.name)
        if packages != set(PIN['registryPackages']) | {record['path'] for record in PIN['resolutionLinks']}:
            raise ValueError('The fork dependencies contain an unpinned package.')
        if sha256(cls.bun) != BASE_PIN['bun']['executableSha256']:
            raise ValueError('The Bun executable changed.')

    def produce(self, root):
        fixture = root / 'fixture'
        fixture.mkdir()
        for name in ('home', 'data', 'config', 'cache', 'state', 'project'):
            (fixture / name).mkdir()
        canary = root / 'outside-canary.txt'
        canary.write_text('Synthetic outside fixture.\n')
        command = sandbox() + [
            '--ro-bind', str(self.source), '/upstream/opencode-v1.19.5',
            '--ro-bind', str(self.dependencies / 'node_modules'), '/base-deps/node_modules',
            '--ro-bind', str(self.extra / 'node_modules'), '/upstream/opencode-runtime-deps/node_modules',
            '--ro-bind', str(FIXTURE), '/upstream/opencode-runtime-deps/fixture',
            '--ro-bind', str(self.extra), '/fork-deps',
            '--ro-bind', str(self.bun), '/runtime/bun', '--bind', str(fixture), '/fixture',
            '--setenv', 'HOME', '/fixture/home', '--setenv', 'OPENCODE_DISABLE_CHANNEL_DB', '1',
            '--setenv', 'OPENCODE_FIXTURE_CANARY_PATH', str(canary),
            '--setenv', 'OPENCODE_FIXTURE_PARENT_NETWORK', os.readlink('/proc/self/ns/net'),
        ]
        for variable, name in [('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config'),
                               ('XDG_CACHE_HOME', 'cache'), ('XDG_STATE_HOME', 'state')]:
            command.extend(['--setenv', variable, '/fixture/' + name])
        result = checked_run(command + ['--chdir', '/fixture', '/runtime/bun', '--no-install',
                                        '/upstream/opencode-runtime-deps/fixture/bootstrap.ts'], timeout=45)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace')[:6000])
        self.assertEqual(json.loads((fixture / 'loaded-source.json').read_text()), PIN['executedSourceFiles'])
        self.assertEqual(json.loads((fixture / 'adapters.json').read_text()), PIN['adapters'])
        self.assertEqual(json.loads((fixture / 'isolation.json').read_text()),
                         {'canaryHidden': True, 'privateNetwork': True, 'privateHome': True})
        self.assertEqual({record['path']: sha256(self.source / record['path']) for record in PIN['sourceFiles']},
                         {record['path']: record['sha256'] for record in PIN['sourceFiles']})
        return fixture

    def test_real_fork_service(self):
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-forks-') as temporary:
            fixture = self.produce(Path(temporary))
            result = json.loads((fixture / 'producer-result.json').read_text())
            self.assertEqual(canonical(result), json.loads((FIXTURE / 'producer-expected.json').read_text()))
            for name, record in result.items():
                with self.subTest(case=name):
                    self.assertEqual(record['v2MessageCount'], 0)
                    self.assertEqual([sum(row['tokens']) for row in record['sessions']],
                                     [173, {'before_usage': 0, 'local_usage': 190}.get(name, 173)])
                    self.assertTrue(all(row['parentID'] is None and row['metadata'] is None
                                        for row in record['sessions']))
                    if name not in ('replayed', 'distinct_calls'):
                        operation = record['operation']
                        parent_ids = {row['id'] for row in record['messages']
                                      if row['sessionID'] == operation['parentID']}
                        fork_ids = {row['id'] for row in record['messages']
                                    if row['sessionID'] == operation['forkID']}
                        self.assertFalse(parent_ids & fork_ids)
                        parent_parts = {row['id'] for row in record['parts'] if row['sessionID'] == operation['parentID']}
                        fork_parts = {row['id'] for row in record['parts'] if row['sessionID'] == operation['forkID']}
                        self.assertFalse(parent_parts & fork_parts)
                        self.assertEqual(len(operation['inheritedMessageIDs']),
                                         {'bounded_fork': 2, 'before_usage': 1}.get(name, 3))
                        for row in record['messages']:
                            if row['sessionID'] == operation['forkID'] and row['role'] == 'assistant':
                                self.assertIn(row['parentID'], fork_ids)
                    connection = sqlite3.connect((fixture / name / 'opencode.db').as_uri() + '?mode=ro&immutable=1', uri=True)
                    try:
                        self.assertEqual(connection.execute('PRAGMA integrity_check').fetchall(), [('ok',)])
                        self.assertEqual(connection.execute('SELECT count(*) FROM message').fetchone()[0], len(record['messages']))
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
        command.extend(['--setenv', 'OPENCODE_DATA_DIR', ','.join(selected), '--chdir', '/home/fixture', '/runtime/ccusage'])
        command.extend(['daily', '--by-agent'] if kind == 'daily' else ['opencode', 'session'])
        return checked_run(command + ['--json', '--offline', '--mode', 'calculate', '--timezone', 'UTC', '--no-color'], timeout=20)

    def collect(self, binary):
        observations = {}
        with tempfile.TemporaryDirectory(prefix='openusage-opencode-forks-scan-') as temporary:
            root = Path(temporary)
            fixture = self.produce(root)
            shutil.copytree(fixture / 'full_fork', fixture / 'copied')
            selected = {name: [fixture / name] for name in
                        ('parent_only', 'full_fork', 'bounded_fork', 'before_usage', 'local_usage', 'distinct_calls', 'replayed')}
            selected.update(copied_roots=[fixture / 'full_fork', fixture / 'copied'],
                            replayed_roots=[fixture / 'full_fork', fixture / 'replayed'],
                            reversed_roots=[fixture / 'replayed', fixture / 'full_fork'],
                            parent_and_fork=[fixture / 'parent_only', fixture / 'full_fork'])
            self.assertTrue(all({path.name for path in folder.iterdir()} <= {'opencode.db', 'opencode.db-wal', 'opencode.db-shm'}
                                for folders in selected.values() for folder in folders))
            before = file_hashes(fixture)
            for name, folders in selected.items():
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
                            observations[name]['reports'][kind] = {'status': 'reported',
                                'totalTokens': report['totals']['totalTokens'], 'totals': report['totals'],
                                'rowCount': len(report['daily' if kind == 'daily' else 'sessions'])}
                        else:
                            self.assertEqual(first.returncode, 1)
                            self.assertEqual(first.stdout, b'')
                            self.assertEqual(first.stderr, b'Error: CliError("OpenCode source capture is unavailable")\n')
                            observations[name]['reports'][kind] = {'status': 'refused', 'exitCode': 1}
                reports = observations[name]['reports'].values()
                observations[name]['qualification'] = ('exact' if all(
                    report.get('totalTokens') == REQUIRED[name] for report in reports) else 'qualification-failure')
            self.assertEqual(file_hashes(fixture), before)
        self.assertEqual(observations['replayed_roots']['reports'], observations['reversed_roots']['reports'])
        return observations

    def test_candidate_fork_outcomes_repeat(self):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_OPENCODE_RECORDS_CANDIDATE')
        if not selected:
            self.skipTest('Set OPENUSAGE_CCUSAGE_OPENCODE_RECORDS_CANDIDATE for the scanner comparison.')
        binary = Path(selected).resolve(strict=True)
        self.assertEqual(sha256(binary), PIN['candidate']['executableSha256'])
        version = checked_run(sandbox() + ['--ro-bind', str(binary), '/runtime/ccusage', '/runtime/ccusage', '--version'])
        self.assertEqual(version.returncode, 0)
        self.assertEqual(version.stdout.decode().strip(), 'ccusage ' + PIN['candidate']['version'])
        self.assertEqual(self.collect(binary), json.loads((FIXTURE / 'qualification.json').read_text()))


if __name__ == '__main__':
    unittest.main()
