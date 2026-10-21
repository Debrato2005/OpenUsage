#!/usr/bin/env python3
"""Record native build provenance; retain executable bytes only with --retain."""
from pathlib import Path
import argparse
import datetime
import fcntl
import hashlib
import json
import os
import shutil
import stat
import subprocess
import tempfile

WORKSPACE = Path(__file__).resolve().parents[1]
ROOT = WORKSPACE.parent.parent if WORKSPACE.parent.name == '.recovery' else WORKSPACE
MAX_FILES = 4
MAX_BYTES = 1536 * 1024 * 1024
RESERVE_BYTES = 64 * 1024 * 1024


def scoped(path, base):
    path = Path(os.path.abspath(path))
    if not path.is_relative_to(base) or path == base:
        raise ValueError('Artifact path is outside its allowed scope.')
    for part in [path, *path.parents]:
        if part.is_symlink():
            raise ValueError('Artifact paths must not contain symbolic links.')
        if part == ROOT:
            break
    return path


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def version(command, cwd):
    try:
        return subprocess.run(command, cwd=cwd, check=True, capture_output=True,
                              text=True, timeout=5).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return None


def _freeze(source, destination, retain=False, build_commands=(), test_reference=None, source_workspace=None):
    os.umask(0o077)
    source = scoped(Path(source), ROOT / '.tools/build-target')
    destination = scoped(Path(destination), ROOT / '.recovery/artifacts')
    receipt = destination.with_name(destination.name + '.receipt.json')
    scoped(receipt, ROOT / '.recovery/artifacts')
    if not stat.S_ISREG(source.stat().st_mode):
        raise ValueError('The native source must be a regular file.')
    if destination.exists() or receipt.exists():
        raise FileExistsError('The artifact label already exists; nothing was replaced.')
    before = source.stat()
    if before.st_size > MAX_BYTES:
        raise ValueError('The native source exceeds the retention size limit.')
    with source.open('rb') as stream:
        if stream.read(4) != b'\x7fELF':
            raise ValueError('Only Linux ELF build artifacts are supported by this helper.')
    source_hash = digest(source)
    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    if retain:
        if destination.parent.stat().st_uid != os.getuid() or stat.S_IMODE(destination.parent.stat().st_mode) & 0o077:
            raise ValueError('Retained executables require an owned private destination directory.')
        retained = {}
        for parent, directories, names in os.walk(ROOT / '.recovery/artifacts', followlinks=False):
            directories[:] = [name for name in directories if not (Path(parent) / name).is_symlink()]
            for name in names:
                path = Path(parent) / name
                if path.is_symlink() or not path.is_file():
                    continue
                if name.startswith('openusage-desktop'):
                    with path.open('rb') as stream:
                        if stream.read(4) == b'\x7fELF':
                            retained[path] = path.stat().st_size
                elif name.endswith('.receipt.json') and path.stat().st_size < 65536:
                    try:
                        saved = json.loads(path.read_text())
                    except (OSError, ValueError):
                        continue
                    if isinstance(saved, dict) and saved.get('format') == 'openusage-native-artifact-receipt-v1' and saved.get('retained') is True:
                        artifact = path.with_name(name.removesuffix('.receipt.json'))
                        scoped(artifact, ROOT / '.recovery/artifacts')
                        if artifact.exists():
                            if not artifact.is_file() or digest(artifact) != saved.get('sha256'):
                                raise ValueError('A retained artifact no longer matches its receipt; review it manually.')
                            retained[artifact] = artifact.stat().st_size
        if len(retained) >= MAX_FILES or sum(retained.values()) + before.st_size > MAX_BYTES:
            raise ValueError('Native retention cap reached. Review existing artifacts before retaining another.')
        if shutil.disk_usage(destination.parent).free < before.st_size + RESERVE_BYTES:
            raise ValueError('Insufficient free space to retain the native artifact safely.')
    workspace = None
    commit = None
    dependency_hashes = None
    dirty = None
    if source_workspace is not None:
        workspace = Path(os.path.abspath(source_workspace))
        locks = [scoped(workspace / name, ROOT) for name in ['Cargo.lock', 'package-lock.json']]
        if not workspace.is_dir() or any(not path.is_file() for path in locks):
            raise ValueError('Source workspace requires its Cargo and npm lockfiles.')
        commit = version(['git', 'rev-parse', 'HEAD'], workspace)
        status = version(['git', 'status', '--porcelain', '--untracked-files=all'], workspace)
        dirty = None if status is None else any(line != '?? scripts/freeze-native-artifact.py' for line in status.splitlines())
        dependency_hashes = {path.name: digest(path) for path in locks}
    record = {
        'format': 'openusage-native-artifact-receipt-v1',
        'recordedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'sourcePath': str(source.relative_to(ROOT)), 'sha256': source_hash,
        'bytes': before.st_size, 'retained': retain,
        'sourceWorkspace': str(workspace.relative_to(ROOT)) if workspace else None,
        'commit': commit, 'dirtySource': dirty, 'dependencyLockSha256': dependency_hashes,
        'toolchain': version([str(ROOT / '.tools/rust/install/bin/rustc'), '-vV'], WORKSPACE),
        'buildCommands': list(build_commands), 'testReference': test_reference,
        'referenceStatus': 'complete' if workspace is not None and commit and dirty is not None and build_commands and test_reference else 'incomplete',
    }
    temporary = None
    linked = False
    receipt_created = False
    try:
        if retain:
            descriptor, name = tempfile.mkstemp(prefix='.native-', dir=destination.parent)
            temporary = Path(name)
            with os.fdopen(descriptor, 'wb') as output, source.open('rb') as original:
                shutil.copyfileobj(original, output, 1024 * 1024)
                output.flush()
                os.fsync(output.fileno())
            if digest(temporary) != source_hash:
                raise ValueError('The native source changed while it was copied.')
        after = source.stat()
        if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns):
            raise ValueError('The native source changed while it was recorded.')
        if retain:
            temporary.chmod(0o555)
            os.link(temporary, destination)
            linked = True
        # Exclusive creation preserves all earlier receipts and artifact labels.
        with receipt.open('x', encoding='utf-8') as output:
            receipt_created = True
            json.dump(record, output, indent=2)
            output.write('\n')
            output.flush()
            os.fsync(output.fileno())
        directory = os.open(destination.parent, os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    except BaseException:
        if linked:
            destination.unlink()
        if receipt_created:
            receipt.unlink()
        raise
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    print(destination.name, source_hash)
    return receipt



def freeze(source, destination, retain=False, build_commands=(), test_reference=None, source_workspace=None):
    base = ROOT / '.recovery/artifacts'
    scoped(base / '.freeze.lock', base)
    base.mkdir(mode=0o700, parents=True, exist_ok=True)
    descriptor = os.open(base / '.freeze.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, 'a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return _freeze(source, destination, retain, build_commands, test_reference, source_workspace)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('destination', type=Path, help='Receipt label; executable destination only with --retain.')
    parser.add_argument('--retain', action='store_true', help='Keep bytes for current active debugging, release or security verification.')
    parser.add_argument('--build-command', action='append', default=[])
    parser.add_argument('--test-reference')
    parser.add_argument('--source-workspace', type=Path, help='Producing checkout inside this repository; omitted provenance stays unknown.')
    args = parser.parse_args()
    freeze(args.source, args.destination, args.retain, args.build_command, args.test_reference, args.source_workspace)


if __name__ == '__main__':
    main()
