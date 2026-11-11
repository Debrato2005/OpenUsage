"""Run the pinned Grok scanner on private synthetic input."""
import copy
import json
import os
from pathlib import Path
import sys
import unittest

from local_log_qualification import KINDS, collect, pinned_binary


GOLDEN = Path(__file__).parent / 'fixtures/grok/qualification.json'
CHAT = 'sessions/project-a/session-a/updates.jsonl'
SUMMARY = CHAT.replace('updates.jsonl', 'summary.json')
USAGE = {'inputTokens': 100, 'outputTokens': 20, 'cachedReadTokens': 40,
         'cacheCreationTokens': 5, 'reasoningTokens': 10, 'totalTokens': 120}
BASE = {'timestamp': 1780000000, 'params': {'sessionId': 'session-a',
        'update': {'sessionUpdate': 'turn_completed', 'usage': {'modelUsage': {'grok-4.5-build': USAGE}}},
        '_meta': {'eventId': 'event-a'}}}
META = {'info': {'id': 'session-a', 'cwd': '/synthetic/project'}, 'current_model_id': 'grok-4.5-build'}


def cases():
    def usage(**values):
        value = copy.deepcopy(BASE)
        value['params']['update']['usage']['modelUsage']['grok-4.5-build'].update(values)
        return value

    def lines(*events):
        return ''.join(json.dumps(event) + '\n' for event in events).encode()

    def single(*events, metadata=True):
        files = {CHAT: lines(*events)}
        if metadata:
            files[SUMMARY] = META
        return {'first': files}

    def peers(first, second):
        return {'first': {CHAT: lines(*first), SUMMARY: META,
                          CHAT.replace('session-a', 'second'): lines(*second),
                          SUMMARY.replace('session-a', 'second'): META}}

    def case(roots, total, *, dated=False, rows=None, fields=None):
        return {'roots': roots, 'required': {'daily': total if dated else None, 'session': total},
                'rows': rows or {}, 'fields': fields or {}}

    distinct = copy.deepcopy(BASE)
    distinct['params']['_meta']['eventId'] = 'event-b'
    conflict = usage(inputTokens=117, totalTokens=137)
    extra = usage(inputTokens=12, outputTokens=5, cachedReadTokens=0,
                  cacheCreationTokens=0, reasoningTokens=0, totalTokens=17)
    extra['params']['_meta']['eventId'] = 'event-new'
    no_id = copy.deepcopy(BASE)
    no_id['params'].pop('_meta')
    no_time = {k: v for k, v in BASE.items() if k != 'timestamp'}
    no_session = copy.deepcopy(BASE)
    no_session['params'].pop('sessionId')
    flat = copy.deepcopy(BASE)
    flat['params']['update']['usage'] = USAGE
    no_model = copy.deepcopy(BASE)
    no_model['params']['update']['usage']['modelUsage'] = {'unknown-synthetic-model': USAGE}
    missing_cache = copy.deepcopy(BASE)
    missing_cache['params']['update']['usage']['modelUsage']['grok-4.5-build'].pop('cachedReadTokens')
    missing_input = copy.deepcopy(BASE)
    missing_input['params']['update']['usage']['modelUsage']['grok-4.5-build'].pop('inputTokens')
    total_only = copy.deepcopy(flat)
    total_only['params']['update']['usage'] = {'totalTokens': 120}
    incomplete = copy.deepcopy(BASE)
    incomplete['params']['update']['usage']['usageIsIncomplete'] = True
    partial_cost = usage(costUsdTicks=10**15, costIsPartial=True)
    partial_cost['params']['update']['usage']['costIsPartial'] = True
    child = copy.deepcopy(extra)
    child['params']['sessionId'] = 'child'
    overlap = single(BASE)
    overlap['first'][CHAT.replace('session-a', 'child')] = lines(child)
    overlap['first'][SUMMARY.replace('session-a', 'child')] = META | {
        'info': META['info'] | {'id': 'child'}}
    writer = copy.deepcopy(BASE) | {'method': '_x.ai/session/update'}
    model = USAGE | {'modelCalls': 1, 'apiDurationMs': 0}
    writer['params']['update'].update(prompt_id='prompt-a', stop_reason='end_turn',
        usage=model | {'modelUsage': {'grok-4.5-build': model}, 'numTurns': 1})
    writer['params']['_meta']['agentTimestampMs'] = 1780000000000
    fork = copy.deepcopy(writer) | {'timestamp': 1780086400}
    fork['params']['sessionId'] = 'fork'
    fork_roots = peers([writer], [fork])
    fork_roots['first'][SUMMARY.replace('session-a', 'second')] = META | {
        'info': META['info'] | {'id': 'fork'}}
    writer_incomplete = copy.deepcopy(writer)
    writer_incomplete['params']['update']['usage']['usageIsIncomplete'] = True
    return {
        'single': case(single(BASE), 120),
        'replay': case(single(BASE, BASE), 120),
        'copied_file': case(peers([BASE], [BASE]), 120),
        'inherited_fork_with_new_call': case(peers([BASE], [BASE, extra]), 137),
        'distinct_ids_equal_counts': case(single(BASE, distinct), 240),
        'distinct_ids_in_separate_files': case(peers([BASE], [distinct]), 240),
        'same_id_conflict': case(single(BASE, conflict), None),
        'same_id_conflict_reversed': case(single(conflict, BASE), None),
        'cross_file_conflict': case(peers([BASE], [conflict]), None),
        'cross_file_conflict_reversed': case(peers([conflict], [BASE]), None),
        'missing_id_single': case(single(no_id), 120),
        'missing_id_replay': case(single(no_id, no_id), 120),
        'missing_id_renamed_copy': case(peers([no_id], [no_id]), 120),
        'missing_time': case(single(no_time), 120, dated=False),
        'agent_time': case(single(BASE | {'params': BASE['params'] | {
            '_meta': {'eventId': 'event-a', 'agentTimestampMs': 1780000000000}}}), 120),
        'missing_session_with_summary': case(single(no_session), 120),
        'missing_session_and_summary': case(single(no_session, metadata=False), 120),
        'flat_usage_with_model': case(single(flat), 120),
        'flat_usage_without_model': case(single(flat, metadata=False), 120, fields={'totalCost': None}),
        'unknown_model': case(single(no_model), 120, fields={'totalCost': None}),
        'total_only': case(single(total_only), 120, fields={'inputTokens': None, 'outputTokens': None}),
        'missing_cache': case(single(missing_cache), 120, fields={'inputTokens': None, 'cacheReadTokens': None}),
        'missing_input_with_total': case(single(missing_input), 120, fields={'inputTokens': None}),
        'measured_zero': case(single(usage(**dict.fromkeys(USAGE, 0))), 0, rows=dict.fromkeys(KINDS, 1)),
        'independent_total_conflict': case(single(usage(totalTokens=1)), None),
        'cache_exceeds_input': case(single(usage(cachedReadTokens=101)), None),
        'reasoning_exceeds_output': case(single(usage(reasoningTokens=21)), None),
        'negative_count': case(single(usage(inputTokens=-1)), None),
        'fractional_count': case(single(usage(inputTokens=100.5)), None),
        'text_count': case(single(usage(inputTokens='100')), None),
        'zero_reported_cost': case(single(usage(costUsdTicks=0)), 120),
        'large_reported_cost': case(single(usage(costUsdTicks=10**15)), 120),
        'partial_reported_cost': case(single(partial_cost), 120),
        'incomplete_usage': case(single(incomplete), None),
        'unmeasured_zero_cache_write': case(single(usage(cacheCreationTokens=0)), 120,
                                           fields={'inputTokens': None, 'cacheCreationTokens': None}),
        'parent_includes_child_usage': case(overlap, 120),
        'source_envelope': case(single(writer), 120),
        'source_rewritten_fork': case(fork_roots, 120),
        'source_incomplete_usage': case(single(writer_incomplete), None),
        'unsupported_method': case(single(writer | {'method': 'unknown/update'}), None),
        'malformed_line': case({'first': {CHAT: lines(BASE) + b'{\n', SUMMARY: META}}, None),
        'malformed_peer': case({'first': {CHAT: lines(BASE),
                            CHAT.replace('session-a', 'broken'): b'{\n'}}, None),
        'invalid_utf8_peer': case({'first': {CHAT: lines(BASE),
                            CHAT.replace('session-a', 'broken'): b'\xff\xfe'}}, None),
        'missing_root': case({'first': None}, None),
        'empty': case(single(), 0, dated=True, rows=dict.fromkeys(KINDS, 0)),
        'metadata_only': case({'first': {SUMMARY: META}}, 0, dated=True, rows=dict.fromkeys(KINDS, 0)),
    }


class GrokQualificationTests(unittest.TestCase):
    def setUp(self):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected or sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            self.skipTest('Set OPENUSAGE_CCUSAGE_BIN on Linux with Bubblewrap.')
        self.binary = pinned_binary(selected)

    def test_pinned_outcomes_and_limits(self):
        selected = cases()
        self.assertTrue(all(set(spec['roots']) == {'first'} for spec in selected.values()))
        observed = collect(self.binary, selected, 'grok', 'GROK_HOME', CHAT)
        self.assertEqual(observed, json.loads(GOLDEN.read_text()))
        self.assertTrue(observed['inherited_fork_with_new_call']['reports']['session']['matchesRequired'])
        self.assertFalse(observed['same_id_conflict']['reports']['session']['matchesRequired'])
        self.assertFalse(observed['malformed_peer']['reports']['session']['matchesRequired'])
        self.assertFalse(observed['source_incomplete_usage']['reports']['session']['matchesRequired'])


if __name__ == '__main__':
    unittest.main()
