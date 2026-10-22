import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

// Native-test WebKitGTK with the approved synthetic provider in a private workspace.
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const evidence = { result: 'failed', scope: 'Linux native-test Tauri/WebKitGTK on isolated X11 with approved synthetic-provider readings. Read-only individual and user-curated aggregation, synchronized donut/heatmap/date controls, strict IPC refusal and genuine restart. No real provider or verified-identity contract qualification.' };
let fixture, driver;
let sequence = 0;
try {
  await mkdir(output, { recursive: true, mode: 0o700 });
  evidence.applicationSha256 = createHash('sha256').update(await readFile(application)).digest('hex');
  fixture = await createFixture('heatmap-window');
  fixture.environment.HOME = join(fixture.root, 'synthetic-home');
  await mkdir(fixture.environment.HOME, { mode: 0o700 });
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'), 'private workspace setup');
  const password = 'OpenUsage synthetic countdown workspace 2026';
  assert.equal((await driver.invoke('load_time_zone')).error, 'workspace_locked');
  await driver.type('#workspace-passphrase', password);
  await driver.type('#workspace-confirmation', password);
  await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'ready workspace');
  for (const label of ['Countdown account A', 'Countdown account B']) {
    await driver.openProfileCreate();
    await driver.type('#profile-label', label);
    await driver.click('.profile-create-form button[type=submit]');
    await waitFor(() => driver.execute('return document.querySelector("#profile-label")?.value==="";'), 'profile created');
  }
  const profiles = (await driver.invoke('list_profiles')).value.profiles;
  const a = profiles.find(p => p.label === 'Countdown account A'), b = profiles.find(p => p.label === 'Countdown account B');
  assert.ok(a && b);
  const executable = join(fixture.root, 'synthetic-collection-provider.py');
  await copyFile(resolve('crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py'), executable);
  await chmod(executable, 0o700);
  await driver.navigate('settings');
  await driver.click('[data-provider-settings]'); await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#provider-executable-approved")&&!document.querySelector("#provider-executable-approved").disabled);'), 'reviewed synthetic source');
  await driver.click('#provider-executable-approved'); await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector(".provider-approval-result"));'), 'fixture approved');
  const workspace = dirname(fixture.database);
  const stateOf = async id => {
    const result = await driver.invoke('load_profile_activity',{profileId:id}); assert.equal(result.ok,true);
    const r=result.value; return {binding:r.bindingId,run:r.collectionRunId,rows:r.dailyUsageBuckets.map(d=>[d.date,d.tokens])};
  };
  for (const p of [a,b]) {
    await writeFile(join(workspace,'profiles',p.id,'synthetic-collection-config.json'),JSON.stringify({mode:'dashboard_success',requireLogin:true}),{mode:0o600});
    await driver.navigate('overview');
    await driver.click(`[data-overview-account="${p.id}"] td:last-child button:first-child`);
    await waitFor(async()=>Boolean((await driver.invoke('load_profile_activity',{profileId:p.id})).value),'selected synthetic sign-in and first activity',45000);
    await driver.request('POST',`/session/${driver.session}/actions`,{actions:[{type:'key',id:'close-account',actions:[{type:'keyDown',value:'\uE00C'},{type:'keyUp',value:'\uE00C'}]}]});
    await waitFor(()=>driver.execute('return !document.querySelector("[data-account-panel]")?.open;'),'account dialog closed');
  }
  const beforeA=await stateOf(a.id),beforeB=await stateOf(b.id);
  const end=new Date().toISOString().slice(0,10),start=new Date(Date.parse(end)-89*86400000).toISOString().slice(0,10);
  const args={profileIds:[a.id,b.id],view:'profile_sum',mode:'daily',startDate:start,endDate:end};
  const files=[fixture.database,fixture.database+'-wal',join(workspace,'database-key.v1')];
  const hashes=async()=>Promise.all(files.map(async path=>createHash('sha256').update(await readFile(path)).digest('hex')));
  const beforeFiles=await hashes();
  const request=async overrides=>{const r=await driver.invoke('load_activity_dashboard',{...args,...overrides});assert.equal(r.ok,true,JSON.stringify(r));return r.value;};
  const expected=beforeA.rows.filter(([date])=>date>=start&&date<=end).reduce((sum,[,tokens])=>sum+BigInt(tokens),0n);
  const daily=await request({});
  assert.equal(daily.totalTokens,String(expected*2n));assert.equal(daily.scopeVerified,false);
  assert.equal(daily.members.length,2);assert.deepEqual(daily.members.map(m=>m.shareBasisPoints),[5000,5000]);
  assert.equal(daily.knownContributions,14);assert.equal(daily.expectedContributions,180);
  assert.equal(daily.cells.length,90);assert.ok(daily.cells.some(c=>c.tokens===null&&c.knownContributions===0));
  for(const mode of ['weekly','cumulative']) {
    const r=await request({mode});assert.equal(r.totalTokens,daily.totalTokens);assert.deepEqual(r.members,daily.members);
    if(mode==='cumulative')assert.equal(r.cells.at(-1).tokens,daily.totalTokens);
    else assert.equal(r.cells.filter(c=>c.tokens!==null).reduce((n,c)=>n+BigInt(c.tokens),0n),expected*2n);
  }
  const verified=await request({view:'verified'});assert.equal(verified.status,'unavailable');assert.equal(verified.totalTokens,null);assert.equal(verified.members.length,0);
  assert.ok(verified.exclusions.every(e=>e.reason==='identity_unverified'));
  for(const invalid of [{profileIds:[a.id,a.id]},{view:'guess'},{mode:'hourly'},{startDate:'2026-02-30'},{startDate:'2025-01-01',endDate:'2026-01-01'},{extra:'refuse'}]) assert.deepEqual(await driver.invoke('load_activity_dashboard',{...args,...invalid}),{ok:false,error:'invalid_arguments'});
  assert.deepEqual(await hashes(),beforeFiles,'Aggregation must not change database, WAL or key bytes');
  await driver.navigate('overview');
  await driver.click('[data-activity-scope] button:nth-child(2)');
  await waitFor(()=>driver.execute('return document.querySelector("[data-overview-account-share] .overview-donut-center strong")?.title===arguments[0];',[new Intl.NumberFormat('en-US').format(expected*2n)]),'native synchronized cohort total');
  assert.equal(await driver.execute('return document.querySelectorAll("[data-share-profile]").length;'),2);
  assert.match(await driver.execute('return document.querySelector(".activity-sum-notice summary").textContent;'),/Unverified/);
  assert.equal(await driver.execute('return [...document.querySelectorAll("[data-share-profile]")].every(n=>n.textContent.includes("50%"));'),true);
  await driver.request('POST', `/session/${driver.session}/window/rect`, {width:1440,height:1100});
  await driver.execute('window.scrollTo(0,0);');
  await driver.screenshot(join(output,'overview-activity-sum.png'));
  const clickMode=async mode=>{await driver.click(`[data-overview] [data-heatmap-mode=${mode}]`);await waitFor(()=>driver.execute('return document.querySelector("[data-overview] [data-token-heatmap]")?.dataset.mode===arguments[0]&&!document.querySelector("[data-overview] [data-token-heatmap]").getAttribute("aria-busy").includes("true");',[mode]),'selected native mode');};
  await clickMode('weekly');await clickMode('cumulative');await clickMode('daily');
  const [date,tokens]=beforeA.rows.find(([,tokens])=>BigInt(tokens)>0n);
  await driver.click('.activity-date-range summary');
  await driver.execute('for(const [name,value] of [["Activity from",arguments[0]],["Activity to",arguments[0]]]){const n=document.querySelector(`[aria-label="${name}"]`);Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,"value").set.call(n,value);n.dispatchEvent(new Event("input",{bubbles:true}));n.dispatchEvent(new Event("change",{bubbles:true}));}',[date]);
  await waitFor(()=>driver.execute('return document.querySelector("[data-overview] [data-token-heatmap]")?.dataset.periodDays==="1"&&document.querySelectorAll("[data-overview] [data-heatmap-cell]").length===1;'),'native custom provider date');
  const exact = new Intl.NumberFormat('en-US');
  await waitFor(()=>driver.execute('return document.querySelector("[data-overview-account-share] .overview-donut-center strong")?.title===arguments[0];',[exact.format(BigInt(tokens)*2n)]),'custom date changes donut denominator');
  await driver.click(`[aria-label="Include ${b.label}"]`);
  await waitFor(()=>driver.execute('return document.querySelectorAll("[data-share-profile]").length===1&&document.querySelector("[data-overview-account-share] .overview-donut-center strong")?.title===arguments[0];',[exact.format(BigInt(tokens))]),'selected account changes both charts');
  assert.equal(await driver.execute('return document.querySelector("[data-overview] [data-heatmap-cell]").dataset.exactValue;'),tokens);
  assert.deepEqual(await stateOf(a.id),beforeA);assert.deepEqual(await stateOf(b.id),beforeB);assert.deepEqual(await hashes(),beforeFiles);
  const queue=[driver.child.pid];let owned;
  for(let n=0;queue.length&&n<64;n++){const pid=queue.shift();try{if(await readlink(`/proc/${pid}/exe`)===application){owned=pid;break;}queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`,'utf8')).trim().split(/\s+/).filter(Boolean).map(Number));}catch(error){if(error.code!=='ENOENT')throw error;}}
  assert.ok(owned);execFileSync('/usr/bin/python3',[resolve('tests/native/x11-close.py'),String(owned)],{env:fixture.environment,encoding:'utf8',timeout:10000});
  await waitFor(async()=>{try{await readlink(`/proc/${owned}/exe`);return false;}catch(error){if(error.code==='ENOENT')return true;throw error;}},'genuine native exit');
  driver.session=undefined;await driver.stop();
  driver=new NativeDriver(application,fixture.environment,join(output,`driver-${++sequence}.log`));await driver.start();
  await waitFor(()=>driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'),'reopened unlock form ready');
  await driver.type('#workspace-passphrase',password);await driver.click('.unlock-form button[type=submit]');
  await waitFor(async()=>(await driver.invoke('bootstrap')).value?.storage==='ready','reopened encrypted workspace');
  assert.deepEqual(await stateOf(a.id),beforeA);assert.deepEqual(await stateOf(b.id),beforeB);
  assert.equal((await request({})).totalTokens,daily.totalTokens);
  evidence.result='passed';evidence.acceptance={twoSyntheticAccounts:true,exactSums:true,allModes:true,knownMissingAndZero:true,synchronizedDonutAndHeatmap:true,customDatesViaNativeInputEvents:true,verifiedGate:true,strictIpc:true,unchangedDbWalAndKey:true,accountIsolation:true,genuineRestart:true};
} catch(error){evidence.error=error.stack??String(error);process.exitCode=1;}
finally{try{await driver?.stop();}finally{stopGroup(fixture?.bus,'SIGKILL');if(fixture)await rm(fixture.root,{recursive:true,force:true});await writeFile(join(output,'result.json'),JSON.stringify(evidence,null,2)+'\n',{mode:0o600});}}
