"""Record the pinned Codebuff scanner outcomes on private synthetic input."""
import copy
import json
import os
from pathlib import Path
import sys
import unittest

from local_log_qualification import KINDS, collect, pinned_binary
from test_ccusage_formats import FIXTURES


GOLDEN = Path(__file__).parent / 'fixtures/codebuff/qualification.json'
CHAT = 'projects/project-z/chats/chat-z/chat-messages.json'
DATED_CHAT = 'projects/project-z/chats/2026-11-08T03-04-05.006Z/chat-messages.json'
BRANCH = 'projects/project-z/chats/chat-branch/chat-messages.json'


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


class CodebuffQualificationTests(unittest.TestCase):
    def test_pinned_outcomes_and_limits(self):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected or sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            self.skipTest('Set OPENUSAGE_CCUSAGE_BIN on Linux with Bubblewrap.')
        results = collect(pinned_binary(selected), cases(), 'codebuff', 'CODEBUFF_DATA_DIR', CHAT)
        self.assertEqual(results, json.loads(GOLDEN.read_text()))
        self.assertEqual(results['single']['reports']['session']['totals']['totalTokens'], 162)
        self.assertEqual(results['renamed_channel_copy']['reports']['session']['totals']['totalTokens'], 324)
        self.assertFalse(results['malformed_peer']['reports']['session']['matchesRequired'])


if __name__ == '__main__':
    unittest.main()
