import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const script = join(root, 'scripts/git-safety.mjs');
const scanner = process.env.GITLEAKS_BIN
  ?? join(root, '.tools/security/gitleaks-8.30.1/gitleaks');
const zero = '0'.repeat(40);

function repository(run) {
  const base = join(root, '.recovery');
  mkdirSync(base, { recursive: true });
  const cwd = mkdtempSync(join(base, 'git-safety-test-'));
  const env = Object.fromEntries(Object.entries(process.env)
    .filter(([key]) => !key.startsWith('GIT_')));
  env.GITLEAKS_BIN = scanner;
  const invoke = (command, args, input) => {
    const result = spawnSync(command, args, {
      cwd, env, input, encoding: 'utf8', timeout: 45_000, maxBuffer: 2 * 1024 * 1024,
    });
    assert.ifError(result.error);
    assert.equal(result.signal, null, 'command must finish within its timeout');
    return result;
  };
  const git = (...args) => {
    const result = invoke('git', args);
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  const put = (path, contents) => {
    const target = join(cwd, path);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, contents);
  };
  const check = (mode, input) => invoke(process.execPath,
    [script, mode, ...(mode === 'push' ? ['origin', 'https://example.invalid/openusage.git'] : [])], input);
  try {
    git('init', '--initial-branch=main');
    git('config', 'user.name', 'Synthetic Git Safety Test');
    git('config', 'user.email', 'git-safety@example.invalid');
    const hooks = join(cwd, '.git', 'disabled-hooks');
    mkdirSync(hooks);
    git('config', 'core.hooksPath', hooks);
    put('README.md', 'Synthetic source-only fixture.\n');
    git('add', 'README.md');
    git('commit', '-m', 'Synthetic initial source');
    return run({ git, put, check });
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
}

test('clean staged source passes the real secret scanner', { timeout: 60_000 }, () => {
  assert.ok(existsSync(scanner), 'install the documented Gitleaks binary before running this suite');
  repository(({ git, put, check }) => {
    put('src/example.mjs', 'export const title = "OpenUsage";\n');
    git('add', 'src/example.mjs');
    const result = check('staged');
    assert.equal(result.status, 0, result.stderr);
  });
});

test('force-staged recovery artifacts and credential files are refused', { timeout: 120_000 }, () => {
  for (const path of ['.recovery/scratch.txt', 'auth.json']) {
    repository(({ git, put, check }) => {
      put('.gitignore', '.recovery/\nauth.json\n');
      put(path, 'Synthetic private artifact without a secret.\n');
      git('add', '.gitignore');
      git('add', '--force', path);
      const result = check('staged');
      assert.notEqual(result.status, 0, `must reject ${path}`);
    });
  }
});

test('staged synthetic token is blocked without printing its value', { timeout: 60_000 }, () => {
  repository(({ git, put, check }) => {
    // Random, nonfunctional test material; no credential validation or network call.
    const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    const token = `ghp_${Array.from(randomBytes(36), byte => alphabet[byte % alphabet.length]).join('')}`;
    put('src/settings.mjs', `export const accessToken = '${token}';\n`);
    git('add', 'src/settings.mjs');
    const result = check('staged');
    assert.notEqual(result.status, 0, 'scanner must reject the synthetic token');
    assert.equal(`${result.stdout}${result.stderr}`.includes(token), false,
      'scanner output must redact the full token');
  });
});

test('push permits main fast-forward and refuses unsafe updates', { timeout: 180_000 }, () => {
  repository(({ git, put, check }) => {
    const base = git('rev-parse', 'HEAD');
    put('README.md', 'Synthetic main change.\n');
    git('add', 'README.md');
    git('commit', '-m', 'Synthetic main update');
    const tip = git('rev-parse', 'HEAD');
    const fastForward = check('push', `refs/heads/main ${tip} refs/heads/main ${base}\n`);
    assert.equal(fastForward.status, 0, fastForward.stderr);

    for (const branch of ['recovery/debug', 'archive/retired']) {
      const result = check('push', `refs/heads/${branch} ${tip} refs/heads/${branch} ${zero}\n`);
      assert.notEqual(result.status, 0, `must refuse publishing ${branch}`);
    }

    git('checkout', '-b', 'synthetic-remote-tip', base);
    put('remote-source.txt', 'Synthetic divergent source.\n');
    git('add', 'remote-source.txt');
    git('commit', '-m', 'Synthetic divergent update');
    const divergent = git('rev-parse', 'HEAD');
    git('checkout', 'main');
    const rewind = check('push', `refs/heads/main ${tip} refs/heads/main ${divergent}\n`);
    assert.notEqual(rewind.status, 0, 'must reject a non-fast-forward main update');

    const deleteMain = check('push', `(delete) ${zero} refs/heads/main ${tip}\n`);
    assert.notEqual(deleteMain.status, 0, 'must reject main deletion');
    const unpublished = check('push', `(delete) ${zero} refs/heads/archive/retired ${tip}\n`);
    assert.notEqual(unpublished.status, 0, 'must preserve public reachability before deleting a branch');
    git('update-ref', 'refs/remotes/origin/main', tip);
    const deleteArchive = check('push', `(delete) ${zero} refs/heads/archive/retired ${tip}\n`);
    assert.equal(deleteArchive.status, 0, deleteArchive.stderr);
    const uniqueArchive = check('push', `(delete) ${zero} refs/heads/archive/changed ${divergent}\n`);
    assert.notEqual(uniqueArchive.status, 0, 'must refuse deleting unpreserved unique work');
  });
});
