import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
HELPER = (ROOT / 'tests/native/login-verifier.mjs').as_uri()
HISTORICAL_PIN = 'a2e4a64707081a9943f3c04f8237c2b16bea673e974e96393d6a8a7d9357f975'
FIXTURE_PIN = '9b7538e31950e56df3f2f87acb239b99433f2f3a40ae01ae73d8c06271d07389'
FIXTURE = b'''#!/bin/sh
printf 'executed\\n' > "$1"
printf '{"verified":true}\\n'
'''


class NativeVerifierPinTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='openusage-verifier-pin-')
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.verifier = self.directory / 'verifier'
        self.verifier.write_bytes(FIXTURE)
        self.verifier.chmod(0o700)
        self.marker = self.directory / 'executed'
        self.environment = {'PATH': os.environ['PATH'], 'HOME': str(self.directory)}

    def run_verifier(self, pin):
        environment = dict(self.environment)
        if pin is not None:
            environment['OPENUSAGE_LOGIN_VERIFIER_SHA256'] = pin
        code = f'''
import {{ loginVerifierDigest, runLoginVerifier }} from {json.dumps(HELPER)};
const [verifier, marker] = process.argv.slice(1);
const raw = await runLoginVerifier(verifier, loginVerifierDigest(), [marker], {{
  encoding: 'utf8', timeout: 1000, env: process.env,
}});
process.stdout.write(raw);
'''
        return subprocess.run(['node', '--input-type=module', '--eval', code, str(self.verifier), str(self.marker)],
                              env=environment, capture_output=True, text=True, timeout=10, cwd=ROOT)

    def test_explicit_pin_allows_the_matching_verifier(self):
        result = self.run_verifier(FIXTURE_PIN)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {'verified': True})
        self.assertEqual(self.marker.read_text(), 'executed\n')

    def test_malformed_pins_never_execute_the_verifier(self):
        invalid = ['', 'a' * 63, 'a' * 65, 'A' * 64, 'g' * 64, 'é' * 64,
                   ' ' + FIXTURE_PIN, FIXTURE_PIN + ' ', FIXTURE_PIN + '\n', FIXTURE_PIN + '\r\n']
        for pin in invalid:
            with self.subTest(pin=repr(pin)):
                result = self.run_verifier(pin)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('ERR_ASSERTION', result.stderr)
                self.assertFalse(self.marker.exists())

    def test_mismatched_pin_never_executes_the_verifier(self):
        result = self.run_verifier('0' * 64)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('ERR_ASSERTION', result.stderr)
        self.assertFalse(self.marker.exists())

    def test_absent_override_keeps_the_historical_pin(self):
        code = f'import {{ loginVerifierDigest }} from {json.dumps(HELPER)}; process.stdout.write(loginVerifierDigest());'
        result = subprocess.run(['node', '--input-type=module', '--eval', code], env=self.environment,
                                capture_output=True, text=True, timeout=10, cwd=ROOT)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, HISTORICAL_PIN)
        result = self.run_verifier(None)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.marker.exists())

    def test_native_journeys_reject_invalid_pins_before_start(self):
        sentinel = self.directory / 'execution-sentinel'
        sentinel.write_text('#!/bin/sh\nprintf executed > "$OPENUSAGE_TEST_EXECUTION_MARKER"\nexit 90\n')
        sentinel.chmod(0o700)
        output = self.directory / 'evidence'
        environment = {**self.environment, 'OPENUSAGE_LOGIN_VERIFIER_SHA256': 'INVALID',
                       'OPENUSAGE_LOGIN_VERIFIER': str(sentinel), 'OPENUSAGE_NATIVE_BINARY': str(sentinel),
                       'OPENUSAGE_NATIVE_ACTIVITY_HISTORY_FIXTURE': str(sentinel),
                       'OPENUSAGE_NATIVE_EVIDENCE': str(output), 'TAURI_DRIVER': str(sentinel),
                       'OPENUSAGE_TEST_EXECUTION_MARKER': str(self.marker)}
        for script in ['local-log-consent.mjs', 'application-export.mjs']:
            with self.subTest(script=script):
                result = subprocess.run(['node', str(ROOT / 'tests/native' / script)], env=environment,
                                        capture_output=True, text=True, timeout=10, cwd=ROOT)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('ERR_ASSERTION', result.stderr)
                self.assertFalse(self.marker.exists())
                self.assertFalse(output.exists())


if __name__ == '__main__':
    unittest.main()
