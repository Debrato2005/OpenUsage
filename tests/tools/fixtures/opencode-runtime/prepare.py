"""Prepare the optional producer dependencies without package install scripts."""
import argparse
import base64
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import urllib.request


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tree_digest(root):
    value = hashlib.sha256()
    for path in sorted(root.rglob('*')):
        if path.is_symlink():
            raise ValueError('A registry package contains a symlink.')
        if path.is_file():
            value.update((path.relative_to(root).as_posix() + '\0' + digest(path) + '\n').encode())
    return value.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--destination', type=Path, required=True)
    args = parser.parse_args()
    source = args.source.resolve(strict=True)
    destination = args.destination.absolute()
    pin = json.loads((Path(__file__).parent / 'pin.json').read_text())
    if destination.exists():
        raise ValueError('The dependency destination must not exist.')
    for record in pin['sourceFiles']:
        name, expected = record['path'], record['sha256']
        if digest(source / name) != expected:
            raise ValueError('The producer source does not match its pin: ' + name)
    link = source / 'node_modules'
    if link.exists() or link.is_symlink():
        if not link.is_symlink() or os.readlink(link) != '../opencode-runtime-deps/node_modules':
            raise ValueError('The producer dependency link already has another target.')
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='opencode-setup-home-') as private:
        for variable in ('HOME', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME'):
            os.environ[variable] = private
        with tempfile.TemporaryDirectory(prefix='.opencode-runtime-setup-', dir=destination.parent) as staged:
            stage = Path(staged)
            for name, record in sorted(pin['registryPackages'].items()):
                with urllib.request.urlopen(record['url'], timeout=30) as response:
                    archive_bytes = response.read(40 * 1024 * 1024 + 1)
                if len(archive_bytes) > 40 * 1024 * 1024:
                    raise ValueError('A dependency archive exceeds its limit.')
                integrity = 'sha512-' + base64.b64encode(hashlib.sha512(archive_bytes).digest()).decode()
                if integrity != record['entry'][3] or hashlib.sha256(archive_bytes).hexdigest() != record['tarballSha256']:
                    raise ValueError('A dependency archive does not match its pin: ' + name)
                package = stage / 'node_modules' / name
                package.mkdir(parents=True)
                with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode='r:gz') as archive:
                    for member in archive.getmembers():
                        if member.name.rstrip('/') == 'package':
                            continue
                        if not member.name.startswith('package/') or not (member.isfile() or member.isdir()):
                            raise ValueError('A dependency archive contains an unsupported member.')
                        member.name = member.name[len('package/'):]
                        if member.name:
                            archive.extract(member, package, filter='data')
                print(record['entry'][0], flush=True)
            subprocess.run(['/usr/bin/patch', '--batch', '--forward', '-p1', '-i',
                            str(source / 'patches/effect@4.0.0-beta.83.patch')],
                           cwd=stage / 'node_modules/effect', check=True, timeout=10,
                           env={'PATH': '/usr/bin', 'HOME': private})
            for name, record in pin['registryPackages'].items():
                if tree_digest(stage / 'node_modules' / name) != record['extractedTreeSha256']:
                    raise ValueError('The extracted dependency does not match its pin: ' + name)
            for name, target in pin['workspacePackages'].items():
                workspace = stage / 'node_modules' / name
                workspace.parent.mkdir(parents=True, exist_ok=True)
                workspace.symlink_to(target)
            destination.mkdir()
            (stage / 'node_modules').rename(destination / 'node_modules')
    if not link.is_symlink():
        link.symlink_to('../opencode-runtime-deps/node_modules')
    print('Prepared 15 packages. No dependency scripts ran.')


if __name__ == '__main__':
    main()
