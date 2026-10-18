import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readlink, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-discovery-evidence-${randomUUID()}`));
const digest = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-PROVIDER-DISCOVERY-001', result: 'failed', startedAt: new Date().toISOString(), scope: 'Actual Tauri discovery and explicit source review with owned synthetic HOME, CODEX_HOME, managed roots and executable files. No provider execution, credentials, OAuth, downloads, live account data, or cross-platform qualification. Unchanged files and absent execution markers do not establish a syscall-level no-read trace.', screenshots: [], views: [], discoveries: [] };
let fixture, driver;
const watched = new Map();
async function ownedDirectory(path) { await mkdir(path, { recursive: true, mode: 0o700 }); await chmod(path, 0o700); }
async function executable(path, known = true) {
  await ownedDirectory(dirname(path));
  if (known) await copyFile(join(harness, 'fixtures/synthetic-provider.py'), path);
  else await writeFile(path, '#!/usr/bin/python3\nfrom pathlib import Path\nPath(__file__).parent.joinpath("unexpected-execution").write_text("executed")\n', { mode: 0o700 });
  await chmod(path, 0o700); watched.set(path, await digest(path));
}
async function unchanged() {
  for (const [path, hash] of watched) assert.equal(await digest(path), hash, `Synthetic source changed: ${path}`);
  for (const directory of new Set([...watched.keys()].map(dirname))) for (const name of ['synthetic-execution-observations.jsonl', 'synthetic-provider-observations.jsonl', 'unexpected-execution']) {
    await assert.rejects(lstat(join(directory, name)), { code: 'ENOENT' });
  }
}
async function databaseHashes() { const result = {}; for (const path of [fixture.database, `${fixture.database}-wal`, join(dirname(fixture.database), 'database-key.v1')]) try { result[path] = await digest(path); } catch (error) { if (error.code !== 'ENOENT') throw error; } return result; }
async function discover(expected) {
  const before = await databaseHashes();
  const result = await driver.invoke('discover_provider_executables'); assert.equal(result.ok, true);
  assert.deepEqual(Object.keys(result.value).sort(), ['candidates', 'truncated']); assert.equal(result.value.truncated, false);
  for (const candidate of result.value.candidates) { assert.deepEqual(Object.keys(candidate).sort(), ['location', 'path']); assert.ok(['path', 'standard', 'npm_package'].includes(candidate.location)); }
  assert.deepEqual(result.value.candidates.map(item => item.path).sort(), [...expected].sort()); evidence.discoveries.push(result.value); assert.deepEqual(await databaseHashes(), before, 'Discovery must preserve encrypted database, WAL and wrapped key bytes.'); await unchanged(); return result.value;
}
async function find() {
  await driver.click('[data-provider-find]');
  await waitFor(async () => await driver.execute('return !document.querySelector("[data-provider-find]").disabled && Boolean(document.querySelector(".provider-discovery"));'), 'discovery results');
  await waitFor(async () => await driver.execute('return document.activeElement?.matches(".provider-discovery,.provider-executable-review,#provider-executable");'), 'discovery next-action focus');
}
async function focusOwnedWindow() {
  const queue = [driver.child.pid]; let pid;
  for (let count = 0; queue.length && count < 64; count++) { const candidate = queue.shift(); try { if (await readlink(`/proc/${candidate}/exe`) === application) { pid = candidate; break; } queue.push(...(await readFile(`/proc/${candidate}/task/${candidate}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); } catch (error) { if (error.code !== 'ENOENT') throw error; } }
  assert.ok(pid, 'Find only the owned native application.'); evidence.nativeWindowFocus = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid), 'focus'], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(async () => await driver.execute('return document.hasFocus();'), 'owned native document focus');
}
async function key(value) { assert.equal(await driver.execute('return document.hasFocus();'), true); await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'discovery-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] }); }
async function resize(width, height) {
  for (let attempt = 0; attempt < 3; attempt++) { const actual = await driver.execute('return {width:innerWidth,height:innerHeight}'); if (actual.width === width && actual.height === height) return; const rect = await driver.request('GET', `/session/${driver.session}/window/rect`); await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - actual.width, height: rect.height + height - actual.height }); }
}
async function capture(name, selector, width = 1280, scale = 1) {
  await resize(width, 850);
  if (scale !== 1) await driver.execute('window.__discoveryFonts=[...document.querySelectorAll("body *")].map(node=>({node,style:node.getAttribute("style"),size:getComputedStyle(node).fontSize}));for(const item of window.__discoveryFonts)item.node.style.fontSize=`${parseFloat(item.size)*arguments[0]}px`;return true;', [scale]);
  try {
    await driver.execute('document.querySelector(arguments[0]).scrollIntoView({block:"center"});return true;', [selector]);
    const view = await driver.execute('const panel=document.querySelector(arguments[0]);return {theme:document.documentElement.dataset.theme,width:innerWidth,height:innerHeight,scrollWidth:document.scrollingElement.scrollWidth,text:panel.textContent,overflow:[panel,...panel.querySelectorAll("*")].filter(node=>{const r=node.getBoundingClientRect();return r.width>1&&r.height>1&&(r.left< -1||r.right>innerWidth+1)}).map(node=>node.tagName)};', [selector]);
    assert.equal(view.width, width); assert.ok(view.scrollWidth <= width + 1); assert.deepEqual(view.overflow, []);
    await driver.screenshot(join(output, name)); evidence.screenshots.push(name); evidence.views.push({ name, textScale: scale, scalingScope: 'Test-side CSS font size changes, not OS zoom.', ...view });
  } finally { if (scale !== 1) await driver.execute('for(const item of window.__discoveryFonts){if(item.style===null)item.node.removeAttribute("style");else item.node.setAttribute("style",item.style);}delete window.__discoveryFonts;return true;'); }
}
async function theme(value) { await driver.click(`#theme option[value="${value}"]`); await waitFor(async () => await driver.execute('return document.documentElement.dataset.theme===arguments[0]&&!document.querySelector("#theme").disabled;', [value]), `${value} appearance`); }

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await digest(application) }; evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim();
  evidence.sourceHashes = {}; const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'apps/desktop/src-tauri/permissions', 'crates/openusage-codex/src', 'crates/openusage-storage/src'], { cwd: repository, encoding: 'utf8' }).trim().split('\n');
  for (const path of [...sources, 'Cargo.toml', 'Cargo.lock', 'crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/capabilities/main-window.json']) evidence.sourceHashes[path] = await digest(join(repository, path));
  evidence.harnessHashes = {}; for (const path of ['provider-discovery.mjs', 'private-fixture.mjs', 'webdriver.mjs', 'x11-close.py', 'fixtures/synthetic-provider.py']) evidence.harnessHashes[path] = await digest(join(harness, path));
  fixture = await createFixture('provider-discovery');
  const home = join(fixture.root, 'synthetic-home'); await ownedDirectory(home); fixture.environment.HOME = home;
  const protectedDefault = join(fixture.root, 'protected-default-physical'); await ownedDirectory(protectedDefault); await symlink(protectedDefault, join(home, '.codex'));
  const customAlias = join(fixture.root, 'custom-codex-alias'); await symlink(fixture.dirs.codex, customAlias); fixture.environment.CODEX_HOME = customAlias;
  const managed = join(dirname(fixture.database), 'profiles'); await ownedDirectory(managed); const managedAlias = join(fixture.root, 'managed-alias'); await symlink(managed, managedAlias);
  const safe = join(home, '.local/bin'); const second = join(home, 'second/bin'); const alias = join(home, 'bin-alias'); const npmBin = join(home, 'npm/bin');
  for (const path of [safe, second, npmBin]) await ownedDirectory(path); await symlink(safe, alias);
  const excluded = [join(home, '.codex'), protectedDefault, customAlias, fixture.dirs.codex, managed, managedAlias];
  for (const path of [protectedDefault, fixture.dirs.codex, managed]) { await executable(join(path, 'codex')); const marker = join(path, 'synthetic-protected-content.txt'); await writeFile(marker, 'Owned synthetic protected content. No credentials.\n', { mode: 0o600 }); watched.set(marker, await digest(marker)); }
  fixture.environment.PATH = [...excluded, safe, alias, second, npmBin, '/usr/bin', '/bin'].join(':');
  evidence.syntheticRoots = { home, codexHome: customAlias, protectedDefault, protectedCustom: fixture.dirs.codex, managed, managedAlias }; evidence.discoveryPath = fixture.environment.PATH;
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log')); await driver.start();
  assert.deepEqual(await driver.invoke('discover_provider_executables'), { ok: false, error: 'workspace_locked' });
  evidence.fixedStandardLocations = { scope: 'The product also checks its fixed system installation locations. Exact candidate assertions fail visibly if another host has an additional Codex executable there.', expectedAdditionalExecutables: 0 };
  await waitFor(async () => await driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'), 'fresh workspace form');
  const password = 'OpenUsage synthetic discovery workspace 2026'; await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password); await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'encrypted discovery workspace');
  await waitFor(async () => await driver.execute('return Boolean(document.querySelector("[data-provider-settings]"));'), 'provider settings'); await driver.click('[data-provider-settings]');
  for (const args of [{ unexpected: true }, { candidates: [] }]) { const result = await driver.invoke('discover_provider_executables', args); assert.equal(result.ok, false); assert.equal(result.error, 'invalid_arguments'); }
  await discover([]); await find(); assert.equal(await driver.execute('return document.querySelector("[data-provider-manual]").closest("details").open && document.activeElement?.id==="provider-executable";'), true);
  await capture('provider-discovery-empty-desktop.png', '.provider-panel');
  const first = join(safe, 'codex'); await executable(first); await discover([first]); await find();
  await waitFor(async () => await driver.execute('return document.activeElement?.matches(".provider-executable-review");'), 'unique source review');
  assert.equal(await driver.execute('return document.querySelector("[data-provider-manual]").closest("details").open;'), false, 'A unique result closes the manual fallback opened by the earlier empty search.');
  assert.deepEqual(await driver.execute('return {checked:document.querySelector("#provider-executable-approved").checked,disabled:document.querySelector(".provider-approval-form button[type=submit]").disabled,fixture:document.querySelector(".provider-executable-review").textContent.includes("Reviewed synthetic provider fixture")};'), { checked: false, disabled: true, fixture: true }); await unchanged();
  await theme('dark'); await capture('provider-discovery-unique-dark-desktop.png', '.provider-executable-review'); await theme('light'); await capture('provider-discovery-unique-light-360.png', '.provider-executable-review', 360);
  const unknown = join(second, 'codex'); await executable(unknown, false); await discover([first, unknown]); await find();
  assert.equal(await driver.execute('return document.querySelectorAll("[name=discovered-codex]").length;'), 2); assert.equal(await driver.execute('return document.querySelector(".provider-discovery-form button[type=submit]").disabled;'), true);
  await focusOwnedWindow(); assert.equal(await driver.execute('return document.activeElement?.matches(".provider-discovery");'), true);
  await key('\uE004'); assert.deepEqual(await driver.execute('return {radio:document.activeElement?.matches("[name=discovered-codex]"),path:document.activeElement?.value};'), { radio: true, path: first });
  await key(' '); assert.equal(await driver.execute('return document.querySelector("[name=discovered-codex]:checked")?.value;'), first);
  await capture('provider-discovery-keyboard-choice-light-360.png', '.provider-discovery', 360);
  await key('\uE004'); assert.deepEqual(await driver.execute('return {review:document.activeElement?.matches(".provider-discovery-form button[type=submit]"),disabled:document.activeElement?.disabled};'), { review: true, disabled: false });
  await key('\uE007'); await waitFor(async () => await driver.execute('return document.activeElement?.matches(".provider-executable-review");'), 'keyboard-selected source review');
  assert.equal(await driver.execute('return document.querySelector(".provider-executable-review .provider-path bdi").textContent;'), first);
  assert.deepEqual(await driver.execute('return {checked:document.querySelector("#provider-executable-approved").checked,disabled:document.querySelector(".provider-approval-form button[type=submit]").disabled};'), { checked: false, disabled: true }); await unchanged();
  evidence.keyboard = { nativeOwnedWindowFocused: true, resultsRegionToRadioByTab: true, spaceSelectedFirstPath: first, tabReachedEnabledReview: true, enterReviewedChosenPath: true, approvalRemainedUnchecked: true };
  await find();
  await driver.click(`[name=discovered-codex][value="${unknown}"]`); await driver.click('.provider-discovery-form button[type=submit]');
  await waitFor(async () => await driver.execute('return document.activeElement?.matches(".provider-executable-review");'), 'selected unverified source review');
  assert.deepEqual(await driver.execute('return {checked:document.querySelector("#provider-executable-approved").checked,checkboxDisabled:document.querySelector("#provider-executable-approved").disabled,submitDisabled:document.querySelector(".provider-approval-form button[type=submit]").disabled};'), { checked: false, checkboxDisabled: true, submitDisabled: true });
  const inspected = await driver.invoke('inspect_provider_executable', { path: unknown }); assert.equal(inspected.ok, true); assert.equal(inspected.value.sourceVerified, false);
  const refused = await driver.invoke('approve_provider_executable', { candidateId: inspected.value.candidateId, confirmed: true }); assert.deepEqual(refused, { ok: false, error: 'provider_source_unverified' }); await unchanged();
  await capture('provider-discovery-unverified-light-360.png', '.provider-executable-review', 360);
  const packageRoot = join(home, 'npm/lib/node_modules/@openai/codex'); const wrapper = join(packageRoot, 'bin/codex.js'); await executable(wrapper, false); await symlink(wrapper, join(npmBin, 'codex'));
  const npmNative = join(dirname(packageRoot), 'codex-linux-x64/vendor/x86_64-unknown-linux-musl/bin/codex'); await executable(npmNative);
  const found = await discover([first, unknown, npmNative]); assert.equal(found.candidates.find(item => item.path === npmNative).location, 'npm_package'); await find();
  assert.equal(await driver.execute('return document.querySelectorAll("[name=discovered-codex]").length;'), 3);
  await capture('provider-discovery-multiple-light-360.png', '.provider-discovery', 360); await capture('provider-discovery-multiple-light-text200-360.png', '.provider-discovery', 360, 2);
  await driver.click(`[name=discovered-codex][value="${npmNative}"]`); await driver.click('.provider-discovery-form button[type=submit]');
  await waitFor(async () => await driver.execute('return document.activeElement?.matches(".provider-executable-review");'), 'npm native source review'); await unchanged();
  assert.equal(await driver.execute('return document.querySelector("#provider-executable-approved").checked;'), false); await driver.click('#provider-executable-approved'); await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(async () => await driver.execute('return document.activeElement?.matches(".provider-approval-result");'), 'explicit source approval'); await unchanged();
  evidence.steps = { syntheticHomeAndCodexHomeBeforeLaunch: true, lexicalAndPhysicalProtectedRootsExcluded: true, zeroCandidatesOpensManualEntry: true, uniqueCandidateInspectedWithoutExecution: true, uniqueAfterEmptyClosesManualFallback: true, symlinkAliasesDeduplicated: true, multipleCandidatesRequireSelection: true, unknownSourceCannotBeApproved: true, fixedNpmVendorBinaryLocatedWithoutWrapperExecution: true, explicitApprovalRequired: true, noExecutableOrProtectedContentChanges: true, discoveryPreservesDatabaseWalAndKey: true, noExecutionMarkers: true };
  for (const [path, hash] of Object.entries(evidence.sourceHashes)) assert.equal(await digest(join(repository, path)), hash); for (const [path, hash] of Object.entries(evidence.harnessHashes)) assert.equal(await digest(join(harness, path)), hash);
  assert.equal(await digest(application), evidence.application.sha256); evidence.watchedSourceHashes = Object.fromEntries(watched); evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {} }
finally { try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; } if (fixture) { stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); } evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); if (evidence.error) console.error(evidence.error); }
