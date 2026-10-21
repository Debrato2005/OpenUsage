import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

function refuse(message) {
  throw new Error(message);
}

function git(args) {
  const result = spawnSync('git', args, { encoding: 'utf8', timeout: 30000, maxBuffer: 16 * 1024 * 1024 });
  if (result.error || result.status !== 0) refuse('Git safety check failed. Check the repository and fetch the current remote state.');
  return result.stdout;
}

function checkPaths(paths) {
  const privatePaths = /(^|\/)(?:\.recovery|\.tools|\.codex|CODEX_HOME|node_modules|target|dist|coverage|\.npm-cache|\.vite|\.pnpm-store|test-results|playwright-report)(?:\/|$)|(^|\/)(?:auth\.json(?:\..*)?|credentials\.json(?:\..*)?|database-key\.v1|\.env(?:\..*)?|\.DS_Store|Thumbs\.db|Desktop\.ini)$|\.(?:db|db3|sqlite|sqlite3)(?:-(?:wal|shm|journal))?$|\.(?:key|pem|p12|pfx|oubackup|exe|dll|so|dylib|rlib|rmeta|o|a|log)$|:Zone\.Identifier$|^docs\/evidence\//i;
  for (const path of paths) {
    if (privatePaths.test(path)) refuse(`Private or generated file is included: ${JSON.stringify(path)}. Keep it outside Git.`);
  }
}

function scan(root, args) {
  const local = join(root, '.tools', 'security', 'gitleaks-8.30.1', process.platform === 'win32' ? 'gitleaks.exe' : 'gitleaks');
  const executable = process.env.GITLEAKS_BIN || (existsSync(local) ? local : 'gitleaks');
  const result = spawnSync(executable, ['git', root, '--redact=100', '--no-banner', '--ignore-gitleaks-allow', '--timeout=30', ...args], {
    stdio: ['ignore', 'inherit', 'inherit'], timeout: 45000,
  });
  if (result.error?.code === 'ENOENT') refuse('Install Gitleaks from its official releases, or set GITLEAKS_BIN to its executable. See CONTRIBUTING.md.');
  if (result.error || result.status !== 0) refuse('Secret check refused the operation. Review the redacted findings or scanner error.');
}

try {
  const root = git(['rev-parse', '--show-toplevel']).trim();
  if (process.argv[2] === 'staged') {
    const paths = git(['diff', '--cached', '--name-only', '--diff-filter=ACMR', '-z']).split('\0').filter(Boolean);
    checkPaths(paths);
    if (paths.length) scan(root, ['--pre-commit', '--staged']);
  } else if (process.argv[2] === 'push') {
    const updates = readFileSync(0, 'utf8').split('\n').filter(Boolean).map(line => line.trim().split(/\s+/));
    const uploads = [];
    for (const fields of updates) {
      const [localRef, localOid, remoteRef, remoteOid] = fields;
      if (fields.length !== 4 || !/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/.test(localOid) || !/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/.test(remoteOid)) refuse('Invalid Git pre-push input.');
      if (/^0+$/.test(localOid)) {
        if (remoteRef === 'refs/heads/main') refuse('Deleting main is refused.');
        if (!remoteRef.startsWith('refs/heads/')) refuse('Only reviewed branch deletions are supported.');
        git(['merge-base', '--is-ancestor', remoteOid, 'main']);
        git(['merge-base', '--is-ancestor', remoteOid, 'origin/main']);
        continue;
      }
      if (localRef !== 'refs/heads/main' || remoteRef !== 'refs/heads/main') refuse('Only main may be published. Recovery, archive and temporary branches stay local.');
      if (!/^0+$/.test(remoteOid)) git(['merge-base', '--is-ancestor', remoteOid, localOid]);
      uploads.push(/^0+$/.test(remoteOid) ? localOid : `${remoteOid}..${localOid}`);
    }
    if (uploads.length > 1) refuse('Push one reviewed main update at a time.');
    for (const range of uploads) {
      const paths = git(['log', '--format=', '--name-only', '--diff-filter=ACMR', '-z', range]).split('\0').map(path => path.replace(/^\n+/, '')).filter(Boolean);
      checkPaths(paths);
      scan(root, [`--log-opts=${range}`]);
    }
  } else {
    refuse('Use: node scripts/git-safety.mjs staged | push');
  }
} catch (error) {
  console.error(`OpenUsage: ${error.message}`);
  process.exitCode = 1;
}
