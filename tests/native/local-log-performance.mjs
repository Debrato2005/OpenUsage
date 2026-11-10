import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { availableParallelism, cpus, release, totalmem } from 'node:os';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_NATIVE_EVIDENCE']) assert.ok(process.env[name], name);
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const scanner = join(repository, 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu');
assert.equal(availableParallelism(), 4, 'Run the benchmark with four CPU affinity slots.');
const pin = JSON.parse(await readFile(join(repository, 'crates/openusage-locallogs/sources/ccusage-20.0.48-linux-x86_64.json'), 'utf8'));
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
assert.equal(await hash(scanner), pin.executableSha256);
const evidence = { requirement: 'OU-NFR-020', scope: 'Linux release native-test build, synthetic Claude Code logs, private password workspace, four CPU affinity slots. Consent-to-visible history includes workspace creation.',
  commit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim(),
  kernel: release(), cpu: cpus()[0].model, cpuSlots: availableParallelism(), memoryBytes: totalmem(),
  applicationSha256: await hash(application), scannerSha256: pin.executableSha256,
  result: 'failed', samples: [], startedAt: new Date().toISOString() };
await mkdir(output, { recursive: true, mode: 0o700 });
let fixture, driver;
const call = async (command, args) => {
  const response = await driver.invoke(command, args);
  assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`);
  return response.value;
};
const generate = (action, folder) => JSON.parse(execFileSync('python3', [join(repository, 'tests/tools/claude_scale_fixture.py'), action, folder], {
  encoding: 'utf8', timeout: 60000, maxBuffer: 1024 * 1024,
}));

try {
  for (let index = 0; index < 3; index++) {
    fixture = await createFixture('local-log-performance');
    const logs = join(fixture.root, 'logs');
    await mkdir(logs, { mode: 0o700 });
    const source = generate('create', logs);
    Object.assign(fixture.environment, { OPENUSAGE_NATIVE_LOG_HOME: logs, OPENUSAGE_NATIVE_CCUSAGE: scanner });
    driver = new NativeDriver(application, fixture.environment, join(output, `driver-${index}.log`));
    await driver.start();
    await waitFor(() => driver.execute('return document.querySelectorAll(".local-log-choice .local-log-tool").length===1;'), 'Claude Code consent');
    assert.deepEqual(await driver.execute('return [...document.querySelectorAll(".local-log-choice .local-log-tool")].map(n=>n.textContent.trim());'), ['Claude Code']);
    await driver.type('#workspace-passphrase', 'OpenUsage synthetic performance 2026');
    await driver.type('#workspace-confirmation', 'OpenUsage synthetic performance 2026');
    await driver.execute(`
      window.__historyTiming={start:performance.now(),end:null};
      const observer=new MutationObserver(()=>{
        const total=document.querySelector('[data-local-overview] [data-local-tool=claude] [data-local-tool-total]');
        if(total?.getAttribute('data-local-tool-total')===arguments[0]&&total.getClientRects().length){
          observer.disconnect();
          requestAnimationFrame(()=>requestAnimationFrame(()=>window.__historyTiming.end=performance.now()));
        }
      });
      observer.observe(document.body,{subtree:true,childList:true,attributes:true});
    `, [String(source.tokens)]);
    await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'consent', actions: [{ type: 'keyDown', value: '\uE007' }, { type: 'keyUp', value: '\uE007' }] }] });
    await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready workspace', 30000);
    assert.equal((await call('bootstrap')).schemaVersion, 13);
    await waitFor(() => driver.execute('return window.__historyTiming.end!==null;'), 'visible saved history', 90000);
    const seconds = await driver.execute('return (window.__historyTiming.end-window.__historyTiming.start)/1000;');
    const report = await call('load_local_usage', { days: 30, period: 'day' });
    assert.equal(report.tools.length, 1);
    assert.equal(report.tools[0].totalTokens, String(source.tokens));
    assert.equal(report.sessions.length, source.files);
    assert.ok(report.sessions.every(session => session.totalTokens === String(source.sessionTokens)));
    assert.deepEqual(generate('hashes', logs), source.hashes);
    evidence.samples.push({ seconds, bytes: source.bytes, files: source.files, records: source.records, tokens: source.tokens });
    console.log(`Native consent sample ${index + 1}: ${seconds.toFixed(3)} seconds`);
    assert.ok(seconds <= 60, `First history took ${seconds} seconds`);
    await driver.stop(); driver = undefined;
    stopGroup(fixture.bus, 'SIGKILL');
    await rm(fixture.root, { recursive: true, force: true }); fixture = undefined;
  }
  evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally {
  try { await driver?.stop(); } finally {
    stopGroup(fixture?.bus, 'SIGKILL');
    if (fixture) await rm(fixture.root, { recursive: true, force: true });
    evidence.finishedAt = new Date().toISOString();
    await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
  }
}
