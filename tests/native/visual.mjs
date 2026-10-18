import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import { arch, platform, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { performance } from 'node:perf_hooks';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { startSecretService, stopSecretService } from './secret-service.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(dirname(fileURLToPath(import.meta.url)), '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-visual-evidence-${randomUUID()}`));
const expectedFont = process.env.OPENUSAGE_VISUAL_FONT;
const passphrase = 'OpenUsage synthetic visual passphrase 2026';
const evidence = {
  requirement: 'NATIVE-VISUAL-001', startedAt: new Date().toISOString(), result: 'failed',
  scope: 'Actual native WebKit rendering with synthetic local data and a private Secret Service. Text enlargement uses test-side CSS font sizes. It is not operating-system zoom, provider acceptance, or release qualification.',
  platform: { os: platform(), release: release(), architecture: arch() },
  measurements: [], startupTimings: [], screenshots: [], violations: [], validationChecks: [],
};
let fixture;
let driver;
let sequence = 0;
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');

async function heading(text) {
  await waitFor(async () => (await driver.execute('return document.querySelector("#startup-heading")?.textContent;')) === text, text);
}

async function bootstrap(storage, protection) {
  const response = await driver.invoke('bootstrap');
  assert.equal(response.ok, true, 'The real native bridge must return storage metadata.');
  assert.equal(response.value.storage, storage);
  if (protection) assert.equal(response.value.keyProtection, protection);
  assert.equal(response.value.monitoring, 'stopped');
  assert.equal(response.value.provider, 'not_configured');
  return response.value;
}

async function openApp() {
  const started = performance.now();
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  evidence.capabilities ??= driver.capabilities;
  assert.equal(driver.capabilities.setWindowRect, true, 'The native driver must support real window resizing.');
  await waitFor(async () => !['Checking local storage…', undefined].includes(await driver.execute('return document.querySelector("#startup-heading")?.textContent;')), 'native startup state');
  const renderer = await driver.execute('const nav=performance.getEntriesByType("navigation")[0]; return {domContentLoadedMs:nav?.domContentLoadedEventEnd || null,loadMs:nav?.loadEventEnd || null,paintEntries:performance.getEntriesByType("paint").map(entry=>({name:entry.name,startMs:entry.startTime}))};');
  evidence.startupTimings.push({ fixture: fixture.name, nativeDriverAndStartupMs: Math.round(performance.now() - started), renderer, scope: 'One synthetic lab launch. Driver creation and polling are included. No percentile or NFR claim.' });
}

async function closeApp() {
  if (!driver) return;
  try { await driver.stop(); } finally { driver = undefined; }
}

async function disposeFixture() {
  if (!fixture) return;
  const errors = [];
  try { await closeApp(); } catch (error) { errors.push(error); }
  try { await stopSecretService(fixture); } catch (error) { errors.push(error); }
  stopGroup(fixture.bus, 'SIGKILL');
  try {
    assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
    assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
    await writeFile(join(output, `${fixture.name}-service.log`), Buffer.concat(fixture.logs), { mode: 0o600 });
    await rm(fixture.root, { recursive: true, force: true });
  } catch (error) { errors.push(error); }
  fixture = undefined;
  if (errors.length) throw errors[0];
}

async function resize(width, height) {
  for (let attempt = 0; attempt < 3; attempt++) {
    const viewport = await driver.execute('return {width:innerWidth,height:innerHeight};');
    if (viewport.width === width && viewport.height === height) return;
    const rect = await driver.request('GET', `/session/${driver.session}/window/rect`);
    await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - viewport.width, height: rect.height + height - viewport.height });
    await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];requestAnimationFrame(()=>requestAnimationFrame(()=>done(true)));', args: [] });
  }
  const actual = await driver.execute('return {width:innerWidth,height:innerHeight};');
  assert.deepEqual(actual, { width, height }, 'The real native viewport must match the requested dimensions.');
}

async function textScale(enabled, multiplier = 1.5) {
  const result = await driver.execute(`
    let restoration=null;
    if (window.__nativeVisualTextSizes) {
      for (const item of window.__nativeVisualTextSizes) {
        if (item.value) item.node.style.setProperty('font-size',item.value,item.priority); else item.node.style.removeProperty('font-size');
        if (!item.hadStyle && item.node.style.length===0) item.node.removeAttribute('style');
      }
      const connected=window.__nativeVisualTextSizes.filter(item=>item.node.isConnected);
      restoration={connected:connected.length,inlineMismatches:connected.filter(item=>item.node.style.getPropertyValue('font-size')!==item.value||item.node.style.getPropertyPriority('font-size')!==item.priority).map(item=>item.node.tagName),computedMismatches:connected.filter(item=>getComputedStyle(item.node).fontSize!==item.computed).map(item=>({tag:item.node.tagName,before:item.computed,after:getComputedStyle(item.node).fontSize}))};
      delete window.__nativeVisualTextSizes;
    }
    const before=parseFloat(getComputedStyle(document.body).fontSize);
    if (arguments[0]) {
      window.__nativeVisualTextSizes=[...document.querySelectorAll('body,body *')].map(node=>({node,computed:getComputedStyle(node).fontSize,value:node.style.getPropertyValue('font-size'),priority:node.style.getPropertyPriority('font-size'),hadStyle:node.hasAttribute('style')}));
      for (const item of window.__nativeVisualTextSizes) {
        const size=parseFloat(item.computed);
        if (Number.isFinite(size)) item.node.style.setProperty('font-size',size*arguments[1]+'px','important');
      }
    }
    return {before,after:parseFloat(getComputedStyle(document.body).fontSize),restoration};
  `, [enabled, multiplier]);
  if (result.restoration) {
    assert.deepEqual(result.restoration.inlineMismatches, [], 'Text scaling must restore each surviving inline font-size property and priority.');
    assert.deepEqual(result.restoration.computedMismatches, [], 'Text scaling must restore each surviving computed font size.');
    (evidence.textRestoration ??= []).push(result.restoration);
  }
  return result;
}

async function theme(value) {
  await driver.selectVisibleOption('#theme', { system: 'System', light: 'Light', dark: 'Dark' }[value]);
  await waitFor(async () => (await driver.execute('return document.documentElement.dataset.theme;')) === value, `native ${value} appearance`);
  await waitFor(async () => (await driver.execute('return document.querySelector("#theme").disabled;')) === false, 'appearance save');
}

async function measure(state, themeName, variant, target) {
  await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>done(true))));', args: [] });
  const data = await driver.execute(`
    const width=innerWidth;
    const visible=node=>{const rect=node.getBoundingClientRect();return node.getClientRects().length && rect.width>1 && rect.height>1 && getComputedStyle(node).visibility!=='hidden';};
    const decorative=node=>Boolean(node.closest('[aria-hidden="true"]')) && !node.matches('button,input,select,a[href]') && !node.querySelector('button,input,select,a[href]');
    const families=[...document.fonts].map(face=>({family:face.family,status:face.status,weight:face.weight}));
    const metrics=[...document.querySelectorAll('.metric')].map(card=>({label:card.querySelector('.metric-label')?.textContent.trim(),value:card.querySelector('.metric-value')?.textContent.trim()}));
    const clipped=[...document.querySelectorAll('body *')].filter(visible).filter(node=>!decorative(node) && node.textContent.trim()).filter(node=>{
      const style=getComputedStyle(node);
      return (['hidden','clip'].includes(style.overflowY) && node.scrollHeight>node.clientHeight+2) || (['hidden','clip'].includes(style.overflowX) && node.scrollWidth>node.clientWidth+2);
    }).map(node=>({tag:node.tagName,id:node.id,text:node.textContent.trim().slice(0,80)}));
    const overflowing=[...document.querySelectorAll('body *')].filter(visible).filter(node=>!decorative(node)).filter(node=>{
      const rect=node.getBoundingClientRect();return rect.right>width+1 || rect.left < -1;
    }).slice(0,12).map(node=>({tag:node.tagName,id:node.id,className:typeof node.className==='string'?node.className:''}));
    const target=arguments[0]?document.querySelector(arguments[0])?.getBoundingClientRect():null;
    const privacy=document.querySelector('[aria-controls="privacy-details"]');
    return {viewport:{width,height:innerHeight,outerWidth,outerHeight,devicePixelRatio},scrollWidth:document.scrollingElement.scrollWidth,bodyFont:getComputedStyle(document.body).fontFamily,fontCheck:document.fonts.check('16px '+JSON.stringify(arguments[1]),'OpenUsage'),fontChecks:[400,500].map(weight=>({weight,loaded:document.fonts.check(weight+' 14px '+JSON.stringify(arguments[1]),'OpenUsage 75%')})),fontStatus:document.fonts.status,fontFaces:families,metrics,clipped,overflowing,privacyRendered:Boolean(privacy && visible(privacy)),targetVisible:!arguments[0]||Boolean(target && target.bottom>0 && target.top<innerHeight)};
  `, [target, expectedFont]);
  const name = `${state}-${themeName}-${variant}.png`;
  await driver.screenshot(join(output, name));
  evidence.screenshots.push(name);
  evidence.measurements.push({ state, theme: themeName, variant, screenshot: name, ...data });
  const fail = message => evidence.violations.push({ screenshot: name, message });
  if (data.scrollWidth > data.viewport.width + 1) fail('The document has horizontal overflow.');
  if (data.overflowing.length) fail('Content extends beyond the native viewport.');
  if (data.clipped.length) fail('A text container clips its content.');
  if (!data.privacyRendered) fail('The privacy control is hidden.');
  if (!data.targetVisible) fail('The state control is absent.');
  if (data.fontStatus !== 'loaded' || !data.fontCheck || data.bodyFont.split(',')[0].replaceAll('"', '').replaceAll("'", '').trim() !== expectedFont
    || !data.fontFaces.some(face => face.family.replaceAll('"', '').replaceAll("'", '') === expectedFont && face.status === 'loaded')) fail('The required local font is not loaded.');
  for (const weight of [400, 500]) {
    if (!data.fontChecks.some(check => check.weight === weight && check.loaded)
      || !data.fontFaces.some(face => face.family.replaceAll('"', '').replaceAll("'", '') === expectedFont && face.weight === String(weight) && face.status === 'loaded')) fail(`The required local font weight ${weight} is not loaded.`);
  }
  for (const label of ['Quota remaining', 'Token activity']) {
    const metric = data.metrics.find(item => item.label?.includes(label));
    if (metric?.value !== 'Not available') fail(`Missing provider metric must remain unavailable: ${label}.`);
  }
}

async function captureState(state, target, narrow = false) {
  for (const value of ['light', 'dark']) {
    await theme(value);
    await resize(1280, 850);
    await driver.execute('window.scrollTo(0,0); if(arguments[0])document.querySelector(arguments[0]).scrollIntoView({block:"center"}); return true;', [target]);
    await measure(state, value, 'desktop', target);
    const scale = await textScale(true);
    try {
      assert.ok(Math.abs(scale.after - scale.before * 1.5) < 0.1, 'Test-side text scaling must reach 150%.');
      await driver.execute('window.scrollTo(0,0); if(arguments[0])document.querySelector(arguments[0]).scrollIntoView({block:"center"}); return true;', [target]);
      await measure(state, value, 'text150', target);
    } finally { await textScale(false); }
    if (narrow) {
      await resize(360, 800);
      await driver.execute('window.scrollTo(0,0); if(arguments[0])document.querySelector(arguments[0]).scrollIntoView({block:"center"}); return true;', [target]);
      await measure(state, value, 'narrow360', target);
    }
    if (['ready-os', 'ready-fallback'].includes(state) && value === 'dark') {
      await resize(360, 800);
      const enlarged = await textScale(true, 2);
      try {
        assert.ok(Math.abs(enlarged.after - enlarged.before * 2) < 0.1, 'READY text scaling must reach 200%.');
        await driver.execute('window.scrollTo(0,0); if(arguments[0])document.querySelector(arguments[0]).scrollIntoView({block:"center"}); return true;', [target]);
        await measure(state, value, 'text200-narrow360', target);
      } finally { await textScale(false); }
    }
  }
  await resize(1280, 850);
}

async function keyboardFocus() {
  for (const value of ['light', 'dark']) {
    await theme(value);
    await driver.execute('window.scrollTo(0,0); document.querySelector(".brand").focus(); return true;');
    await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'native-visual-keyboard', actions: [{ type: 'keyDown', value: '\uE004' }, { type: 'keyUp', value: '\uE004' }] }] });
    const focus = await driver.execute('const node=document.activeElement,style=getComputedStyle(node),rect=node.getBoundingClientRect();return {tag:node.tagName,id:node.id,label:node.textContent.trim().slice(0,80),currentNavigation:node.matches("nav a[aria-current=page]"),insideViewport:rect.left>=0&&rect.top>=0&&rect.right<=innerWidth&&rect.bottom<=innerHeight,focusVisible:node.matches(":focus-visible"),outlineStyle:style.outlineStyle,outlineWidth:parseFloat(style.outlineWidth),outlineColor:style.outlineColor};');
    const name = `keyboard-focus-${value}.png`;
    await driver.screenshot(join(output, name));
    evidence.screenshots.push(name);
    evidence.measurements.push({ state: 'keyboard-focus', theme: value, screenshot: name, focus });
    assert.equal(focus.currentNavigation, true, 'Native Tab must move from the brand to current navigation.');
    assert.equal(focus.insideViewport, true, 'The focused navigation control must remain visible.');
    assert.equal(focus.focusVisible, true, 'Native Tab must expose keyboard focus.');
    assert.ok(focus.outlineStyle !== 'none' && focus.outlineWidth >= 2 && focus.outlineColor !== 'rgba(0, 0, 0, 0)', 'Keyboard focus needs a visible outline.');
  }
}

async function clickButton(text) {
  const element = await driver.execute('return [...document.querySelectorAll("button")].find(node=>node.textContent.trim()===arguments[0]);', [text]);
  assert.ok(element, `Missing native button: ${text}`);
  await driver.request('POST', `/session/${driver.session}/element/${element['element-6066-11e4-a52e-4f735466cecf']}/click`, {});
}

async function replaceValue(selector, value) {
  const element = await driver.element(selector);
  await driver.request('POST', `/session/${driver.session}/element/${element}/clear`, {});
  await driver.type(selector, value);
}

async function validation(field, otherField, errorId, name) {
  await waitFor(async () => (await driver.execute('return document.activeElement?.id;')) === field, `focus after ${name}`);
  const result = await driver.execute('const field=document.getElementById(arguments[0]),other=document.getElementById(arguments[1]),error=document.getElementById(arguments[2]),control=field.closest("[data-slot=input-control]"),controlRect=control?.getBoundingClientRect(),controlStyle=control?getComputedStyle(control):null;return {focused:document.activeElement===field,invalid:field.getAttribute("aria-invalid"),otherInvalid:other.getAttribute("aria-invalid"),describedBy:field.getAttribute("aria-describedby"),alert:error?.getAttribute("role"),message:error?.textContent,errorColor:error?getComputedStyle(error).color:null,invalidControl:{ownsField:Boolean(control&&control.contains(field)),visible:Boolean(controlRect&&controlRect.width>0&&controlRect.height>0&&controlStyle.visibility!=="hidden"&&controlStyle.display!=="none"),borderWidth:controlStyle?parseFloat(controlStyle.borderTopWidth):0,borderStyle:controlStyle?.borderTopStyle,borderColor:controlStyle?.borderTopColor}};', [field, otherField, errorId]);
  assert.equal(result.invalid, 'true');
  assert.notEqual(result.otherInvalid, 'true');
  assert.ok(result.describedBy?.split(' ').includes(errorId));
  assert.equal(result.alert, 'alert');
  assert.ok(result.message);
  assert.equal(result.focused, true);
  assert.equal(result.invalidControl.ownsField, true, 'The selected field must belong to its coss input control.');
  assert.equal(result.invalidControl.visible, true, 'The selected invalid control must be visible.');
  assert.ok(result.invalidControl.borderWidth > 0, 'The invalid control needs a visible border.');
  assert.equal(result.invalidControl.borderStyle, 'solid');
  assert.equal(result.errorColor, result.invalidControl.borderColor, 'The error text and owning invalid control must use the same semantic error color.');
  evidence.validationChecks.push({ name, ...result });
  const screenshot = `${name}.png`;
  await driver.screenshot(join(output, screenshot));
  evidence.screenshots.push(screenshot);
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); fixture?.daemon?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  assert.equal(platform(), 'linux');
  assert.ok(expectedFont, 'Set OPENUSAGE_VISUAL_FONT to the required local font family.');
  evidence.application = { path: application, sha256: await hash(application), commit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim() };
  evidence.expectedFont = expectedFont;
  evidence.sourceDirtyPaths = execFileSync('git', ['status', '--porcelain'], { cwd: repository, encoding: 'utf8' }).trim().split('\n').filter(Boolean);
  evidence.sourceHashes = {};
  for (const path of ['Cargo.lock', 'apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/src-tauri/src/main.rs', 'apps/desktop/src-tauri/src/workspace.rs', 'apps/desktop/src/main.tsx', 'apps/desktop/src/styles.css', 'apps/desktop/src/contracts.ts', ...['Regular', 'Medium', 'SemiBold'].map(weight => `apps/desktop/public/fonts/JetBrainsMono-${weight}.woff2`), 'apps/desktop/public/fonts/JetBrainsMono-OFL.txt']) evidence.sourceHashes[path] = await hash(join(repository, path));

  fixture = await createFixture('visual-os');
  await startSecretService(fixture);
  await openApp();
  await bootstrap('setup_required', 'os_store');
  await captureState('setup-os', '.startup-panel');
  await driver.click('.startup-content .primary-button');
  await heading('Workspace ready');
  await bootstrap('ready', 'os_store');
  await captureState('ready-os', '.startup-panel', true);
  await keyboardFocus();
  await clickButton('Create a backup');
  await captureState('backup-form', '.backup-form', true);
  await driver.type('#backup-password', 'OpenUsage synthetic backup password 2026');
  await driver.type('#backup-confirmation', 'A different synthetic backup password');
  await driver.click('#backup-records');
  await driver.click('.backup-form button[type="submit"]');
  await validation('backup-confirmation', 'backup-password', 'backup-error', 'backup-password-mismatch');
  await replaceValue('#backup-password', 'é'.repeat(600));
  await replaceValue('#backup-confirmation', 'é'.repeat(600));
  assert.equal(await driver.execute('return document.querySelector("#backup-password").value.length;'), 600, 'The native Unicode text must reach the field.');
  await driver.click('.backup-form button[type="submit"]');
  await validation('backup-password', 'backup-confirmation', 'backup-error', 'backup-server-password-validation');
  assert.match(evidence.validationChecks.at(-1).message, /shorter backup password/);
  assert.deepEqual(await readdir(fixture.dirs.downloads), []);
  await driver.type('#backup-password', 'OpenUsage synthetic backup password 2026');
  await driver.type('#backup-confirmation', 'OpenUsage synthetic backup password 2026');
  await driver.click('.backup-form button[type="submit"]');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-receipt");')), 'native backup receipt', 30000);
  await waitFor(async () => (await driver.execute('return document.activeElement?.textContent.trim();')) === 'Create a backup', 'focus after backup form collapse');
  assert.equal(await driver.execute('return Boolean(document.querySelector(".backup-form"));'), false, 'The completed backup form must close.');
  evidence.backupCompletion = { formClosed: true, focusReturnedToDisclosure: true, receiptRetained: true };
  await captureState('backup-receipt', '.backup-receipt', true);
  await closeApp();
  await rename(fixture.database, `${fixture.database}.preserved`);
  await openApp();
  await bootstrap('recovery_required', 'os_store');
  await heading('Your local data needs recovery');
  await captureState('recovery-missing-database', '.startup-panel');
  await disposeFixture();

  fixture = await createFixture('visual-fallback');
  await openApp();
  await bootstrap('setup_required', 'passphrase_required');
  await captureState('setup-fallback', '.unlock-form', true);
  await driver.type('#workspace-passphrase', passphrase);
  await driver.type('#workspace-confirmation', 'A different synthetic workspace passphrase');
  await driver.click('.unlock-form button[type="submit"]');
  await validation('workspace-confirmation', 'workspace-passphrase', 'workspace-error', 'workspace-passphrase-mismatch');
  await replaceValue('#workspace-passphrase', 'é'.repeat(600));
  await replaceValue('#workspace-confirmation', 'é'.repeat(600));
  assert.equal(await driver.execute('return document.querySelector("#workspace-passphrase").value.length;'), 600, 'The native Unicode text must reach the field.');
  await driver.click('.unlock-form button[type="submit"]');
  await validation('workspace-passphrase', 'workspace-confirmation', 'workspace-error', 'workspace-server-passphrase-validation');
  assert.match(evidence.validationChecks.at(-1).message, /shorter passphrase/);
  await driver.type('#workspace-passphrase', passphrase);
  await driver.type('#workspace-confirmation', passphrase);
  await driver.click('.unlock-form button[type="submit"]');
  await heading('Workspace ready');
  await bootstrap('ready', 'passphrase');
  await closeApp();
  await openApp();
  await bootstrap('locked', 'passphrase');
  await heading('Unlock your local workspace');
  await captureState('locked-fallback', '.unlock-form');
  await driver.type('#workspace-passphrase', passphrase);
  await driver.click('.unlock-form button[type="submit"]');
  await heading('Workspace ready');
  await captureState('ready-fallback', '.startup-panel');
  await disposeFixture();
  assert.deepEqual(evidence.violations, [], 'Native visual checks found layout or font failures.');
  assert.equal(await hash(application), evidence.application.sha256, 'The native binary changed during the run.');
  for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest, `The source changed during the run: ${path}`);
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.message;
  process.exitCode = 1;
  if (driver?.session) {
    try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch { /* Preserve the original failure. */ }
  }
} finally {
  try { await disposeFixture(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  if (evidence.error) console.error(evidence.error);
}
