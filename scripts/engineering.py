#!/usr/bin/env python3
"""Offline environment checks, change-aware verification and content-free trace metrics."""
import argparse
from collections import Counter
from datetime import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import stat
import subprocess
import sys
import threading
import time

import storage

ROOT = Path(__file__).absolute().parents[1]
MAX_LINE = 16 * 1024 * 1024
PACKAGES = {'crates/openusage-storage/': 'openusage-storage',
            'crates/openusage-codex/': 'openusage-codex',
            'crates/openusage-import/': 'openusage-import',
            'apps/desktop/src-tauri/': 'openusage-desktop'}
SIGNALS = {'permission': r'permission denied|operation not permitted',
           'missing_dependency': r'not found|no such file|cannot find',
           'failed': r'\bFAILED\b|\bFAIL\b',
           'timeout': r'timed out|timeout',
           'connection': r'connection refused|ENOTFOUND|EAI_AGAIN'}


def environment(root):
    env = os.environ.copy()
    if storage.linked(root / '.tools') or storage.linked(root / '.tools/build-target'):
        raise ValueError('Shared build directories must not be links')
    local = root / '.tools/rust/install/bin'
    if (local / ('cargo.exe' if os.name == 'nt' else 'cargo')).is_file():
        env['PATH'] = str(local) + os.pathsep + env.get('PATH', '')
    env.update(CARGO_TARGET_DIR=str(root / '.tools/build-target'),
               CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='1')
    return env


def git(root, *args):
    result = subprocess.run(['git', *args], cwd=root, capture_output=True,
                            timeout=15, check=False)
    if result.returncode:
        raise ValueError('Git check failed; check the selected reference and repository')
    return result.stdout.decode('utf-8', errors='strict')


def doctor(root):
    env = environment(root)
    tools = {}
    expected = json.loads((root / 'package.json').read_text())['engines']['node']
    for name, args in {'git': ['--version'], 'node': ['--version'],
                       'cargo': ['--version'], 'rustc': ['--version'],
                       'pkg-config': ['--version']}.items():
        executable = shutil.which(name, path=env.get('PATH'))
        version = None
        if executable:
            try:
                result = subprocess.run([executable, *args], cwd=root, env=env,
                                        capture_output=True, timeout=5)
                match = re.search(rb'\d+\.\d+(?:\.\d+)?', result.stdout[:4096])
                version = match.group().decode() if result.returncode == 0 and match else None
            except (OSError, subprocess.TimeoutExpired):
                pass
        tools[name] = {'available': bool(version), 'version': version}
    native = {}
    if sys.platform.startswith('linux'):
        for package in ('webkit2gtk-4.1', 'gtk+-3.0', 'librsvg-2.0'):
            try:
                check = subprocess.run(['pkg-config', '--exists', package], cwd=root,
                                       env=env, capture_output=True, timeout=5)
                native[package] = check.returncode == 0
            except (OSError, subprocess.TimeoutExpired):
                native[package] = False
    hooks = subprocess.run(['git', 'config', '--get', 'core.hooksPath'], cwd=root,
                           capture_output=True, timeout=5)
    return {'platform': platform.system(), 'python': platform.python_version(),
            'tools': tools, 'expectedNode': expected,
            'nodeMatches': tools['node']['version'] == expected,
            'nodeDependencies': (root / 'node_modules/typescript').is_dir(),
            'nativeLinuxPackages': native,
            'displayConfigured': bool(env.get('DISPLAY') or env.get('WAYLAND_DISPLAY')),
            'desktopBusConfigured': bool(env.get('DBUS_SESSION_BUS_ADDRESS')),
            'sharedCargoTarget': '.tools/build-target',
            'duplicateRootTarget': (root / 'target').exists(),
            'hooksConfigured': hooks.returncode == 0 and hooks.stdout.strip() == b'.githooks',
            'scope': 'Dependency diagnostics only; no install, provider execution or credential access'}


def changed_paths(root, base=None, staged=False):
    revision = []
    if base:
        revision = [git(root, 'rev-parse', '--verify', base + '^{commit}').strip()]
    args = ['diff', '--name-only', '--no-renames', '-z', *(revision or (['--cached'] if staged else ['HEAD'])), '--']
    paths = git(root, *args).split('\0')
    if not staged:
        paths += git(root, 'ls-files', '--others', '--exclude-standard', '-z').split('\0')
    return sorted(set(path for path in paths if path))


def plan(paths, renderer_tests=(), milestone=False):
    commands = [{'id': 'whitespace', 'argv': ['git', 'diff', '--check']}]
    gates = []
    tooling = any(p.startswith(('scripts/', 'tests/tools/')) for p in paths)
    if tooling:
        commands.append({'id': 'python_tools', 'argv': [sys.executable, '-m', 'unittest', 'discover', '-s', 'tests/tools', '-p', 'test_*.py']})
    if any(p in ('scripts/git-safety.mjs', 'tests/tools/git-safety.test.mjs') for p in paths):
        commands.append({'id': 'git_safety', 'argv': ['node', '--test', 'tests/tools/git-safety.test.mjs']})
    shared_renderer = any(p in ('package.json', 'package-lock.json', 'playwright.config.ts', 'tsconfig.json') for p in paths)
    renderer = shared_renderer or any(p.startswith(('apps/desktop/src/', 'tests/renderer/', 'tests/contract/')) for p in paths)
    selected = sorted(set(renderer_tests or [p for p in paths if re.fullmatch(r'tests/renderer/[\w-]+\.spec\.ts', p)]))
    if renderer:
        if milestone:
            commands.append({'id': 'renderer_milestone', 'argv': ['npm', 'run', 'check']})
        else:
            commands += [{'id': 'renderer_build', 'argv': ['npm', 'run', 'build']},
                         {'id': 'contracts', 'argv': ['npm', 'run', 'test:contract']}]
            if selected:
                commands.append({'id': 'renderer_targeted', 'argv': ['npm', 'run', 'test:renderer', '--', *selected]})
            else:
                gates.append('Select relevant --renderer-test files or use --milestone')
            if shared_renderer:
                gates.append('Shared renderer configuration/dependencies require milestone regression before integration')
    packages = sorted({name for prefix, name in PACKAGES.items() if any(p.startswith(prefix) and not p.endswith('.md') for p in paths)})
    workspace = any(p in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml') or p.startswith('.cargo/') for p in paths)
    if packages or workspace:
        scope = ['--workspace'] if milestone or workspace else [arg for p in packages for arg in ('-p', p)]
        commands += [{'id': 'rust_tests', 'argv': ['cargo', 'test', '--locked', *scope]},
                     {'id': 'rust_clippy', 'argv': ['cargo', 'clippy', '--locked', *scope, '--all-targets', '--', '-D', 'warnings']}]
        gates.append('Run changed native acceptance journeys against the current build; include security/restore gates for affected boundaries')
    if any(p.startswith('tests/native/backup-verifier/') and p.endswith(('.rs', '.toml', '.lock')) for p in paths):
        commands.append({'id': 'native_oracle_tests', 'argv': ['cargo', 'test', '--locked', '--manifest-path', 'tests/native/backup-verifier/Cargo.toml']})
    for path in paths:
        if path.startswith('tests/native/') and path.endswith('.mjs'):
            commands.append({'id': 'native_syntax', 'argv': ['node', '--check', path]})
    if any(p.startswith('tests/native/') for p in paths):
        gates.append('Native harness changes require their actual native journey; syntax checks do not qualify acceptance')
    if any(p.startswith('.github/') for p in paths):
        gates.append('Hosted CI execution remains separate from local checks')
    if any(p.startswith('scripts/') and p not in ('scripts/engineering.py', 'scripts/storage.py', 'scripts/git-safety.mjs') for p in paths):
        gates.append('Other tool changes require their own relevant behavioral checks')
    known = ('.github/', 'docs/', 'scripts/', 'tests/', 'apps/desktop/', 'crates/')
    unknown = [p for p in paths if not p.startswith(known) and not p.endswith('.md') and p not in
               ('.gitignore', 'Cargo.toml', 'Cargo.lock', 'package.json', 'package-lock.json', 'tsconfig.json', 'playwright.config.ts')]
    if unknown:
        gates.append('Unclassified changes require an explicit verification decision')
    return {'changedFiles': len(paths), 'commands': commands, 'manualGates': gates,
            'scope': 'Change-based suggestions, not proof of SRS/native acceptance'}


def stop_process(process):
    if os.name == 'nt':
        taskkill = Path(os.environ.get('SystemRoot', r'C:\Windows')) / 'System32/taskkill.exe'
        subprocess.run([str(taskkill), '/PID', str(process.pid), '/T', '/F'],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5)
    else:
        try:
            os.killpg(process.pid, signal.SIGTERM)
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            pass
        except ProcessLookupError:
            return
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    process.wait(timeout=5)


def execute(argv, root, env, timeout):
    """Drain output without retaining message text; keep fixed diagnostic counters."""
    executable = shutil.which(argv[0], path=env.get('PATH'))
    if not executable:
        return {'status': 'missing_dependency', 'elapsedSeconds': 0}
    started = time.monotonic()
    counts = Counter()
    flags = {'creationflags': subprocess.CREATE_NEW_PROCESS_GROUP} if os.name == 'nt' else {'start_new_session': True}
    process = subprocess.Popen([executable, *argv[1:]], cwd=root, env=env,
                               stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, **flags)
    def drain():
        tail = ''
        for chunk in iter(lambda: process.stdout.read(65536), b''):
            counts['outputBytes'] += len(chunk)
            text = tail + chunk.decode('utf-8', errors='replace')
            for name, pattern in SIGNALS.items():
                counts[name] += sum(match.end() > len(tail) for match in re.finditer(pattern, text, re.I))
            tail = text[-128:]
    reader = threading.Thread(target=drain, daemon=True)
    reader.start()
    status = 'passed'
    try:
        process.wait(timeout=timeout)
        if process.returncode:
            status = 'failed'
    except subprocess.TimeoutExpired:
        status = 'timeout'
        stop_process(process)
    except BaseException:
        stop_process(process)
        raise
    finally:
        reader.join(timeout=1)
        if reader.is_alive():
            stop_process(process)
            reader.join(timeout=2)
    if reader.is_alive():
        raise RuntimeError('Child output did not close after bounded process cleanup')
    process.stdout.close()
    return {'status': status, 'exitCode': process.returncode,
            'elapsedSeconds': round(time.monotonic() - started, 3), **counts}


def verify(root, args):
    paths = changed_paths(root, args.base, args.staged)
    for path in args.renderer_test:
        if not re.fullmatch(r'tests/renderer/[\w-]+\.spec\.ts', path) or not (root / path).is_file() or any(
                storage.linked(p) for p in (root / 'tests', root / 'tests/renderer', root / path)):
            raise ValueError('Select an existing regular renderer spec inside tests/renderer')
    result = plan(paths, args.renderer_test, args.milestone)
    if args.base:
        result['commands'][0]['argv'].append(git(root, 'rev-parse', '--verify', args.base + '^{commit}').strip())
    elif args.staged:
        result['commands'][0]['argv'].append('--cached')
    else:
        result['commands'][0]['argv'].append('HEAD')
    ignored = git(root, 'ls-files', '-ci', '--exclude-standard', '-z')
    result['trackedIgnoredFiles'] = len([p for p in ignored.split('\0') if p])
    if result['trackedIgnoredFiles']:
        raise ValueError('Tracked ignored artifacts require review before verification')
    if not args.run:
        return result
    if any(gate.startswith('Select relevant') for gate in result['manualGates']):
        raise ValueError('Choose targeted renderer tests before running this plan')
    env = environment(root)
    deadline = time.monotonic() + args.timeout
    with storage.diagnostic_run(root, 'engineering', count=3, max_bytes=16 * 1024**2) as directory:
        result['logDirectory'] = str(directory.relative_to(root))
        steps = []
        for command in result['commands']:
            remaining = deadline - time.monotonic()
            outcome = execute(command['argv'], root, env, remaining) if remaining > 0 else {'status': 'timeout', 'elapsedSeconds': 0}
            steps.append({'id': command['id'], **outcome})
            if outcome['status'] != 'passed':
                break
        result['results'] = steps
        result['status'] = 'checks_passed' if len(steps) == len(result['commands']) and all(s['status'] == 'passed' for s in steps) else 'failed'
        (directory / 'checks.log').write_text(json.dumps(result, indent=2), encoding='utf-8')
    return result


def trace(path):
    # Only an explicitly supplied regular trace is read. Never discover credential homes.
    path = Path(os.path.abspath(path))
    if path.suffix != '.jsonl' or any(storage.linked(p) for p in (path, *path.parents)):
        raise ValueError('Use an explicit regular JSONL trace without linked ancestors')
    flags = os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_NONBLOCK', 0)
    descriptor = os.open(path, flags)
    if not stat.S_ISREG(os.fstat(descriptor).st_mode):
        os.close(descriptor)
        raise ValueError('Trace must be a regular file')
    counts, categories, seen = Counter(), Counter(), set()
    tokens = {}
    start = end = None
    owner = None
    inherited = False
    started = time.monotonic()
    patterns = {'build': r'cargo\s+build|npm\s+run\s+build',
                'broad_tests': r'npm\s+run\s+check|cargo\s+test\s+--workspace',
                'agent_spawn': r'spawn_agent', 'wait': r'write_stdin|wait_agent|functions.wait'}
    size = os.fstat(descriptor).st_size
    if size > 1024**3:
        os.close(descriptor)
        raise ValueError('Trace exceeds the 1 GiB safety limit')
    with os.fdopen(descriptor, 'rb') as source:
        remaining = size
        while remaining > 0:
            if time.monotonic() - started > 120:
                raise ValueError('Trace analysis reached its two-minute deadline')
            line = source.readline(min(MAX_LINE + 1, remaining))
            remaining -= len(line)

            if not line:
                break
            if len(line) > MAX_LINE:
                raise ValueError('Trace record exceeds the 16 MiB safety limit')
            counts['records'] += 1
            try:
                value = json.loads(line)
            except (ValueError, UnicodeDecodeError, RecursionError):
                counts['malformedRecords'] += 1
                continue
            if not isinstance(value, dict) or not isinstance(value.get('payload'), dict):
                continue
            payload = value['payload']
            if value.get('type') == 'session_meta' and isinstance(payload.get('id'), str):
                if owner is None:
                    owner = payload['id']
                elif payload['id'] != owner:
                    inherited = True
            timestamp = value.get('timestamp')
            if isinstance(timestamp, str):
                try:
                    parsed = datetime.fromisoformat(timestamp.replace('Z', '+00:00'))
                    if parsed.tzinfo:
                        start = parsed if start is None else min(start, parsed)
                        end = parsed if end is None else max(end, parsed)
                except ValueError:
                    pass
            kind = payload.get('type')
            if kind in ('function_call', 'custom_tool_call') and value.get('type') == 'response_item':
                counts['toolCalls'] += 1
                name = str(payload.get('name', ''))
                counts['agentSpawns'] += name.endswith('spawn_agent')
                counts['agentFollowups'] += name.endswith('followup_task')
                arguments = payload.get('arguments', payload.get('input', ''))
                text = name + json.dumps(arguments, sort_keys=True)
                digest = hashlib.sha256(text.encode()).digest()
                if digest in seen:
                    counts['repeatedExactCalls'] += 1
                seen.add(digest)
                for category, pattern in patterns.items():
                    categories[category] += bool(re.search(pattern, text))
                categories['native'] += 'tests/native/' in text and '.mjs' in text
                categories['documentation_read'] += bool(re.search(r'\b(?:cat|sed|rg)\b', text)) and any(
                    'docs/' + name in text for name in ('SRS', 'implementation-status', 'acceptance-traceability'))
            if kind in ('function_call_output', 'custom_tool_call_output') and value.get('type') == 'response_item':
                output = json.dumps(payload.get('output', ''))
                counts['toolOutputBytes'] += len(output.encode())
                counts['failureSignalOutputs'] += bool(re.search(r'exit_code[\\"\s:]+[1-9]|exited with code [1-9]|Tool execution failed|socket failed 1|model is at capacity', output, re.I))
            info = payload.get('info')
            usage = None
            if value.get('type') == 'token_usage_record' and (owner is None or payload.get('thread_id') == owner):
                usage = payload.get('thread_token_usage')
            elif not inherited and kind == 'token_count' and isinstance(info, dict):
                usage = info.get('total_token_usage')
            if isinstance(usage, dict):
                for field in ('input_tokens', 'cached_input_tokens', 'output_tokens', 'total_tokens'):
                    number = usage.get(field)
                    if type(number) is int and number >= 0:
                        tokens[field] = max(tokens.get(field, 0), number)
    return {'metrics': dict(counts), 'heuristicCallCategories': dict(categories),
            'containsInheritedHistory': inherited,
            'reportedTokenHighWatermark': tokens or None,
            'elapsedSpanSeconds': (end - start).total_seconds() if start and end else None,
            'analysisSeconds': round(time.monotonic() - started, 3),
            'scope': 'Stored records from one trace, possibly including replayed parent calls. Repeats/error signals are indicators, not proof of waste or agent actions. Token snapshots are owner-scoped maxima where IDs exist, never summed or treated as billing. No messages, paths, arguments or tool output retained.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('doctor')
    check = commands.add_parser('verify')
    selection = check.add_mutually_exclusive_group()
    selection.add_argument('--base')
    selection.add_argument('--staged', action='store_true')
    check.add_argument('--renderer-test', action='append', default=[])
    check.add_argument('--milestone', action='store_true')
    check.add_argument('--run', action='store_true')
    check.add_argument('--timeout', type=int, default=600)
    audit = commands.add_parser('trace')
    audit.add_argument('path', type=Path)
    args = parser.parse_args()
    started = time.monotonic()
    try:
        if args.command == 'trace':
            result = trace(args.path)
        else:
            root = storage.workspace(ROOT)
            if args.command == 'doctor':
                result = doctor(root)
            else:
                if not 1 <= args.timeout <= 3600:
                    raise ValueError('Use a total verification timeout from 1 to 3600 seconds')
                result = verify(root, args)
        result['elapsedSeconds'] = round(time.monotonic() - started, 3)
        print(json.dumps(result, sort_keys=True))
        return 1 if result.get('status') == 'failed' else 0
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        # Exception text can contain private paths, arguments or child output.
        reason = str(error) if type(error) is ValueError else 'Check repository, dependencies and file safety; no raw error retained'
        print(json.dumps({'status': 'error', 'reason': reason}))
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
