"""Record the pinned Droid scanner's cumulative snapshot behavior."""
import json
import os
from pathlib import Path
import sys
import unittest

from local_log_qualification import KINDS, collect, pinned_binary
from test_ccusage_formats import FIXTURES


GOLDEN = Path(__file__).parent / 'fixtures/droid/qualification.json'
SETTINGS = 'session-z.settings.json'


def cases():
    inputs = json.loads((FIXTURES / 'inputs.json').read_text())
    base = next(iter(inputs['droid']['files'].values()))
    usage = base['tokenUsage']

    def with_usage(**fields):
        return dict(base, tokenUsage=usage | fields)

    def without(record, key):
        return {name: value for name, value in record.items() if name != key}

    def case(roots, session, *, rows=None, fields=None, daily=None):
        return {'roots': roots, 'required': {'daily': daily, 'session': session},
                'rows': rows or {}, 'fields': fields or {}}

    def single(record, **other):
        return {'first': {SETTINGS: record, **other}}

    def peers(first, second):
        return {'first': {SETTINGS: first}, 'second': {SETTINGS: second}}

    conflicting = with_usage(inputTokens=137, totalTokens=190)
    later = dict(conflicting, providerLockTimestamp='2026-11-09T03:04:05.006Z')
    lower = dict(with_usage(inputTokens=17, totalTokens=70),
                 providerLockTimestamp='2026-11-09T03:04:05.006Z')
    zero = dict(base, tokenUsage=dict.fromkeys(usage, 0))
    missing_model = without(base, 'model')
    return {
        'single': case(single(base), 173),
        'same_name_copy': case(peers(base, base), 173),
        'exact_snapshot_replay': case(peers(base, base) | {'third': {SETTINGS: base}}, 173),
        'renamed_copy': case({'first': {SETTINGS: base, 'renamed.settings.json': base}}, 173),
        'synthetic_fork_with_new_usage': case({'first': {SETTINGS: base, 'fork.settings.json': later}}, 190),
        'distinct_sessions_equal_counts': case({'first': {SETTINGS: base, 'independent.settings.json': base}}, 346),
        'distinct_sessions_same_filename': case(peers(base, base), 346),
        'conflicting_copy': case(peers(base, conflicting), None),
        'conflicting_copy_reversed': case(peers(conflicting, base), None),
        'later_lock_is_not_a_revision': case(peers(base, later), None),
        'lower_snapshot_is_not_a_reset': case(peers(base, lower), None),
        'zero_peer_is_not_absence': case(peers(base, zero), None),
        'unknown_total_peer_is_not_absence': case(peers(base, without(base, 'tokenUsage')), None),
        'missing_lock_time': case(single(without(base, 'providerLockTimestamp')), 173),
        'invalid_lock_time': case(single(dict(base, providerLockTimestamp='unknown')), 173),
        'empty': case({'first': {}}, 0, daily=0, rows=dict.fromkeys(KINDS, 0)),
        'measured_zero': case(single(zero), 0, rows={'session': 1}),
        'total_only': case(single(dict(base, tokenUsage={'totalTokens': 173})), 173,
                           fields={'outputTokens': None}),
        'independent_remainder': case(single(with_usage(totalTokens=190)), 190,
                                     fields={'reasoningTokens': None}),
        'negative_count': case(single(with_usage(inputTokens=-1)), None),
        'fractional_count': case(single(with_usage(inputTokens=1.5)), None),
        'contradictory_total': case(single(with_usage(totalTokens=1)), None),
        'malformed_peer': case(peers(base, b'{'), None),
        'invalid_utf8_peer': case(peers(base, b'\xff\xfe'), None),
        'missing_root': case({'first': {SETTINGS: base}, 'missing': None}, None),
        'missing_model': case(single(missing_model), 173, fields={'totalCost': None}),
        'textual_model_hint': case(single(missing_model, **{'session-z.jsonl': b'Model: gpt-5\n'}), 173,
                                   fields={'totalCost': None}),
        'unknown_model': case(single(dict(base, model='synthetic-model-not-priced')), 173,
                              fields={'totalCost': None}),
    }


class DroidQualificationTests(unittest.TestCase):
    def test_pinned_outcomes_and_limits(self):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_BIN')
        if not selected or sys.platform != 'linux' or not Path('/usr/bin/bwrap').is_file():
            self.skipTest('Set OPENUSAGE_CCUSAGE_BIN on Linux with Bubblewrap.')
        observed = collect(pinned_binary(selected), cases(), 'droid', 'DROID_SESSIONS_DIR', SETTINGS)
        self.assertEqual(observed, json.loads(GOLDEN.read_text()))
        self.assertEqual(observed['conflicting_copy']['reports']['session']['totals']['totalTokens'], 190)
        self.assertEqual(observed['conflicting_copy_reversed']['reports']['session']['totals']['totalTokens'], 173)
        self.assertEqual(observed['same_name_copy']['reports']['session']['sha256'],
                         observed['distinct_sessions_same_filename']['reports']['session']['sha256'])


if __name__ == '__main__':
    unittest.main()
