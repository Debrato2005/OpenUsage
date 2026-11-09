"""Pi qualification uses only private synthetic JSONL files."""
import copy
import json
from pathlib import Path
import shutil
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, FormatComparisons, file_hashes, make_source


TIME = '2026-05-28T20:26:40.000Z'
USAGE = {'input': 100, 'output': 20, 'cacheRead': 30, 'cacheWrite': 5, 'totalTokens': 162}


def header(identifier='root', parent=None):
    value = {'type': 'session', 'version': 3, 'id': identifier,
             'timestamp': '2026-05-28T00:00:00.000Z', 'cwd': '/synthetic/project'}
    if parent is not None:
        value.update(parentSession=parent, timestamp='2026-05-28T21:00:00.000Z')
    return value


def message(identifier='aaaaaaaa', parent=None, total=162):
    usage = USAGE if total == 162 else {'input': total - 5, 'output': 5,
                                       'cacheRead': 0, 'cacheWrite': 0, 'totalTokens': total}
    return {'type': 'message', 'id': identifier, 'parentId': parent, 'timestamp': TIME,
            'message': {'role': 'assistant', 'model': 'gpt-5', 'provider': 'openai',
                        'content': [{'type': 'text', 'text': 'Synthetic.'}],
                        'usage': copy.deepcopy(usage)}}


def cases():
    base = message()
    extra = message('bbbbbbbb', 'aaaaaaaa', 17)
    after_fork = extra | {'timestamp': '2026-05-28T22:00:00.000Z'}
    sibling = message('cccccccc', 'aaaaaaaa', 23)
    no_id = {key: value for key, value in base.items() if key not in ('id', 'parentId')}
    missing_model = copy.deepcopy(base)
    del missing_model['message']['model']
    missing_details = copy.deepcopy(base)
    missing_details['message']['usage'] = {'totalTokens': 162}
    unknown = copy.deepcopy(base)
    unknown['message']['model'] = 'openusage-unpriced-model'
    compact = {'type': 'compaction', 'id': 'dddddddd', 'parentId': 'aaaaaaaa',
               'timestamp': TIME, 'summary': 'Synthetic.', 'firstKeptEntryId': 'aaaaaaaa',
               'tokensBefore': 999, 'usage': message(total=49)['message']['usage']}
    cache = {'type': 'usage', 'id': 'eeeeeeee', 'parentId': 'aaaaaaaa', 'timestamp': TIME,
             'kind': 'cache_warm', 'provider': 'openai', 'model': 'gpt-5',
             'usage': extra['message']['usage']}
    root = 'first/sessions/project-a/root.jsonl'
    child = 'first/sessions/project-a/child.jsonl'
    original = [header(), base]
    fork = [header('child', '@parent'), base]
    def case(files, expected, **options):
        return {'files': files, 'expected': expected, **options}
    return {
        'baseline': case({root: original}, 162),
        'copied_same_project': case({root: original}, 162, copy_to='second/sessions/project-a/root.jsonl'),
        'copied_different_project': case({root: original}, 162, copy_to='second/sessions/project-b/root.jsonl'),
        'copied_renamed_file': case({root: original}, 162, copy_to='second/sessions/project-a/renamed.jsonl'),
        'legacy_copy': case({root: [header(), no_id]}, 162, copy_to='second/sessions/project-b/root.jsonl'),
        'distinct_equal_usage': case({root: [header(), base, message('bbbbbbbb', 'aaaaaaaa')]}, 324),
        'replayed_entry': case({root: [header(), base, base]}, 162),
        'conflicting_entry': case({root: [header(), base, message(total=192)]}, None),
        'forked_prefix': case({root: original, child: fork}, 162),
        'fork_with_new_usage': case({root: original, child: fork + [after_fork]}, 179),
        'fork_inactive_branch': case({root: original + [extra, sibling], child: fork + [extra]}, 202),
        'clone_branched_history': case({root: original + [extra, sibling], child: fork + [extra, sibling]}, 202),
        'malformed_lineage': case({root: original, child: [header('child', 42), base]}, None),
        'missing_parent': case({child: fork}, 162),
        'compaction_usage': case({root: original + [compact]}, 211),
        'compaction_context_only': case({root: original + [{k: v for k, v in compact.items() if k != 'usage'}]}, 162),
        'branch_summary_usage': case({root: original + [compact | {'type': 'branch_summary', 'fromId': 'aaaaaaaa'}]}, 211),
        'cache_warming_usage': case({root: original + [cache]}, 179),
        'unknown_usage_kind': case({root: original + [cache | {'kind': 'synthetic-new-operation'}]}, 179),
        'unknown_price': case({root: [header(), unknown]}, 162),
        'missing_model': case({root: [header(), missing_model]}, 162),
        'missing_details': case({root: [header(), missing_details]}, 162),
        'missing_time': case({root: [header(), {k: v for k, v in base.items() if k != 'timestamp'}]}, 162),
        'malformed_peer': case({root: original, 'second/sessions/project-b/bad.jsonl': ['@malformed']}, None),
        'invalid_utf8_peer': case({root: original, 'second/sessions/project-b/bad.jsonl': ['@invalid-utf8']}, None),
        'subagent_artifact': case({root: original, 'first/sessions/project-a/subagent-artifacts/copy.jsonl': [no_id]}, 162),
        'fresh_child': case({root: original, 'first/sessions/project-a/run-a/run-0/session.jsonl': [header('session'), extra | {'parentId': None}]}, 179),
        'missing_project': case({'first/custom/project-a/root.jsonl': original}, 162, custom=True),
        'empty': case({root: [header()]}, 0),
    }


def collect(test):
    reports = {}
    for name, case in cases().items():
        with test.subTest(case=name), tempfile.TemporaryDirectory(prefix='openusage-pi-') as temporary:
            root = Path(temporary)
            make_source(root / 'not-selected', test.inputs['zcode'])
            for relative, rows in case['files'].items():
                path = root / relative
                path.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
                if rows == ['@invalid-utf8']:
                    path.write_bytes(b'\xff\xfe\n')
                elif rows == ['@malformed']:
                    path.write_text('{"type":"message","message":{"usage":\n')
                else:
                    serialized = '\n'.join(json.dumps(row) for row in rows)
                    parent = str(root / 'first/sessions/project-a/root.jsonl')
                    path.write_text(serialized.replace('@parent', parent) + '\n')
            if target := case.get('copy_to'):
                destination = root / target
                destination.parent.mkdir(parents=True, mode=0o700)
                shutil.copyfile(root / 'first/sessions/project-a/root.jsonl', destination)
            roots = [root / 'first' / ('custom' if case.get('custom') else 'sessions')]
            if (root / 'second').exists():
                roots.append(root / 'second/sessions')
            before = file_hashes(root)
            reports[name] = {kind: FormatComparisons.run_report(
                test, root, 'pi', {'variable': 'PI_AGENT_DIR'}, ','.join(map(str, roots)), kind)
                for kind in ('daily', 'session')}
            test.assertEqual(file_hashes(root), before)
    return reports


class PiQualificationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        FormatComparisons.setUpClass.__func__(cls)

    def test_recorded_reports_and_known_qualification_gaps(self):
        self.assertEqual(collect(self), json.loads((FIXTURES / 'pi-qualification.json').read_text()))


if __name__ == '__main__':
    unittest.main()
