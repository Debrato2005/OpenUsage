import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { cp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

// Share images from saved synthetic local totals: a preview, an explicit save as PNG and SVG,
// no private field in either file, and missing days kept distinct (AT-82, OU-SHARE-001 to OU-SHARE-006).
for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_NATIVE_EVIDENCE']) assert.ok(process.env[name], name);
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const ccusage = join(repository, 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu');
const password = 'OpenUsage synthetic share workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = {
  requirement: 'NATIVE-SHARE-IMAGES-001', result: 'failed', startedAt: new Date().toISOString(),
  scope: 'Actual Linux Tauri native-test build, a private SQLCipher 4 password workspace, synthetic Claude Code and Codex logs in a private home, and the real pinned ccusage 20.0.48. No network check, no real logs and no account.',
  expectedResult: ['Share shows a preview and saves nothing before an explicit save.', 'The PNG is a PNG at twice the image size.', 'The SVG holds the exact total and no path, project name or account label.'],
  steps: [], files: [],
};
let fixture, driver;
const call = async (command, args) => { const response = await driver.invoke(command, args); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; };
async function key(value) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'share-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
}
async function activate(selector) {
  await waitFor(() => driver.execute('const n=document.querySelector(arguments[0]);return Boolean(n&&!n.disabled&&n.getClientRects().length);', [selector]), `available ${selector}`);
  assert.equal(await driver.execute('const n=document.querySelector(arguments[0]);n.scrollIntoView({block:"nearest"});n.focus({preventScroll:true});return document.activeElement===n;', [selector]), true);
  await key('');
}
const shareDir = () => join(fixture.dirs.downloads, 'OpenUsage');
async function saved() { try { return (await readdir(shareDir())).sort(); } catch (error) { if (error.code !== 'ENOENT') throw error; return []; } }

try {
  await mkdir(output, { recursive: true, mode: 0o700 });
  evidence.applicationSha256 = await hash(application);
  fixture = await createFixture('share-images');
  const logs = join(fixture.root, 'logs-home');
  await cp(join(repository, 'crates/openusage-locallogs/tests/fixtures/home'), logs, { recursive: true });
  const shifted = offset => { const d = new Date(); d.setUTCDate(d.getUTCDate() - offset); return d.toISOString().slice(0, 10); };
  const [older, newer] = [shifted(2), shifted(1)];
  const claudeLogs = join(logs, '.claude/projects/-home-dev-alpha');
  for (const name of readdirSync(claudeLogs)) {
    const path = join(claudeLogs, name);
    await writeFile(path, (await readFile(path, 'utf8')).replace(/2026-11-0([45])T/g, (_, day) => `${day === '4' ? older : newer}T`), { mode: 0o600 });
  }
  Object.assign(fixture.environment, { OPENUSAGE_NATIVE_LOG_HOME: logs, OPENUSAGE_NATIVE_CCUSAGE: ccusage });

  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log')); await driver.start();
  await waitFor(() => driver.execute('return document.querySelectorAll(".local-log-choice .local-log-tool").length===2&&Boolean(document.querySelector("#workspace-confirmation"));'), 'password setup with the consent list');
  await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password); await key('');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready share workspace');
  await waitFor(async () => (await call('load_local_logs')).scan.lastScanAtUtc !== null, 'the first local log scan', 90000);
  await waitFor(() => driver.execute('return Boolean(document.querySelector("[data-local-overview] [data-share-open]"));'), 'the This device section', 30000);
  evidence.steps.push('The app scanned the synthetic Claude Code logs, and Overview showed This device with Share.');

  await activate('[data-local-overview] [data-share-open]');
  await waitFor(() => driver.execute('const i=document.querySelector("[data-share-dialog] img");return Boolean(i&&i.complete&&i.naturalWidth===1200);'), 'the Wrapped preview');
  assert.deepEqual(await saved(), [], 'A preview saves nothing.');
  const saveButton = label => `[data-share-dialog] .share-actions button:nth-child(${label === 'png' ? 1 : 2})`;
  await activate(saveButton('png'));
  await waitFor(async () => (await saved()).some(name => name.endsWith('.png')), 'the saved PNG');
  await activate(saveButton('svg'));
  await waitFor(async () => (await saved()).some(name => name.endsWith('.svg')), 'the saved SVG');
  evidence.steps.push('Save PNG and Save SVG each wrote one file in Downloads/OpenUsage.');

  const names = await saved();
  assert.equal(names.length, 2);
  for (const name of names) assert.match(name, /^openusage-wrapped-[A-Za-z0-9_-]{16,64}\.(png|svg)$/);
  const pngName = names.find(n => n.endsWith('.png')), svgName = names.find(n => n.endsWith('.svg'));
  const png = await readFile(join(shareDir(), pngName));
  assert.deepEqual([...png.subarray(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
  assert.deepEqual([png.readUInt32BE(16), png.readUInt32BE(20)], [2400, 1260]);
  const svg = await readFile(join(shareDir(), svgName), 'utf8');
  assert.ok(svg.startsWith('<svg xmlns="http://www.w3.org/2000/svg"'));
  assert.ok(svg.includes('19,550 tokens'), 'The exact total of both tools appears.');
  for (const secret of [logs, fixture.root, 'dev-alpha', '-home-dev', '/home/', 'synthetic share workspace']) assert.ok(!svg.includes(secret), `The SVG holds no ${secret}`);
  assert.ok(!png.includes(Buffer.from('dev-alpha')) && !png.includes(Buffer.from(fixture.root)));
  for (const name of names) evidence.files.push({ name, bytes: (await readFile(join(shareDir(), name))).length, sha256: await hash(join(shareDir(), name)) });
  evidence.steps.push('The PNG is 2400 by 1260 pixels. The SVG holds 19,550 tokens and no path or project name.');
  assert.equal(await hash(application), evidence.applicationSha256);
  evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally {
  try { await driver?.stop(); } finally {
    stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true });
    evidence.finishedAt = new Date().toISOString();
    await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
    console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  }
}
