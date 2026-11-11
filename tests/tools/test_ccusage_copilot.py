"""Run the pinned Copilot scanner on private synthetic input."""
import copy
import json
import os
from pathlib import Path
import sys
import unittest

from local_log_qualification import KINDS, collect, pinned_binary


GOLDEN = Path(__file__).parent / 'fixtures/copilot/qualification.json'
CHAT = 'session-state/session-a/events.jsonl'
OTEL = 'otel/trace.jsonl'
TIME = '2026-05-28T20:26:40.000Z'
USAGE = {'inputTokens': 100, 'outputTokens': 20, 'cacheReadTokens': 10,
         'cacheWriteTokens': 5, 'reasoningTokens': 7}
BASE = {'type': 'session.shutdown', 'id': '11111111-1111-4111-8111-111111111111',
        'parentId': None, 'timestamp': TIME,
        'data': {'modelMetrics': {'gpt-5.4': {'usage': USAGE, 'requests': {'count': 1}}},
                 'codeChanges': {'filesModified': [], 'linesAdded': 0, 'linesRemoved': 0},
                 'sessionStartTime': 1780000000000, 'shutdownType': 'routine', 'totalApiDurationMs': 0}}
SPAN = {'type': 'span', 'name': 'chat gpt-5.4', 'traceId': '1' * 32, 'spanId': '2' * 16,
        'startTime': [1780000000, 0], 'endTime': [1780000000, 0],
        'attributes': {'gen_ai.operation.name': 'chat', 'gen_ai.request.model': 'gpt-5.4',
                       'gen_ai.conversation.id': 'session-a', 'gen_ai.response.id': 'response-a',
                       'gen_ai.usage.input_tokens': 100, 'gen_ai.usage.output_tokens': 20,
                       'gen_ai.usage.cache_read.input_tokens': 10,
                       'gen_ai.usage.cache_write.input_tokens': 5,
                       'gen_ai.usage.reasoning.output_tokens': 7, 'gen_ai.usage.total_tokens': 120}}


def cases():
    def shutdown(**values):
        value = copy.deepcopy(BASE)
        value['data']['modelMetrics']['gpt-5.4']['usage'].update(values)
        return value

    def span(**values):
        return SPAN | {'attributes': SPAN['attributes'] | values}

    def lines(*events):
        return ''.join(json.dumps(event) + '\n' for event in events).encode()

    def single(*events, path=CHAT):
        return {'first': {path: lines(*events)}}

    def pair(first, second):
        return {'first': {CHAT: lines(*first), CHAT.replace('session-a', 'second'): lines(*second)}}

    def case(roots, total, *, dated=False, rows=None, fields=None):
        return {'roots': roots, 'required': {'daily': total if dated else None, 'session': total},
                'rows': rows or {}, 'fields': fields or {}}

    def missing_usage(key):
        value = copy.deepcopy(BASE)
        value['data']['modelMetrics']['gpt-5.4']['usage'].pop(key)
        return value

    growth = shutdown(inputTokens=112, outputTokens=25) | {
        'id': '33333333-3333-4333-8333-333333333333', 'parentId': BASE['id'],
        'timestamp': '2026-05-29T20:26:40.000Z'}
    growth['data']['modelMetrics']['gpt-5.4']['requests']['count'] = 2
    conflict = shutdown(inputTokens=117)
    independent = copy.deepcopy(BASE) | {'id': '44444444-4444-4444-8444-444444444444'}
    start = {'type': 'session.start', 'id': '55555555-5555-4555-8555-555555555555',
             'parentId': None, 'timestamp': TIME, 'data': {'sessionId': 'session-a',
             'producer': 'copilot-agent', 'copilotVersion': '1.0.109', 'version': 1, 'startTime': TIME}}
    snapshot = {'modelMetrics': BASE['data']['modelMetrics'], 'codeChanges': BASE['data']['codeChanges'],
                'sessionStartTime': TIME, 'lastCallInputTokens': 100, 'lastCallOutputTokens': 20,
                'totalApiDurationMs': 0, 'totalPremiumRequestCost': 0, 'totalUserRequests': 1,
                'totalNanoAiu': 0, 'aiCreditsStatus': 'unavailable'}
    checkpoint = {'type': 'session.usage_checkpoint', 'id': '66666666-6666-4666-8666-666666666666',
                  'parentId': start['id'], 'timestamp': TIME, 'data': {'totalNanoAiu': 0,
                  'accountingSnapshot': snapshot, 'usageAccountingWatermarks': {'session-a': 1}}}
    link = {'type': 'session.journal_link', 'id': '77777777-7777-4777-8777-777777777777',
            'parentId': None, 'timestamp': TIME,
            'data': {'previousFile': 'events.1.jsonl', 'sessionId': 'session-a', 'version': 1}}
    no_time = {k: v for k, v in BASE.items() if k != 'timestamp'}
    no_model = copy.deepcopy(BASE)
    no_model['data']['modelMetrics'] = {'unknown-synthetic-model': BASE['data']['modelMetrics']['gpt-5.4']}
    no_requests = copy.deepcopy(BASE)
    no_requests['data']['modelMetrics']['gpt-5.4'].pop('requests')
    different_span = SPAN | {'spanId': '3' * 16,
        'attributes': SPAN['attributes'] | {'gen_ai.response.id': 'response-b'}}
    otel_conflict = span(**{'gen_ai.usage.input_tokens': 117, 'gen_ai.usage.total_tokens': 137})
    return {
        'state_single': case(single(BASE), 120),
        'state_header_and_shutdown': case(single(start, BASE), 120),
        'state_replay': case(single(BASE, BASE), 120),
        'state_renamed_copy': case(pair([BASE], [BASE]), 120),
        'state_fork_with_new_usage': case(pair([BASE], [BASE, growth]), 137),
        'state_independent_sessions': case(pair([BASE], [independent]), 240),
        'state_conflict': case(single(BASE, conflict), None),
        'state_conflict_reversed': case(single(conflict, BASE), None),
        'state_cumulative_growth': case(single(BASE, growth), 137),
        'state_cumulative_reversed': case(single(growth, BASE), 137),
        'state_cumulative_replay': case(single(BASE, growth, growth), 137),
        'state_measured_zero': case(single(shutdown(**dict.fromkeys(USAGE, 0))), 0, rows={'session': 1}),
        'state_missing_input': case(single(missing_usage('inputTokens')), None),
        'state_missing_output': case(single(missing_usage('outputTokens')), None),
        'state_missing_cache_read': case(single(missing_usage('cacheReadTokens')), 120,
                                        fields={'inputTokens': None, 'cacheReadTokens': None}),
        'state_missing_reasoning': case(single(missing_usage('reasoningTokens')), 120,
                                       fields={'reasoningTokens': None}),
        'state_missing_requests': case(single(no_requests), 120),
        'state_missing_time': case(single(no_time), 120),
        'state_unpriced_model': case(single(no_model), 120, fields={'totalCost': None}),
        'state_negative_count': case(single(shutdown(inputTokens=-1)), None),
        'state_fractional_count': case(single(shutdown(inputTokens=100.5)), None),
        'state_text_count': case(single(shutdown(inputTokens='100')), None),
        'state_cache_exceeds_input': case(single(shutdown(cacheReadTokens=101)), None),
        'state_reasoning_exceeds_output': case(single(shutdown(reasoningTokens=21)), None),
        'state_malformed_line': case({'first': {CHAT: lines(BASE) + b'{\n'}}, None),
        'state_malformed_peer': case({'first': {CHAT: lines(BASE),
                                              CHAT.replace('session-a', 'broken'): b'{\n'}}, None),
        'state_invalid_utf8_peer': case({'first': {CHAT: lines(BASE),
                                                 CHAT.replace('session-a', 'broken'): b'\xff\xfe'}}, None),
        'state_missing_root': case({'first': None}, None),
        'state_empty': case(single(), 0, dated=True, rows=dict.fromkeys(KINDS, 0)),
        'state_header_only': case(single(start), 0, dated=True, rows=dict.fromkeys(KINDS, 0)),
        'state_checkpoint_tokens': case(single(start, checkpoint), 120),
        'state_linked_checkpoint': case({'first': {CHAT: lines(link, checkpoint | {'checkpoint': True}),
                                                  CHAT.replace('events.jsonl', 'events.1.jsonl'): lines(start, BASE)}}, 120),
        'state_missing_archive': case(single(link, BASE), None),
        'state_malformed_archive': case({'first': {CHAT: lines(link, BASE),
                                                  CHAT.replace('events.jsonl', 'events.1.jsonl'): b'{\n'}}, None),
        'otel_single': case(single(SPAN, path=OTEL), 120, dated=True),
        'otel_replay': case(single(SPAN, SPAN, path=OTEL), 120, dated=True),
        'otel_copied_file': case({'first': {OTEL: lines(SPAN), 'otel/copy.jsonl': lines(SPAN)}}, 120, dated=True),
        'otel_distinct_calls_equal_counts': case(single(SPAN, different_span, path=OTEL), 240, dated=True),
        'otel_conflict': case(single(SPAN, otel_conflict, path=OTEL), None, dated=True),
        'otel_conflict_reversed': case(single(otel_conflict, SPAN, path=OTEL), None, dated=True),
        'otel_missing_counts': case(single(span(**dict.fromkeys(
            [key for key in SPAN['attributes'] if key.startswith('gen_ai.usage.')], None)), path=OTEL),
            None, dated=True),
        'otel_measured_zero': case(single(span(**dict.fromkeys(
            [key for key in SPAN['attributes'] if key.startswith('gen_ai.usage.')], 0)), path=OTEL),
            0, dated=True, rows=dict.fromkeys(KINDS, 1)),
        'otel_missing_time': case(single({k: v for k, v in SPAN.items() if k not in ('startTime', 'endTime')}, path=OTEL), 120),
        'otel_malformed_peer': case({'first': {OTEL: lines(SPAN), 'otel/broken.jsonl': b'{\n'}}, None, dated=True),
        'mixed_same_call': case({'first': {CHAT: lines(BASE), OTEL: lines(SPAN)}}, 120),
        'mixed_new_call_after_shutdown': case({'first': {CHAT: lines(BASE),
            OTEL: lines(SPAN | {'startTime': [1780000001, 0], 'endTime': [1780000002, 0]})}}, 240),
        'mixed_partial_otel': case({'first': {CHAT: lines(growth), OTEL: lines(SPAN)}}, 137),
        'mixed_conflicting_totals': case({'first': {CHAT: lines(BASE), OTEL: lines(otel_conflict)}}, None),
    }


class CopilotQualificationTests(unittest.TestCase):
    def setUp(self):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected or sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            self.skipTest('Set OPENUSAGE_CCUSAGE_BIN on Linux with Bubblewrap.')
        self.binary = pinned_binary(selected)

    def test_pinned_outcomes_and_limits(self):
        selected = cases()
        self.assertTrue(all(set(spec['roots']) == {'first'} for spec in selected.values()))
        observed = collect(self.binary, selected, 'copilot', 'COPILOT_HOME', CHAT)
        self.assertEqual(observed, json.loads(GOLDEN.read_text()))
        self.assertEqual(observed['state_renamed_copy']['reports']['session']['totals']['totalTokens'], 240)
        self.assertFalse(observed['state_malformed_peer']['reports']['session']['matchesRequired'])
        self.assertFalse(observed['state_checkpoint_tokens']['reports']['session']['matchesRequired'])


if __name__ == '__main__':
    unittest.main()
