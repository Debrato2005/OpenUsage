// Production native subscription CRUD plus isolated SQLCipher/WAL restart and recovery.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

for (const name of ['OPENUSAGE_NATIVE_BINARY','OPENUSAGE_NATIVE_EVIDENCE','OPENUSAGE_ACTIVITY_VERIFIER']) assert.ok(process.env[name],name);
const repository=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const application=resolve(process.env.OPENUSAGE_NATIVE_BINARY), output=resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE), verifier=resolve(process.env.OPENUSAGE_ACTIVITY_VERIFIER);
const passphrase='OpenUsage synthetic subscription workspace 2026';
const backupPassword='OpenUsage synthetic subscription archive 2026';
const evidence={requirement:'NATIVE-SUBSCRIPTIONS-001',result:'failed',scope:'Actual production Linux Tauri application with private unauthenticated synthetic account profiles. GUI create/edit/archive/delete, exact integer money, account isolation, genuine restart and native typed-IPC encrypted backup/restore/Undo. Independent read-only 31-table SQLCipher and authenticated archive verification. No real provider session or Windows/macOS qualification.',oracles:[],closes:[]};
let fixture,driver,sequence=0;
const hash=async path=>createHash('sha256').update(await readFile(path)).digest('hex');
const call=async(command,args)=>{const response=await driver.invoke(command,args);assert.equal(response.ok,true,`${command}: ${JSON.stringify(response)}`);return response.value;};
const list=async profileId=>(await call('list_subscriptions',{profileId,afterId:null})).records;
async function key(value){await driver.request('POST',`/session/${driver.session}/actions`,{actions:[{type:'key',id:'subscription-key',actions:[{type:'keyDown',value},{type:'keyUp',value}]}]});await driver.request('DELETE',`/session/${driver.session}/actions`);}
async function activate(selector){await waitFor(()=>driver.execute('const n=document.querySelector(arguments[0]);return Boolean(n&&!n.disabled&&n.getClientRects().length);',[selector]),`available ${selector}`);assert.equal(await driver.execute('const n=document.querySelector(arguments[0]);n.scrollIntoView({block:"nearest"});n.focus({preventScroll:true});return document.activeElement===n;',[selector]),true);await key('\uE007');}
async function settings(){await activate('[data-workspace-nav="settings"]');await waitFor(()=>driver.execute('return Boolean(document.querySelector("[data-subscriptions-toggle]")?.getClientRects().length);'),'subscription panel');await activate('[data-subscriptions-toggle]');await waitFor(()=>driver.execute('return Boolean(document.querySelector("[data-subscription-add]")&&!document.querySelector("[data-subscription-add]").disabled);'),'subscription list');}
async function chooseAccount(profile){
  await activate('[data-subscription-select="account"]');
  assert.equal(await driver.execute('document.querySelectorAll("[data-subscription-option]").forEach(n=>delete n.dataset.subscriptionOption);const n=Array.from(document.querySelectorAll("[role=option]")).find(n=>n.textContent.trim()===arguments[0]);if(!n)return false;n.dataset.subscriptionOption="selected";return true;',[profile.label]),true);
  await driver.click('[data-subscription-option="selected"]');
  await waitFor(()=>driver.execute('return document.querySelector("[data-subscription-account]")?.dataset.subscriptionAccount===arguments[0];',[profile.id]),'exact selected profile');
  await waitFor(()=>driver.execute('return Boolean(document.querySelector("[data-subscription-add]")&&!document.querySelector("[data-subscription-add]").disabled);'),'selected account list');
}
async function saveRecord(profileId,plan,price,editId){
  await activate(editId?`[data-subscription-record="${editId}"] [data-subscription-edit]`:'[data-subscription-add]');
  await driver.type('.subscription-form [name="plan"]',plan);
  await driver.type('.subscription-form [name="price"]',price);
  await activate('[data-subscription-save]');
  await waitFor(async()=>{const rows=await list(profileId);return rows.some(row=>row.plan===plan);},'GUI subscription saved');
  await waitFor(()=>driver.execute('return !document.querySelector(".subscription-form");'),'save form closed');
  return (await list(profileId)).find(row=>row.plan===plan);
}
async function open(setup=false){
  driver=new NativeDriver(application,fixture.environment,join(output,`driver-${++sequence}.log`));await driver.start();
  await waitFor(()=>driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'),'workspace password');
  assert.equal((await driver.invoke('list_subscriptions',{profileId:randomUUID(),afterId:null})).error,'workspace_locked');
  await driver.type('#workspace-passphrase',passphrase);
  if(setup)await driver.type('#workspace-confirmation',passphrase);
  await key('\uE007');await waitFor(async()=>(await call('bootstrap')).storage==='ready','ready encrypted workspace');
  assert.equal((await call('bootstrap')).schemaVersion,9);
}
async function close(){
  const queue=[driver.child.pid];let owned;
  for(let n=0;queue.length&&n<64;n++){
    const pid=queue.shift();
    try{if(await readlink(`/proc/${pid}/exe`)===application){owned=pid;break;}queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`,'utf8')).trim().split(/\s+/).filter(Boolean).map(Number));}catch(error){if(error.code!=='ENOENT')throw error;}
  }
  assert.ok(owned,'Native app must be an owned child');
  evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3',[join(repository,'tests/native/x11-close.py'),String(owned)],{env:fixture.environment,encoding:'utf8',timeout:10000})));
  await waitFor(async()=>{try{await readlink(`/proc/${owned}/exe`);return false;}catch(error){if(error.code==='ENOENT')return true;throw error;}},'genuine native exit');
  driver.session=undefined;await driver.stop();driver=undefined;
}
async function liveHashes(){const values={};for(const path of[fixture.database,`${fixture.database}-wal`,join(dirname(fixture.database),'database-key.v1')]){try{values[path]=await hash(path);}catch(error){if(error.code!=='ENOENT')throw error;values[path]=null;}}return values;}
async function oracle(label,archive){
  const before=await liveHashes(),temporary=join(fixture.root,`oracle-${randomUUID()}`);await mkdir(temporary,{mode:0o700});
  try{
    const input=join(temporary,archive?'archive.oubackup':'database.db');await copyFile(archive??fixture.database,input);await chmod(input,0o600);
    if(!archive&&before[`${fixture.database}-wal`]){await copyFile(`${fixture.database}-wal`,`${input}-wal`);await chmod(`${input}-wal`,0o600);}
    const raw=execFileSync(verifier,[archive?'backup':'database',input,join(dirname(fixture.database),'database-key.v1'),'--profile-state'],{input:JSON.stringify({passphrase,backupPassword,expectedSchemaVersion:9}),env:fixture.environment,encoding:'utf8',timeout:30000,maxBuffer:8*1024**2});
    const report=JSON.parse(execFileSync('/usr/bin/python3',['-c','import json,sys; print(json.dumps(json.load(sys.stdin,parse_int=str)))'],{input:raw,encoding:'utf8',timeout:10000,maxBuffer:8*1024**2}));
    assert.equal(report.verified,true);assert.equal(report.schemaVersion,'9');assert.equal(report.plaintextDiskStaging,false);assert.equal(report.inputUnchanged,true);assert.equal(Object.keys(report.state.tables).length,31);
    assert.deepEqual(await liveHashes(),before);
    evidence.oracles.push({label,archiveAuthenticated:report.archiveAuthenticated,tables:31,subscriptionRows:report.state.tables.subscriptions.length,stateSha256:createHash('sha256').update(JSON.stringify(report.state.tables)).digest('hex')});
    return report.state.tables;
  }finally{await rm(temporary,{recursive:true,force:true});}
}
function terms(record){const {provider:_,id:__,profileId:___,amountMinor:____,currencyScale:_____,archived:______,revision:_______,source:________,createdAtUtc:_________,confirmedAtUtc:__________,updatedAtUtc:___________,...input}=record;return input;}
try{
  await mkdir(output,{recursive:true,mode:0o700});evidence.applicationSha256=await hash(application);evidence.verifierSha256=await hash(verifier);
  fixture=await createFixture('subscriptions',output);await open(true);
  const a=(await call('create_profile',{label:'Synthetic account A'})).profiles[0];
  const profiles=(await call('create_profile',{label:'Synthetic account B'})).profiles;const b=profiles.find(p=>p.id!==a.id);
  await driver.execute('location.reload();return true;');await waitFor(()=>driver.execute('return Boolean(document.querySelector("[data-workspace-nav=settings]"));'),'reloaded native renderer');
  await settings();await chooseAccount(a);
  let first=await saveRecord(a.id,'Synthetic Plus','20.00');assert.equal(first.amountMinor,'2000');
  first=await saveRecord(a.id,'Exact updated Plus','90071992547409.93',first.id);assert.equal(first.amountMinor,'9007199254740993');
  await chooseAccount(b);const second=await saveRecord(b.id,'Independent plan','90071992547409.93');
  assert.deepEqual(await list(a.id),[first]);assert.deepEqual(await list(b.id),[second]);
  assert.deepEqual(await driver.invoke('update_subscription',{profileId:b.id,subscriptionId:first.id,expectedRevision:first.revision,input:terms(first)}),{ok:false,error:'subscription_unavailable'});
  assert.deepEqual(await driver.invoke('delete_subscription',{profileId:a.id,subscriptionId:first.id,expectedRevision:randomUUID(),confirmed:true}),{ok:false,error:'subscription_changed'});
  assert.equal((await driver.invoke('delete_subscription',{profileId:a.id,subscriptionId:first.id,expectedRevision:first.revision,confirmed:false})).ok,false);
  await chooseAccount(a);await activate(`[data-subscription-record="${first.id}"] [data-subscription-delete]`);await key('\uE00C');assert.deepEqual(await list(a.id),[first]);
  await activate(`[data-subscription-record="${first.id}"] [data-subscription-delete]`);await activate('[data-subscription-delete-confirm]');await waitFor(async()=>(await list(a.id)).length===0,'GUI deletion');assert.deepEqual(await list(b.id),[second]);
  first=await saveRecord(a.id,'Retained zero-price plan','0');assert.equal(first.price,'0.00');
  await activate(`[data-subscription-record="${first.id}"] [data-subscription-archive]`);await waitFor(async()=>(await list(a.id))[0].archived,'GUI archive');
  first=(await list(a.id))[0];
  await activate(`[data-subscription-record="${first.id}"] [data-subscription-archive]`);await waitFor(async()=>!(await list(a.id))[0].archived,'GUI unarchive');
  first=(await list(a.id))[0];
  for(const input of [{...terms(first),price:'92233720368547758.08'},{...terms(first),startDate:'2026-02-30'}])assert.equal((await driver.invoke('create_subscription',{profileId:a.id,input})).error,'invalid_subscription');
  assert.equal((await driver.invoke('create_subscription',{profileId:a.id,input:{...terms(first),credential:'synthetic-disallowed-field'}})).error,'invalid_arguments');
  first=await call('update_subscription',{profileId:a.id,subscriptionId:first.id,expectedRevision:first.revision,input:{...terms(first),startDate:'2026-10-22',renewalDate:'2026-11-22',endDate:'2027-10-22',cancellationDate:'2026-11-20',autoRenew:true,intervalUnit:'month',intervalCount:1}});
  const saved=await oracle('before genuine restart');assert.ok(saved.subscriptions.some(r=>r.amount_minor==='9007199254740993'));assert.equal(saved.subscriptions[0].amount_minor==='0'||saved.subscriptions[1].amount_minor==='0',true);
  await close();await open();assert.deepEqual((await oracle('after genuine restart')).subscriptions,saved.subscriptions);assert.deepEqual(await list(a.id),[first]);assert.deepEqual(await list(b.id),[second]);
  await settings();await chooseAccount(a);await driver.execute('document.querySelector("[data-subscriptions]").scrollIntoView({block:"start"});return true;');await driver.screenshot(join(output,'subscriptions-native.png'));
  const archive=await call('create_backup',{password:backupPassword});const archivePath=join(archive.directory,archive.fileName);
  assert.deepEqual(await oracle('authenticated backup contains every table',archivePath),saved);
  assert.equal((await driver.invoke('prepare_restore_preview',{fileName:archive.fileName,password:'Wrong synthetic archive password 2026'})).error,'backup_authentication_failed');
  const preview=await call('prepare_restore_preview',{fileName:archive.fileName,password:backupPassword});
  first=await call('update_subscription',{profileId:a.id,subscriptionId:first.id,expectedRevision:first.revision,input:{...terms(first),plan:'Changed after backup',price:'123.45'}});
  assert.equal((await driver.invoke('apply_restore',{previewId:preview.previewId,confirmed:true})).error,'stale_restore_preview');
  await call('delete_subscription',{profileId:b.id,subscriptionId:second.id,expectedRevision:second.revision,confirmed:true});
  const changed=await oracle('state before confirmed restore');const fresh=await call('prepare_restore_preview',{fileName:archive.fileName,password:backupPassword});
  assert.equal((await driver.invoke('apply_restore',{previewId:fresh.previewId,confirmed:false})).ok,false);
  const receipt=await call('apply_restore',{previewId:fresh.previewId,confirmed:true});assert.ok(receipt);
  assert.deepEqual((await oracle('archive restore')).subscriptions,saved.subscriptions);assert.deepEqual(await list(b.id),[second]);
  const undo=await call('prepare_checkpoint_restore_preview',{checkpointId:fresh.checkpoint.id});await call('apply_restore',{previewId:undo.previewId,confirmed:true});
  assert.deepEqual((await oracle('checkpoint Undo')).subscriptions,changed.subscriptions);assert.deepEqual(await list(b.id),[]);
  await close();await open();assert.deepEqual((await oracle('restart after Undo')).subscriptions,changed.subscriptions);
  assert.equal(await readFile(join(fixture.dirs.codex,'synthetic-fixture.txt'),'utf8'),CODEX_SENTINEL);
  assert.equal(await hash(application),evidence.applicationSha256);assert.equal(await hash(verifier),evidence.verifierSha256);await close();
  evidence.acceptance={guiCreateEditDelete:true,guiArchiveUnarchive:true,invalidAmountsDatesAndFieldsRefused:true,cancelPreservesRecord:true,accountIsolation:true,staleRevisionRefused:true,exactMoney:true,dateOnlyTerms:true,genuineRestart:true,authenticatedBackup:true,restoreAndUndo:true,stalePreviewRefused:true,allTables:31};evidence.result='passed';
}catch(error){evidence.error=error.stack??String(error);process.exitCode=1;}
finally{try{await driver?.stop();}finally{stopGroup(fixture?.bus,'SIGKILL');if(fixture)await rm(fixture.root,{recursive:true,force:true});await writeFile(join(output,'result.json'),JSON.stringify(evidence,null,2)+'\n',{mode:0o600});}}
