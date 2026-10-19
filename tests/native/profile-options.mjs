import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const forwardOnly = process.argv.includes('--forward-tab');
assert.ok(process.argv.slice(2).every(value => value === '--forward-tab'), 'Only the explicit --forward-tab scope is supported.');
const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-profile-options-${randomUUID()}`));
const password = 'OpenUsage synthetic management workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const providers = [
  { name: 'check', file: 'synthetic-provider.py', config: 'synthetic-provider-config.json', log: 'synthetic-execution-observations.jsonl', digest: '91c56f02d6d020bcb9a29c69c00bb108c306f169849971b5c42dbcd085398080', mode: 'hang', stage: 'account-read-pending', cancel: 'data-provider-cancel', result: '.provider-diagnostic', message: 'Connection check cancelled.' },
  { name: 'quota', file: 'synthetic-quota-provider.py', config: 'synthetic-quota-provider-config.json', log: 'synthetic-quota-observations.jsonl', digest: '4f41b9d2d682d885318982840e0edc2730c04ec524b24e241608d14cc96d3566', mode: 'pending', stage: 'quota-read-pending', cancel: 'data-quota-cancel', result: '.quota-result', message: 'Quota read stopped.' },
  { name: 'login', file: 'synthetic-login-provider.py', config: 'synthetic-login-provider-config.json', log: 'synthetic-login-observations.jsonl', digest: '25476e3759cbdc1cc74bcd9e46f4a01d1d85ea8ccc34425ddb45a38df27b9859', mode: 'pending', stage: 'challenge-emitted', cancel: 'data-login-cancel', result: '.login-result', message: 'Sign-in stopped' },
];
const evidence = { requirement: forwardOnly ? 'NATIVE-PROFILE-OPTIONS-FORWARD-001' : 'NATIVE-PROFILE-OPTIONS-001', result: 'failed', startedAt: new Date().toISOString(), scope: 'Actual native disclosure, keyboard and operation-continuity checks using three reviewed static synthetic providers. No real OAuth, existing credential homes, inference, accepted readings, release performance, screen reader or cross-platform qualification.', keyboardScope: forwardOnly ? 'Forward navigation only. The separate strict reverse-Tab qualification remains open.' : 'Strict reverse-Tab destination qualification.', steps: [], screenshots: [], views: [], cases: [] };
let fixture, driver, workspace, profile, other, ownedPid;
const row = () => `[data-profile-id="${profile.id}"]`;
const options = () => `${row()} [data-profile-action="options"]`;
async function observations(provider) {
  try { return (await readFile(join(fixture.root, provider.log), 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); }
  catch (error) { if (error.code === 'ENOENT') return []; throw error; }
}
async function liveHashes() {
  const value = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) {
    try { value[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  return value;
}
async function key(value, shift = false) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'management-keyboard', actions: [...(shift ? [{ type: 'keyDown', value: '\uE008' }] : []), { type: 'keyDown', value }, { type: 'keyUp', value }, ...(shift ? [{ type: 'keyUp', value: '\uE008' }] : [])] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
}
async function optionsState() {
  return driver.execute('const trigger=document.querySelector(arguments[0]),panel=trigger?.closest("[data-profile-id]").querySelector(".profile-management[data-slot=collapsible-panel]");return {controlsMatch:trigger?.getAttribute("aria-expanded")!=="true"||trigger?.getAttribute("aria-controls")===panel?.id,expanded:trigger?.getAttribute("aria-expanded"),panelPresent:Boolean(panel),panelVisible:Boolean(panel&&panel.getClientRects().length&&getComputedStyle(panel).display!=="none"),actions:panel?[...panel.querySelectorAll("button")].map(node=>({text:node.textContent.trim(),visible:Boolean(node.getClientRects().length)})):[]};', [options()]);
}
async function clickLabelCenter(selector, label) {
  await driver.execute('document.querySelector(arguments[0]).scrollIntoView({block:"center",inline:"nearest"});return true;', [selector]);
  await focusWindow();
  const point = await driver.execute('const node=document.querySelector(arguments[0]),span=node?.firstElementChild;if(!node||!span)return null;const button=node.getBoundingClientRect(),text=span.getBoundingClientRect(),x=Math.round(text.left+text.width/2),y=Math.round(text.top+text.height/2);return {tag:node.tagName,name:node.textContent.trim(),ariaLabel:node.getAttribute("aria-label"),disabled:node.disabled,tabIndex:node.tabIndex,spanText:span.textContent,spanPointerEvents:getComputedStyle(span).pointerEvents,spanHidden:span.getAttribute("aria-hidden"),x,y,button:{left:button.left,right:button.right,top:button.top,bottom:button.bottom},text:{left:text.left,right:text.right,top:text.top,bottom:text.bottom},viewport:{width:innerWidth,height:innerHeight}};', [selector]);
  assert.ok(point); assert.equal(point.tag, 'BUTTON'); assert.equal(point.name, label); assert.equal(point.ariaLabel, null); assert.equal(point.spanText, label); assert.equal(point.spanPointerEvents, 'none'); assert.notEqual(point.spanHidden, 'true'); assert.equal(point.disabled, false); assert.equal(point.tabIndex, 0);
  for (const bounds of [point.button, point.text]) assert.ok(point.x > bounds.left && point.x < bounds.right && point.y > bounds.top && point.y < bounds.bottom, 'The real pointer must hit the center of the visible label inside its button.');
  assert.ok(point.x > 0 && point.x < point.viewport.width && point.y > 0 && point.y < point.viewport.height);
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'pointer', id: 'native-button-label', parameters: { pointerType: 'mouse' }, actions: [{ type: 'pointerMove', origin: 'viewport', x: point.x, y: point.y }, { type: 'pointerDown', button: 0 }, { type: 'pointerUp', button: 0 }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
  (evidence.centeredLabelClicks ??= []).push({ selector, ...point });
}
async function manage(open) {
  assert.equal((await optionsState()).controlsMatch, true, 'An expanded trigger must control its selected profile panel.');
  if ((await optionsState()).expanded !== String(open)) await clickLabelCenter(options(), 'Manage profile');
  await waitFor(async () => { const state = await optionsState(); return state.expanded === String(open) && state.panelVisible === open; }, `${open ? 'open' : 'closed'} Manage profile`);
}
async function create(label) {
  await waitFor(async () => driver.execute('return Boolean(document.querySelector("#profile-label")&&!document.querySelector("#profile-label").disabled);'), 'profile creation control');
  await driver.type('#profile-label', label);
  await driver.click('.profile-create-form button[type=submit]');
  await waitFor(async () => driver.execute('const field=document.querySelector("#profile-label");return field&&!field.disabled&&field.value==="";'), 'created profile');
  const result = await driver.invoke('list_profiles'); assert.equal(result.ok, true);
  const added = result.value.profiles.find(value => value.label === label); assert.ok(added); return added;
}
async function approve(provider) {
  const executable = join(fixture.root, provider.name + '-synthetic-provider');
  await copyFile(join(harness, 'fixtures', provider.file), executable); await chmod(executable, 0o700);
  assert.equal(await hash(executable), provider.digest);
  await writeFile(join(fixture.root, provider.config), JSON.stringify({ defaultMode: provider.mode, modesByHome: {} }) + '\n', { mode: 0o600 });
  for (const selector of ['[data-provider-settings]', '[data-provider-manual]']) {
    if (await driver.execute('return !document.querySelector(arguments[0]).closest("details").open;', [selector])) await driver.click(selector);
  }
  await driver.type('#provider-executable', executable);
  await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(async () => driver.execute('const node=document.querySelector("#provider-executable-approved");return Boolean(node&&node.getAttribute("role")==="checkbox"&&node.getAttribute("aria-disabled")!=="true"&&!node.disabled&&node.getClientRects().length);'), 'visible reviewed-source approval checkbox');
  const before = await driver.execute('const node=document.querySelector("#provider-executable-approved");return {role:node.getAttribute("role"),checked:node.getAttribute("aria-checked"),visible:Boolean(node.getClientRects().length),submitDisabled:document.querySelector(".provider-approval-form button[type=submit]").disabled,review:document.querySelector(".provider-executable-review").textContent};');
  assert.equal(before.role, 'checkbox'); assert.equal(before.checked, 'false'); assert.equal(before.visible, true); assert.equal(before.submitDisabled, true); assert.match(before.review, /fixture/i);
  await driver.click('#provider-executable-approved');
  await waitFor(async () => driver.execute('return document.querySelector("#provider-executable-approved")?.getAttribute("aria-checked")==="true"&&!document.querySelector(".provider-approval-form button[type=submit]").disabled;'), 'explicit checked source consent');
  await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(async () => driver.execute('return document.activeElement?.matches(".provider-approval-result");'), 'approved-source result focus');
  assert.deepEqual(await observations(provider), [], 'Inspection and explicit approval must execute no provider.');
  await manage(false);
  return executable;
}
async function pending(provider, offset) {
  let selected;
  await waitFor(async () => {
    const rows = await observations(provider), starts = new Map(rows.filter(value => value.kind === 'start').map(value => [value.pid, value]));
    selected = rows.slice(offset).find(value => value.kind === provider.stage && starts.get(value.pid)?.environment.CODEX_HOME === join(workspace, 'profiles', profile.id));
    return Boolean(selected);
  }, `${provider.name} pending in selected synthetic profile`);
  return selected;
}
async function reaped(processRow) {
  await waitFor(() => { for (const id of [processRow.pid, -processRow.processGroup]) { try { process.kill(id, 0); return false; } catch (error) { if (error.code !== 'ESRCH') throw error; } } return true; }, 'owned provider and process group reaped');
}
async function visible(selector) {
  return driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&!node.disabled&&node.getClientRects().length&&getComputedStyle(node).visibility!=="hidden");', [selector]);
}
async function viewport(width) {
  for (let attempt = 0; attempt < 4; attempt++) {
    const actual = await driver.execute('return {width:innerWidth,height:innerHeight};');
    if (actual.width === width && actual.height === 850) break;
    const rect = await driver.request('GET', `/session/${driver.session}/window/rect`);
    await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - actual.width, height: rect.height + 850 - actual.height });
  }
  assert.deepEqual(await driver.execute('return {width:innerWidth,height:innerHeight};'), { width, height: 850 });
}
async function ordinaryButton() {
  await viewport(1280);
  const selector = '.sidebar-note button[aria-controls="privacy-details"]', overview = 'nav[aria-label="Main"] a[aria-current="page"][href="#main"]';
  const before = await liveHashes();
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).getAttribute("aria-expanded");', [selector]), 'false');
  await driver.execute('window.__privacyClickCount=0;window.__privacyClickListener=()=>window.__privacyClickCount++;document.querySelector(arguments[0]).addEventListener("click",window.__privacyClickListener,true);return true;', [selector]);
  try {
    await clickLabelCenter(selector, 'About your privacy');
    await waitFor(async () => driver.execute('const n=document.querySelector(arguments[0]),panel=document.getElementById("privacy-details");return n.getAttribute("aria-expanded")==="true"&&n.textContent.trim()==="Hide privacy details"&&panel?.getClientRects().length>0;', [selector]), 'one visible privacy expansion');
    const activation = await driver.execute('const n=document.querySelector(arguments[0]);return {clicks:window.__privacyClickCount,active:document.activeElement===n,tag:n.tagName,name:n.textContent.trim(),ariaLabel:n.getAttribute("aria-label")};', [selector]);
    assert.equal(activation.clicks, 1); assert.equal(activation.active, true); assert.equal(activation.tag, 'BUTTON'); assert.equal(activation.name, 'Hide privacy details'); assert.equal(activation.ariaLabel, null);
    const nativeReverseTab = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-reverse-tab.py'), String(evidence.nativeWindowFocus.windowId)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
    await waitFor(async () => driver.execute('return document.activeElement===document.querySelector(arguments[0]);', [overview]), 'ordinary button reverse Tab reaches exact Overview link', 1000);
    const destination = await driver.execute('const n=document.activeElement,s=getComputedStyle(n);return {documentFocused:document.hasFocus(),tag:n.tagName,name:n.textContent.trim(),exactOverview:n===document.querySelector(arguments[0]),focusVisible:n.matches(":focus-visible"),outlineWidth:s.outlineWidth,outlineStyle:s.outlineStyle,outlineColor:s.outlineColor,clicks:window.__privacyClickCount,privacyExpanded:document.querySelector(arguments[1]).getAttribute("aria-expanded")};', [overview, selector]);
    assert.equal(destination.documentFocused, true); assert.equal(destination.tag, 'A'); assert.equal(destination.name, 'Overview'); assert.equal(destination.exactOverview, true); assert.equal(destination.focusVisible, true); assert.ok(parseFloat(destination.outlineWidth) >= 2); assert.equal(destination.outlineStyle, 'solid'); assert.notEqual(destination.outlineColor, 'rgba(0, 0, 0, 0)'); assert.equal(destination.clicks, 1); assert.equal(destination.privacyExpanded, 'true');
    assert.deepEqual(await liveHashes(), before, 'Privacy expansion and reverse Tab must preserve database, WAL, and key bytes.');
    await driver.screenshot(join(output, 'ordinary-button-privacy-reverse-light-desktop.png')); evidence.screenshots.push('ordinary-button-privacy-reverse-light-desktop.png');
    evidence.ordinaryButton = { activation, nativeReverseTab, destination, storageHashesPreserved: true };
    await clickLabelCenter(selector, 'Hide privacy details');
    await waitFor(async () => driver.execute('return document.querySelector(arguments[0]).getAttribute("aria-expanded")==="false"&&!document.getElementById("privacy-details");', [selector]), 'privacy restored closed');
    assert.equal(await driver.execute('return window.__privacyClickCount;'), 2);
    assert.deepEqual(await liveHashes(), before);
  } finally {
    await driver.execute('document.querySelector(arguments[0]).removeEventListener("click",window.__privacyClickListener,true);delete window.__privacyClickListener;delete window.__privacyClickCount;return true;', [selector]);
  }
}
async function capture(name, width = 360, scale = 1) {
  await viewport(width);
  if (scale !== 1) await driver.execute('window.__managementFontStyles=[...document.querySelectorAll("body,body *")].map(node=>({node,value:node.style.getPropertyValue("font-size"),priority:node.style.getPropertyPriority("font-size"),hadStyle:node.hasAttribute("style"),size:getComputedStyle(node).fontSize}));for(const item of window.__managementFontStyles)item.node.style.setProperty("font-size",`${parseFloat(item.size)*arguments[0]}px`,"important");return true;', [scale]);
  try {
    await driver.execute('document.querySelector(arguments[0]).scrollIntoView({block:"center"});return true;', [row()]);
    const view = await driver.execute('const panel=document.querySelector(arguments[0]);return {width:innerWidth,height:innerHeight,scrollWidth:document.scrollingElement.scrollWidth,theme:document.documentElement.dataset.theme,bodyFont:getComputedStyle(document.body).fontFamily,overflow:[panel,...panel.querySelectorAll("*")].filter(node=>{const rect=node.getBoundingClientRect();return rect.width>1&&rect.height>1&&(rect.left< -1||rect.right>innerWidth+1);}).map(node=>({tag:node.tagName,text:node.textContent.slice(0,80)}))};', [row()]);
    assert.equal(view.width, width); assert.ok(view.scrollWidth <= width + 1); assert.deepEqual(view.overflow, []);
    await driver.screenshot(join(output, name)); evidence.screenshots.push(name); evidence.views.push({ name, textScale: scale, scalingScope: 'Test-side CSS enlargement, not OS zoom or full accessibility acceptance.', ...view });
  } finally {
    if (scale !== 1) {
      const restoration = await driver.execute('for(const item of window.__managementFontStyles){if(item.value)item.node.style.setProperty("font-size",item.value,item.priority);else item.node.style.removeProperty("font-size");if(!item.hadStyle&&item.node.style.length===0)item.node.removeAttribute("style");}const connected=window.__managementFontStyles.filter(item=>item.node.isConnected),result={connected:connected.length,inlineMismatches:connected.filter(item=>item.node.style.getPropertyValue("font-size")!==item.value||item.node.style.getPropertyPriority("font-size")!==item.priority).map(item=>item.node.tagName),computedMismatches:connected.filter(item=>getComputedStyle(item.node).fontSize!==item.size).map(item=>({tag:item.node.tagName,before:item.size,after:getComputedStyle(item.node).fontSize}))};delete window.__managementFontStyles;return result;');
      (evidence.textRestoration ??= []).push(restoration);
      assert.deepEqual(restoration.inlineMismatches, [], 'Text scaling must restore inline font-size values and priorities.');
      assert.deepEqual(restoration.computedMismatches, [], 'Text scaling must restore surviving computed font sizes.');
    }
  }
}
async function focusWindow() {
  const queue = [driver.child.pid];
  for (let count = 0; queue.length && count < 64; count++) {
    const candidate = queue.shift();
    try {
      if (await readlink(`/proc/${candidate}/exe`) === application) { ownedPid = candidate; break; }
      queue.push(...(await readFile(`/proc/${candidate}/task/${candidate}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number));
    } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(ownedPid);
  evidence.nativeWindowFocus = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(ownedPid), 'focus'], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(async () => driver.execute('return document.hasFocus();'), 'owned native keyboard focus');
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await hash(application) };
  evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim();
  evidence.sourceHashes = {};
  const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'apps/desktop/src-tauri/permissions', 'apps/desktop/public/fonts', 'crates/openusage-codex/src', 'crates/openusage-storage/src'], { cwd: repository, encoding: 'utf8' }).trim().split('\n');
  for (const path of [...sources, 'Cargo.toml', 'Cargo.lock', 'package.json', 'package-lock.json', 'apps/desktop/index.html', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/src-tauri/capabilities/main-window.json']) evidence.sourceHashes[path] = await hash(join(repository, path));
  evidence.harnessHashes = {};
  for (const path of ['profile-options.mjs', 'private-fixture.mjs', 'webdriver.mjs', 'x11-close.py', 'x11-reverse-tab.py', ...providers.map(value => 'fixtures/' + value.file)]) evidence.harnessHashes[path] = await hash(join(harness, path));
  fixture = await createFixture('profile-options'); workspace = dirname(fixture.database);
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log')); await driver.start();
  await waitFor(async () => driver.execute('return ["workspace-passphrase","workspace-confirmation"].every(id=>{const node=document.getElementById(id);return Boolean(node&&!node.disabled);});'), 'enabled setup fields');
  await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password); await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'synthetic workspace setup');
  profile = await create('Selected synthetic management profile'); other = await create('Other synthetic management profile');
  await focusWindow();
  await driver.selectVisibleOption('#theme', 'Light');
  await waitFor(async () => driver.execute('return document.documentElement.dataset.theme==="light"&&!document.querySelector("#theme").disabled;'), 'saved light appearance');
  await ordinaryButton();
  for (const provider of providers) {
    provider.executable = await approve(provider);
    const idle = await driver.execute('const row=document.querySelector(arguments[0]);return [...row.querySelectorAll("button")].filter(node=>node.getClientRects().length&&!node.disabled).map(node=>node.textContent.trim());', [row()]);
    assert.deepEqual(idle, ['Sign in', 'Read quotas', 'Manage profile']);
    const state = await optionsState(); assert.equal(state.panelPresent, true); assert.equal(state.panelVisible, false); assert.ok(state.actions.length >= 3 && state.actions.every(value => !value.visible));
    const before = await liveHashes(), offset = (await observations(provider)).length;
    if (provider.name === 'check') { await manage(true); await driver.click(`${row()} [data-provider-check]`); }
    if (provider.name === 'quota') await driver.click(`${row()} [data-quota-read]`);
    if (provider.name === 'login') {
      await driver.click(`${row()} [data-login-start]`);
      await waitFor(async () => visible(`${row()} [data-login-consent] button[type=submit]`), 'explicit profile consent');
      assert.equal((await observations(provider)).length, offset, 'Sign-in consent alone must execute nothing.');
      await driver.click(`${row()} [data-login-consent] button[type=submit]`);
    }
    const processRow = await pending(provider, offset), cancel = `${row()} [${provider.cancel}="${profile.id}"]`;
    await waitFor(async () => visible(cancel), `${provider.name} visible Cancel`);
    await manage(true); await manage(false);
    await key(' '); await waitFor(async () => (await optionsState()).panelVisible, 'keyboard reopen during pending operation');
    await key('\uE007'); await waitFor(async () => !(await optionsState()).panelVisible, 'keyboard close during pending operation');
    assert.equal(await visible(cancel), true, 'Closing Manage must keep active Cancel visible.');
    process.kill(processRow.pid, 0);
    const beforeCancel = await observations(provider);
    assert.equal(beforeCancel.slice(offset).filter(value => value.kind === 'start' && value.arguments[0] === 'app-server').length, 1, 'Disclosure must not replace the active provider process.');
    assert.equal(beforeCancel.filter(value => value.kind === 'request' && value.request.method === 'account/login/cancel').length, 0, 'Disclosure must not cancel a sign-in.');
    assert.equal(await driver.execute('return Boolean(document.querySelector(arguments[0]));', [`[data-profile-id="${other.id}"] [${provider.cancel}]`]), false, 'Cancel must belong only to the selected profile.');
    await capture(`management-${provider.name}-pending-light-360.png`);
    const focusSnapshot = () => driver.execute('const node=document.activeElement;return {documentFocused:document.hasFocus(),tag:node?.tagName,text:node?.textContent.trim().slice(0,100),disabled:node?.disabled??null,ariaDisabled:node?.getAttribute("aria-disabled"),manage:node?.matches(arguments[0]),cancel:node?.matches(arguments[1])};', [options(), cancel]);
    const keyboardFocus = { operation: provider.name, afterResize: await focusSnapshot() };
    await focusWindow();
    keyboardFocus.afterOwnedWindowFocus = await focusSnapshot();
    keyboardFocus.rowControls = await driver.execute('const row=document.querySelector(arguments[0]),manage=document.querySelector(arguments[1]);return [...row.querySelectorAll("button,input,textarea,select,a[href],[tabindex]")].map(node=>{const rect=node.getBoundingClientRect(),style=getComputedStyle(node);return {tag:node.tagName,id:node.id,text:node.textContent.trim().slice(0,60),role:node.getAttribute("role"),disabled:node.disabled??null,ariaDisabled:node.getAttribute("aria-disabled"),tabIndex:node.tabIndex,display:style.display,visibility:style.visibility,rect:{x:rect.x,y:rect.y,width:rect.width,height:rect.height},blockedAncestors:[...function*(start){for(let current=start;current;current=current.parentElement)if(current.hasAttribute("hidden")||current.hasAttribute("inert")||current.matches("fieldset[disabled]"))yield current;}(node)].map(ancestor=>({tag:ancestor.tagName,id:ancestor.id,hidden:ancestor.getAttribute("hidden"),inert:ancestor.hasAttribute("inert"),disabled:ancestor.hasAttribute("disabled")})),beforeManage:Boolean(node.compareDocumentPosition(manage)&Node.DOCUMENT_POSITION_FOLLOWING)};});', [row(), options()]);
    await driver.execute('window.__managementKeyTrace=[];window.__managementKeyListener=event=>{window.__managementKeyTrace.push({type:event.type,key:event.key,code:event.code,keyCode:event.keyCode,shift:event.shiftKey,phase:event.eventPhase,listener:"capture",prevented:event.defaultPrevented,target:event.target?.textContent.trim().slice(0,60),active:document.activeElement?.textContent.trim().slice(0,60)});};window.__managementKeyBubble=event=>{window.__managementKeyTrace.push({type:event.type,key:event.key,code:event.code,keyCode:event.keyCode,shift:event.shiftKey,phase:event.eventPhase,listener:"bubble",prevented:event.defaultPrevented,target:event.target?.textContent.trim().slice(0,60),active:document.activeElement?.textContent.trim().slice(0,60)});};for(const name of ["keydown","keyup","focusin","focusout"]){window.addEventListener(name,window.__managementKeyListener,true);window.addEventListener(name,window.__managementKeyBubble,false);}return true;');
    if (forwardOnly) {
      keyboardFocus.forwardTraversal = [];
      for (let count = 0; count < 64; count++) {
        const previous = await driver.execute('window.__managementPreviousFocus=document.activeElement;return true;');
        assert.equal(previous, true);
        await key('\uE004');
        await waitFor(async () => driver.execute('return document.activeElement!==window.__managementPreviousFocus;'), 'native forward Tab focus change', 1500);
        const target = await driver.execute('const node=document.activeElement;return {tag:node?.tagName,id:node?.id,text:node?.textContent.trim().slice(0,100),profile:node?.closest("[data-profile-id]")?.getAttribute("data-profile-id"),cancel:node?.matches(arguments[0]),closedManagement:Boolean(node?.closest(".profile-management[data-closed]")),hidden:Boolean(node?.closest("[hidden],[inert]"))};', [cancel]);
        keyboardFocus.forwardTraversal.push(target);
        assert.equal(target.closedManagement, false, 'Forward Tab must skip closed maintenance controls.');
        assert.equal(target.hidden, false, 'Forward Tab must skip hidden or inert controls.');
        if (target.cancel) break;
      }
      keyboardFocus.afterForwardTab = await focusSnapshot();
    } else {
      keyboardFocus.nativeReverseTab = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-reverse-tab.py'), String(evidence.nativeWindowFocus.windowId)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
      keyboardFocus.immediatelyAfterReverseTab = await focusSnapshot();
      await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];requestAnimationFrame(()=>requestAnimationFrame(()=>done(true)));', args: [] });
      keyboardFocus.afterTwoFrames = await focusSnapshot();
      try { await waitFor(async () => driver.execute('return document.activeElement?.matches(arguments[0]);', [cancel]), 'settled keyboard focus on active Cancel', 1000); }
      catch (error) { keyboardFocus.focusWaitError = error.message; }
      keyboardFocus.afterReverseTab = await focusSnapshot();
    }
    keyboardFocus.events = await driver.execute('const events=window.__managementKeyTrace;for(const name of ["keydown","keyup","focusin","focusout"]){window.removeEventListener(name,window.__managementKeyListener,true);window.removeEventListener(name,window.__managementKeyBubble,false);}delete window.__managementKeyTrace;delete window.__managementKeyListener;delete window.__managementKeyBubble;delete window.__managementPreviousFocus;return events;');
    evidence.keyboardFocusChecks ??= []; evidence.keyboardFocusChecks.push(keyboardFocus);
    assert.ok(keyboardFocus.events.some(event => event.type === 'keydown' && (event.code === 'Tab' || event.keyCode === 9) && event.shift === !forwardOnly), 'Actual native events must identify physical Tab with the requested modifier.');
    assert.equal((forwardOnly ? keyboardFocus.afterForwardTab : keyboardFocus.afterReverseTab).cancel, true, `${forwardOnly ? 'Forward' : 'Reverse'} Tab must reach the active Cancel while Manage is closed.`);
    await key('\uE007');
    await waitFor(async () => driver.execute('const result=document.querySelector(arguments[0]);return Boolean(result&&result.getClientRects().length&&result.textContent.includes(arguments[1]));', [`${row()} ${provider.result}`, provider.message]), 'visible terminal result');
    await reaped(processRow);
    assert.deepEqual(await liveHashes(), before, 'Provider read/cancel and disclosure must preserve database, WAL, and key bytes.');
    evidence.cases.push({ operation: provider.name, selectedProfileId: profile.id, processId: processRow.pid, processGroup: processRow.processGroup, disclosurePreservedOwnedProcess: true, closedControlsHidden: true, actualKeyboardToggle: true, ...(forwardOnly ? { forwardTabReachesActiveCancel: true } : { reverseTabReachesActiveCancel: true }), cancelVisibleOutsidePanel: true, keyboardCancelReapedProcess: true, storageHashesPreserved: true });
  }
  await capture('management-idle-light-desktop.png', 1280); await capture('management-idle-light-360.png'); await manage(true); await capture('management-expanded-light-360.png'); await capture('management-expanded-light-text200-360.png', 360, 2); await manage(false);
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  for (const provider of providers) { assert.equal(await hash(provider.executable), provider.digest); for (const start of (await observations(provider)).filter(value => value.kind === 'start')) { assert.equal(start.environment.CODEX_HOME, join(workspace, 'profiles', profile.id)); assert.deepEqual(start.unexpectedEnvironmentKeys, []); await reaped(start); } }
  evidence.keyboardMethod = forwardOnly ? { forwardTab: 'Actual WebDriver Tab from Manage, bounded to 64 visible focus stops, then Enter on the exact selected Cancel.', qualification: 'This scope does not pass or replace the preserved strict reverse-Tab test.' } : { reverseTab: 'Owned private X11 XTEST Shift+Tab with observed physical Tab code and shiftKey events. WebDriver supplies Space, Enter and pointer input.', reference: 'https://xorg.freedesktop.org/archive/X11R7.7/doc/libXtst/xtestlib.html', qualification: 'WebKitGTK may expose ISO_Left_Tab as Unidentified. The exact Cancel destination remains mandatory. Plain native buttons traverse despite that key string; no platform-only root cause is claimed.' };
  evidence.steps.push({ threeIdleActions: true, explicitSourceConsentThroughVisibleRoleCheckbox: true, inspectionExecutesNothing: true, maintenanceDisclosurePreservesAllThreeOperationKinds: true, noExistingCredentialHomes: true });
  assert.equal(await hash(application), evidence.application.sha256);
  for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest, `Source drift: ${path}`);
  for (const [path, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, path)), digest, `Harness drift: ${path}`);
  evidence.nativeClose = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(ownedPid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(async () => { try { await readlink(`/proc/${ownedPid}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native exit');
  driver.session = undefined; await driver.stop(); driver = undefined; evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {}
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) {
    evidence.observations = {};
    for (const provider of providers) {
      const rows = await observations(provider); evidence.observations[provider.name] = rows;
      if (evidence.result !== 'passed' && provider.executable) for (const start of rows.filter(value => value.kind === 'start')) {
        try { const command = await readFile(`/proc/${start.pid}/cmdline`); if (start.pid === start.processGroup && command.includes(Buffer.from(provider.executable + '\0'))) process.kill(-start.processGroup, 'SIGTERM'); }
        catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) throw error; }
      }
    }
    stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true });
  }
  evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); if (evidence.error) console.error(evidence.error);
}
