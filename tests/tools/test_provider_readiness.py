"""Pause synthetic fixture writes to check cancellation readiness."""
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]
QUOTA = ROOT / 'crates/openusage-codex/tests/fixtures/synthetic-quota-provider.py'
PAUSE_RUNTIME_WRITE = '''
import pathlib, runpy, sys
write_text = pathlib.Path.write_text
def paused_write(path, value, *args, **kwargs):
    if path.name == 'synthetic-runtime-home':
        write_text(path, '')
        print('runtime-write-paused', flush=True)
        assert sys.stdin.readline() == 'release\\n'
    return write_text(path, value, *args, **kwargs)
pathlib.Path.write_text = paused_write
script = sys.argv.pop(1)
sys.argv[0] = script
runpy.run_path(script, run_name='__main__')
'''


@unittest.skipUnless(os.name == 'posix', 'Uses Unix pipe readiness.')
class ProviderReadinessTests(unittest.TestCase):
    def test_quota_pid_is_visible_only_after_the_runtime_path_is_complete(self):
        for reused in [False, True]:
            with self.subTest(reused=reused), tempfile.TemporaryDirectory(prefix='openusage-readiness-') as temporary:
                root = Path(temporary)
                profile = root / 'profile'
                runtime = root / 'runtime'
                profile.mkdir(mode=0o700)
                runtime.mkdir(mode=0o700)
                pid_marker = profile / 'synthetic-child-pid'
                runtime_marker = profile / 'synthetic-runtime-home'
                if reused:
                    runtime_marker.write_text(str(root / 'removed-runtime'))
                child = subprocess.Popen(
                    [sys.executable, '-c', PAUSE_RUNTIME_WRITE, str(QUOTA)],
                    cwd=root, env={'HOME': str(runtime), 'CODEX_HOME': str(profile)},
                    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                    text=True,
                )
                try:
                    self.assertTrue(select.select([child.stdout], [], [], 5)[0], 'The fixture did not reach the write.')
                    self.assertEqual(child.stdout.readline(), 'runtime-write-paused\n')
                    self.assertEqual(runtime_marker.read_text(), '')
                    self.assertFalse(pid_marker.exists(), 'A PID cannot signal readiness during the path write.')
                    child.stdin.write('release\n')
                    child.stdin.flush()
                    deadline = time.monotonic() + 5
                    while not pid_marker.exists() or not pid_marker.read_text():
                        self.assertIsNone(child.poll(), 'The fixture exited before readiness.')
                        self.assertLess(time.monotonic(), deadline, 'The fixture did not report readiness.')
                        time.sleep(0.01)
                    self.assertEqual(pid_marker.read_text(), str(child.pid))
                    self.assertEqual(runtime_marker.read_text(), str(runtime))
                    child.terminate()
                    child.wait(timeout=5)
                    self.assertEqual(runtime_marker.read_text(), str(runtime))
                finally:
                    if child.poll() is None:
                        child.kill()
                    child.communicate(timeout=5)


if __name__ == '__main__':
    unittest.main()
