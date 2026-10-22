#!/usr/bin/python3
"""Synthetic selected-profile logout. No network or real credentials."""
import json
import os
import pathlib
import sys
import time

home = pathlib.Path(os.environ['CODEX_HOME'])
log = home / 'synthetic-logout-observations.jsonl'
marker = home / 'synthetic-monitored-login-completed'
allowed = {'HOME', 'CODEX_HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_CACHE_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR', 'TMPDIR', 'PATH', 'LANG', 'LC_CTYPE'}
assert not set(os.environ) - allowed


def observe(kind, **fields):
    with log.open('a', encoding='utf8') as stream:
        stream.write(json.dumps({'kind': kind, 'pid': os.getpid(), 'processGroup': os.getpgrp(), **fields}) + '\n')
    log.chmod(0o600)


def mode():
    try:
        value = json.loads((home / 'synthetic-logout-config.json').read_text(encoding='utf8'))
    except FileNotFoundError:
        return 'success'
    assert set(value) == {'mode'}
    assert value['mode'] in {'success', 'malformed', 'malformed_ack', 'rpc_error', 'hang', 'still_connected', 'notice_before', 'invalid_notice'}
    return value['mode']


def send(value):
    print(json.dumps(value), flush=True)


def request():
    line = sys.stdin.readline()
    if not line:
        sys.exit(0)
    value = json.loads(line)
    observe('request', request=value)
    return value


def account(request_id):
    value = {'account': None, 'requiresOpenaiAuth': True}
    if marker.exists():
        value['account'] = {'type': 'chatgpt', 'email': 'synthetic-collection@example.invalid', 'planType': 'plus'}
    send({'id': request_id, 'result': value})


observe('start', arguments=sys.argv[1:])
if sys.argv[1:] == ['--version']:
    print('codex-cli 0.163.0', flush=True)
    sys.exit(0)
assert sys.argv[1:] == ['app-server', '--listen', 'stdio://', '-c', 'cli_auth_credentials_store="file"', '-c', 'analytics.enabled=false', '-c', 'feedback.enabled=false', '-c', 'otel.exporter="none"', '-c', 'otel.metrics_exporter="none"', '-c', 'check_for_update_on_startup=false']
(home / 'synthetic-child-pid').write_text(str(os.getpid()))
(home / 'synthetic-runtime-home').write_text(os.environ['HOME'])
assert request()['method'] == 'initialize'
send({'id': 1, 'result': {'userAgent': 'synthetic logout', 'codexHome': str(home), 'platformFamily': 'unix', 'platformOs': 'linux'}})
assert request() == {'method': 'initialized', 'params': {}}
value = request()
if value == {'id': 2, 'method': 'account/read', 'params': {'refreshToken': False}}:
    account(2)
    time.sleep(60)
    sys.exit(0)
assert value == {'id': 3, 'method': 'account/logout', 'params': {}}
selected = mode()
observe('logout-pending')
while mode() == 'hang':
    time.sleep(0.01)
if selected in {'malformed', 'malformed_ack'}:
    send({'id': 3, 'result': {'unexpected': True}})
    time.sleep(60)
elif selected == 'rpc_error':
    send({'id': 3, 'error': {'code': -32603, 'message': 'Synthetic logout failure'}})
    time.sleep(60)
else:
    notice = {'method': 'account/updated', 'params': {'authMode': None, 'planType': None}}
    if selected == 'notice_before':
        send(notice)
    if selected not in {'still_connected', 'invalid_notice'}:
        marker.unlink(missing_ok=True)
    send({'id': 3, 'result': {}})
    if selected == 'invalid_notice':
        send({'method': 'account/updated', 'params': {'authMode': 'chatgpt', 'planType': 'plus'}})
    elif selected != 'notice_before':
        send(notice)
    observe('logout-completed')
    assert request() == {'id': 4, 'method': 'account/read', 'params': {'refreshToken': False}}
    account(4)
    time.sleep(60)
