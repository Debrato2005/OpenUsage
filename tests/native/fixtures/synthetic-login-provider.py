#!/usr/bin/python3
"""Reviewed synthetic provider. No network, OAuth, credential files, or subprocesses."""
import json
import os
from pathlib import Path
import select
import sys
import time

ROOT = Path(__file__).resolve().parent
LOG = ROOT / 'synthetic-login-observations.jsonl'
CONFIG = ROOT / 'synthetic-login-provider-config.json'
ALLOWED_ENVIRONMENT = {'HOME', 'CODEX_HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME',
                       'XDG_CACHE_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR',
                       'TMPDIR', 'PATH', 'LANG', 'LC_CTYPE'}
MODES = {'success', 'early', 'pending', 'start_hang', 'wrong_completion',
         'null_completion', 'failed', 'updated_only', 'cancel_unresponsive',
         'bad_url_origin', 'bad_url_query', 'bad_url_fragment', 'bad_url_userinfo',
         'bad_url_encoded', 'bad_code_control', 'bad_code_bidi', 'bad_code_empty',
         'bad_code_long', 'account_missing', 'unsupported_account', 'no_email'}
LOGIN_ID = 'synthetic-device-attempt'


def observe(kind, **fields):
    with LOG.open('a', encoding='utf8') as stream:
        stream.write(json.dumps({'kind': kind, 'pid': os.getpid(),
                                 'processGroup': os.getpgrp(), **fields}) + '\n')
    LOG.chmod(0o600)


observe('start', arguments=sys.argv[1:],
        environment={name: os.environ[name] for name in sorted(ALLOWED_ENVIRONMENT)
                     if name in os.environ},
        unexpectedEnvironmentKeys=sorted(set(os.environ) - ALLOWED_ENVIRONMENT))
HOME = Path(os.environ['CODEX_HOME'])
STATE = HOME / 'synthetic-selected-login.json'


def mode():
    try:
        value = json.loads(CONFIG.read_text(encoding='utf8'))
    except FileNotFoundError:
        value = {}
    assert set(value) <= {'defaultMode', 'modesByHome'}
    selected = value.get('modesByHome', {}).get(str(HOME), value.get('defaultMode', 'success'))
    assert selected in MODES
    return selected


initial_mode = mode()
if sys.argv[1:] == ['--version']:
    print('codex-cli 0.163.0', flush=True)
    sys.exit(0)
assert sys.argv[1:] == ['app-server', '--listen', 'stdio://',
                       '-c', 'cli_auth_credentials_store="file"',
                       '-c', 'analytics.enabled=false', '-c', 'feedback.enabled=false',
                       '-c', 'otel.exporter="none"', '-c', 'otel.metrics_exporter="none"',
                       '-c', 'check_for_update_on_startup=false']


def read_request():
    line = sys.stdin.readline()
    if not line:
        sys.exit(0)
    value = json.loads(line)
    observe('request', request=value)
    return value


def send(value):
    print(json.dumps(value), flush=True)


def account(request_id, selected_mode):
    if selected_mode == 'account_missing':
        value = None
    elif selected_mode == 'unsupported_account':
        value = {'type': 'apiKey'}
    else:
        value = {'type': 'chatgpt', 'planType': 'plus'}
        if selected_mode != 'no_email':
            value['email'] = 'synthetic-login@example.invalid'
    send({'id': request_id, 'result': {'account': value, 'requiresOpenaiAuth': True}})


def complete(selected_mode):
    identifier = 'another-synthetic-attempt' if selected_mode == 'wrong_completion' else LOGIN_ID
    if selected_mode == 'null_completion':
        identifier = None
    successful = selected_mode != 'failed'
    if successful and identifier == LOGIN_ID:
        # This marker is controlled test data, never a provider credential or token.
        STATE.write_text(json.dumps({'synthetic': True, 'completed': True}) + '\n', encoding='utf8')
        STATE.chmod(0o600)
    send({'method': 'account/login/completed', 'params': {
        'loginId': identifier, 'success': successful,
        'error': None if successful else 'synthetic private provider detail; never expose'}})
    observe('completion-emitted', matching=identifier == LOGIN_ID, successful=successful)


assert read_request() == {'id': 1, 'method': 'initialize', 'params': {
    'clientInfo': {'name': 'openusage', 'title': 'OpenUsage', 'version': '0.1.0'},
    'capabilities': {'experimentalApi': False, 'requestAttestation': False}}}
send({'id': 1, 'result': {'userAgent': 'reviewed synthetic login provider',
                         'codexHome': str(HOME), 'platformFamily': 'unix', 'platformOs': 'linux'}})
assert read_request() == {'method': 'initialized', 'params': {}}
request = read_request()
if request['method'] == 'account/read':
    assert request == {'id': 2, 'method': 'account/read', 'params': {'refreshToken': False}}
    if STATE.exists():
        assert json.loads(STATE.read_text(encoding='utf8')) == {'synthetic': True, 'completed': True}
        account(2, initial_mode)
    else:
        send({'id': 2, 'result': {'account': None, 'requiresOpenaiAuth': True}})
    while True:
        time.sleep(1)
assert request == {'id': 3, 'method': 'account/login/start', 'params': {'type': 'chatgptDeviceCode'}}
if initial_mode == 'start_hang':
    observe('start-pending')
    while True:
        time.sleep(1)
if initial_mode == 'early':
    complete(initial_mode)
urls = {'bad_url_origin': 'https://chatgpt.com/codex/device',
        'bad_url_query': 'https://auth.openai.com/codex/device?synthetic=1',
        'bad_url_fragment': 'https://auth.openai.com/codex/device#synthetic',
        'bad_url_userinfo': 'https://synthetic@auth.openai.com/codex/device',
        'bad_url_encoded': 'https://auth.openai.com/codex/%64evice'}
codes = {'bad_code_control': 'SYNTH\n1234', 'bad_code_bidi': 'SYNTH\u202e1234',
         'bad_code_empty': '', 'bad_code_long': 'S' * 65}
send({'id': 3, 'result': {'type': 'chatgptDeviceCode', 'loginId': LOGIN_ID,
                         'verificationUrl': urls.get(initial_mode, 'https://auth.openai.com/codex/device'),
                         'userCode': codes.get(initial_mode, 'SYNTH-1234')}})
observe('challenge-emitted', valid=not initial_mode.startswith('bad_'))
if initial_mode == 'updated_only':
    send({'method': 'account/updated', 'params': {'authMode': 'chatgpt'}})
    observe('account-updated-emitted')
completion_sent = initial_mode == 'early'
if initial_mode not in {'early', 'pending', 'updated_only', 'cancel_unresponsive'} and not initial_mode.startswith('bad_'):
    complete(initial_mode)
    completion_sent = True
while True:
    if not completion_sent and initial_mode in {'pending', 'updated_only'}:
        selected = mode()
        if selected in {'success', 'failed'}:
            complete(selected)
            completion_sent = True
    ready, _, _ = select.select([sys.stdin], [], [], 0.05)
    if not ready:
        continue
    request = read_request()
    if request['method'] == 'account/login/cancel':
        assert request == {'id': 5, 'method': 'account/login/cancel', 'params': {'loginId': LOGIN_ID}}
        observe('cancel-received')
        if initial_mode != 'cancel_unresponsive':
            send({'id': 5, 'result': {'status': 'canceled'}})
        while True:
            time.sleep(1)
    assert request == {'id': 4, 'method': 'account/read', 'params': {'refreshToken': False}}
    assert completion_sent, 'A completion is required before the post-login account read.'
    account(4, initial_mode)
    observe('post-login-account-read')
    while True:
        time.sleep(1)
