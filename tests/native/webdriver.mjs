import { spawn } from 'node:child_process';
import assert from 'node:assert/strict';
import { createServer } from 'node:net';
import { resolve } from 'node:path';
import { writeFile } from 'node:fs/promises';
import { setTimeout as delay } from 'node:timers/promises';

export async function waitFor(check, label, timeout = 15000) {
  const deadline = Date.now() + timeout;
  do {
    if (await check()) return;
    await delay(100);
  } while (Date.now() < deadline);
  throw new Error(`Timed out: ${label}`);
}

async function unusedPort() {
  const server = createServer();
  await new Promise((done, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', done); });
  const port = server.address().port;
  await new Promise((done, reject) => server.close(error => error ? reject(error) : done()));
  return port;
}

export class NativeDriver {
  constructor(application, environment, logPath) {
    this.application = application;
    this.environment = environment;
    this.logPath = logPath;
    this.logs = [];
  }

  async request(method, path, body) {
    const response = await fetch(`${this.endpoint}${path}`, {
      method, headers: { 'content-type': 'application/json' },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      signal: AbortSignal.timeout(30000),
    });
    const payload = await response.json();
    if (!response.ok) throw new Error(`WebDriver ${method} ${path}: ${payload.value?.message ?? response.status}`);
    return payload.value;
  }

  async start() {
    const [port, nativePort] = await Promise.all([unusedPort(), unusedPort()]);
    this.endpoint = `http://127.0.0.1:${port}`;
    const args = ['--port', String(port), '--native-port', String(nativePort), '--native-host', '127.0.0.1'];
    if (process.env.WEBKIT_WEBDRIVER) args.push('--native-driver', resolve(process.env.WEBKIT_WEBDRIVER));
    this.child = spawn(process.env.TAURI_DRIVER ?? 'tauri-driver', args, {
      detached: true, stdio: ['ignore', 'pipe', 'pipe'], env: this.environment,
      cwd: this.environment.XDG_RUNTIME_DIR,
    });
    this.child.stdout.on('data', chunk => this.logs.push(chunk));
    this.child.stderr.on('data', chunk => this.logs.push(chunk));
    this.child.once('error', error => { this.failure = error; });
    this.child.once('exit', (code, signal) => { this.failure = new Error(`Native driver exited: ${code ?? signal}`); });
    await waitFor(async () => {
      if (this.failure) throw this.failure;
      try { return (await this.request('GET', '/status')).ready === true; } catch { return false; }
    }, 'native driver readiness');
    const created = await this.request('POST', '/session', { capabilities: { alwaysMatch: { 'tauri:options': { application: this.application } } } });
    this.session = created.sessionId;
    this.capabilities = created.capabilities;
    if (typeof this.session !== 'string') throw new Error('The native driver did not create a session.');
    await this.request('POST', `/session/${this.session}/timeouts`, { script: 15000, pageLoad: 30000, implicit: 0 });
    await waitFor(async () => Boolean(await this.execute('return window.__TAURI__?.core?.invoke && document.querySelector("#startup-heading");')), 'native bridge and startup view');
  }

  execute(script, args = []) { return this.request('POST', `/session/${this.session}/execute/sync`, { script, args }); }

  invoke(command, args) {
    return this.request('POST', `/session/${this.session}/execute/async`, {
      script: 'const done=arguments[arguments.length-1]; window.__TAURI__.core.invoke(arguments[0],arguments[1]).then(value=>done({ok:true,value}),error=>done({ok:false,error}));',
      args: [command, args ?? {}],
    });
  }

  async waitForProfileRefresh(profileId, timeout = 45000) {
    const deadline = Date.now() + timeout;
    await waitFor(async () => {
      const response = await this.invoke('load_profile_refresh_status', { profileId });
      assert.equal(response.ok, true);
      assert.deepEqual(Object.keys(response.value).sort(), ['profileId', 'retryAfterSeconds']);
      assert.equal(response.value.profileId, profileId);
      const seconds = response.value.retryAfterSeconds;
      assert.ok(Number.isInteger(seconds) && seconds >= 0 && seconds <= 4320);
      if (seconds === 0) return true;
      await delay(Math.min(seconds * 1000, 30000, Math.max(0, deadline - Date.now())));
      return false;
    }, 'real per-profile refresh eligibility', timeout);
  }

  async navigate(view) {
    assert.ok(['overview', 'accounts', 'usage', 'settings'].includes(view));
    // Account management remains testable through Settings without restoring a fourth route.
    if (await this.execute('const panel=document.querySelector("[data-account-panel]");return Boolean(panel?.open&&panel.getClientRects().length);')) {
      await this.click('[data-account-panel-close]');
      await waitFor(async () => this.execute('const panel=document.querySelector("[data-account-panel]");return Boolean(panel&&!panel.open&&!panel.getClientRects().length);'), 'account dialog closed before navigation');
    }
    const destination = view === 'accounts' ? 'settings' : view;
    const selector = `nav[aria-label="Main"] [data-workspace-nav="${destination}"]`;
    assert.equal(await this.execute('return document.querySelectorAll("nav[aria-label=Main] [data-workspace-nav=accounts]").length;'), 0);
    await waitFor(async () => this.execute('const link=document.querySelector(arguments[0]);return Boolean(link&&link.getClientRects().length&&!link.disabled);', [selector]), `available ${destination} navigation link`);
    if (!await this.execute('return document.querySelector(arguments[0])?.getAttribute("aria-current")==="page";', [selector])) await this.click(selector);
    await waitFor(async () => this.execute('const link=document.querySelector(arguments[0]),panel=document.querySelector(`[data-workspace-view="${arguments[1]}"]`),views=[...document.querySelectorAll("[data-workspace-view]")].filter(node=>["overview","usage","settings"].includes(node.getAttribute("data-workspace-view")));return link?.getAttribute("aria-current")==="page"&&views.length===3&&Boolean(panel&&!panel.hidden&&panel.getClientRects().length)&&views.every(node=>node===panel||node.hidden);', [selector, destination]), `visible ${destination} navigation destination`);
    if (view === 'accounts') {
      await this.click('[data-account-manager-open]');
      await waitFor(async () => this.execute('const panel=document.querySelector("[data-account-panel]"),profiles=panel?.querySelector(".accounts-section");return Boolean(panel?.open&&!panel.hidden&&panel.getClientRects().length&&profiles?.getClientRects().length)&&document.querySelector("nav[aria-label=Main] [data-workspace-nav=settings]")?.getAttribute("aria-current")==="page";'), 'visible account management dialog over Settings');
    } else {
      await waitFor(async () => this.execute('const panel=document.querySelector("[data-account-panel]");return Boolean(panel&&!panel.open&&!panel.getClientRects().length);'), 'account dialog stays closed on a primary view');
    }
  }

  async setAppearance(theme) {
    assert.ok(['system', 'light', 'dark'].includes(theme));
    const previous = await this.execute('return document.querySelector(".startup-panel[data-storage=ready]") ? document.querySelector("nav[aria-label=Main] [aria-current=page]")?.getAttribute("data-workspace-nav") : null;');
    if (previous) await this.navigate('settings');
    await this.selectVisibleOption('#theme', { system: 'System', light: 'Light', dark: 'Dark' }[theme]);
    await waitFor(async () => this.execute('return document.documentElement.dataset.theme===arguments[0]&&!document.querySelector("#theme").disabled;', [theme]), 'saved appearance');
    if (previous) await this.navigate(previous);
  }

  async openProfileCreate() {
    await this.navigate('accounts');
    if (!await this.execute('return document.querySelector("[data-profile-add]")?.getAttribute("aria-expanded")==="true";')) await this.click('[data-profile-add]');
    await waitFor(async () => this.execute('const field=document.querySelector("#profile-label");return Boolean(field&&!field.disabled&&field.getClientRects().length);'), 'visible Add account form');
  }

  async openProfileReadings(profileId) {
    assert.match(profileId, /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i);
    await this.navigate('accounts');
    const selector = `[data-profile-id="${profileId}"] [data-profile-action="readings"]`;
    if (!await this.execute('return document.querySelector(arguments[0])?.getAttribute("aria-expanded")==="true";', [selector])) await this.click(selector);
    await waitFor(async () => this.execute('const trigger=document.querySelector(arguments[0]),panel=document.getElementById(trigger?.getAttribute("aria-controls"));return trigger?.getAttribute("aria-expanded")==="true"&&Boolean(panel&&panel.getClientRects().length);', [selector]), 'visible profile readings');
  }

  async element(selector) {
    const element = await this.request('POST', `/session/${this.session}/element`, { using: 'css selector', value: selector });
    return element['element-6066-11e4-a52e-4f735466cecf'];
  }

  async click(selector) {
    const element = await this.element(selector);
    await this.request('POST', `/session/${this.session}/element/${element}/click`, {});
  }

  async readProfileQuota(profileId) {
    await this.openProfileOptions(profileId);
    await this.click(`[data-quota-read="${profileId}"]`);
  }

  async openProfileOptions(profileId) {
    await this.openProfileReadings(profileId);
    if (!/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(profileId)) throw new Error('A profile options action requires a UUID.');
    const selector = `[data-profile-id="${profileId}"] [data-profile-action="options"]`;
    await waitFor(async () => this.execute('const trigger=document.querySelector(arguments[0]);if(!trigger||trigger.disabled)return false;const rect=trigger.getBoundingClientRect();return trigger.tagName==="BUTTON"&&["true","false"].includes(trigger.getAttribute("aria-expanded"))&&rect.width>0&&rect.height>0&&getComputedStyle(trigger).visibility!=="hidden";', [selector]), `enabled Manage profile action for ${profileId}`);
    if (await this.execute('return document.querySelector(arguments[0]).getAttribute("aria-expanded")==="false";', [selector])) await this.click(selector);
    await waitFor(async () => this.execute('const trigger=document.querySelector(arguments[0]),panel=document.getElementById(trigger.getAttribute("aria-controls"));if(trigger.getAttribute("aria-expanded")!=="true"||!panel)return false;const rect=panel.getBoundingClientRect();return !panel.hidden&&rect.width>0&&rect.height>0&&getComputedStyle(panel).visibility!=="hidden";', [selector]), `visible profile options for ${profileId}`);
  }

  async selectVisibleOption(selector, label) {
    await waitFor(async () => this.execute('const trigger=document.querySelector(arguments[0]);return Boolean(trigger&&trigger.getAttribute("role")==="combobox"&&!trigger.disabled&&trigger.getAttribute("aria-disabled")!=="true");', [selector]), `enabled combobox ${selector}`);
    await this.click(selector);
    let option;
    await waitFor(async () => {
      option = await this.execute('const trigger=document.querySelector(arguments[0]);if(trigger?.getAttribute("aria-expanded")!=="true")return null;const roots=(trigger.getAttribute("aria-controls")??"").split(/\\s+/).filter(Boolean).map(id=>document.getElementById(id)).filter(Boolean);const matches=[...new Set(roots.flatMap(root=>[...root.querySelectorAll("[role=option]")]))].filter(node=>{const rect=node.getBoundingClientRect(),style=getComputedStyle(node);return node.textContent.trim()===arguments[1]&&node.getAttribute("aria-disabled")!=="true"&&!node.closest("[hidden],[aria-hidden=true]")&&rect.width>0&&rect.height>0&&style.visibility!=="hidden"&&style.display!=="none";});return matches.length===1?matches[0]:null;', [selector, label]);
      return Boolean(option);
    }, `one visible enabled ${label} option controlled by ${selector}`);
    await this.execute('arguments[0].scrollIntoView({block:"nearest",inline:"nearest",behavior:"instant"});return true;', [option]);
    await this.request('POST', `/session/${this.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];requestAnimationFrame(()=>requestAnimationFrame(()=>done(true)));', args: [] });
    await waitFor(async () => this.execute('const node=arguments[0],rect=node.getBoundingClientRect(),left=Math.max(0,rect.left),right=Math.min(innerWidth,rect.right),top=Math.max(0,rect.top),bottom=Math.min(innerHeight,rect.bottom);if(right<=left||bottom<=top)return false;const hit=document.elementFromPoint((left+right)/2,(top+bottom)/2);return hit===node||node.contains(hit);', [option]), `pointer-interactable ${label} option controlled by ${selector}`);
    await this.request('POST', `/session/${this.session}/element/${option['element-6066-11e4-a52e-4f735466cecf']}/click`, {});
    await waitFor(async () => this.execute('return document.querySelector(arguments[0])?.getAttribute("aria-expanded")==="false";', [selector]), `closed combobox ${selector}`);
    await waitFor(async () => this.execute('return document.querySelector(arguments[0])?.textContent.trim()===arguments[1];', [selector, label]), `selected ${label} in ${selector}`);
  }

  async replaceText(selector, text) {
    await this.click(selector);
    await this.request('POST', `/session/${this.session}/actions`, { actions: [{ type: 'key', id: 'replace-text', actions: [{ type: 'keyDown', value: '\uE009' }, { type: 'keyDown', value: 'a' }, { type: 'keyUp', value: 'a' }, { type: 'keyUp', value: '\uE009' }, { type: 'keyDown', value: '\uE003' }, { type: 'keyUp', value: '\uE003' }] }] });
    await this.request('DELETE', `/session/${this.session}/actions`);
    await waitFor(async () => this.execute('return document.querySelector(arguments[0])?.value==="";', [selector]), `keyboard-cleared ${selector}`);
    const element = await this.element(selector);
    await this.request('POST', `/session/${this.session}/element/${element}/value`, { text });
    await waitFor(async () => this.execute('return document.querySelector(arguments[0])?.value===arguments[1];', [selector, text]), `exact replacement text in ${selector}`);
  }

  async type(selector, text) {
    const element = await this.element(selector);
    await this.request('POST', `/session/${this.session}/element/${element}/clear`, {});
    await this.request('POST', `/session/${this.session}/element/${element}/value`, { text });
  }

  async screenshot(path) {
    await this.request('POST', `/session/${this.session}/execute/async`, {
      script: 'const done=arguments[arguments.length-1]; document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>done(true))));', args: [],
    });
    const encoded = await this.request('GET', `/session/${this.session}/screenshot`);
    await writeFile(path, Buffer.from(encoded, 'base64'), { mode: 0o600 });
  }

  kill(signal) {
    if (!this.child?.pid) return;
    try { process.kill(-this.child.pid, signal); } catch (error) { if (error.code !== 'ESRCH') throw error; }
  }

  async stop() {
    try {
      if (this.session) await this.request('DELETE', `/session/${this.session}`);
    } finally {
      this.session = undefined;
      this.kill('SIGTERM');
      await delay(200);
      this.kill('SIGKILL');
      await writeFile(this.logPath, Buffer.concat(this.logs), { mode: 0o600 });
    }
  }
}
