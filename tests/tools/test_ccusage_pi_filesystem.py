"""Bounded local Pi filesystem candidate, with private synthetic inputs only."""
import hashlib
import json
import os
from pathlib import Path
import socket
import sys
import tempfile
import unittest

from test_ccusage_formats import FIXTURES, PIN, ROOT, file_hashes, make_source
from test_ccusage_pi import header, message
import test_ccusage_pi_format as format_candidate


CASES = ['baseline', 'duplicate_root', 'missing_root', 'file_root', 'root_link',
         'file_link', 'directory_link', 'dangling_link', 'socket', 'fifo',
         'unreadable_file', 'unreadable_directory', 'deep_directory',
         'oversize_file', 'too_many_roots', 'token_overflow']


def collect_filesystem(test):
    reports = {}
    for name in CASES:
        with test.subTest(case=name), tempfile.TemporaryDirectory(prefix='openusage-pi-filesystem-') as temporary:
            root = Path(temporary)
            make_source(root / 'not-selected', test.inputs['zcode'])
            folder = root / 'sessions'
            folder.mkdir(mode=0o700)
            file = folder / 'root.jsonl'
            file.write_text('\n'.join(map(json.dumps, [header(), message()])) + '\n')
            roots = [folder]
            permissions = []
            listener = None
            expected = 162 if name in ('baseline', 'duplicate_root') else None
            if name == 'duplicate_root':
                roots.append(folder)
            elif name == 'missing_root':
                roots.append(root / 'missing')
            elif name == 'file_root':
                roots = [file]
            elif name == 'root_link':
                link = root / 'alias'
                link.symlink_to(folder, target_is_directory=True)
                roots = [link]
            elif name == 'file_link':
                (folder / 'copy.jsonl').symlink_to(file)
            elif name == 'directory_link':
                (folder / 'linked').symlink_to(root / 'not-selected', target_is_directory=True)
            elif name == 'dangling_link':
                (folder / 'missing.jsonl').symlink_to(root / 'absent')
            elif name == 'socket':
                listener = socket.socket(socket.AF_UNIX)
                listener.bind(str(folder / 'socket.jsonl'))
            elif name == 'fifo':
                os.mkfifo(folder / 'pipe.jsonl', 0o600)
            elif name == 'unreadable_file':
                permissions = [(file, file.stat().st_mode)]
            elif name == 'unreadable_directory':
                blocked = folder / 'blocked'
                blocked.mkdir(mode=0o700)
                (blocked / 'other.jsonl').write_bytes(file.read_bytes())
                permissions = [(blocked, blocked.stat().st_mode)]
            elif name == 'deep_directory':
                nested = folder.joinpath(*(['nested'] * 65))
                nested.mkdir(parents=True)
                (nested / 'copy.jsonl').write_bytes(file.read_bytes())
            elif name == 'oversize_file':
                with (folder / 'large.jsonl').open('wb') as output:
                    output.truncate((32 << 20) + 1)
            elif name == 'too_many_roots':
                roots = [root / f'selected-{index}' for index in range(65)]
                for selected in roots:
                    selected.mkdir(mode=0o700)
            elif name == 'token_overflow':
                rows = [header(), message(total=2**64 - 1), message('bbbbbbbb', 'aaaaaaaa', 5)]
                file.write_text('\n'.join(map(json.dumps, rows)) + '\n')
            before = file_hashes(root)
            try:
                for path, _ in permissions:
                    path.chmod(0o0)
                reports[name] = {kind: test.candidate_report(root, command, roots, expected)
                                 for kind, command in [('daily', ['daily', '--by-agent']), ('session', ['pi', 'session'])]}
            finally:
                for path, mode in permissions:
                    path.chmod(mode)
                if listener is not None:
                    listener.close()
            test.assertEqual(file_hashes(root), before)
    return reports


@unittest.skipUnless(sys.platform == 'linux' and hasattr(os, 'getuid') and os.getuid() != 0,
                     'Filesystem qualification requires an unprivileged Linux process.')
class PiFilesystemCandidateTests(format_candidate.PiFormatCandidateTests):
    @classmethod
    def setUpClass(cls):
        selected = os.environ.get('OPENUSAGE_CCUSAGE_PI_FILESYSTEM_CANDIDATE')
        if not selected:
            raise unittest.SkipTest('Set OPENUSAGE_CCUSAGE_PI_FILESYSTEM_CANDIDATE for the local filesystem candidate.')
        cls.binary = Path(selected).resolve(strict=True)
        if hashlib.sha256(cls.binary.read_bytes()).hexdigest() != os.environ.get('OPENUSAGE_CCUSAGE_PI_FILESYSTEM_CANDIDATE_SHA256'):
            raise ValueError('Filesystem candidate digest does not match the explicit expected SHA-256.')
        cls.reference = ROOT / 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu'
        if hashlib.sha256(cls.reference.read_bytes()).hexdigest() != json.loads(PIN.read_text())['executableSha256']:
            raise ValueError('The reference does not match the production pin.')
        cls.inputs = json.loads((FIXTURES / 'inputs.json').read_text())
        cls.expected = json.loads((FIXTURES / 'expected.json').read_text())

    def test_filesystem_results_preserve_source_files(self):
        reports = collect_filesystem(self)
        self.assertEqual(reports, json.loads((FIXTURES / 'pi-filesystem-candidate.json').read_text()))
        baseline = json.loads((FIXTURES / 'pi-identity-candidate.json').read_text())['baseline']
        self.assertEqual(reports['baseline'], baseline)
        self.assertEqual(reports['duplicate_root'], baseline)


if __name__ == '__main__':
    unittest.main()
