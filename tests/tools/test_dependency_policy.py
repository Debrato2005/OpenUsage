"""Behavioral checks for the offline dependency policy; all lockfiles are synthetic."""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).parents[2] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
SPEC = importlib.util.spec_from_file_location('dependency_policy', SCRIPTS / 'dependency-policy.py')
policy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(policy)
import storage  # noqa: E402

REGISTRY = 'source = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "' + 'a' * 64 + '"\n'


class DependencyPolicyTests(unittest.TestCase):
    def setUp(self):
        parent = storage.diagnostics(policy.ROOT, create=True).parent / 'tool-test-tmp'
        parent.mkdir(exist_ok=True, mode=0o700)
        temporary = tempfile.TemporaryDirectory(prefix='dependency-policy-', dir=parent)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.write('Cargo.toml', '[workspace]\nmembers = ["app"]\n\n[patch.crates-io]\nglib = { path = "vendor/glib" }\n')
        self.write('app/Cargo.toml', '[package]\nname = "app"\nversion = "0.1.0"\n')
        self.write('vendor/glib/Cargo.toml', '[package]\nname = "glib"\nversion = "0.18.5"\n')
        self.write('tests/native/backup-verifier/Cargo.toml', '[package]\nname = "verifier"\nversion = "0.1.0"\n')
        self.cargo('Cargo.lock', '[[package]]\nname = "app"\nversion = "0.1.0"\n\n[[package]]\nname = "glib"\nversion = "0.18.5"\n\n[[package]]\nname = "serde"\nversion = "1.0.0"\n' + REGISTRY)
        self.cargo('tests/native/backup-verifier/Cargo.lock', '[[package]]\nname = "verifier"\nversion = "0.1.0"\n')
        self.bun({'react': ['react@19.0.0', '', {}, 'sha512-AAAA'],
                  '@scope/pkg': ['@scope/pkg@1.0.0', 'https://registry.npmjs.org/@scope/pkg/-/pkg-1.0.0.tgz',
                                 {'dependencies': {'react': '^19'}}, 'sha512-BBBB']})

    def write(self, path, text):
        (self.root / path).parent.mkdir(parents=True, exist_ok=True)
        (self.root / path).write_text(text)

    def cargo(self, path, packages):
        self.write(path, 'version = 4\n\n' + packages)

    def bun(self, packages, version=1):
        # Bun's text lockfile ends every member with a comma.
        entries = ''.join(f'    {json.dumps(key)}: {json.dumps(entry)},\n' for key, entry in packages.items())
        self.write('bun.lock', f'{{\n  "lockfileVersion": {version},\n  "workspaces": {{\n    "": {{\n      "name": "synthetic",\n    }},\n  }},\n'
                               f'  "packages": {{\n{entries}  }},\n}}\n')

    def test_registry_packages_and_local_crates_pass(self):
        self.assertEqual(policy.check(self.root), {'packages': {'Cargo.lock': 3, 'tests/native/backup-verifier/Cargo.lock': 1, 'bun.lock': 2}, 'problems': []})

    def test_trailing_commas_are_removed_only_outside_strings(self):
        self.assertEqual(policy.without_trailing_commas('{"a": "x,]", "b": [1, 2 ,\n],\t}'), '{"a": "x,]", "b": [1, 2 \n]\t}')

    def test_git_sources_missing_checksums_and_unknown_local_crates_fail(self):
        self.cargo('Cargo.lock', '[[package]]\nname = "forked"\nversion = "1.0.0"\nsource = "git+https://example.invalid/forked"\n\n'
                   '[[package]]\nname = "unpinned"\nversion = "1.0.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n\n'
                   '[[package]]\nname = "stray"\nversion = "1.0.0"\n')
        self.assertEqual(policy.check(self.root)['problems'], [
            'Cargo.lock: forked 1.0.0 comes from git+https://example.invalid/forked, not crates.io',
            'Cargo.lock: unpinned 1.0.0 has no checksum',
            'Cargo.lock: stray 1.0.0 has no registry source and is not a local crate'])

    def test_npm_packages_need_the_official_registry_and_sha512(self):
        self.bun({'mirror': ['mirror@1.0.0', 'https://mirror.example.invalid/mirror-1.0.0.tgz', {}, 'sha512-AAAA'],
                  'weak': ['weak@1.0.0', '', {}, 'sha1-AAAA'],
                  'forked': ['forked@github:example/forked#0123abc', {}, 'example-forked-0123abc'],
                  'local': ['local@file:../local', {}]})
        self.assertEqual(policy.check(self.root)['problems'], [
            'bun.lock: mirror does not resolve from registry.npmjs.org',
            'bun.lock: weak has no sha512 integrity',
            'bun.lock: forked does not resolve from registry.npmjs.org',
            'bun.lock: forked has no sha512 integrity',
            'bun.lock: local does not resolve from registry.npmjs.org',
            'bun.lock: local has no sha512 integrity'])

    def test_registry_redirection_and_non_registry_specs_fail(self):
        self.write('package.json', json.dumps({'dependencies': {'react': '19.0.0', 'forked': 'github:example/forked'},
                                               'overrides': {'nested': 'https://example.invalid/nested.tgz'}}))
        self.write('bunfig.toml', '[install]\nexact = true\nregistry = "https://mirror.example.invalid/"\n')
        self.write('.npmrc', '@scope:registry=https://mirror.example.invalid/\n')
        self.assertEqual(policy.check(self.root)['problems'], [
            'bunfig.toml: install.registry redirects package resolution',
            '.npmrc: registry override',
            'package.json: dependencies.forked is not a registry version',
            'package.json: overrides.nested is not a registry version'])

    def test_exact_registry_versions_and_exact_saves_pass(self):
        self.write('package.json', json.dumps({'dependencies': {'react': '19.0.0'}, 'trustedDependencies': []}))
        self.write('bunfig.toml', '[install]\nexact = true\n')
        self.assertEqual(policy.check(self.root)['problems'], [])

    def test_a_second_lockfile_or_unknown_bun_lock_version_fails(self):
        self.bun({'react': ['react@19.0.0', '', {}, 'sha512-AAAA']}, version=2)
        self.write('package-lock.json', '{}')
        self.assertEqual(policy.check(self.root)['problems'], [
            'bun.lock: lockfileVersion must be 1',
            'package-lock.json: only bun.lock may lock npm packages'])


if __name__ == '__main__':
    unittest.main()
