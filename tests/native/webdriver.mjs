import { spawn } from 'node:child_process';
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

  async element(selector) {
    const element = await this.request('POST', `/session/${this.session}/element`, { using: 'css selector', value: selector });
    return element['element-6066-11e4-a52e-4f735466cecf'];
  }

  async click(selector) {
    const element = await this.element(selector);
    await this.request('POST', `/session/${this.session}/element/${element}/click`, {});
  }

  async selectVisibleOption(selector, label) {
    await waitFor(async () => this.execute('const trigger=document.querySelector(arguments[0]);return Boolean(trigger&&trigger.getAttribute("role")==="combobox"&&!trigger.disabled&&trigger.getAttribute("aria-disabled")!=="true");', [selector]), `enabled combobox ${selector}`);
    await this.click(selector);
    let option;
    await waitFor(async () => {
      option = await this.execute('const trigger=document.querySelector(arguments[0]);if(trigger?.getAttribute("aria-expanded")!=="true")return null;const roots=(trigger.getAttribute("aria-controls")??"").split(/\\s+/).filter(Boolean).map(id=>document.getElementById(id)).filter(Boolean);const matches=[...new Set(roots.flatMap(root=>[...root.querySelectorAll("[role=option]")]))].filter(node=>{const rect=node.getBoundingClientRect(),style=getComputedStyle(node);return node.textContent.trim()===arguments[1]&&node.getAttribute("aria-disabled")!=="true"&&!node.closest("[hidden],[aria-hidden=true]")&&rect.width>0&&rect.height>0&&style.visibility!=="hidden"&&style.display!=="none";});return matches.length===1?matches[0]:null;', [selector, label]);
      return Boolean(option);
    }, `one visible enabled ${label} option controlled by ${selector}`);
    await this.request('POST', `/session/${this.session}/element/${option['element-6066-11e4-a52e-4f735466cecf']}/click`, {});
    await waitFor(async () => this.execute('return document.querySelector(arguments[0])?.getAttribute("aria-expanded")==="false";', [selector]), `closed combobox ${selector}`);
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
