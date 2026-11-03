#!/usr/bin/env python3
"""Offline dependency policy: every locked package comes from its official registry with a pinned digest."""
import json
from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
CRATES_IO = 'registry+https://github.com/rust-lang/crates.io-index'
NPM = 'https://registry.npmjs.org/'
CARGO_LOCKS = ('Cargo.lock', 'tests/native/backup-verifier/Cargo.lock')


def local_crates(root):
    # Unregistered packages must be workspace members, patched sources or a lock's own crate.
    workspace = tomllib.loads((root / 'Cargo.toml').read_text())
    directories = [*workspace['workspace']['members'],
                   *(value['path'] for value in workspace.get('patch', {}).get('crates-io', {}).values()),
                   *(str(Path(lock).parent) for lock in CARGO_LOCKS)]
    manifests = (tomllib.loads((root / directory / 'Cargo.toml').read_text()) for directory in directories)
    return {manifest['package']['name'] for manifest in manifests if 'package' in manifest}


def cargo(root, lock, allowed):
    problems, count = [], 0
    for package in tomllib.loads((root / lock).read_text())['package']:
        count += 1
        name, source = f"{package['name']} {package['version']}", package.get('source')
        if source is None:
            if package['name'] not in allowed:
                problems.append(f'{lock}: {name} has no registry source and is not a local crate')
        elif source != CRATES_IO:
            problems.append(f'{lock}: {name} comes from {source}, not crates.io')
        elif not re.fullmatch(r'[0-9a-f]{64}', package.get('checksum', '')):
            problems.append(f'{lock}: {name} has no checksum')
    return problems, count


def npm(root):
    lock = json.loads((root / 'package-lock.json').read_text())
    problems = [] if lock.get('lockfileVersion') == 3 else ['package-lock.json: lockfileVersion must be 3']
    count = 0
    for key, package in lock.get('packages', {}).items():
        # Bundled packages are covered by their parent's integrity digest.
        if not key or package.get('link') or package.get('inBundle'):
            continue
        count += 1
        if not package.get('resolved', '').startswith(NPM):
            problems.append(f'package-lock.json: {key} does not resolve from registry.npmjs.org')
        if not package.get('integrity', '').startswith('sha512-'):
            problems.append(f'package-lock.json: {key} has no sha512 integrity')
    return problems, count


def check(root=ROOT):
    allowed, problems, counts = local_crates(root), [], {}
    for lock in CARGO_LOCKS:
        found, counts[lock] = cargo(root, lock, allowed)
        problems += found
    found, counts['package-lock.json'] = npm(root)
    return {'packages': counts, 'problems': problems + found}


if __name__ == '__main__':
    result = check(Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else ROOT)
    print(json.dumps(result, indent=2))
    sys.exit(1 if result['problems'] else 0)
