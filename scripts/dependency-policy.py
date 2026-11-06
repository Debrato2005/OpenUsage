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
BUN_LOCK = 'bun.lock'
OTHER_LOCKS = ('package-lock.json', 'npm-shrinkwrap.json', 'yarn.lock', 'pnpm-lock.yaml', 'bun.lockb')


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


def without_trailing_commas(text):
    # bun.lock is JSON plus trailing commas; drop only commas that close an object or array, never inside strings.
    out, index, in_string = [], 0, False
    while index < len(text):
        char = text[index]
        if in_string:
            if char == '\\':
                out.append(text[index:index + 2])
                index += 2
                continue
            in_string = char != '"'
        elif char == '"':
            in_string = True
        elif char == ',':
            following = index + 1
            while following < len(text) and text[following] in ' \t\r\n':
                following += 1
            if following < len(text) and text[following] in '}]':
                index += 1
                continue
        out.append(char)
        index += 1
    return ''.join(out)


def bun(root):
    lock = json.loads(without_trailing_commas((root / BUN_LOCK).read_text()))
    problems = [] if lock.get('lockfileVersion') == 1 else [f'{BUN_LOCK}: lockfileVersion must be 1']
    problems += [f'{name}: only {BUN_LOCK} may lock npm packages' for name in OTHER_LOCKS if (root / name).exists()]
    count = 0
    for key, entry in lock.get('packages', {}).items():
        count += 1
        # A registry package is [name@version, registry, metadata, integrity]. An empty registry is the default,
        # registry.npmjs.org; Bun records the full tarball URL for any other registry. Git, tarball, file,
        # link and workspace entries have other shapes or a protocol in their version.
        registry_shape = (isinstance(entry, list) and len(entry) == 4 and isinstance(entry[0], str)
                          and isinstance(entry[1], str) and isinstance(entry[2], dict) and isinstance(entry[3], str)
                          and '@' in entry[0][1:] and ':' not in entry[0][1:].split('@', 1)[1])
        if not registry_shape or (entry[1] and not entry[1].startswith(NPM)):
            problems.append(f'{BUN_LOCK}: {key} does not resolve from registry.npmjs.org')
        if not registry_shape or not entry[3].startswith('sha512-'):
            problems.append(f'{BUN_LOCK}: {key} has no sha512 integrity')
    return problems, count


def install_config(root):
    # Registry or scope redirection and non-registry specs would bypass the official-registry guarantee.
    problems = []
    bunfig = root / 'bunfig.toml'
    if bunfig.exists():
        install = tomllib.loads(bunfig.read_text()).get('install', {})
        problems += [f'bunfig.toml: install.{key} redirects package resolution' for key in ('registry', 'scopes') if key in install]
    npmrc = root / '.npmrc'
    if npmrc.exists() and re.search(r'^\s*(?:@[^:\s]+:)?registry\s*=', npmrc.read_text(), re.MULTILINE):
        problems.append('.npmrc: registry override')
    manifest = root / 'package.json'
    if manifest.exists():
        package = json.loads(manifest.read_text())
        for field in ('dependencies', 'devDependencies', 'optionalDependencies', 'peerDependencies', 'overrides', 'resolutions'):
            for name, spec in (package.get(field) or {}).items():
                if not isinstance(spec, str) or ':' in spec or '/' in spec:
                    problems.append(f'package.json: {field}.{name} is not a registry version')
    return problems


def check(root=ROOT):
    allowed, problems, counts = local_crates(root), [], {}
    for lock in CARGO_LOCKS:
        found, counts[lock] = cargo(root, lock, allowed)
        problems += found
    found, counts[BUN_LOCK] = bun(root)
    return {'packages': counts, 'problems': problems + found + install_config(root)}


if __name__ == '__main__':
    result = check(Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else ROOT)
    print(json.dumps(result, indent=2))
    sys.exit(1 if result['problems'] else 0)
