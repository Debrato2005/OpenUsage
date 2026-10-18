import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readdir, readlink, rename, rm, writeFile } from 'node:fs/promises';
import { platform, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';
const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const verifier = resolve(process.env.OPENUSAGE_CHECKPOINT_VERIFIER ?? join(harness, 'backup-verifier/target/debug/openusage-native-checkpoint-verifier'));
const generator = resolve(process.env.OPENUSAGE_BACKUP_FIXTURE_GENERATOR ?? join(harness, 'backup-verifier/target/debug/openusage-native-backup-fixtures'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-restore-apply-evidence-${randomUUID()}`));
const passphrase = 'OpenUsage synthetic apply workspace 2026';
const password = 'OpenUsage synthetic apply archive 2026';
const hash = async (path) => createHash('sha256').update(await readFile(path)).digest('hex');
const liveVerifier = resolve(process.env.OPENUSAGE_LIVE_VERIFIER ?? join(harness, 'backup-verifier/target/debug/openusage-native-live-verifier'));
const evidence = {requirement:'NATIVE-RESTORE-APPLY-001',startedAt:new Date().toISOString(),result:'failed',scope:'Actual Tauri GUI confirmation and atomic native restore with synthetic records only. Direct production IPC fault checks. Exact selected semantic fields under the protected SQLCipher key and fresh homes after restart. Timestamps and unselected columns are not compared. No provider login, undo, interrupted-commit crash qualification, or cross-platform acceptance.',platform:{os:platform(),release:release()},screenshots:[],steps:[],checkpoints:[],views:[]};
let fixture, driver, workspace, checkpointDirectory, archive, fileName, sequence = 0;
const markers = new Map();
let expectedA, expectedB, oldHomes;
async function click(text) { const node = await driver.execute('return [...document.querySelectorAll("button")].find(node=>node.textContent.trim()===arguments[0]);', [text]); assert.ok(node, `Missing button: ${text}`); await driver.request('POST', `/session/${driver.session}/element/${node['element-6066-11e4-a52e-4f735466cecf']}/click`, {}); }
async function state() { const bootstrap = await driver.invoke('bootstrap'), profiles = await driver.invoke('list_profiles'); assert.equal(profiles.ok, true); return { theme: bootstrap.value.theme, profiles: profiles.value.profiles.sort((a, b) => a.id.localeCompare(b.id)), accounts: '0' }; }
async function checkpoints() { try {
    return (await readdir(checkpointDirectory)).sort();
}
catch (error) {
    if (error.code === 'ENOENT')
        return [];
    throw error;
} }
async function unchanged(expected) { assert.deepEqual(await state(), expected); for (const [path, digest] of markers)
    assert.equal(await hash(path), digest); assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); }
async function liveHashes() { const result = {}; for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) {
    try {
        result[path] = await hash(path);
    }
    catch (error) {
        if (error.code !== 'ENOENT')
            throw error;
    }
} return result; }
async function rejected(command, args, error) { assert.deepEqual(await driver.invoke(command, args), { ok: false, error }); }
async function theme(value) { await driver.selectVisibleOption('#theme', { system: 'System', light: 'Light', dark: 'Dark' }[value]); await waitFor(async () => await driver.execute('return !document.querySelector("#theme").disabled&&document.documentElement.dataset.theme===arguments[0];', [value]), 'appearance saved'); }
async function create(label) { await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]'); await waitFor(async () => await driver.execute('const input=document.querySelector("#profile-label");return !input.disabled&&input.value==="";'), 'profile form ready'); return (await state()).profiles.find(profile => profile.label === label); }
async function open() { driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start(); }
async function unlock() { await driver.type('#workspace-passphrase', passphrase); await driver.click('.unlock-form button[type=submit]'); await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'workspace unlocked'); }
async function inspect() { await click('Inspect a backup'); await waitFor(async () => Boolean(await driver.execute('return document.querySelector("#backup-file")&&!document.querySelector("#backup-file").disabled;')), 'inspection catalog ready'); await driver.click(`#backup-file option[value="${fileName}"]`); await driver.type('#backup-inspection-password', password); await driver.click('.backup-inspection-form button[type=submit]'); await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-inspection-receipt");')), 'authenticated inspection'); }
async function prepareGui() { await click('Preview restore'); await waitFor(async () => Boolean(await driver.execute('return document.querySelector("#restore-preview-password");')), 'preview password form'); await driver.type('#restore-preview-password', password); await driver.click('.restore-preview-form button[type=submit]'); await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".restore-preview");')), 'checkpoint-backed restore preview', 30000); await waitFor(async () => Boolean(await driver.execute('return document.activeElement?.matches(".restore-preview");')), 'preview focus'); const ids = await driver.execute('const preview=document.querySelector(".restore-preview");return{previewId:preview.dataset.previewId,checkpointId:preview.dataset.checkpointId};'); assert.match(ids.previewId, /^[0-9a-f-]{36}$/); assert.match(ids.checkpointId, /^[0-9a-f-]{36}$/); return ids; }
async function verifyCheckpoint(id, expected) { const path = join(checkpointDirectory, `${id}.sqlite`); const metadata = await lstat(path); assert.equal(metadata.mode & 0o777, 0o600); assert.equal((await lstat(checkpointDirectory)).mode & 0o777, 0o700); const before = await hash(path); const result = JSON.parse(execFileSync(verifier, [path, join(workspace, 'database-key.v1')], { input: JSON.stringify({ passphrase, expected }), encoding: 'utf8', env: fixture.environment, cwd: fixture.root, timeout: 30000, maxBuffer: 1024 * 1024 })); assert.equal(result.verified, true); assert.equal(result.readOnly, true); assert.equal(result.exactStateMatched, true); assert.equal(await hash(path), before); await copyFile(path, join(output, `${id}.sqlite`)); await chmod(join(output, `${id}.sqlite`), 0o600); evidence.checkpoints.push({ id, sha256: before, fileBytes: String(metadata.size), verification: result }); }

async function resize(width,height) {
  for(let attempt=0;attempt<3;attempt++) {
    const actual=await driver.execute('return{width:innerWidth,height:innerHeight}');
    if(actual.width===width&&actual.height===height)return;
    const rect=await driver.request('GET',`/session/${driver.session}/window/rect`);
    await driver.request('POST',`/session/${driver.session}/window/rect`,{width:rect.width+width-actual.width,height:rect.height+height-actual.height});
  }
}
async function capture(name,selector,width=1280,height=850) {
  await resize(width,height);
  await driver.execute('document.querySelector(arguments[0]).scrollIntoView({block:"center"});return true;',[selector]);
  const view=await driver.execute('const panel=document.querySelector(arguments[0]);return{width:innerWidth,height:innerHeight,scrollWidth:document.scrollingElement.scrollWidth,focus:document.activeElement?.className,text:panel.textContent,overflow:[panel,...panel.querySelectorAll("*")].filter(node=>{const r=node.getBoundingClientRect();return r.width>1&&r.height>1&&(r.left< -1||r.right>innerWidth+1)}).map(node=>node.tagName)}',[selector]);
  assert.equal(view.width,width);assert.equal(view.height,height);assert.ok(view.scrollWidth<=width+1);assert.deepEqual(view.overflow,[]);
  await driver.screenshot(join(output,name));evidence.screenshots.push(name);evidence.views.push({name,...view});
}
async function allHomes() {return(await readdir(join(workspace,'profiles'))).sort();}
async function markersUnchanged() {for(const[path,digest]of markers)assert.equal(await hash(path),digest);}
async function prepareDirect() {
  const result=await driver.invoke('prepare_restore_preview',{fileName,password});assert.equal(result.ok,true);assert.equal(result.value.restoreAvailable,true);
  assert.deepEqual(result.value.current,{activeProfiles:3,archivedProfiles:'0',accounts:'0'});
  assert.deepEqual(result.value.incoming,{activeProfiles:1,archivedProfiles:'1',accounts:'0'});
  return result.value;
}
async function applyRejected(preview,error) {
  const hashes=await liveHashes(),homes=await allHomes();
  await rejected('apply_restore',{previewId:preview.previewId,confirmed:true},error);
  assert.deepEqual(await liveHashes(),hashes);assert.deepEqual(await allHomes(),homes);await unchanged(expectedB);
}
async function closeNormally() {
  const queue=[driver.child.pid];let pid;
  for(let count=0;queue.length&&count<64;count++){
    const current=queue.shift();try{if(await readlink(`/proc/${current}/exe`)===application){pid=current;break;}queue.push(...(await readFile(`/proc/${current}/task/${current}/children`,'utf8')).trim().split(/\s+/).filter(Boolean).map(Number));}catch(error){if(error.code!=='ENOENT')throw error;}
  }
  assert.ok(pid,'Cannot identify owned native child.');
  assert.deepEqual(await driver.request('DELETE',`/session/${driver.session}/window`),[]);
  await waitFor(async()=>{try{await readlink(`/proc/${pid}/exe`);return false;}catch(error){if(error.code==='ENOENT')return true;throw error;}},'normal native exit');
  driver.session=undefined;await driver.stop();driver=undefined;
}
async function verifyLive(closedCopy=false) {
 const hashes=await liveHashes();let database=fixture.database;
 if(closedCopy){
   const directory=join(fixture.root,'closed-ciphertext-verification');await mkdir(directory,{mode:0o700});database=join(directory,'closed-encrypted.db');
   await copyFile(fixture.database,database);await chmod(database,0o600);assert.equal(await hash(database),await hash(fixture.database));
   try{await copyFile(`${fixture.database}-wal`,`${database}-wal`);await chmod(`${database}-wal`,0o600);assert.equal(await hash(`${database}-wal`),await hash(`${fixture.database}-wal`));}catch(error){if(error.code!=='ENOENT')throw error;}
 }
 const result=JSON.parse(execFileSync(liveVerifier,[database,join(workspace,'database-key.v1')],{input:JSON.stringify({passphrase,expected:expectedA,forbiddenHomes:oldHomes,homeRoot:workspace,allowEmptyWalCreation:closedCopy}),encoding:'utf8',env:fixture.environment,timeout:30000,maxBuffer:1024*1024}));
 assert.equal(result.verified,true);assert.equal(result.freshPrivateEmptyHomes,2);assert.equal(result.walRecordsUnchanged,true);if(!closedCopy)assert.equal(result.walUnchanged,true);assert.deepEqual(await liveHashes(),hashes);
 if(closedCopy){await copyFile(database,join(output,'verified-closed-live.sqlite'));await chmod(join(output,'verified-closed-live.sqlite'),0o600);evidence.closedDatabaseArtifact={fileName:'verified-closed-live.sqlite',sha256:await hash(database)};try{await copyFile(`${fixture.database}-wal`,join(output,'verified-closed-live.sqlite-wal'));await chmod(join(output,'verified-closed-live.sqlite-wal'),0o600);}catch(error){if(error.code!=='ENOENT')throw error;}}
 return {scope:closedCopy?'Read-only verification of byte-identical private encrypted DB and existing-WAL copies after normal native exit. Original closed files stay unchanged.':'Read-only actual live SQLCipher connection with WAL.',sourceHashes:hashes,...result};
}

await mkdir(output,{recursive:true,mode:0o700});
process.once('exit',()=>{driver?.kill('SIGKILL');stopGroup(fixture?.bus,'SIGKILL');});
try {
 assert.equal(platform(),'linux');
 for(const[key,path]of Object.entries({application,verifier,generator,liveVerifier}))evidence[key]={path,sha256:await hash(path)};
 evidence.sourceCommit=execFileSync('git',['rev-parse','HEAD'],{cwd:repository,encoding:'utf8'}).trim();
 evidence.sourceHashes={};const sources=execFileSync('rg',['--files','apps/desktop/src','apps/desktop/src-tauri/src','crates/openusage-storage/src','crates/openusage-import/src'],{cwd:repository,encoding:'utf8'}).trim().split('\n');
 for(const path of [...sources,'Cargo.toml','Cargo.lock','crates/openusage-storage/Cargo.toml','crates/openusage-import/Cargo.toml','apps/desktop/src-tauri/Cargo.toml','apps/desktop/src-tauri/tauri.conf.json','apps/desktop/src-tauri/build.rs','apps/desktop/src-tauri/capabilities/main-window.json'])evidence.sourceHashes[path]=await hash(join(repository,path));
 evidence.harnessHashes={};for(const path of ['restore-apply.mjs','private-fixture.mjs','webdriver.mjs','backup-verifier/Cargo.toml','backup-verifier/Cargo.lock','backup-verifier/src/bin/openusage-native-checkpoint-verifier.rs','backup-verifier/src/bin/openusage-native-live-verifier.rs','backup-verifier/src/bin/openusage-native-backup-fixtures.rs'])evidence.harnessHashes[path]=await hash(join(harness,path));
 fixture=await createFixture('restore-apply');workspace=dirname(fixture.database);checkpointDirectory=join(workspace,'restore-checkpoints');await open();
 await rejected('apply_restore',{previewId:randomUUID(),confirmed:true},'workspace_locked');
 for(const args of [[],{previewId:'../unsafe',confirmed:true},{previewId:randomUUID()},{previewId:randomUUID(),confirmed:false},{previewId:randomUUID(),confirmed:1},{previewId:randomUUID(),confirmed:'true'},{previewId:randomUUID(),confirmed:true,path:'../unsafe'}])await rejected('apply_restore',args,'invalid_arguments');
 await driver.type('#workspace-passphrase',passphrase);await driver.type('#workspace-confirmation',passphrase);await driver.click('.unlock-form button[type=submit]');await waitFor(async()=>(await driver.invoke('bootstrap')).value?.storage==='ready','workspace created');
 await theme('light');const first=await create('Restore original first'),second=await create('Restore original archived');
 await driver.click(`[data-profile-id="${second.id}"] [data-profile-action="archive"]`);await waitFor(async()=>(await state()).profiles.find(p=>p.id===second.id)?.archived===true,'state A archive');
 expectedA=await state();evidence.stateA=expectedA;
 await click('Create a backup');await driver.type('#backup-password',password);await driver.type('#backup-confirmation',password);await driver.click('#backup-records');await driver.click('.backup-form button[type=submit]');await waitFor(async()=>Boolean(await driver.execute('return document.querySelector(".backup-receipt")')),'archive A');
 const directory=join(fixture.dirs.downloads,'OpenUsage');[fileName]=await readdir(directory);archive=join(directory,fileName);const originalArchive=await readFile(archive);evidence.originalArchiveSha256=await hash(archive);
 await driver.click(`[data-profile-id="${first.id}"] [data-profile-action="rename"]`);await driver.type('.profile-rename-form input','Restore live renamed');await driver.click('.profile-rename-form button[type=submit]');await waitFor(async()=>!await driver.execute('return Boolean(document.querySelector(".profile-rename-form"))'),'state B rename');
 await driver.execute('document.querySelector(".archived-profiles").open=true;return true;');
 await driver.click(`[data-profile-id="${second.id}"] [data-profile-action="restore"]`);await waitFor(async()=>(await state()).profiles.find(p=>p.id===second.id)?.archived===false,'state B unarchive');
 await create('Restore third live only');await theme('dark');expectedB=await state();evidence.stateB=expectedB;oldHomes=await allHomes();assert.equal(oldHomes.length,3);
 for(const id of oldHomes){const path=join(workspace,'profiles',id,'restore-synthetic-marker.txt');await writeFile(path,'Synthetic provider home marker. Preserve this existing home.\n',{mode:0o600,flag:'wx'});markers.set(path,await hash(path));}
 const keyHash=await hash(join(workspace,'database-key.v1'));
 await inspect();
 // Operational guards consume previews and never create homes or alter records.
 let preview=await prepareDirect();assert.equal((await driver.invoke('set_theme',{theme:'light'})).ok,true);assert.equal((await driver.invoke('set_theme',{theme:'dark'})).ok,true);await applyRejected(preview,'stale_restore_preview');
 preview=await prepareDirect();assert.equal((await driver.invoke('rename_profile',{id:first.id,label:'Restore live interim'})).ok,true);assert.equal((await driver.invoke('rename_profile',{id:first.id,label:'Restore live renamed'})).ok,true);await applyRejected(preview,'stale_restore_preview');
 preview=await prepareDirect();await driver.invoke('cancel_restore_preview',{previewId:preview.previewId});await applyRejected(preview,'stale_restore_preview');
 preview=await prepareDirect();let checkpoint=join(checkpointDirectory,`${preview.checkpoint.id}.sqlite`);await rename(checkpoint,`${checkpoint}.held`);await applyRejected(preview,'restore_checkpoint_failed');await rename(`${checkpoint}.held`,checkpoint);
 preview=await prepareDirect();checkpoint=join(checkpointDirectory,`${preview.checkpoint.id}.sqlite`);const originalCheckpoint=await readFile(checkpoint),corrupt=Buffer.from(originalCheckpoint);corrupt[corrupt.length-1]^=1;await writeFile(checkpoint,corrupt);await applyRejected(preview,'restore_checkpoint_failed');await writeFile(checkpoint,originalCheckpoint);
 preview=await prepareDirect();await chmod(join(workspace,'profiles'),0o755);try{await applyRejected(preview,'restore_failed');}finally{await chmod(join(workspace,'profiles'),0o700);}
 evidence.steps.push({strictLiteralConfirmationRequired:true,staleThemeAndProfileAndCancelledTokensRejected:true,missingAndCorruptCheckpointRejected:true,unsafeHomeParentRejected:true,failureRecordsKeyAndHomeSetUnchanged:true});
 // Return through the GUI for the actual destructive confirmation.
 const final=await prepareGui();await verifyCheckpoint(final.checkpointId,expectedB);await rejected('apply_restore',{previewId:final.previewId,confirmed:false},'invalid_arguments');
 const confirmedBefore=await driver.execute('const input=document.querySelector("#restore-confirmed"),button=document.querySelector(".restore-confirmation-form button[type=submit]");return{checked:input.checked,disabled:button.disabled,label:input.labels?.[0]?.textContent};');assert.equal(confirmedBefore.checked,false);assert.equal(confirmedBefore.disabled,true);assert.ok(confirmedBefore.label?.trim());
 await capture('restore-confirmation-dark-desktop.png','.restore-preview');await capture('restore-confirmation-dark-360.png','.restore-preview',360,800);
 // The retained authenticated archive, rather than unchecked replacement bytes, supplies the restore.
 const changed=Buffer.from(originalArchive);changed[changed.length-1]^=1;await writeFile(archive,changed);const changedHash=await hash(archive);
 await driver.click('#restore-confirmed');assert.equal(await driver.execute('return document.querySelector(".restore-confirmation-form button[type=submit]").disabled'),false);
 const applyStarted=performance.now();await driver.click('.restore-confirmation-form button[type=submit]');await waitFor(async()=>Boolean(await driver.execute('return document.querySelector(".restore-result")')),'restore result',30000);await waitFor(async()=>Boolean(await driver.execute('return document.activeElement?.matches(".restore-result")')),'restore result focus');evidence.restoreToFocusMilliseconds=performance.now()-applyStarted;
 await waitFor(async()=>(await driver.invoke('bootstrap')).value?.theme==='light'&&await driver.execute('return document.documentElement.dataset.theme==="light"'),'restored theme refresh');await unchanged(expectedA);
 assert.equal(await hash(archive),changedHash);assert.equal(await hash(join(workspace,'database-key.v1')),keyHash);
 const rendered=await driver.execute('return{labels:[...document.querySelectorAll("[data-profile-id]")].map(node=>node.textContent),result:document.querySelector(".restore-result").textContent,navigation:performance.getEntriesByType("navigation")[0]?.toJSON()??null,paint:performance.getEntriesByType("paint").map(entry=>entry.toJSON())};');assert.ok(rendered.labels.some(text=>text.includes('Restore original first')));assert.equal(rendered.labels.some(text=>text.includes('Restore third live only')),false);evidence.rendered=rendered;
 await capture('restore-result-light-desktop.png','.restore-result');await capture('restore-result-light-360.png','.restore-result',360,800);
 evidence.liveBeforeExit=await verifyLive();await markersUnchanged();await rejected('apply_restore',{previewId:final.previewId,confirmed:true},'stale_restore_preview');
 await closeNormally();evidence.closedLiveVerification=await verifyLive(true);await open();await unlock();await unchanged(expectedA);await markersUnchanged();assert.equal(await hash(join(workspace,'database-key.v1')),keyHash);await verifyCheckpoint(final.checkpointId,expectedB);
 const homes=await allHomes();assert.equal(homes.length,5);assert.ok(oldHomes.every(id=>homes.includes(id)));for(const id of homes.filter(id=>!oldHomes.includes(id))){assert.equal((await lstat(join(workspace,'profiles',id))).mode&0o777,0o700);assert.deepEqual(await readdir(join(workspace,'profiles',id)),[]);}
 for(const name of ['restore-staging','backup-inspection']){try{assert.deepEqual(await readdir(join(workspace,name)),[]);}catch(error){if(error.code!=='ENOENT')throw error;}}
 assert.deepEqual(await readdir(fixture.dirs.codex),['synthetic-fixture.txt']);
 evidence.steps.push({guiExplicitConfirmation:true,retainedAuthenticatedArchiveUsedAfterSourceTampering:true,restoredSelectedSemanticStateAAfterRestart:true,originalKeyAndWalRetained:true,allImportedProfilesRequireAuthentication:true,twoFreshPrivateEmptyHomesForActiveAndArchivedProfiles:true,threeOriginalHomesAndMarkersRetained:true,normalWindowClose:true,noPlaintextDiskStaging:true,performanceScope:'Debug native GUI lab timing only. No release performance qualification.'});
 for(const key of ['application','verifier','generator','liveVerifier'])assert.equal(await hash(evidence[key].path),evidence[key].sha256);
 for(const[path,digest]of Object.entries(evidence.sourceHashes))assert.equal(await hash(join(repository,path)),digest,`Source drift: ${path}`);
 for(const[path,digest]of Object.entries(evidence.harnessHashes))assert.equal(await hash(join(harness,path)),digest,`Harness drift: ${path}`);
 evidence.result='passed';
} catch(error){evidence.error=error.stack??String(error);process.exitCode=1;if(driver?.session)try{await driver.screenshot(join(output,'failure.png'));evidence.screenshots.push('failure.png');}catch{}}
finally{try{await driver?.stop();}catch(error){evidence.cleanupError=error.message;evidence.result='failed';process.exitCode=1;}if(fixture){stopGroup(fixture.bus,'SIGKILL');await rm(fixture.root,{recursive:true,force:true});}evidence.finishedAt=new Date().toISOString();await writeFile(join(output,'result.json'),JSON.stringify(evidence,null,2)+'\n',{mode:0o600});console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);if(evidence.error)console.error(evidence.error);}
