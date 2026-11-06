#!/usr/bin/env python3
"""Prepare the pinned official ccusage binary for packaging, without running it (OU-LOG-005)."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import time
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--offline', action='store_true')
parser.add_argument('--cache-dir', type=Path, default=ROOT / '.tools')
args = parser.parse_args()
host = subprocess.check_output(['rustc', '-vV'], text=True, timeout=5)
target = next(line.removeprefix('host: ') for line in host.splitlines() if line.startswith('host: '))
if target != 'x86_64-unknown-linux-gnu':
    raise SystemExit('This ccusage pin covers only Linux x86_64 so far. No substitute binary was selected.')
manifest = json.loads((ROOT / 'crates/openusage-locallogs/sources/ccusage-20.0.48-linux-x86_64.json').read_text())
source = urllib.parse.urlsplit(manifest['sourceUrl'])
# Defence in depth for an edited manifest: download only the official npm package over HTTPS.
if source.scheme != 'https' or source.hostname != 'registry.npmjs.org' or not source.path.startswith('/@ccusage/ccusage-'):
    raise SystemExit('The ccusage source must be an official https://registry.npmjs.org/@ccusage/ package.')
cache = args.cache_dir.resolve()
cache.mkdir(exist_ok=True)
archive = cache / f"ccusage-{manifest['version']}-{manifest['target']}.tgz"

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

if not archive.exists():
    if args.offline:
        raise SystemExit('The pinned ccusage archive is not cached. Run this build preparation once with network access.')
    descriptor, temporary = tempfile.mkstemp(prefix='ccusage-', dir=cache)
    try:
        started, count = time.monotonic(), 0
        with os.fdopen(descriptor, 'wb') as output, urllib.request.urlopen(manifest['sourceUrl'], timeout=15) as response:
            while chunk := response.read(1024 * 1024):
                count += len(chunk)
                if count > manifest['archive']['bytes'] or time.monotonic() - started > 90:
                    raise ValueError('The archive exceeded its pinned size or download deadline.')
                output.write(chunk)
            output.flush()
            os.fsync(output.fileno())
        if count != manifest['archive']['bytes'] or digest(Path(temporary)) != manifest['archive']['sha256']:
            raise ValueError('The downloaded archive does not match the reviewed pin.')
        os.replace(temporary, archive)
    finally:
        Path(temporary).unlink(missing_ok=True)
if archive.is_symlink() or archive.stat().st_size != manifest['archive']['bytes'] or digest(archive) != manifest['archive']['sha256']:
    raise SystemExit('The cached ccusage archive does not match the reviewed pin.')

directory = ROOT / 'apps/desktop/src-tauri/binaries'
if directory.is_symlink():
    raise SystemExit('The build binaries directory must not be a symbolic link.')
directory.mkdir(exist_ok=True)
destination = directory / f'openusage-ccusage-{target}'
descriptor, temporary = tempfile.mkstemp(prefix='ccusage-', dir=directory)
try:
    with os.fdopen(descriptor, 'wb') as output, tarfile.open(archive, 'r:gz') as source:
        members = {member.name: member for member in source.getmembers()}
        if set(members) != {manifest['executableMember'], 'package/package.json'}:
            raise ValueError('The archive does not contain exactly the reviewed members.')
        member = members[manifest['executableMember']]
        if not member.isfile() or member.size != manifest['executableBytes']:
            raise ValueError('The executable member is not the reviewed file.')
        with source.extractfile(member) as executable:
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
print(f'Prepared official ccusage {manifest["version"]} for {target}. No executable was launched.')
