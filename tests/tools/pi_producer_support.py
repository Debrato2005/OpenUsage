"""Pinned Pi writer execution in a private, offline Linux mount namespace."""
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import subprocess

from test_ccusage_formats import ROOT

HARNESS = ROOT / 'tests/tools/fixtures/pi-producer'
PIN = json.loads((HARNESS / 'pin.json').read_text())


def verify_source(source):
    source = Path(source).resolve(strict=True)
    for relative, digest in PIN['files'].items():
        if hashlib.sha256((source / relative).read_bytes()).hexdigest() != digest:
            raise ValueError('The Pi source or locked dependency differs from its fixture pin: ' + relative)
    return source


def producer_command(source, root, action, pause=None):
    node = Path(shutil.which('node')).resolve(strict=True)
    home = root / 'producer-home'
    home.mkdir(mode=0o700, exist_ok=True)
    args = ['bwrap', '--die-with-parent', '--unshare-net', '--unshare-pid',
            '--ro-bind', '/usr', '/usr', '--symlink', 'usr/lib', '/lib',
            '--symlink', 'usr/lib', '/lib64', '--proc', '/proc', '--dev', '/dev',
            '--tmpfs', '/tmp', '--ro-bind', str(node), '/runtime/node',
            '--ro-bind', str(source), '/producer', '--ro-bind', str(HARNESS), '/harness',
            '--bind', str(root), str(root), '--clearenv', '--setenv', 'HOME', str(home),
            '--setenv', 'PATH', '/usr/bin', '--setenv', 'TZ', 'UTC', '--chdir', str(root)]
    if pause:
        args += ['--setenv', 'PI_PRODUCER_PAUSE', pause]
    return args + ['/runtime/node', '--disable-warning=ExperimentalWarning', '--import',
                   '/harness/loader.mjs', '/harness/producer.mjs', action]


def produce(source, root, action):
    result = subprocess.run(producer_command(source, root, action), stdin=subprocess.DEVNULL,
                            capture_output=True, timeout=20, check=True)
    if list((root / 'producer-home').iterdir()):
        raise AssertionError('The Pi writer used an unexpected home path')
    return json.loads(result.stdout)


class PausedProducer:
    def __init__(self, source, root, action, stage):
        self.command = producer_command(source, root, action, stage)
        self.process = None
        self.stage = stage

    def __enter__(self):
        self.process = subprocess.Popen(self.command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, start_new_session=True)
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(self.process.stdout, selectors.EVENT_READ)
                if not selector.select(timeout=10):
                    raise TimeoutError('The Pi writer did not reach the selected write boundary')
            ready = self.process.stdout.readline()
            if json.loads(ready) != {'paused': self.stage}:
                raise AssertionError('Unexpected Pi write boundary')
            return self
        except BaseException:
            self.close()
            raise

    def resume(self):
        stdout, stderr = self.process.communicate(input=b'\n', timeout=10)
        if self.process.returncode:
            raise AssertionError(('Pi writer failed after resume', self.process.returncode, stderr))
        return json.loads(stdout)

    def close(self):
        if self.process and self.process.poll() is None:
            os.killpg(self.process.pid, signal.SIGKILL)
            self.process.communicate(timeout=5)
        if self.process:
            for stream in [self.process.stdin, self.process.stdout, self.process.stderr]:
                stream.close()

    def __exit__(self, *unused):
        self.close()


def normalize_report(report, root, state):
    # Normalize only captured report paths and the producer's random branch ID.
    report = json.loads(json.dumps(report).replace(str(root), '/synthetic'))
    for session in report['session']['sessions']:
        if session['sessionId'] == state['id'] and state['id'] not in ['root', 'fork']:
            session['sessionId'] = 'branched'
    report['session']['sessions'].sort(key=lambda row: row['sessionId'])
    return normalize_costs(report)


def normalize_costs(value):
    # Random IDs can change addition order for these unavailable float estimates.
    # Token counts and all other report fields remain exact.
    if isinstance(value, dict):
        return {key: round(item, 12) if key in ['cost', 'totalCost'] and isinstance(item, float)
                else normalize_costs(item) for key, item in value.items()}
    if isinstance(value, list):
        return [normalize_costs(item) for item in value]
    return value
