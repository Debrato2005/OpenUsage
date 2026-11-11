"""Record pinned Gemini JSON and JSONL report outcomes on private fixtures."""
import json
import os
from pathlib import Path
import sys
import unittest

from local_log_qualification import KINDS, collect, pinned_binary
from test_ccusage_formats import FIXTURES


GOLDEN = Path(__file__).parent / 'fixtures/gemini/qualification.json'
CHAT = 'project/chats/session-a.json'
CHAT_LINES = 'project/chats/session-a.jsonl'


def cases():
    inputs = json.loads((FIXTURES / 'inputs.json').read_text())['gemini']
    base = inputs['files'][CHAT]
    message = base['messages'][0]
    tokens = message['tokens']
    header = {key: value for key, value in base.items() if key != 'messages'}
    line_header = header | {'projectHash': 'synthetic-project', 'lastUpdated': header['startTime']}

    def message_with_tokens(**values):
        return dict(message, tokens=tokens | values)

    def chat(*messages, **values):
        return header | {'messages': list(messages)} | values

    def lines(*records):
        return ''.join(json.dumps(record) + '\n' for record in records).encode()

    def case(roots, total, *, undated=False, rows=None, fields=None):
        return {'roots': roots, 'required': {'daily': None if undated else total, 'session': total},
                'rows': rows or {}, 'fields': fields or {}}

    def single(value, path=CHAT):
        return {'first': {path: value}}

    def peers(first, second, path=CHAT):
        return {'first': {path: first}, 'second': {path: second}}

    conflict = message_with_tokens(input=117, total=142)
    extra = dict(message, id='message-new', tokens={'input': 12, 'output': 5, 'cached': 0,
                                                    'thoughts': 0, 'tool': 0, 'total': 17})
    summary = {'id': 'summary-a', 'type': 'info', 'content': 'Synthetic summary'}
    stats = {'sessionId': base['sessionId'], 'timestamp': message['timestamp'],
             'stats': {'models': {message['model']: {'tokens': tokens}}}}
    undated = {key: value for key, value in message.items() if key != 'timestamp'}
    no_id = {key: value for key, value in message.items() if key != 'id'}
    no_model = {key: value for key, value in message.items() if key != 'model'}
    return {
        'single_json': case(single(base), 125),
        'inclusive_cache_and_tool_prompt': case(single(chat(message_with_tokens(tool=7, total=132))), 132,
                                                fields={'inputTokens': 97, 'cacheReadTokens': 10}),
        'defaulted_provider_total': case(single(chat(message_with_tokens(total=0))), None),
        'json_copy': case(peers(base, base), 125),
        'json_replay': case(single(chat(message, message)), 125),
        'json_distinct_calls': case(single(chat(message, dict(message, id='distinct'))), 250),
        'json_fork_with_new_call': case(peers(base, chat(message, extra, sessionId='fork')), 142),
        'json_conflict': case(peers(base, chat(conflict)), None),
        'json_conflict_reversed': case(peers(chat(conflict), base), None),
        'json_compaction_notice': case(single(chat(message, summary)), 125),
        'jsonl_single': case(single(lines(header, message), CHAT_LINES), 125),
        'jsonl_copy': case(peers(lines(header, message), lines(header, message), CHAT_LINES), 125),
        'jsonl_replay': case(single(lines(header, message, message), CHAT_LINES), 125),
        'jsonl_token_enrichment': case(single(lines(line_header, dict(message, tokens=None), message), CHAT_LINES), 125),
        'jsonl_usage_before_rewind': case(single(lines(line_header, message, extra, {'$rewindTo': extra['id']}), CHAT_LINES), 142),
        'jsonl_usage_before_patch_removal': case(single(lines(line_header, message, extra, {'$patch': {'removeIds': [extra['id']]}}), CHAT_LINES), 142),
        'jsonl_invalid_patch': case(single(lines(line_header, message, {'$patch': {'removeIds': False}}), CHAT_LINES), None),
        'jsonl_legacy_checkpoint': case(single(lines(line_header, {'$set': {'messages': [message]}}), CHAT_LINES), 125),
        'jsonl_conflict': case(single(lines(header, message, conflict), CHAT_LINES), None),
        'jsonl_conflict_reversed': case(single(lines(header, conflict, message), CHAT_LINES), None),
        'json_and_jsonl_copy': case({'first': {CHAT: base, CHAT_LINES: lines(header, message)}}, 125),
        'stats_only': case(single(stats), 125),
        'stats_with_overlapping_messages': case({'first': {CHAT: base, 'project/stats.json': stats}}, None),
        'explicit_zero': case(single(chat(message_with_tokens(**dict.fromkeys(tokens, 0)))), 0,
                              rows=dict.fromkeys(KINDS, 1)),
        'total_only': case(single(chat(dict(message, tokens={'total': 125}))), 125,
                           fields={'outputTokens': None}),
        'missing_model': case(single(chat(no_model)), 125, fields={'totalCost': None}),
        'missing_usage_time': case(single(chat(undated)), 125, undated=True),
        'missing_message_id': case(peers(chat(no_id), chat(no_id)), None),
        'negative_count': case(single(chat(message_with_tokens(input=-1))), None),
        'fractional_count': case(single(chat(message_with_tokens(input=100.5))), None),
        'contradictory_total': case(single(chat(message_with_tokens(total=1))), None),
        'malformed_json_peer': case(peers(base, b'{'), None),
        'malformed_jsonl_line': case(single(lines(header, message) + b'{\n', CHAT_LINES), None),
        'invalid_utf8_peer': case(peers(base, b'\xff\xfe'), None),
        'missing_root': case(single(base) | {'missing': None}, None),
        'empty_chat': case(single(chat()), 0, rows=dict.fromkeys(KINDS, 0)),
    }


class GeminiQualificationTests(unittest.TestCase):
    def test_pinned_outcomes_and_limits(self):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected or sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            self.skipTest('Set OPENUSAGE_CCUSAGE_BIN on Linux with Bubblewrap.')
        observed = collect(pinned_binary(selected), cases(), 'gemini', 'GEMINI_DATA_DIR', CHAT)
        self.assertEqual(observed, json.loads(GOLDEN.read_text()))
        self.assertEqual(observed['json_copy']['reports']['session']['totals']['totalTokens'], 250)
        self.assertEqual(observed['jsonl_conflict']['reports']['session']['totals']['totalTokens'], 142)
        self.assertEqual(observed['jsonl_conflict_reversed']['reports']['session']['totals']['totalTokens'], 125)


if __name__ == '__main__':
    unittest.main()
