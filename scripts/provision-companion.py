#!/usr/bin/env python3
"""Prepare the pinned official companion for Tauri packaging, without running it."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--offline', action='store_true')
parser.add_argument('--cache-dir', type=Path, default=ROOT / '.tools')
args = parser.parse_args()
host = subprocess.check_output(['rustc', '-vV'], text=True, timeout=5)
target = next(line.removeprefix('host: ') for line in host.splitlines() if line.startswith('host: '))
if target != 'x86_64-unknown-linux-gnu':
    raise SystemExit('This companion pin is qualified only for Linux x86_64. No substitute binary was selected.')
manifest = json.loads((ROOT / 'crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json').read_text())
cache = args.cache_dir.resolve()
cache.mkdir(exist_ok=True)
archive = cache / f"codex-{manifest['version']}-{manifest['target']}.tar.gz"

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

if not archive.exists():
    if args.offline:
        raise SystemExit('The pinned official archive is not cached. Run this build preparation once with network access.')
    descriptor, temporary = tempfile.mkstemp(prefix='companion-', dir=cache)
    try:
        started, count = time.monotonic(), 0
        with os.fdopen(descriptor, 'wb') as output, urllib.request.urlopen(manifest['sourceUrl'], timeout=15) as response:
            while chunk := response.read(1024 * 1024):
                count += len(chunk)
                if count > manifest['archive']['bytes'] or time.monotonic() - started > 90:
                    raise ValueError('The official archive exceeded its pinned size or download deadline.')
                output.write(chunk)
            output.flush()
            os.fsync(output.fileno())
        if count != manifest['archive']['bytes'] or digest(Path(temporary)) != manifest['archive']['sha256']:
            raise ValueError('The downloaded official archive does not match the reviewed pin.')
        os.replace(temporary, archive)
    finally:
        Path(temporary).unlink(missing_ok=True)
if archive.is_symlink() or archive.stat().st_size != manifest['archive']['bytes'] or digest(archive) != manifest['archive']['sha256']:
    raise SystemExit('The cached official archive does not match the reviewed pin.')

directory = ROOT / 'apps/desktop/src-tauri/binaries'
if directory.is_symlink():
    raise SystemExit('The companion build directory must not be a symbolic link.')
directory.mkdir(exist_ok=True)
destination = directory / f'openusage-codex-{target}'
descriptor, temporary = tempfile.mkstemp(prefix='companion-', dir=directory)
try:
    with os.fdopen(descriptor, 'wb') as output, tarfile.open(archive, 'r:gz') as source:
        members = source.getmembers()
        if len(members) != 1 or members[0].name != f"codex-{manifest['target']}" or not members[0].isfile() or members[0].size != manifest['executableBytes']:
            raise ValueError('The archive does not contain the exact reviewed executable member.')
        with source.extractfile(members[0]) as executable:
            count = 0
            while chunk := executable.read(1024 * 1024):
                count += len(chunk)
                if count > manifest['executableBytes']:
                    raise ValueError('The executable exceeds its pinned size.')
                output.write(chunk)
        output.flush()
        os.fsync(output.fileno())
    if count != manifest['executableBytes'] or digest(Path(temporary)) != manifest['executableSha256']:
        raise ValueError('The extracted executable does not match the reviewed pin.')
    os.chmod(temporary, 0o555)
    os.replace(temporary, destination)
finally:
    Path(temporary).unlink(missing_ok=True)
print(f'Prepared official Codex {manifest["version"]} for {target}. No executable was launched.')
