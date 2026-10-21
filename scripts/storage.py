#!/usr/bin/env python3
"""Report disk use and retain only explicitly owned, finished diagnostic runs."""
import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import shutil
import stat
import time
import uuid

REPOSITORY_ROOT = Path(os.path.abspath(__file__)).parents[1]
MARKER = '.openusage-disposable.json'
PROTECTED = {'.git', '.keep', 'src', 'crates', 'scripts', 'tests', 'node_modules'}
SUFFIXES = ('.db', '.sqlite', '.sqlite3', '.db-wal', '.db-shm', '.wal', '.shm',
            '.oubackup', '.key', '.pem', '.p12', '.pfx', '.rs', '.tsx', '.ts',
            '.js', '.py', '.sh', '.sql', '.toml', '.lock', '.bundle', '.db3',
            '.db-journal', '.sqlite-wal', '.sqlite-shm', '.sqlite-journal',
            '.sqlite3-wal', '.sqlite3-shm', '.sqlite3-journal', '.db3-wal',
            '.db3-shm', '.db3-journal', '.mjs', '.cjs', '.jsx', '.c', '.cpp',
            '.h', '.yml', '.yaml', '.md', '.journal', '.css', '.html', '.svg')


def linked(path):
    try:
        reparse = getattr(path.lstat(), 'st_file_attributes', 0) & 0x400
    except FileNotFoundError:
        return False
    return bool(reparse) or path.is_symlink() or getattr(path, 'is_junction', lambda: False)()


def workspace(root):
    root = Path(os.path.abspath(root))
    if root != REPOSITORY_ROOT and REPOSITORY_ROOT not in root.parents:
        raise ValueError('Root must stay inside this repository')
    for path in reversed((root, *root.parents)):
        if linked(path):
            raise ValueError('Linked workspace paths are refused')
    if root.parent == root or root.resolve() != root:
        raise ValueError('Use the real workspace directory, without links')
    if (linked(root / 'AGENTS.md') or linked(root / '.git')
            or not (root / 'AGENTS.md').is_file() or not (root / '.git').exists()):
        raise ValueError('Workspace markers are missing')
    return root


def diagnostics(root, create=False):
    path = workspace(root)
    for name in ('.recovery', 'diagnostics'):
        path = path / name
        if linked(path):
            raise ValueError('Linked diagnostic directories are refused')
        if create:
            path.mkdir(mode=0o700, exist_ok=True)
        if path.exists() and not path.is_dir():
            raise ValueError('Diagnostic path is not a directory')
    return path


def allocated(info):
    return info.st_blocks * 512 if hasattr(info, 'st_blocks') else info.st_size


def scan(path, protected=False, seen=None):
    """Return inode fingerprints and allocated bytes without traversing links."""
    rows, total = [], 0
    seen = set() if seen is None else seen
    pending = [path]
    while pending:
        current = pending.pop()
        if linked(current):
            if protected:
                raise ValueError('Linked content is preserved')
            continue
        info = current.lstat()
        relative = current.relative_to(path)
        name = current.name.lower()
        if protected and current != path:
            if (name in PROTECTED or name.endswith(SUFFIXES)
                    or any(word in name for word in ('credential', 'secret', 'auth', 'backup', 'database', 'session', 'token', 'key'))
                    or name in ('key', 'keys', 'tokens', 'token', 'workspace')):
                raise ValueError('Protected content is preserved')
            if current.name == MARKER and relative != Path(MARKER):
                raise ValueError('Nested ownership markers are preserved')
        if protected and not stat.S_ISREG(info.st_mode) and not stat.S_ISDIR(info.st_mode):
            raise ValueError('Special files are preserved')
        if protected and stat.S_ISREG(info.st_mode) and info.st_nlink != 1:
            raise ValueError('Hard-linked content is preserved')
        identity = (info.st_dev, info.st_ino)
        if identity not in seen:
            total += allocated(info)
            seen.add(identity)
        rows.append((str(relative), info.st_dev, info.st_ino, info.st_mode,
                     info.st_size, info.st_mtime_ns, info.st_ctime_ns))
        if stat.S_ISDIR(info.st_mode):
            pending.extend(sorted(current.iterdir(), reverse=True))
    return sorted(rows), total


@contextmanager
def cleanup_lock(directory):
    lock = directory / '.cleanup.lock'
    try:
        lock.mkdir(mode=0o700)
    except FileExistsError as error:
        raise RuntimeError('Another diagnostic lifecycle operation is active') from error
    identity = lock.stat()
    try:
        yield
    finally:
        current = lock.lstat()
        if linked(lock) or (current.st_dev, current.st_ino) != (identity.st_dev, identity.st_ino):
            raise RuntimeError('Cleanup lock changed; it was preserved')
        lock.rmdir()


def marker(path):
    file = path / MARKER
    if linked(file) or not file.is_file() or file.stat().st_size > 4096:
        raise ValueError('Ownership marker is missing or unsafe')
    value = json.loads(file.read_text(encoding='utf-8'))
    if (not isinstance(value, dict) or type(value.get('version')) is not int
            or value.get('version') != 1 or value.get('state') != 'finished'
            or value.get('pinned', False) or not isinstance(value.get('finishedAt'), str)):
        raise ValueError('Run is active, pinned or unsupported')
    finished = datetime.fromisoformat(value['finishedAt'].replace('Z', '+00:00'))
    if finished.tzinfo is None:
        raise ValueError('Finish time must include its timezone')
    return finished.timestamp()


def clean(root, *, apply=False, days=7, count=10, max_bytes=512 * 1024**2, now=None):
    if not math.isfinite(days) or days < 0 or count < 0 or max_bytes < 0:
        raise ValueError('Retention bounds must be nonnegative')
    directory = diagnostics(root)
    result = {'apply': apply, 'selected': [], 'preserved': []}
    if not directory.exists():
        return result
    if (directory / '.keep').exists() or (directory.parent / '.keep').exists():
        result['preserved'].append({'run': '.', 'reason': 'Diagnostic parent is pinned'})
        return result
    now = time.time() if now is None else now
    with cleanup_lock(directory):
        candidates = []
        for path in sorted(directory.iterdir()):
            if path.name == '.cleanup.lock':
                continue
            try:
                if linked(path) or not path.is_dir() or (path / '.keep').exists():
                    raise ValueError('Unowned or pinned entry is preserved')
                finished = marker(path)
                if finished > now:
                    raise ValueError('Future finish time is preserved')
                fingerprint, size = scan(path, protected=True)
                candidates.append((finished, path.name, path, fingerprint, size))
            except (OSError, ValueError, KeyError, TypeError) as error:
                result['preserved'].append({'run': path.name, 'reason': str(error)})
        remaining, total = len(candidates), sum(item[4] for item in candidates)
        for finished, name, path, fingerprint, size in sorted(candidates):
            if now - finished <= days * 86400 and remaining <= count and total <= max_bytes:
                continue
            if apply:
                if marker(path) != finished or scan(path, protected=True)[0] != fingerprint:
                    raise RuntimeError('Run changed; cleanup stopped')
                shutil.rmtree(path)
            result['selected'].append({'run': name, 'allocatedBytes': size, 'removed': apply})
            remaining -= 1
            total -= size
    return result


@contextmanager
def diagnostic_run(root, label='run', *, days=7, count=10, max_bytes=512 * 1024**2):
    """Caller writes disposable logs only; active markers are never retention candidates."""
    if not label or any(not (character.isalnum() or character in '-_') for character in label):
        raise ValueError('Use a simple diagnostic label')
    directory = diagnostics(root, create=True)
    with cleanup_lock(directory):
        path = directory / (label + '-' + uuid.uuid4().hex)
        path.mkdir(mode=0o700)
        (path / MARKER).write_text(json.dumps({'version': 1, 'state': 'active'}))
        identity = path.stat()
    try:
        yield path
    finally:
        with cleanup_lock(directory):
            if linked(path) or not path.is_dir():
                raise RuntimeError('Diagnostic directory changed')
            current = path.stat()
            if (current.st_dev, current.st_ino) != (identity.st_dev, identity.st_ino):
                raise RuntimeError('Diagnostic directory changed')
            value = {'version': 1, 'state': 'finished',
                     'finishedAt': datetime.now(timezone.utc).isoformat()}
            temporary = path / '.marker.pending'
            with temporary.open('x', encoding='utf-8') as output:
                json.dump(value, output)
            os.replace(temporary, path / MARKER)
        clean(root, apply=True, days=days, count=count, max_bytes=max_bytes)


def check(root, warn_gib=10):
    root = workspace(root)
    if not math.isfinite(warn_gib) or warn_gib < 0:
        raise ValueError('Warning threshold must be nonnegative')
    free = shutil.disk_usage(root).free
    top = []
    seen = set()
    for path in sorted(root.iterdir()):
        if linked(path):
            continue
        _, size = scan(path, seen=seen)
        top.append({'path': path.name, 'allocatedBytes': size})
    total = sum(row['allocatedBytes'] for row in top) + allocated(root.lstat())
    return {'freeBytes': free, 'totalAllocatedBytes': total,
            'warning': total > warn_gib * 1024**3,
            'sizeBasis': 'allocated where st_blocks is available; logical otherwise',
            'topDirectories': sorted(top, key=lambda row: (-row['allocatedBytes'], row['path']))}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).absolute().parents[1])
    commands = parser.add_subparsers(dest='command', required=True)
    report = commands.add_parser('check')
    report.add_argument('--warn-gib', type=float, default=10)
    report.add_argument('--strict', action='store_true')
    retention = commands.add_parser('clean')
    retention.add_argument('--apply', action='store_true')
    retention.add_argument('--days', type=float, default=7)
    retention.add_argument('--count', type=int, default=10)
    retention.add_argument('--max-mib', type=float, default=512)
    args = parser.parse_args()
    if args.command == 'check':
        result = check(args.root, args.warn_gib)
        print(json.dumps(result, indent=2))
        return 1 if args.strict and result['warning'] else 0
    result = clean(args.root, apply=args.apply, days=args.days, count=args.count,
                   max_bytes=int(args.max_mib * 1024**2))
    print(json.dumps(result, indent=2))
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError) as error:
        print(json.dumps({'error': str(error)}))
        raise SystemExit(2)
