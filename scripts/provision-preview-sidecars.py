#!/usr/bin/env python3
"""Stage the reviewed preview sidecar files for a build."""

import argparse
import hashlib
import json
from pathlib import Path
import tarfile
import tempfile
import time
from urllib.parse import urlsplit
from urllib.request import urlopen


ROOT = Path(__file__).resolve().parents[1]
SOURCES = {
    'codex': ('github.com', '/openai/codex/releases/download/', 'sha256'),
    'ccusage': ('registry.npmjs.org', '/@ccusage/ccusage-', 'sha512'),
}
TARGETS = ('aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-pc-windows-msvc')
CHUNK_BYTES = 1024 * 1024


def copy_checked(source, destination, size, algorithm, expected, label):
    digest = hashlib.new(algorithm)
    count = 0
    deadline = time.monotonic() + 120
    with destination.open('wb') as output:
        while chunk := source.read(CHUNK_BYTES):
            count += len(chunk)
            if count > size or time.monotonic() > deadline:
                raise ValueError(f'{label} exceeds its expected size or read deadline.')
            digest.update(chunk)
            output.write(chunk)
    if count != size or digest.hexdigest() != expected:
        raise ValueError(f'{label} does not match its pin.')


def stage_sidecar(tool, pin, archive_dir, archive, destination):
    host, path_prefix, algorithm = SOURCES[tool]
    url = urlsplit(pin['sourceUrl'])
    if (url.scheme != 'https' or url.netloc != host or not url.path.startswith(path_prefix)
            or url.query or url.fragment):
        raise ValueError(f'{tool} requires its official HTTPS source.')
    archive_name = url.path.rsplit('/', 1)[-1]
    if not archive_name.endswith(('.tgz', '.tar.gz')):
        raise ValueError(f'{tool} requires a gzip archive.')
    archive_algorithm, _, archive_digest = pin['archiveDigest'].partition('-')
    if archive_algorithm != algorithm:
        raise ValueError(f'{tool} requires an {algorithm} archive pin.')
    source = ((archive_dir / archive_name).open('rb') if archive_dir is not None
              else urlopen(pin['sourceUrl'], timeout=15))
    with source:
        copy_checked(source, archive, pin['archiveBytes'], algorithm, archive_digest, f'{tool} archive')

    with tarfile.open(archive, 'r:gz') as package:
        matches = [member for member in package if member.name == pin['executableMember']]
        if len(matches) != 1 or not matches[0].isfile() or matches[0].size != pin['executableBytes']:
            raise ValueError(f'{tool} archive must contain one exact executable member.')
        with package.extractfile(matches[0]) as executable:
            copy_checked(executable, destination, pin['executableBytes'], 'sha256',
                         pin['executableSha256'], f'{tool} executable')
    destination.chmod(0o755)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', choices=TARGETS, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--archive-dir', type=Path, help='Read archives from this directory.')
    args = parser.parse_args()

    try:
        if args.out.exists() or args.out.is_symlink():
            raise ValueError('The output directory must be new.')
        pins = json.loads((ROOT / 'packaging/preview-sidecars.json').read_text(encoding='utf-8'))[args.target]
        suffix = '.exe' if args.target.endswith('-windows-msvc') else ''
        with tempfile.TemporaryDirectory(prefix='openusage-sidecars-', dir=args.out.absolute().parent) as temporary:
            work = Path(temporary)
            ready = work / 'sidecars'
            ready.mkdir()
            for tool in SOURCES:
                destination = ready / f'openusage-{tool}-{args.target}{suffix}'
                stage_sidecar(tool, pins[tool], args.archive_dir, work / 'input.tar.gz', destination)
            ready.rename(args.out)
    except (KeyError, OSError, ValueError, tarfile.TarError) as error:
        parser.exit(1, f'{error}\n')

    print(f'Checked and staged two preview sidecars for {args.target}.')


if __name__ == '__main__':
    main()
