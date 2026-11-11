"""Prepare exact registry archives and official source files without install scripts."""
import argparse
import base64
from concurrent.futures import ThreadPoolExecutor
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import urllib.request


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tree_digest(root):
    value = hashlib.sha256()
    for path in sorted(root.rglob('*')):
        if path.is_symlink():
            raise ValueError('A registry tree contains a link.')
        if path.is_file():
            value.update((path.relative_to(root).as_posix() + '\0' + sha256(path) + '\n').encode())
    return value.hexdigest()


def download(url, limit):
    with urllib.request.urlopen(url, timeout=30) as response:
        data = response.read(limit + 1)
    if len(data) > limit:
        raise ValueError('The download exceeds its limit.')
    return data


def official_source(source, commit, record, source_url, home):
    result = subprocess.run(['/usr/bin/git', '-C', str(source), 'cat-file', 'blob', commit + ':' + record['path']],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10,
                            env={'PATH': '/usr/bin', 'HOME': home, 'GIT_NO_LAZY_FETCH': '1',
                                 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null'})
    data = result.stdout if result.returncode == 0 else download(source_url + record['path'], 1024 * 1024)
    if len(data) > 1024 * 1024 or hashlib.sha256(data).hexdigest() != record['sha256']:
        raise ValueError('An official source object changed: ' + record['path'])
    return data


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--runtime-dependencies', type=Path, required=True)
    parser.add_argument('--destination', type=Path, required=True)
    args = parser.parse_args()
    fixture = Path(__file__).parent
    pin = json.loads((fixture / 'pin.json').read_text())
    old = json.loads((fixture.parent / 'opencode-runtime/pin.json').read_text())
    source = args.source.resolve(strict=True)
    runtime = args.runtime_dependencies.resolve(strict=True)
    destination = args.destination.absolute()
    if destination.exists() or destination.is_symlink():
        raise ValueError('The dependency destination must not exist.')
    if sha256(fixture.parent / 'opencode-runtime/pin.json') != pin['runtimePinSha256']:
        raise ValueError('The runtime pin changed.')
    for name, record in old['registryPackages'].items():
        if tree_digest(runtime / 'node_modules' / name) != record['extractedTreeSha256']:
            raise ValueError('An existing dependency changed: ' + name)
    source_url = 'https://raw.githubusercontent.com/anomalyco/opencode/' + pin['commit'] + '/'
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='openusage-fork-setup-home-') as home:
        for variable in ('HOME', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME'):
            os.environ[variable] = home
        for record in pin['sourceFiles']:
            path = source / record['path']
            if not path.exists() and record['path'] in pin['additionalSourceFiles']:
                data = download(source_url + record['path'], 1024 * 1024)
                if hashlib.sha256(data).hexdigest() != record['sha256']:
                    raise ValueError('An official source download changed.')
                path.parent.mkdir(parents=True, exist_ok=True)
                with path.open('xb') as target:
                    target.write(data)
            if path.is_symlink() or sha256(path) != record['sha256']:
                raise ValueError('An official source file changed: ' + record['path'])
        lock = (source / 'bun.lock').read_text()
        with tempfile.TemporaryDirectory(prefix='.opencode-fork-stage-', dir=destination.parent) as temporary:
            stage = Path(temporary)
            for name, record in pin['registryPackages'].items():
                prefix = '    ' + json.dumps(record['lockKey']) + ': '
                lines = [line for line in lock.splitlines() if line.startswith(prefix + '[')]
                if len(lines) != 1 or json.loads(lines[0][len(prefix):].rstrip(',')) != record['entry']:
                    raise ValueError('A registry pin differs from the official lock: ' + name)
                data = download(record['url'], 40 * 1024 * 1024)
                integrity = 'sha512-' + base64.b64encode(hashlib.sha512(data).digest()).decode()
                if integrity != record['entry'][3] or hashlib.sha256(data).hexdigest() != record['tarballSha256']:
                    raise ValueError('A registry archive changed: ' + name)
                package = stage / 'node_modules' / name
                package.mkdir(parents=True)
                with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
                    for member in archive.getmembers():
                        if member.name.rstrip('/') == 'package':
                            continue
                        if not member.name.startswith('package/') or not (member.isfile() or member.isdir()):
                            raise ValueError('A registry archive contains an unsupported member.')
                        member.name = member.name[len('package/'):]
                        if member.name:
                            archive.extract(member, package, filter='data')
                if tree_digest(package) != record['extractedTreeSha256']:
                    raise ValueError('An extracted registry tree changed: ' + name)
            print('Registry archives match all 11 pins.', flush=True)
            with ThreadPoolExecutor(max_workers=4) as pool:
                downloads = list(pool.map(lambda record: official_source(source, pin['commit'], record, source_url, home),
                                          pin['supplementalSource']))
            for record, data in zip(pin['supplementalSource'], downloads, strict=True):
                path = stage / 'source' / record['path']
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            for record in pin['resolutionPackages']:
                path = stage / record['path']
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(record['contents'])
            for record in pin['resolutionLinks']:
                path = stage / 'node_modules' / record['path']
                path.parent.mkdir(parents=True, exist_ok=True)
                path.symlink_to(record['target'])
            destination.mkdir()
            for child in stage.iterdir():
                child.rename(destination / child.name)
    print('Prepared 11 registry packages and 21 official utility files. No install scripts ran.')


if __name__ == '__main__':
    main()
