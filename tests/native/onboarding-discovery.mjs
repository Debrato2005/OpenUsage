// Actual production GUI, fresh synthetic workspace, official executable metadata and signed-out check.
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';
const output=resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE),application=resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const official='/example/home/.codex/packages/standalone/releases/0.163.0-x86_64-unknown-linux-musl/bin/codex';
const expected='399ff69c431c066e40de318fba45c2b07f45a76b19939d247ee4828fc70c4215';
const password='OpenUsage synthetic onboarding check 2026';
const inlineAccount=process.env.OPENUSAGE_NATIVE_INLINE_ACCOUNT==='1';
const evidence={requirement:'NATIVE-ONBOARDING-DISCOVERY-001',result:'failed',scope:'Production native GUI, fresh synthetic workspace, explicit installed executable metadata, approved official source and fresh signed-out connection. No real credentials, device authorization request, authenticated collection, or OS key-store availability claim.',screenshots:[],startedAt:new Date().toISOString()};
let fixture,driver;
const hash=async p=>createHash('sha256').update(await readFile(p)).digest('hex');
async function call(command,args={}){const r=await driver.invoke(command,args);assert.equal(r.ok,true,command);return r.value;}
async function capture(name){await driver.screenshot(join(output,name));evidence.screenshots.push(name);}
await mkdir(output,{recursive:true,mode:0o700});
try{
 assert.equal(await hash(official),expected);
 fixture=await createFixture('onboarding-discovery');
 fixture.environment.HOME='/home/debrato';fixture.environment.PATH='/example/home/.local/bin:/usr/bin:/bin';
 driver=new NativeDriver(application,fixture.environment,join(output,'driver.log'));await driver.start();
 await waitFor(()=>driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'),'explicit fallback setup');
 const shell=await driver.execute('return {onboarding:document.querySelector(".app-shell.onboarding-shell")!==null,visibleNav:[...document.querySelectorAll("nav[aria-label=Main] a")].filter(n=>n.getClientRects().length).length,visibleLocked:[...document.querySelectorAll(".metric-grid")].some(n=>n.getClientRects().length),visibleMonitoring:[...document.querySelectorAll(".status-pill")].some(n=>n.getClientRects().length)};');
 assert.deepEqual(shell,{onboarding:true,visibleNav:0,visibleLocked:false,visibleMonitoring:false});
 await capture('setup-focused-light.png');
 await driver.type('#workspace-passphrase',password);await driver.type('#workspace-confirmation',password);await driver.click('.unlock-form button[type=submit]');
 await waitFor(async()=>(await driver.invoke('bootstrap')).value?.storage==='ready','private SQLCipher workspace');
 assert.equal((await call('bootstrap')).schemaVersion,4);
 if(inlineAccount){await driver.navigate('accounts');await driver.click('[data-account-connect]');}
 else{await driver.openProfileCreate();await driver.type('#profile-label','Synthetic official provider check');await driver.click('.profile-create-form button[type=submit]');}
 let profile;await waitFor(async()=>{profile=(await call('list_profiles')).profiles[0];return Boolean(profile);},'fresh profile');
 if(!inlineAccount){await driver.navigate('settings');if(await driver.execute('return !document.querySelector("[data-provider-settings]").closest("details").open;'))await driver.click('[data-provider-settings]');await driver.click('[data-provider-find]');}
 await waitFor(()=>driver.execute('return Boolean(document.querySelector(".provider-executable-review"))&&!document.querySelector("[data-provider-find]").disabled;'),'official installed source discovered',30000);
 if(inlineAccount){assert.equal(profile.label,'Account A');assert.equal(await driver.execute('return document.querySelector("[data-workspace-nav=accounts]").getAttribute("aria-current");'),'page');}
 const review=await driver.execute('return document.querySelector(".provider-executable-review").textContent;');assert.ok(review.includes(official));assert.ok(review.includes('Reviewed official artifact'));
 assert.equal(await driver.execute('return document.querySelector("#provider-executable-approved").getAttribute("aria-checked");'),'false');
 assert.equal(await driver.execute('return document.querySelector(".provider-approval-form button[type=submit]").disabled;'),true);
 await capture('official-codex-review.png');
 await driver.click('#provider-executable-approved');const approvalStarted=Date.now();await driver.click('.provider-approval-form button[type=submit]');
 await waitFor(()=>driver.execute(inlineAccount ? 'return document.activeElement?.matches("[data-login-start]");' : 'return document.activeElement?.matches(".provider-approval-result");'),'explicit source approval',60000);
 evidence.sourceApprovalDurationMs=Date.now()-approvalStarted;
 if(inlineAccount){await driver.click(`[data-login-start="${profile.id}"]`);await waitFor(()=>driver.execute('return Boolean(document.querySelector("[data-login-consent]"));'),'named-account sign-in consent');await capture('inline-account-sign-in.png');await driver.click('[data-login-consent] button[type=button]');evidence.inlineAccount={automaticIsolatedProfile:true,defaultLabel:profile.label,automaticDiscovery:true,noSettingsDetour:true,namedSignInConsent:true,noDeviceAuthorizationStarted:true};}
 await driver.openProfileOptions(profile.id);await driver.click(`[data-provider-check="${profile.id}"]`);
 await waitFor(()=>driver.execute('return document.activeElement?.matches(".provider-diagnostic");'),'actual official signed-out response',45000);
 const text=await driver.execute('return document.querySelector(".provider-diagnostic").textContent;');assert.match(text,/sign in|sign-in|authentication/i);
 assert.equal((await call('list_profiles')).profiles[0].connectionState,'authentication_required');
 assert.equal(await call('load_profile_quota',{profileId:profile.id}),null);assert.equal(await call('load_profile_activity',{profileId:profile.id}),null);
 await capture('official-provider-sign-in-required.png');
 evidence.steps={focusedSetup:shell,officialHash:expected,normalPathAliasDiscovered:true,explicitApprovalRequired:true,actualOfficialSignedOutResponse:true,noFakeMetrics:true,noDeviceAuthorizationStarted:true};evidence.result='passed';
}catch(error){evidence.error=error.stack??String(error);process.exitCode=1;if(driver?.session)try{await capture('failure.png');}catch{}}
finally{try{await driver?.stop();}catch(error){evidence.cleanupError=error.message;process.exitCode=1;}stopGroup(fixture?.bus,'SIGKILL');if(fixture)await rm(fixture.root,{recursive:true,force:true});evidence.finishedAt=new Date().toISOString();await writeFile(join(output,'result.json'),JSON.stringify(evidence,null,2)+'\n',{mode:0o600});console.log(`${evidence.requirement}: ${evidence.result}`);if(evidence.error)console.error(evidence.error);}
