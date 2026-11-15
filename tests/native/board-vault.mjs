import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { startSecretService, stopSecretService } from './secret-service.mjs';

const run = promisify(execFile);
const repository = fileURLToPath(new URL('../../', import.meta.url));
const executable = resolve(process.env.OPENUSAGE_VAULT_TEST_BINARY ?? '');
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? '');
assert.ok(process.env.OPENUSAGE_VAULT_TEST_BINARY && process.env.OPENUSAGE_NATIVE_EVIDENCE);
assert.equal(process.platform, 'linux');
const evidence = {
  requirement: 'OU-BOARD-002, OU-SEC-003, OU-CRYPT-003/006/011',
  result: 'failed', startedAt: new Date().toISOString(), platform: process.platform,
  scope: 'The private Linux Secret Service keeps synthetic database and leaderboard credentials in separate slots. No live account or user credential store is read.',
  expectedResult: [
    'Synthetic credentials survive a private daemon restart.',
    'Occupied, malformed, duplicate, locked, and unavailable entries refuse safely.',
    'Expected-value removal preserves unrelated credentials.',
  ],
  phases: [],
};
let fixture;
process.once('exit', () => { fixture?.daemon?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  await mkdir(output, { recursive: true, mode: 0o700 });
  evidence.sourceCommit = (await run('git', ['rev-parse', 'HEAD'], { cwd: repository, timeout: 10000 })).stdout.trim();
  evidence.dirtySource = (await run('git', ['status', '--porcelain'], { cwd: repository, timeout: 10000 })).stdout.length > 0;
  evidence.executableSha256 = createHash('sha256').update(await readFile(executable)).digest('hex');
  fixture = await createFixture('board-vault');
  const home = join(fixture.root, 'home'); await mkdir(home, { mode: 0o700 });
  Object.assign(fixture.environment, { HOME: home, OPENUSAGE_PRIVATE_KEYRING_FIXTURE: fixture.root });
  await startSecretService(fixture);
  for (const phase of ['seed', 'reopen', 'erasure', 'unavailable']) {
    if (phase === 'reopen' || phase === 'erasure') { await stopSecretService(fixture); await startSecretService(fixture); }
    if (phase === 'unavailable') await stopSecretService(fixture);
    const { stdout } = await run(executable, ['--ignored', '--exact', 'private_os_store_keeps_database_and_board_credentials_separate'], {
      env: { ...fixture.environment, OPENUSAGE_BOARD_VAULT_PHASE: phase }, timeout: 30000, maxBuffer: 65536,
    });
    assert.match(stdout, /1 passed/);
    evidence.phases.push({ phase, passed: true });
  }
  evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally {
  await stopSecretService(fixture); stopGroup(fixture?.bus, 'SIGKILL');
  if (fixture) await rm(fixture.root, { recursive: true, force: true });
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
  console.log(`Private board vault: ${evidence.result}. Evidence: ${output}`);
  if (evidence.error) console.error(evidence.error);
}
