#!/usr/bin/python3
"""Reviewed quota fixture. No network, OAuth, credential files, or subprocesses."""
import json
import os
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parent
LOG = ROOT / 'synthetic-quota-observations.jsonl'
CONFIG = ROOT / 'synthetic-quota-provider-config.json'
ALLOWED_ENVIRONMENT = {'HOME', 'CODEX_HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME',
                       'XDG_CACHE_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR',
                       'TMPDIR', 'PATH', 'LANG', 'LC_CTYPE'}
MODES = {'multi', 'legacy', 'empty', 'missing', 'invalid', 'fractional',
         'string_percent', 'mismatch', 'null_windows', 'unknown_identity',
         'signed_out', 'unsupported', 'changed_email', 'changed_plan',
         'changed_auth', 'post_unknown', 'early_auth', 'auth_notice',
         'late_auth', 'sparse_event', 'sparse_only', 'pending',
         'credits_absent', 'credits_null', 'credits_zero', 'credits_nonzero',
         'credits_null_details', 'credits_empty_details', 'credits_above_safe',
         'credits_max', 'credits_negative', 'credits_fractional',
         'credits_out_of_range', 'credits_missing_count', 'credits_null_count'}


def observe(kind, **fields):
    descriptor = os.open(LOG, os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, 'a', encoding='utf8') as stream:
        stream.write(json.dumps({'kind': kind, 'pid': os.getpid(),
                                 'processGroup': os.getpgrp(), **fields}) + '\n')


observe('start', arguments=sys.argv[1:],
        environment={name: os.environ[name] for name in sorted(ALLOWED_ENVIRONMENT)
                     if name in os.environ},
        unexpectedEnvironmentKeys=sorted(set(os.environ) - ALLOWED_ENVIRONMENT))
HOME = Path(os.environ['CODEX_HOME'])


def mode():
    try:
        value = json.loads(CONFIG.read_text(encoding='utf8'))
    except FileNotFoundError:
        value = {}
    assert isinstance(value, dict) and set(value) <= {'defaultMode', 'modesByHome'}
    selected = value.get('modesByHome', {}).get(str(HOME), value.get('defaultMode', 'multi'))
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


def account(identifier, final=False):
    result = {'account': {'type': 'chatgpt', 'email': 'synthetic-quota@example.invalid',
                          'planType': 'plus'}, 'requiresOpenaiAuth': True}
    if initial_mode == 'unknown_identity':
        result['account']['email'] = None
    if initial_mode == 'unsupported':
        result['account'] = {'type': 'apiKey'}
    if initial_mode == 'signed_out' or initial_mode == 'post_unknown' and final:
        result['account'] = None
    if initial_mode == 'changed_email' and final:
        result['account']['email'] = 'changed-synthetic-quota@example.invalid'
    if initial_mode == 'changed_plan' and final:
        result['account']['planType'] = 'pro'
    if initial_mode == 'changed_auth' and final:
        result['requiresOpenaiAuth'] = False
    send({'id': identifier, 'result': result})


def notice(method, params):
    send({'method': method, 'params': params})
    observe('notification-emitted', method=method)


assert read_request() == {'id': 1, 'method': 'initialize', 'params': {
    'clientInfo': {'name': 'openusage', 'title': 'OpenUsage', 'version': '0.1.0'},
    'capabilities': {'experimentalApi': False, 'requestAttestation': False}}}
send({'id': 1, 'result': {'userAgent': 'reviewed synthetic quota provider',
                         'codexHome': str(HOME), 'platformFamily': 'unix', 'platformOs': 'linux'}})
assert read_request() == {'method': 'initialized', 'params': {}}
assert read_request() == {'id': 2, 'method': 'account/read', 'params': {'refreshToken': False}}
if initial_mode == 'early_auth':
    notice('account/updated', {'authMode': 'chatgpt'})
account(2)
request = read_request()
assert request == {'id': 3, 'method': 'account/rateLimits/read', 'params': {}}
observe('quota-read-pending', mode=initial_mode)
selected = initial_mode
if selected in {'sparse_event', 'sparse_only'}:
    notice('account/rateLimits/updated', {'rateLimits': {'limitId': 'codex',
                                                      'primary': {'usedPercent': 90}}})
while selected in {'pending', 'sparse_only'}:
    time.sleep(0.05)
    selected = mode()
if selected == 'auth_notice':
    notice('account/updated', {'authMode': 'chatgpt'})

primary = {'usedPercent': 25, 'windowDurationMins': 300, 'resetsAt': 1893456000}
codex = {'limitId': 'codex', 'limitName': None, 'primary': primary,
         'secondary': {'usedPercent': 100, 'windowDurationMins': 10080, 'resetsAt': None},
         'rateLimitReachedType': 'rate_limit_reached'}
other = {'limitId': None, 'limitName': 'Synthetic returned bucket',
         'primary': {'usedPercent': 0, 'windowDurationMins': None, 'resetsAt': None},
         'secondary': None}
result = {'accountId': 'synthetic-private-account-id',
          'rateLimitsByLimitId': {'codex': codex, 'codex_other': other},
          'rateLimits': {'limitId': 'legacy-must-not-be-counted',
                         'primary': {'usedPercent': 99}},
          'rateLimitResetCredits': {'availableCount': 3, 'syntheticPrivateCredit': 123}}
if selected == 'credits_absent':
    del result['rateLimitResetCredits']
elif selected == 'credits_null':
    result['rateLimitResetCredits'] = None
elif selected.startswith('credits_'):
    summary = result['rateLimitResetCredits']
    if selected == 'credits_missing_count':
        del summary['availableCount']
    else:
        summary['availableCount'] = {
            'credits_zero': 0, 'credits_nonzero': 3,
            'credits_null_details': 3, 'credits_empty_details': 7,
            'credits_above_safe': 9007199254740993,
            'credits_max': 9223372036854775807, 'credits_negative': -1,
            'credits_fractional': 1.5, 'credits_out_of_range': 9223372036854775808,
            'credits_null_count': None,
        }[selected]
    if selected == 'credits_null_details':
        summary['credits'] = None
    elif selected == 'credits_empty_details':
        summary['credits'] = []
if selected == 'legacy':
    result['rateLimitsByLimitId'] = None
    result['rateLimits'] = {'limitId': 'returned-legacy', 'limitName': 'Synthetic legacy bucket',
                            'primary': {'usedPercent': 42, 'windowDurationMins': 1440,
                                        'resetsAt': None}, 'secondary': None}
elif selected == 'empty':
    result['rateLimitsByLimitId'] = {}
    result['rateLimits'] = {'invalid': 'authoritative empty map must not resurrect legacy'}
elif selected == 'missing':
    result = {'accountId': 'synthetic-private-account-id'}
elif selected in {'invalid', 'fractional', 'string_percent'}:
    primary['usedPercent'] = {'invalid': 101, 'fractional': 25.5, 'string_percent': '25'}[selected]
elif selected == 'mismatch':
    codex['limitId'] = 'different-bucket'
elif selected == 'null_windows':
    result['rateLimitsByLimitId'] = {'codex': {'primary': None, 'secondary': None}}
send({'id': 3, 'result': result})
observe('full-quota-emitted', mode=selected)
assert read_request() == {'id': 4, 'method': 'account/read', 'params': {'refreshToken': False}}
if initial_mode == 'late_auth':
    notice('account/updated', {'authMode': 'chatgpt'})
account(4, final=True)
observe('final-account-emitted')
while sys.stdin.readline():
    raise AssertionError('No login, inference, credits, or additional operations are allowed.')
