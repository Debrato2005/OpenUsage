#!/usr/bin/python3
"""Owned protocol fixture. Never connects to a provider or reads credentials."""
import json
import os
from pathlib import Path
import sys
import time

home_value = os.environ.get('CODEX_HOME')
home = Path(home_value) if home_value else None
allowed = {'HOME', 'CODEX_HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_CACHE_HOME',
           'XDG_STATE_HOME', 'XDG_RUNTIME_DIR', 'TMPDIR', 'PATH', 'LANG', 'LC_CTYPE'}
observations = [Path(__file__).resolve().parent / 'synthetic-execution-observations.jsonl']
if home is not None:
    observations.append(home / 'synthetic-provider-observations.jsonl')

def record(kind, **fields):
    item = {'kind': kind, 'pid': os.getpid(), 'processGroup': os.getpgrp(), **fields}
    for path in observations:
        descriptor = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
        with os.fdopen(descriptor, 'a', encoding='utf-8') as output:
            output.write(json.dumps(item) + '\n')
            output.flush()

config_path = Path(__file__).resolve().parent / 'synthetic-provider-config.json'
config = json.loads(config_path.read_text(encoding='utf-8')) if config_path.exists() else {}
assert isinstance(config, dict) and set(config) <= {'defaultMode', 'modesByHome'}
mode = config.get('modesByHome', {}).get(str(home), config.get('defaultMode', 'success'))
assert mode in {'success', 'unsupported-version', 'hang', 'malformed', 'provisional'}
record('start', arguments=sys.argv[1:], environment={name: os.environ[name] for name in allowed if name in os.environ},
       unexpectedEnvironmentKeys=sorted(set(os.environ) - allowed), mode=mode)
assert home is not None, 'The fixture needs an isolated CODEX_HOME.'
if sys.argv[1:] == ['--version']:
    print('codex-cli 0.163.0' if mode != 'unsupported-version' else 'codex-cli 0.0.0', flush=True)
    raise SystemExit(0)

assert sys.argv[1:4] == ['app-server', '--listen', 'stdio://'], 'Unexpected fixture arguments.'
expected = ['-c', 'cli_auth_credentials_store="file"', '-c', 'analytics.enabled=false', '-c', 'feedback.enabled=false',
            '-c', 'otel.exporter="none"', '-c', 'otel.metrics_exporter="none"', '-c', 'check_for_update_on_startup=false']
assert sys.argv[4:] == expected, 'Unexpected fixture configuration.'
for line in sys.stdin:
    request = json.loads(line)
    record('request', request=request)
    method = request['method']
    if method == 'initialize':
        print(json.dumps({'id': request['id'], 'result': {'userAgent': 'OpenUsage synthetic native fixture',
              'codexHome': str(home), 'platformFamily': 'unix', 'platformOs': 'linux'}}), flush=True)
    elif method == 'initialized':
        assert request == {'method': 'initialized', 'params': {}}
    elif method == 'account/read':
        assert request['params'] == {'refreshToken': False}
        record('account-read-pending')
        if mode == 'hang':
            while True:
                time.sleep(1)
        elif mode == 'malformed':
            print('{"id":2,"result":{"account":42}}', flush=True)
        elif mode == 'provisional':
            print(json.dumps({'id': request['id'], 'result': {'account': {'type': 'chatgpt',
                  'email': 'synthetic-unverified@example.invalid', 'planType': 'synthetic-plan'}, 'requiresOpenaiAuth': True}}), flush=True)
        else:
            print(json.dumps({'id': request['id'], 'result': {'account': None, 'requiresOpenaiAuth': True}}), flush=True)
    else:
        raise RuntimeError('Unexpected fixture operation. No login, inference, credits, or network operations are allowed.')
