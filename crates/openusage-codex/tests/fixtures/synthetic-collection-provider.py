#!/usr/bin/python3
"""Owned synthetic collection fixture. No network, credentials, real authentication, or inference."""
import json
import os
import pathlib
import select
import sys
import time

home = pathlib.Path(os.environ['CODEX_HOME'])
config = home / 'synthetic-collection-config.json'
log = home / 'synthetic-collection-observations.jsonl'
allowed = {'HOME', 'CODEX_HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_CACHE_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR', 'TMPDIR', 'PATH', 'LANG', 'LC_CTYPE'}
assert not set(os.environ) - allowed
modes = {'success', 'quota_unsupported', 'activity_unsupported', 'both_unsupported', 'quota_invalid', 'activity_invalid', 'quota_rpc_error', 'activity_rpc_error', 'unknown', 'signed_out', 'no_auth_required', 'unsupported_auth', 'changed_email', 'changed_plan', 'changed_auth', 'account_notice', 'quota_hang', 'activity_hang', 'post_hold', 'cleanup_fault', 'stable_changed_identity', 'login_pending', 'login_failed'}


def observe(kind, **fields):
    with log.open('a', encoding='utf8') as stream:
        stream.write(json.dumps({'kind': kind, 'pid': os.getpid(), 'processGroup': os.getpgrp(), 'monotonicNs': str(time.monotonic_ns()), **fields}) + '\n')
    log.chmod(0o600)


def configuration():
    try:
        value = json.loads(config.read_text(encoding='utf8'))
    except FileNotFoundError:
        value = {'mode': 'success'}
    assert set(value) in ({'mode'}, {'mode', 'requireLogin'}) and value['mode'] in modes
    assert type(value.get('requireLogin', False)) is bool
    return value


def mode():
    return configuration()['mode']


observe('start', arguments=sys.argv[1:])
if sys.argv[1:] == ['--version']:
    print('codex-cli 0.163.0', flush=True)
    sys.exit(0)
assert sys.argv[1:] == ['app-server', '--listen', 'stdio://', '-c', 'cli_auth_credentials_store="file"', '-c', 'analytics.enabled=false', '-c', 'feedback.enabled=false', '-c', 'otel.exporter="none"', '-c', 'otel.metrics_exporter="none"', '-c', 'check_for_update_on_startup=false']
(home / 'synthetic-child-pid').write_text(str(os.getpid()))
(home / 'synthetic-runtime-home').write_text(os.environ['HOME'])
selected = mode()
require_login = configuration().get('requireLogin', False)
login_marker = home / 'synthetic-monitored-login-completed'


def read_request():
    line = sys.stdin.readline()
    if not line:
        sys.exit(0)
    value = json.loads(line)
    recorded = value if value.get('method') != 'account/login/cancel' else {'id': value.get('id'), 'method': 'account/login/cancel', 'params': {'syntheticAttempt': True}}
    observe('request', request=recorded)
    return value


def send(value):
    print(json.dumps(value), flush=True)


def account(request_id, final=False):
    value = {'account': {'type': 'chatgpt', 'email': 'synthetic-collection@example.invalid', 'planType': 'plus'}, 'requiresOpenaiAuth': True}
    if selected == 'unknown':
        value['account']['email'] = None
    if selected in ('signed_out', 'no_auth_required'):
        value['account'] = None
    if selected == 'no_auth_required':
        value['requiresOpenaiAuth'] = False
    if selected == 'unsupported_auth':
        value['account'] = {'type': 'apiKey'}
    if selected == 'stable_changed_identity':
        value['account']['email'] = 'changed@example.invalid'
    if final and selected == 'changed_email':
        value['account']['email'] = 'changed@example.invalid'
    if final and selected == 'changed_plan':
        value['account']['planType'] = 'pro'
    if final and selected == 'changed_auth':
        value['requiresOpenaiAuth'] = False
    if require_login and not login_marker.exists():
        value['account'] = None
    send({'id': request_id, 'result': value})


def capability(request_id, kind, result):
    observe(kind + '-pending')
    while mode() == kind + '_hang':
        time.sleep(0.01)
    if selected in {kind + '_unsupported', 'both_unsupported'}:
        send({'id': request_id, 'error': {'code': -32601, 'message': 'Synthetic unsupported method'}})
    elif selected == kind + '_rpc_error':
        send({'id': request_id, 'error': {'code': -32603, 'message': 'Synthetic provider failure'}})
    else:
        send({'id': request_id, 'result': {} if selected == kind + '_invalid' else result})


assert read_request()['method'] == 'initialize'
send({'id': 1, 'result': {'userAgent': 'synthetic combined collection', 'codexHome': str(home), 'platformFamily': 'unix', 'platformOs': 'linux'}})
assert read_request() == {'method': 'initialized', 'params': {}}
request = read_request()
if request.get('method') == 'account/login/start':
    assert require_login
    assert request == {'id': 3, 'method': 'account/login/start', 'params': {'type': 'chatgptDeviceCode'}}
    attempt = 'synthetic-monitored-attempt'
    send({'id': 3, 'result': {'type': 'chatgptDeviceCode', 'loginId': attempt, 'verificationUrl': 'https://auth.openai.com/codex/device', 'userCode': 'SYNTH-1234'}})
    observe('login-challenge-emitted')
    while mode() == 'login_pending':
        ready, _, _ = select.select([sys.stdin], [], [], 0.01)
        if ready:
            cancel = read_request()
            assert cancel == {'id': 5, 'method': 'account/login/cancel', 'params': {'loginId': attempt}}
            send({'id': 5, 'result': {'status': 'canceled'}})
            observe('login-cancelled')
            time.sleep(60)
            sys.exit(0)
    success = mode() != 'login_failed'
    send({'method': 'account/login/completed', 'params': {'loginId': attempt, 'success': success, 'error': None}})
    if success:
        login_marker.write_text('Synthetic matching login completion. No credentials.\n', encoding='utf8')
        login_marker.chmod(0o600)
    observe('login-completion-emitted', success=success)
    request = read_request()
    assert success and request == {'id': 4, 'method': 'account/read', 'params': {'refreshToken': False}}
    account(4)
    observe('post-login-account-read')
    time.sleep(60)
    sys.exit(0)
assert request == {'id': 2, 'method': 'account/read', 'params': {'refreshToken': False}}
account(2)
assert read_request() == {'id': 3, 'method': 'account/rateLimits/read', 'params': {}}
if selected == 'account_notice':
    send({'method': 'account/updated', 'params': {'authMode': 'chatgpt'}})
capability(3, 'quota', {'rateLimitsByLimitId': {'codex': {'limitId': 'codex', 'primary': {'usedPercent': 25, 'windowDurationMins': 300, 'resetsAt': 1893456000}, 'secondary': None}}})
assert read_request() == {'id': 4, 'method': 'account/usage/read', 'params': {}}
capability(4, 'activity', {'summary': {'lifetimeTokens': 9007199254740993, 'peakDailyTokens': 9223372036854775807, 'longestRunningTurnSec': None, 'currentStreakDays': 0, 'longestStreakDays': None}, 'dailyUsageBuckets': None})
assert read_request() == {'id': 5, 'method': 'account/read', 'params': {'refreshToken': False}}
observe('post-read-pending')
while mode() == 'post_hold':
    time.sleep(0.01)
if selected == 'cleanup_fault':
    root = pathlib.Path(os.environ['HOME']).parent
    root.rename(root.with_name(root.name + '-moved'))
account(5, final=True)
time.sleep(60)
