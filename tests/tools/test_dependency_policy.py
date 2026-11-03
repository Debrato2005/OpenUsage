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
        self.npm({'node_modules/react': {'resolved': 'https://registry.npmjs.org/react/-/react-19.0.0.tgz', 'integrity': 'sha512-AAAA'},
                  'node_modules/parent/node_modules/bundled': {'inBundle': True}})

    def write(self, path, text):
        (self.root / path).parent.mkdir(parents=True, exist_ok=True)
        (self.root / path).write_text(text)

    def cargo(self, path, packages):
        self.write(path, 'version = 4\n\n' + packages)

    def npm(self, packages):
        self.write('package-lock.json', json.dumps({'lockfileVersion': 3, 'packages': {'': {'name': 'synthetic'}, **packages}}))

    def test_registry_packages_local_crates_and_bundles_pass(self):
        self.assertEqual(policy.check(self.root), {'packages': {'Cargo.lock': 3, 'tests/native/backup-verifier/Cargo.lock': 1, 'package-lock.json': 1}, 'problems': []})

    def test_git_sources_missing_checksums_and_unknown_local_crates_fail(self):
        self.cargo('Cargo.lock', '[[package]]\nname = "forked"\nversion = "1.0.0"\nsource = "git+https://example.invalid/forked"\n\n'
                   '[[package]]\nname = "unpinned"\nversion = "1.0.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n\n'
                   '[[package]]\nname = "stray"\nversion = "1.0.0"\n')
        self.assertEqual(policy.check(self.root)['problems'], [
            'Cargo.lock: forked 1.0.0 comes from git+https://example.invalid/forked, not crates.io',
            'Cargo.lock: unpinned 1.0.0 has no checksum',
            'Cargo.lock: stray 1.0.0 has no registry source and is not a local crate'])

    def test_npm_packages_need_the_official_registry_and_sha512(self):
        self.npm({'node_modules/mirror': {'resolved': 'https://mirror.example.invalid/mirror.tgz', 'integrity': 'sha512-AAAA'},
                  'node_modules/weak': {'resolved': 'https://registry.npmjs.org/weak/-/weak-1.0.0.tgz', 'integrity': 'sha1-AAAA'}})
        self.assertEqual(policy.check(self.root)['problems'], [
            'package-lock.json: node_modules/mirror does not resolve from registry.npmjs.org',
            'package-lock.json: node_modules/weak has no sha512 integrity'])


if __name__ == '__main__':
    unittest.main()
