#!/usr/bin/env python3
"""Check native preview scanner output with private synthetic logs."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[2]
LOCAL_LOGS = ROOT / 'crates/openusage-locallogs'
FIXTURES = LOCAL_LOGS / 'tests/fixtures'
EXPECTED = FIXTURES / 'ccusage-20.0.48-daily-by-agent.json'
LINUX = 'x86_64-unknown-linux-gnu'
HOST_TARGETS = {
    ('darwin', 'arm64'): 'aarch64-apple-darwin',
    ('darwin', 'x86_64'): 'x86_64-apple-darwin',
    ('linux', 'x86_64'): LINUX,
    ('win32', 'amd64'): 'x86_64-pc-windows-msvc',
}
INPUTS = {
    'claude': ('home/.claude', 'CLAUDE_CONFIG_DIR'),
    'codex': ('codex', 'CODEX_HOME'),
}
TOKEN_FIELDS = ('inputTokens', 'outputTokens', 'cacheCreationTokens', 'cacheReadTokens', 'totalTokens')


def file_hash(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def token_rows(report):
    rows = []
    for day in report['daily']:
        for agent in day['agents']:
            counts = tuple(agent[field] for field in TOKEN_FIELDS)
            if any(type(count) is not int for count in counts):
                raise ValueError('The scanner returned a token count that is not an integer.')
            rows.append((day['period'], agent['agent'], *counts))
    return sorted(rows)


def scan_fixture(binary, tool, expected):
    fixture, variable = INPUTS[tool]
    with tempfile.TemporaryDirectory(prefix='openusage-preview-scan-') as temporary:
        root = Path(temporary)
        home = root / 'home'
        home.mkdir()
        logs = root / 'consented logs'
        shutil.copytree(FIXTURES / fixture, logs)
        before = {file.relative_to(logs): file_hash(file) for file in logs.rglob('*.jsonl')}
        if not before:
            raise ValueError(f'The {tool} fixture has no logs.')

        env = dict.fromkeys((
            'HOME', 'USERPROFILE', 'APPDATA', 'LOCALAPPDATA', 'XDG_CONFIG_HOME',
            'XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'TMP', 'TEMP', 'TMPDIR',
        ), str(home))
        env.update({variable: str(logs), 'NO_COLOR': '1', 'TZ': 'UTC'})
        if sys.platform == 'win32':
            env['SystemRoot'] = os.environ['SystemRoot']
            env['HOMEDRIVE'] = home.drive
            env['HOMEPATH'] = str(home)[len(home.drive):]

        process = subprocess.run([
            str(binary), 'daily', '--json', '--offline', '--by-agent', '--no-color',
            '--mode', 'calculate', '--timezone', 'UTC',
        ], stdin=subprocess.DEVNULL, capture_output=True, env=env, cwd=home, timeout=30)
        if process.returncode != 0:
            raise ValueError(f'The {tool} scanner exited with code {process.returncode}.')
        if token_rows(json.loads(process.stdout)) != expected:
            raise ValueError(f'The {tool} report did not match the expected daily token counts.')
        after = {file.relative_to(logs): file_hash(file) for file in logs.rglob('*.jsonl')}
        if before != after:
            raise ValueError(f'The {tool} fixture changed during the scan.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', choices=HOST_TARGETS.values(), required=True)
    parser.add_argument('--binary', type=Path, required=True)
    args = parser.parse_args()

    try:
        if HOST_TARGETS.get((sys.platform, platform.machine().lower())) != args.target:
            raise ValueError('The host does not match the requested target.')
        if args.target == LINUX:
            pin = json.loads((LOCAL_LOGS / 'sources/ccusage-20.0.48-linux-x86_64.json').read_text())
        else:
            pins = json.loads((ROOT / 'packaging/preview-sidecars.json').read_text())
            pin = pins[args.target]['ccusage']
        if (args.binary.is_symlink() or not args.binary.is_file()
                or args.binary.stat().st_size != pin['executableBytes']
                or file_hash(args.binary) != pin['executableSha256']):
            raise ValueError('The scanner does not match its pin.')
        expected = token_rows(json.loads(EXPECTED.read_text()))
        for tool in INPUTS:
            scan_fixture(args.binary.resolve(strict=True), tool, [row for row in expected if row[1] == tool])
    except (OSError, KeyError, ValueError, subprocess.TimeoutExpired) as error:
        parser.exit(1, f'{error}\n')

    print(f'The {args.target} scanner matched the synthetic Claude Code and Codex token counts.')


if __name__ == '__main__':
    main()
