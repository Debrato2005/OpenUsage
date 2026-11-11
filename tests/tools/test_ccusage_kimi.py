"""Record pinned Kimi wire report outcomes on private synthetic fixtures."""
import json
import os
from pathlib import Path
import sys
import unittest

from local_log_qualification import KINDS, collect, pinned_binary


GOLDEN = Path(__file__).parent / 'fixtures/kimi/qualification.json'
OLD = 'sessions/workspace/session-a/wire.jsonl'
NEW = 'sessions/workspace/session-a/agents/agent-a/wire.jsonl'
COUNTS = {'input_other': 100, 'output': 20, 'input_cache_creation': 5,
          'input_cache_read': 10, 'total': 135}
OLD_RECORD = {'timestamp': 1780000000, 'message': {'type': 'StatusUpdate',
              'payload': {'message_id': 'message-a', 'token_usage': COUNTS}}}
NEW_RECORD = {'type': 'usage.record', 'usageScope': 'turn', 'agentId': 'agent-a',
              'time': 1780000000000, 'model': 'kimi-code/kimi-k2.6',
              'usage': {'inputOther': 100, 'output': 20, 'inputCacheCreation': 5,
                        'inputCacheRead': 10}}


def cases():
    def old(*, message_id='message-a', counts=COUNTS, **values):
        return OLD_RECORD | {'message': {'type': 'StatusUpdate',
                             'payload': {'message_id': message_id, 'token_usage': counts}}} | values

    def lines(*records):
        return ''.join(json.dumps(record) + '\n' for record in records).encode()

    def single(*records, path=OLD):
        return {'first': {path: lines(*records)}}

    def peers(first, second, path=OLD):
        return {'first': {path: lines(*first)}, 'second': {path: lines(*second)}}

    def case(roots, total, *, undated=False, rows=None, fields=None):
        return {'roots': roots, 'required': {'daily': None if undated else total, 'session': total},
                'rows': rows or {}, 'fields': fields or {}}

    conflict = old(counts=COUNTS | {'input_other': 117, 'total': 152})
    extra = old(message_id='message-new', counts={'input_other': 12, 'output': 5,
                'input_cache_creation': 0, 'input_cache_read': 0, 'total': 17})
    return {
        'old_single': case(single(OLD_RECORD), 135),
        'old_copied_root': case(peers([OLD_RECORD], [OLD_RECORD]), 135),
        'old_replay': case(single(OLD_RECORD, OLD_RECORD), 135),
        'old_distinct_messages_equal_values': case(single(OLD_RECORD, old(message_id='message-b')), 270),
        'old_same_id_changed_time': case(peers([OLD_RECORD], [old(timestamp=1780000001)]), None),
        'old_conflicting_counters': case(peers([OLD_RECORD], [conflict]), None),
        'old_conflicting_counters_reversed': case(peers([conflict], [OLD_RECORD]), None),
        'old_renamed_session_copy': case({'first': {OLD: lines(OLD_RECORD)},
                                         'second': {OLD.replace('session-a', 'fork'): lines(OLD_RECORD)}}, 135),
        'old_fork_with_new_call': case({'first': {OLD: lines(OLD_RECORD)},
                                      'second': {OLD.replace('session-a', 'fork'): lines(OLD_RECORD, extra)}}, 152),
        'old_compaction_notice': case(single(OLD_RECORD, {'timestamp': 1780000001,
                                      'message': {'type': 'CompactionBegin', 'payload': {}}}), 135),
        'old_missing_message_id': case(peers([old(message_id=None)], [old(message_id=None)]), None),
        'old_total_only': case(single(old(counts={'total': 135})), 135, fields={'outputTokens': None}),
        'old_missing_time': case(single({k: v for k, v in OLD_RECORD.items() if k != 'timestamp'}), 135, undated=True),
        'old_default_model': case(single(OLD_RECORD), 135, fields={'totalCost': None}),
        'old_negative_count': case(single(old(counts=COUNTS | {'input_other': -1})), None),
        'old_fractional_count': case(single(old(counts=COUNTS | {'input_other': 100.5})), None),
        'old_text_count': case(single(old(counts=COUNTS | {'input_other': '100'})), None),
        'old_contradictory_total': case(single(old(counts=COUNTS | {'total': 1})), None),
        'old_explicit_zero': case(single(old(counts=dict.fromkeys(COUNTS, 0))), 0,
                                  rows=dict.fromkeys(KINDS, 1)),
        'old_missing_counts': case(single(old(counts={})), None),
        'new_single': case(single(NEW_RECORD, path=NEW), 135),
        'new_copied_root': case(peers([NEW_RECORD], [NEW_RECORD], path=NEW), 135),
        'new_replay': case(single(NEW_RECORD, NEW_RECORD, path=NEW), 135),
        'new_distinct_calls_equal_values': case(single(NEW_RECORD, NEW_RECORD, path=NEW), 270),
        'new_distinct_agents_equal_values': case({'first': {NEW: lines(NEW_RECORD),
                                         NEW.replace('agent-a', 'agent-b'): lines(NEW_RECORD | {'agentId': 'agent-b'})}}, 270),
        'new_renamed_session_copy': case({'first': {NEW: lines(NEW_RECORD)},
                                         'second': {NEW.replace('session-a', 'fork'): lines(NEW_RECORD)}}, 135),
        'new_turn_and_session_requests': case(single(NEW_RECORD, NEW_RECORD | {'usageScope': 'session'}, path=NEW), 270),
        'new_session_request': case(single(NEW_RECORD | {'usageScope': 'session'}, path=NEW), 135),
        'new_prediction_request': case(single(NEW_RECORD | {'usageScope': 'prediction'}, path=NEW), 135),
        'new_missing_cache_count': case(single(NEW_RECORD | {'usage': {'inputOther': 100, 'output': 20,
                                                                     'inputCacheCreation': 5}}, path=NEW), None),
        'new_explicit_zero': case(single(NEW_RECORD | {'usage': dict.fromkeys(NEW_RECORD['usage'], 0)}, path=NEW), 0,
                                  rows=dict.fromkeys(KINDS, 1)),
        'new_missing_time': case(single({k: v for k, v in NEW_RECORD.items() if k != 'time'}, path=NEW), 135, undated=True),
        'new_missing_model': case(single({k: v for k, v in NEW_RECORD.items() if k != 'model'}, path=NEW), 135,
                                 fields={'totalCost': None}),
        'malformed_peer': case({'first': {OLD: lines(OLD_RECORD)}, 'second': {OLD: b'{\n'}}, None),
        'malformed_line': case({'first': {OLD: lines(OLD_RECORD) + b'{\n'}}, None),
        'invalid_utf8_peer': case({'first': {OLD: lines(OLD_RECORD)}, 'second': {OLD: b'\xff\xfe'}}, None),
        'missing_root': case(single(OLD_RECORD) | {'missing': None}, None),
        'empty_wire': case(single(), 0, rows=dict.fromkeys(KINDS, 0)),
    }


class KimiQualificationTests(unittest.TestCase):
    def test_pinned_outcomes_and_limits(self):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected or sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            self.skipTest('Set OPENUSAGE_CCUSAGE_BIN on Linux with Bubblewrap.')
        observed = collect(pinned_binary(selected), cases(), 'kimi', 'KIMI_DATA_DIR', OLD)
        self.assertEqual(observed, json.loads(GOLDEN.read_text()))
        self.assertEqual(observed['old_fork_with_new_call']['reports']['session']['totals']['totalTokens'], 287)
        self.assertEqual(observed['new_distinct_calls_equal_values']['reports']['session']['totals']['totalTokens'], 135)
        self.assertEqual(observed['new_distinct_calls_equal_values']['reports']['session']['sha256'],
                         observed['new_replay']['reports']['session']['sha256'])


if __name__ == '__main__':
    unittest.main()
