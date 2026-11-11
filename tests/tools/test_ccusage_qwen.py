"""Record pinned Qwen report outcomes on private synthetic fixtures."""
import json
import os
from pathlib import Path
import sys
import unittest

from local_log_qualification import KINDS, collect, pinned_binary


GOLDEN = Path(__file__).parent / 'fixtures/qwen/qualification.json'
CHAT = 'projects/project-a/chats/session-a.jsonl'
BASE = {'type': 'assistant', 'uuid': 'call-a', 'parentUuid': None,
        'sessionId': 'session-a', 'timestamp': '2026-11-04T09:00:00Z',
        'model': 'qwen3-coder', 'message': {'role': 'model', 'parts': [{'text': 'Synthetic'}]},
        'usageMetadata': {'promptTokenCount': 100, 'candidatesTokenCount': 20,
                          'cachedContentTokenCount': 0, 'thoughtsTokenCount': 0,
                          'totalTokenCount': 120}}


def cases():
    def record(**values):
        return BASE | values

    def usage(**values):
        return record(usageMetadata=BASE['usageMetadata'] | values)

    def lines(*records):
        return ''.join(json.dumps(value) + '\n' for value in records).encode()

    def single(*records, path=CHAT):
        return {'first': {path: lines(*records)}}

    def peers(first, second, path=CHAT):
        return {'first': {path: lines(*first)}, 'second': {path: lines(*second)}}

    def case(roots, total, *, undated=False, rows=None, fields=None):
        return {'roots': roots, 'required': {'daily': None if undated else total, 'session': total},
                'rows': rows or {}, 'fields': fields or {}}

    conflict = usage(promptTokenCount=117, totalTokenCount=137)
    extra = record(uuid='call-new', parentUuid='call-a',
                   usageMetadata={'promptTokenCount': 12, 'candidatesTokenCount': 5,
                                  'cachedContentTokenCount': 0, 'thoughtsTokenCount': 0,
                                  'totalTokenCount': 17})
    no_time = {key: value for key, value in BASE.items() if key != 'timestamp'}
    no_session = {key: value for key, value in BASE.items() if key != 'sessionId'}
    no_model = {key: value for key, value in BASE.items() if key != 'model'}
    no_uuid = {key: value for key, value in BASE.items() if key != 'uuid'}
    return {
        'single': case(single(BASE), 120),
        'copied_root': case(peers([BASE], [BASE]), 120),
        'renamed_copy': case({'first': {CHAT: lines(BASE),
                                       CHAT.replace('session-a', 'renamed'): lines(BASE)}}, 120),
        'replayed_record': case(single(BASE, BASE), 120),
        'distinct_calls_equal_values_and_time': case(single(BASE, record(uuid='call-b')), 240),
        'distinct_calls_later_time': case(single(BASE, record(uuid='call-b', timestamp='2026-11-04T09:00:01Z')), 240),
        'same_call_changed_time': case(peers([BASE], [record(timestamp='2026-11-04T09:00:01Z')]), None),
        'same_call_changed_session': case(peers([BASE], [record(sessionId='fork')]), 120),
        'inherited_fork_with_new_call': case(peers([BASE], [record(sessionId='fork'), extra | {'sessionId': 'fork'}]), 137),
        'conflicting_counters': case(peers([BASE], [conflict]), None),
        'conflicting_counters_reversed': case(peers([conflict], [BASE]), None),
        'changed_model_same_call': case(peers([BASE], [record(model='qwen-plus')]), None),
        'token_enrichment': case(single(record(usageMetadata=None), BASE), 120),
        'ordered_usage_replacement': case(single(BASE, conflict), 137),
        'compaction_notice': case(single(BASE, {'type': 'system', 'subtype': 'compaction', 'uuid': 'summary'}), 120),
        'inclusive_cache': case(single(usage(cachedContentTokenCount=10)), 120,
                                fields={'inputTokens': 90, 'cacheReadTokens': 10}),
        'thoughts_already_in_output': case(single(usage(thoughtsTokenCount=5)), 120),
        'defaulted_provider_total': case(single(usage(totalTokenCount=0)), None),
        'explicit_zero': case(single(record(usageMetadata=dict.fromkeys(BASE['usageMetadata'], 0))), 0,
                              rows=dict.fromkeys(KINDS, 1)),
        'missing_usage': case(single(record(usageMetadata=None)), 0, rows=dict.fromkeys(KINDS, 0)),
        'total_only': case(single(record(usageMetadata={'totalTokenCount': 120})), 120,
                           fields={'outputTokens': None}),
        'missing_model': case(single(no_model), 120, fields={'totalCost': None}),
        'missing_usage_time': case(single(no_time), 120, undated=True),
        'missing_session': case(single(no_session), 120),
        'missing_id_ambiguous_copy': case(peers([no_uuid], [no_uuid]), None),
        'negative_count': case(single(usage(promptTokenCount=-1)), None),
        'fractional_count': case(single(usage(promptTokenCount=100.5)), None),
        'text_count': case(single(usage(promptTokenCount='100')), None),
        'contradictory_total': case(single(usage(totalTokenCount=1)), None),
        'malformed_peer': case({'first': {CHAT: lines(BASE)}, 'second': {CHAT: b'{\n'}}, None),
        'malformed_line': case({'first': {CHAT: lines(BASE) + b'{\n'}}, None),
        'invalid_utf8_peer': case({'first': {CHAT: lines(BASE)}, 'second': {CHAT: b'\xff\xfe'}}, None),
        'missing_root': case(single(BASE) | {'missing': None}, None),
        'empty_chat': case(single(), 0, rows=dict.fromkeys(KINDS, 0)),
    }


class QwenQualificationTests(unittest.TestCase):
    def test_pinned_outcomes_and_limits(self):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected or sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            self.skipTest('Set OPENUSAGE_CCUSAGE_BIN on Linux with Bubblewrap.')
        observed = collect(pinned_binary(selected), cases(), 'qwen', 'QWEN_DATA_DIR', CHAT)
        self.assertEqual(observed, json.loads(GOLDEN.read_text()))
        self.assertEqual(observed['distinct_calls_equal_values_and_time']['reports']['session']['totals']['totalTokens'], 120)
        self.assertEqual(observed['conflicting_counters']['reports']['session']['totals']['totalTokens'], 257)


if __name__ == '__main__':
    unittest.main()
