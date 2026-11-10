"""Single-file protocol experiment. This is not an OpenUsage storage implementation."""
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import tempfile

if sys.platform == 'linux':
    import fcntl


class Unavailable(Exception):
    pass


def regular_bytes(path, limit):
    fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as source:
        before = os.fstat(source.fileno())
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_size > limit:
            raise Unavailable('Unsupported publication file')
        data = source.read(limit + 1)
        after = os.fstat(source.fileno())
        stamp = lambda s: (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
        if len(data) > limit or len(data) != before.st_size or stamp(before) != stamp(after):
            raise Unavailable('Publication changed during the read')
        return data, stamp(after)


class Publication:
    """A private fixture directory with one session and one completion record."""
    def __init__(self, root):
        self.root = Path(root)
        self.root.mkdir(mode=0o700)
        self.sessions = self.root / 'sessions'
        self.sessions.mkdir(mode=0o700)
        self.control = self.root / 'control'
        self.control.mkdir(mode=0o700)
        self.lock = self.control / 'lease'
        with self.lock.open('xb'):
            pass
        info = self.lock.stat()
        self.lock_identity = (info.st_dev, info.st_ino)
        self.target = self.sessions / 'root.jsonl'
        self.seal = self.control / 'committed.json'
        self._write_seal(None)

    @classmethod
    def attach(cls, root, lock_identity):
        """Join a fixture through the lock identity supplied by its owner."""
        handle = cls.__new__(cls)
        handle.root = Path(root)
        handle.sessions = handle.root / 'sessions'
        handle.control = handle.root / 'control'
        handle.lock = handle.control / 'lease'
        handle.target = handle.sessions / 'root.jsonl'
        handle.seal = handle.control / 'committed.json'
        handle.lock_identity = tuple(lock_identity)
        return handle

    def _verify_lock(self, fd):
        descriptor, path = os.fstat(fd), self.lock.lstat()
        if (not stat.S_ISREG(path.st_mode) or path.st_nlink != 1
                or (descriptor.st_dev, descriptor.st_ino) != self.lock_identity
                or (path.st_dev, path.st_ino) != self.lock_identity):
            raise Unavailable('The publication lock changed')

    @contextmanager
    def lease(self, exclusive):
        try:
            fd = os.open(self.lock, os.O_RDWR | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
        except OSError as error:
            raise Unavailable('The publication lock is unavailable') from error
        try:
            self._verify_lock(fd)
            try:
                fcntl.flock(fd, (fcntl.LOCK_EX if exclusive else fcntl.LOCK_SH) | fcntl.LOCK_NB)
            except BlockingIOError as error:
                raise Unavailable('A publication operation is active') from error
            self._verify_lock(fd)
            yield fd, exclusive
            self._verify_lock(fd)
        finally:
            os.close(fd)

    def scan(self, read_report):
        with self.lease(False):
            before = self.current()
            report = read_report()
            if self.current() != before:
                raise Unavailable('The publication changed during collection')
            return report

    def current(self):
        def unique(pairs):
            result = {}
            for key, value in pairs:
                if key in result:
                    raise Unavailable('Repeated completion field')
                result[key] = value
            return result
        try:
            raw, seal_stamp = regular_bytes(self.seal, 1024)
            seal = json.loads(raw, object_pairs_hook=unique)
            if set(seal) != {'schemaVersion', 'sha256'} or type(seal['schemaVersion']) is not int or seal['schemaVersion'] != 1:
                raise Unavailable('Unsupported completion record')
            digest = seal['sha256']
            if digest is not None and (not isinstance(digest, str) or len(digest) != 64
                                       or any(c not in '0123456789abcdef' for c in digest)):
                raise Unavailable('Invalid completion digest')
            members = sorted(p.name for p in self.sessions.iterdir())
            if members != ([] if digest is None else ['root.jsonl']):
                raise Unavailable('The published file set differs from its completion record')
            if digest is None:
                return None, None, seal_stamp
            data, stamp = regular_bytes(self.target, 1024 * 1024)
            if hashlib.sha256(data).hexdigest() != digest:
                raise Unavailable('Published bytes lack a matching completion record')
            return digest, stamp, seal_stamp
        except (OSError, ValueError, TypeError, KeyError) as error:
            raise Unavailable('The completion record is unavailable') from error

    def _write_seal(self, digest):
        fd, name = tempfile.mkstemp(prefix='commit-', dir=self.control)
        try:
            with os.fdopen(fd, 'wb') as output:
                output.write(json.dumps({'schemaVersion': 1, 'sha256': digest}).encode())
                output.flush()
                os.fsync(output.fileno())
            os.replace(name, self.seal)
            self._sync_directory(self.control)
        finally:
            Path(name).unlink(missing_ok=True)

    @staticmethod
    def _sync_directory(path):
        fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)

    def publish(self, lease, completed_file, expected, fault=lambda stage: None):
        fd, exclusive = lease
        if not exclusive:
            raise Unavailable('Publication requires an exclusive lease')
        self._verify_lock(fd)
        if self.current() != expected:
            raise Unavailable('The producer prepared an obsolete revision')
        data, _ = regular_bytes(completed_file, 1024 * 1024)
        source_fd = os.open(completed_file, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
        try:
            os.fsync(source_fd)
        finally:
            os.close(source_fd)
        fault('before_data')
        os.replace(completed_file, self.target)
        self._sync_directory(self.sessions)
        fault('after_data')
        self._write_seal(hashlib.sha256(data).hexdigest())
        fault('after_commit')
