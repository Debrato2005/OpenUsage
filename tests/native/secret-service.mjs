import { spawn, execFile } from 'node:child_process';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import { waitFor } from './webdriver.mjs';

const run = promisify(execFile);

export async function startSecretService(fixture) {
  const sdk = process.env.OPENUSAGE_SECRET_SERVICE_SDK;
  const environment = { ...fixture.environment };
  if (sdk) environment.LD_LIBRARY_PATH = join(resolve(sdk), 'runtime/usr/lib/x86_64-linux-gnu');
  const binary = sdk ? join(resolve(sdk), 'runtime/usr/bin/gnome-keyring-daemon') : 'gnome-keyring-daemon';
  const child = spawn(binary, ['--foreground', '--components=secrets', '--unlock', `--control-directory=${fixture.dirs.control}`], { env: environment, stdio: ['pipe', 'pipe', 'pipe'] });
  fixture.daemon = child;
  child.stdout.on('data', chunk => fixture.logs.push(chunk));
  child.stderr.on('data', chunk => fixture.logs.push(chunk));
  let failed;
  child.once('error', error => { failed = error; });
  child.stdin.once('error', error => { failed = error; });
  child.once('exit', code => { failed = new Error(`Private Secret Service exited: ${code}`); });
  child.stdin.end('OpenUsage synthetic keyring fixture passphrase');
  await waitFor(async () => {
    if (failed) throw failed;
    try {
      const alias = await run('gdbus', ['call', '--session', '--dest', 'org.freedesktop.secrets', '--object-path', '/org/freedesktop/secrets', '--method', 'org.freedesktop.Secret.Service.ReadAlias', 'default'], { env: environment, timeout: 2000 });
      const collection = alias.stdout.match(/'(\/org\/freedesktop\/secrets\/collection\/[^']+)'/)?.[1];
      if (!collection) return false;
      const locked = await run('gdbus', ['call', '--session', '--dest', 'org.freedesktop.secrets', '--object-path', collection, '--method', 'org.freedesktop.DBus.Properties.Get', 'org.freedesktop.Secret.Collection', 'Locked'], { env: environment, timeout: 2000 });
      return locked.stdout.includes('false');
    } catch { return false; }
  }, 'real private Secret Service');
}

export async function stopSecretService(fixture) {
  const child = fixture?.daemon;
  if (!child) return;
  if (child.exitCode === null && child.signalCode === null) {
    const exit = new Promise(done => child.once('exit', done));
    child.kill('SIGTERM');
    await Promise.race([exit, delay(3000)]);
    if (child.exitCode === null && child.signalCode === null) {
      child.kill('SIGKILL');
      await exit;
    }
  }
  fixture.daemon = null;
}
