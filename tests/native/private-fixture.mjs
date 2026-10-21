import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { chmod, lstat, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { waitFor } from './webdriver.mjs';

export const CODEX_SENTINEL = 'OpenUsage synthetic Codex home. No provider credentials.\n';

export function stopGroup(child, signal = 'SIGTERM') {
  if (!child?.pid) return;
  try { process.kill(-child.pid, signal); } catch (error) { if (error.code !== 'ESRCH') throw error; }
}

export async function createFixture(name) {
  const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
  const temporary = join(repository, '.recovery', 'tmp');
  for (const path of [join(repository, '.recovery'), temporary]) {
    try { await mkdir(path, { mode: 0o700 }); } catch (error) { if (error.code !== 'EEXIST') throw error; }
    const metadata = await lstat(path);
    assert.ok(metadata.isDirectory() && !metadata.isSymbolicLink(), 'Native temporary storage must be a real repository directory.');
  }
  await chmod(temporary, 0o700);
  // A short owned name keeps the private D-Bus Unix socket below its path limit.
  const root = await mkdtemp(join(temporary, 'n-'));
  let bus;
  try {
    await chmod(root, 0o700);
    const dirs = Object.fromEntries(['data', 'config', 'cache', 'state', 'runtime', 'control', 'codex', 'downloads'].map(part => [part, join(root, part)]));
    await Promise.all(Object.values(dirs).map(path => mkdir(path, { mode: 0o700 })));
    await writeFile(join(dirs.config, 'user-dirs.dirs'), `XDG_DOWNLOAD_DIR="${dirs.downloads}"\n`, { mode: 0o600 });
    await writeFile(join(dirs.codex, 'synthetic-fixture.txt'), CODEX_SENTINEL, { mode: 0o600 });
    const environment = { ...process.env, XDG_DATA_HOME: dirs.data, XDG_CONFIG_HOME: dirs.config, XDG_CACHE_HOME: dirs.cache, XDG_STATE_HOME: dirs.state, XDG_RUNTIME_DIR: dirs.runtime, GNOME_KEYRING_CONTROL: dirs.control, CODEX_HOME: dirs.codex };
    const busConfig = join(root, 'session-bus.conf');
    // No service directories: only explicitly started test services can use this bus.
    await writeFile(busConfig, `<busconfig><type>session</type><listen>unix:path=${join(dirs.runtime, 'bus')}</listen><auth>EXTERNAL</auth><policy user="${process.getuid()}"><allow own="*"/><allow send_destination="*"/><allow receive_sender="*"/></policy></busconfig>\n`, { mode: 0o600 });
    const logs = [];
    bus = spawn('dbus-daemon', ['--nofork', '--print-address=1', `--config-file=${busConfig}`], { env: environment, detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let address = '';
    let busError;
    bus.stdout.on('data', chunk => { address += chunk.toString(); });
    bus.stderr.on('data', chunk => logs.push(chunk));
    bus.once('error', error => { busError = error; });
    bus.once('exit', code => { busError = new Error(`Private D-Bus exited: ${code}`); });
    await waitFor(() => { if (busError) throw busError; return address.includes('\n'); }, 'private D-Bus address');
    environment.DBUS_SESSION_BUS_ADDRESS = address.trim().split('\n')[0];
    assert.ok(environment.DBUS_SESSION_BUS_ADDRESS.startsWith(`unix:path=${dirs.runtime}/`), 'The private bus socket must stay inside its synthetic fixture.');
    return { root, dirs, environment, bus, logs, name, daemon: null, database: join(dirs.data, 'app.openusage.desktop/workspace/openusage.db') };
  } catch (error) {
    stopGroup(bus);
    await rm(root, { recursive: true, force: true });
    throw error;
  }
}
