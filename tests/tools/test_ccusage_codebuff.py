"""Record the pinned Codebuff scanner outcomes on private synthetic input."""
import copy
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, PIN, file_hashes
from test_ccusage_opencode_runtime import checked_run, sandbox


GOLDEN = Path(__file__).parent / 'fixtures/codebuff/qualification.json'
CHAT = 'projects/project-z/chats/chat-z/chat-messages.json'
DATED_CHAT = 'projects/project-z/chats/2026-11-08T03-04-05.006Z/chat-messages.json'
BRANCH = 'projects/project-z/chats/chat-branch/chat-messages.json'
KINDS = ('daily', 'session')


def cases():
    inputs = json.loads((FIXTURES / 'inputs.json').read_text())
    base = next(iter(inputs['codebuff']['files'].values()))[0]
    usage = base['metadata']['usage']
    conflict = copy.deepcopy(base)
    conflict['metadata']['usage'].update(inputTokens=137, totalTokens=179)
    distinct = dict(base, id='message-distinct')
    extra = copy.deepcopy(base)
    extra['id'] = 'message-new'
    extra['metadata']['usage'] = {'inputTokens': 12, 'outputTokens': 5,
                                  'cacheCreationInputTokens': 0, 'cacheReadInputTokens': 0,
                                  'totalTokens': 17}
    no_id = {key: value for key, value in base.items() if key != 'id'}
    no_time = {key: value for key, value in base.items() if key != 'timestamp'}

    def with_usage(value):
        return dict(base, metadata=dict(base['metadata'], usage=value))

    def case(roots, daily, session, *, rows=None, fields=None):
        return {'roots': roots, 'required': {'daily': daily, 'session': session},
                'rows': rows or {}, 'fields': fields or {}}

    def root(messages):
        return {CHAT: messages}

    def selections(messages):
        return {'first/manicode': root(messages)}

    fallback = dict(base, metadata={'model': base['metadata']['model'],
                                   'codebuff': {'usage': usage}})
    provider = {'model': base['metadata']['model'], 'usage': usage}
    history = [{'role': 'assistant', 'providerOptions': {'codebuff': provider}}]
    nested = dict(base, metadata={'runState': {'sessionState': {'mainAgentState': {
        'messageHistory': history}}}})
    cost_only = copy.deepcopy(nested)
    cost_only['metadata']['runState']['sessionState']['mainAgentState']['messageHistory'][0][
        'providerOptions']['codebuff']['usage'] = {'cost': 0.01, 'costDetails': {}}
    cost_only['credits'] = 2
    zero = with_usage(dict.fromkeys(usage, 0))
    explicit_zero = with_usage(usage | {'inputTokens': 0, 'totalTokens': 42})
    explicit_zero['metadata']['codebuff'] = {'usage': {'inputTokens': 120}}
    missing_credit = dict(cost_only)
    del missing_credit['credits']
    current_message = {'id': 'ai-1794107045006-a1', 'variant': 'ai', 'content': 'Synthetic response',
                       'blocks': [], 'timestamp': '03:04 AM', 'isComplete': True,
                       'metadata': {'allowInlineAds': True}}

    return {
        'single': case(selections([base]), 162, 162),
        'same_channel_copy': case({'first/manicode': root([base]), 'second/manicode': root([base])}, 162, 162),
        'renamed_channel_copy': case({'first/manicode': root([base]), 'second/archive': root([base])}, 162, 162),
        'copied_chat': case({'first/manicode': root([base]) | {BRANCH: [base]}}, 162, 162),
        'replay': case(selections([base, base]), 162, 162),
        'branch_with_new_call': case({'first/manicode': root([base]) | {BRANCH: [base, extra]}}, 179, 179),
        'distinct_calls': case(selections([base, distinct]), 324, 324),
        'conflict_first': case({'first/manicode': root([base]), 'second/manicode': root([conflict])}, None, None),
        'conflict_reversed': case({'second/manicode': root([base]), 'first/manicode': root([conflict])}, None, None),
        'missing_id': case({'first/manicode': root([no_id]), 'second/archive': root([no_id])}, None, None),
        'compaction_notice': case(selections([base, {'id': 'sys-1794107046000', 'variant': 'ai',
                                                     'content': 'Synthetic compaction notice'}]), 162, 162),
        'empty': case(selections([]), 0, 0, rows=dict.fromkeys(KINDS, 0)),
        'measured_zero': case(selections([zero]), 0, 0, rows=dict.fromkeys(KINDS, 1)),
        'explicit_zero_beats_fallback': case(selections([explicit_zero]), 42, 42,
                                             fields={'inputTokens': 0}),
        'reported_total_only': case(selections([with_usage({'totalTokens': 162})]), 162, 162,
                                    fields={'outputTokens': None}),
        'negative_count': case(selections([with_usage(usage | {'inputTokens': -1})]), None, None),
        'fractional_count': case(selections([with_usage(usage | {'inputTokens': 1.5})]), None, None),
        'contradictory_total': case(selections([with_usage(usage | {'totalTokens': 1})]), None, None),
        'metadata_codebuff_usage': case(selections([fallback]), 162, 162),
        'nested_provider_usage': case(selections([nested]), 162, 162),
        'credits_without_tokens': case(selections([cost_only]), None, None),
        'cost_without_tokens': case(selections([missing_credit]), None, None),
        'current_ui_message_with_credits': case({'first/manicode': {DATED_CHAT: [dict(current_message, credits=2)]}}, None, None),
        'current_ui_message_without_credits': case({'first/manicode': {DATED_CHAT: [current_message]}}, None, None),
        'chat_creation_is_not_a_usage_date': case({'first/manicode': {DATED_CHAT: [no_time]}}, None, 162),
        'mtime_is_not_a_usage_date': case(selections([no_time]), None, 162),
        'malformed_peer': case({'first/manicode': root([base]), 'second/manicode': root(b'[{')}, None, None),
        'invalid_utf8_peer': case({'first/manicode': root([base]), 'second/manicode': root(b'\xff\xfe')}, None, None),
        'missing_root': case({'first/manicode': root([base]), 'missing/manicode': None}, None, None),
        'unexpected_layout': case({'first/manicode': {'projects/extra/chat-messages.json': [base]}}, None, None),
    }


def report(binary, root, names, kind):
    with tempfile.TemporaryDirectory(dir=root, prefix='empty-home-') as temporary:
        home = Path(temporary)
        command = sandbox() + ['--ro-bind', str(binary), '/runtime/ccusage',
                               '--bind', str(home), '/home/fixture', '--setenv', 'HOME', '/home/fixture']
        selected = []
        for name in names:
            mount = '/selected/' + name
            if (root / name).exists():
                command.extend(['--ro-bind', str(root / name), mount])
            selected.append(mount)
        command.extend(['--setenv', 'CODEBUFF_DATA_DIR', ','.join(selected), '--chdir', '/home/fixture',
                        '/runtime/ccusage', 'codebuff', kind, '--json', '--offline', '--mode',
                        'calculate', '--timezone', 'UTC', '--no-color'])
        result = checked_run(command, timeout=15)
        if list(home.iterdir()):
            raise ValueError('The scanner changed the private home.')
        return result


def collect(binary):
    observations = {}
    for name, spec in cases().items():
        with tempfile.TemporaryDirectory(prefix='openusage-codebuff-') as temporary:
            root = Path(temporary)
            canary = root / 'not-selected' / CHAT
            canary.parent.mkdir(parents=True)
            canary.write_text('[{"id":"private-marker"}]\n')
            for selected, files in spec['roots'].items():
                for relative, value in (files or {}).items():
                    path = root / selected / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(value if isinstance(value, bytes) else json.dumps(value).encode() + b'\n')
                    os.utime(path, (1767312000, 1767312000))
            before = file_hashes(root)
            reports = {}
            for kind in KINDS:
                first, second = [report(binary, root, spec['roots'], kind) for _ in range(2)]
                if (first.returncode, first.stdout, first.stderr) != (second.returncode, second.stdout, second.stderr):
                    raise ValueError(f'Nonrepeatable report: {name}, {kind}.')
                if first.returncode:
                    if first.stdout or b'private-marker' in first.stderr:
                        raise ValueError(f'Refusal exposed source data: {name}, {kind}.')
                    reports[kind] = {'exitCode': first.returncode, 'matchesRequired': spec['required'][kind] is None}
                    continue
                output = json.loads(first.stdout)
                totals = output['totals']
                rows = output['daily' if kind == 'daily' else 'sessions']
                key = 'date' if kind == 'daily' else 'sessionId'
                required = spec['required'][kind]
                matches = required is not None and totals['totalTokens'] == required
                matches &= len(rows) == spec['rows'].get(kind, len(rows))
                matches &= all(totals.get(field) == value for field, value in spec['fields'].items())
                reports[kind] = {'totals': totals, 'rows': [[row[key], row['totalTokens']] for row in rows],
                                 'sha256': hashlib.sha256(json.dumps(output, sort_keys=True).encode()).hexdigest(),
                                 'matchesRequired': matches}
            if file_hashes(root) != before:
                raise ValueError(f'The scanner changed source files: {name}.')
            observations[name] = {'requiredTokens': spec['required'], 'requiredRows': spec['rows'],
                                  'requiredFields': spec['fields'], 'reports': reports}
    return observations


def pinned_binary(path):
    binary = Path(path).resolve(strict=True)
    pin = json.loads(PIN.read_text())
    if hashlib.sha256(binary.read_bytes()).hexdigest() != pin['executableSha256']:
        raise ValueError('The scanner digest does not match the production pin.')
    return binary


class CodebuffQualificationTests(unittest.TestCase):
    def test_pinned_outcomes_and_limits(self):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected or sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            self.skipTest('Set OPENUSAGE_CCUSAGE_BIN on Linux with Bubblewrap.')
        results = collect(pinned_binary(selected))
        self.assertEqual(results, json.loads(GOLDEN.read_text()))
        self.assertEqual(results['single']['reports']['session']['totals']['totalTokens'], 162)
        self.assertEqual(results['renamed_channel_copy']['reports']['session']['totals']['totalTokens'], 324)
        self.assertFalse(results['malformed_peer']['reports']['session']['matchesRequired'])


if __name__ == '__main__':
    unittest.main()
