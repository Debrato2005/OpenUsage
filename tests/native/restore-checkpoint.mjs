import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readdir, readlink, rename, rm, rmdir, writeFile } from 'node:fs/promises';
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
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-checkpoint-recovery-evidence-${randomUUID()}`));
const passphrase = 'OpenUsage synthetic apply workspace 2026';
const password = 'OpenUsage synthetic apply archive 2026';
const hash = async (path) => createHash('sha256').update(await readFile(path)).digest('hex');
const liveVerifier = resolve(process.env.OPENUSAGE_LIVE_VERIFIER ?? join(harness, 'backup-verifier/target/debug/openusage-native-live-verifier'));
const evidence = {requirement:'NATIVE-CHECKPOINT-WM-REGRESSION-001',startedAt:new Date().toISOString(),result:'failed',scope:'Actual native encrypted backup restore followed by confirmed checkpoint recovery across genuine X11 WM_DELETE_WINDOW exits. A pending authenticated archive preview is rejected after restart. Synthetic homes only. Selected semantic fields and original home mappings match state B. Timestamps and unselected columns are not compared. No provider login, forced interruption, crash-durability, or cross-platform acceptance.',platform:{os:platform(),release:release()},screenshots:[],steps:[],checkpoints:[],views:[]};
let fixture, driver, workspace, checkpointDirectory, archive, fileName, sequence = 0;
evidence.nativeCloses=[];
async function storageFiles(){const result={};for(const path of[fixture.database,`${fixture.database}-wal`,`${fixture.database}-shm`,join(workspace,'database-key.v1')]){try{const metadata=await lstat(path);result[path]={present:true,fileBytes:String(metadata.size),mode:metadata.mode&0o777,sha256:await hash(path)};}catch(error){if(error.code!=='ENOENT')throw error;result[path]={present:false};}}return result;}
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
async function theme(value) { await driver.click(`#theme option[value="${value}"]`); await waitFor(async () => await driver.execute('return !document.querySelector("#theme").disabled&&document.documentElement.dataset.theme===arguments[0];', [value]), 'appearance saved'); }
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
  const view=await driver.execute('const panel=document.querySelector(arguments[0]);return{width:innerWidth,height:innerHeight,scrollWidth:document.scrollingElement.scrollWidth,focus:document.activeElement?.className,text:panel.textContent,overflow:[panel,...panel.querySelectorAll("*")].filter(node=>{const header=node.closest("thead");if(header&&getComputedStyle(header).clipPath!=="none")return false;const r=node.getBoundingClientRect();return r.width>1&&r.height>1&&(r.left< -1||r.right>innerWidth+1)}).map(node=>node.tagName)}',[selector]);
  assert.equal(view.width,width);assert.equal(view.height,height);assert.ok(view.scrollWidth<=width+1);assert.deepEqual(view.overflow,[]);
  await driver.screenshot(join(output,name));evidence.screenshots.push(name);evidence.views.push({name,...view});
}
async function enlargedText() {
  await resize(360,800);
  await driver.execute('window.__checkpointFontRestore=[...document.querySelectorAll("body *")].map(node=>({node,style:node.getAttribute("style"),size:getComputedStyle(node).fontSize}));for(const item of window.__checkpointFontRestore)item.node.style.fontSize=`${parseFloat(item.size)*2}px`;return true;');
  try {
    await capture('checkpoint-catalog-light-text-200-360.png','.checkpoint-catalog',360,800);
    await capture('checkpoint-comparison-light-text-200-360.png','.restore-comparison',360,800);
    const words=await driver.execute('const words=[];for(const header of document.querySelectorAll(".checkpoint-catalog label,.checkpoint-catalog button,.restore-comparison th,.comparison-label,#restore-preview-title,.restore-confirmation-form button")){const hiddenHeader=header.closest("thead");if(header.getBoundingClientRect().width<2||(hiddenHeader&&getComputedStyle(hiddenHeader).clipPath!=="none"))continue;const walker=document.createTreeWalker(header,NodeFilter.SHOW_TEXT);while(walker.nextNode()){const node=walker.currentNode;for(const match of node.textContent.matchAll(/[A-Za-z]+/g)){const range=document.createRange();range.setStart(node,match.index);range.setEnd(node,match.index+match[0].length);words.push({word:match[0],lines:[...new Set([...range.getClientRects()].filter(rect=>rect.width>0).map(rect=>Math.round(rect.top)))]});}}}return words;');
    assert.deepEqual(words.filter(item=>item.lines.length>1),[],'Natural label words must remain whole with enlarged text.');
    assert.equal(await driver.execute('return document.querySelector(".restore-comparison thead").getBoundingClientRect().width<=1'),true,'The narrow enlarged-text comparison must reflow into labeled pairs.');
    assert.deepEqual(await driver.execute('return [...document.querySelectorAll(".restore-comparison tbody tr")].map(row=>[...row.querySelectorAll(".comparison-label")].map(node=>node.textContent))'),Array.from({length:3},()=>['Current','Checkpoint']));
    await driver.execute('document.querySelector(".restore-preview").focus();return true;');
    await driver.request('POST',`/session/${driver.session}/actions`,{actions:[{type:'key',id:'checkpoint-enlarged-keyboard',actions:[{type:'keyDown',value:'\uE004'},{type:'keyUp',value:'\uE004'}]}]});
    assert.equal(await driver.execute('return document.activeElement?.id'), 'restore-confirmed');
    const controls=await driver.execute('return [...document.querySelectorAll(".restore-confirmation-form input,.restore-confirmation-form button")].map(node=>({selector:node.id||node.textContent.trim(),disabled:node.disabled}));');
    assert.deepEqual(controls.map(item=>item.disabled),[false,true,false]);
    await capture('checkpoint-confirmation-light-text-200-360.png','.restore-confirmation-form',360,800);
    evidence.enlargedText={scale:2,viewport:{width:360,height:800},scope:'Test-side CSS text scaling only. Not OS zoom, screen-reader qualification, or full WCAG acceptance.',naturalWords:words,comparisonReflow:true,keyboardReachedUncheckedConfirmation:true,controls};
  } finally {
    await driver.execute('for(const item of window.__checkpointFontRestore){if(item.style===null)item.node.removeAttribute("style");else item.node.setAttribute("style",item.style);}delete window.__checkpointFontRestore;return true;');
  }
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
  const before=await storageFiles();const receipt=JSON.parse(execFileSync('/usr/bin/python3',[join(harness,'x11-close.py'),String(pid)],{env:fixture.environment,encoding:'utf8',timeout:10000}));
  await waitFor(async()=>{try{await readlink(`/proc/${pid}/exe`);return false;}catch(error){if(error.code==='ENOENT')return true;throw error;}},'normal native exit');
  evidence.nativeCloses.push({receipt,before,after:await storageFiles(),scope:'WM_DELETE_WINDOW sent while WebDriver remains alive; exact owned desktop PID exited before driver cleanup. SQLite may checkpoint or remove WAL/SHM during normal close; no cross-close DB byte equality claim.'});driver.session=undefined;await driver.stop();driver=undefined;
}
async function verifyLive(closedCopy=false, expected=expectedA, homeMode='fresh') {
 const hashes=await liveHashes();let database=fixture.database;
 if(closedCopy){
   const directory=join(fixture.root,'closed-ciphertext-verification');await mkdir(directory,{mode:0o700});database=join(directory,'closed-encrypted.db');
   await copyFile(fixture.database,database);await chmod(database,0o600);assert.equal(await hash(database),await hash(fixture.database));
   try{await copyFile(`${fixture.database}-wal`,`${database}-wal`);await chmod(`${database}-wal`,0o600);assert.equal(await hash(`${database}-wal`),await hash(`${fixture.database}-wal`));}catch(error){if(error.code!=='ENOENT')throw error;}
 }
 const result=JSON.parse(execFileSync(liveVerifier,[database,join(workspace,'database-key.v1')],{input:JSON.stringify({passphrase,expected,forbiddenHomes:oldHomes,homeRoot:workspace,allowEmptyWalCreation:closedCopy,homeMode,expectedHomeReferences:homeMode==='existing'?expected.profiles.map(profile=>profile.id):undefined}),encoding:'utf8',env:fixture.environment,timeout:30000,maxBuffer:1024*1024}));
 assert.equal(result.verified,true);if(homeMode==='fresh')assert.equal(result.freshPrivateEmptyHomes,2);else{assert.equal(result.originalManagedHomesMatched,true);assert.equal(result.freshPrivateEmptyHomes,0);}assert.equal(result.walRecordsUnchanged,true);if(!closedCopy)assert.equal(result.walUnchanged,true);assert.deepEqual(await liveHashes(),hashes);
 if(closedCopy){await copyFile(database,join(output,'verified-closed-live.sqlite'));await chmod(join(output,'verified-closed-live.sqlite'),0o600);evidence.closedDatabaseArtifact={fileName:'verified-closed-live.sqlite',sha256:await hash(database)};try{await copyFile(`${fixture.database}-wal`,join(output,'verified-closed-live.sqlite-wal'));await chmod(join(output,'verified-closed-live.sqlite-wal'),0o600);}catch(error){if(error.code!=='ENOENT')throw error;}}
 return {scope:closedCopy?'Read-only verification of byte-identical private encrypted DB and existing-WAL copies after normal native exit. Original closed files stay unchanged.':'Read-only actual live SQLCipher connection with WAL.',sourceHashes:hashes,...result};
}

await mkdir(output,{recursive:true,mode:0o700});
process.once('exit',()=>{driver?.kill('SIGKILL');stopGroup(fixture?.bus,'SIGKILL');});
try {
 assert.equal(platform(),'linux');
 for(const[key,path]of Object.entries({application,verifier,generator,liveVerifier}))evidence[key]={path,sha256:await hash(path)};
 evidence.sourceCommit=execFileSync('git',['rev-parse','HEAD'],{cwd:repository,encoding:'utf8'}).trim();
 evidence.sourceHashes={};const sources=execFileSync('rg',['--files','apps/desktop/src','apps/desktop/src-tauri/src','crates/openusage-storage/src','crates/openusage-import/src','crates/openusage-codex/src'],{cwd:repository,encoding:'utf8'}).trim().split('\n');
 for(const path of [...sources,'crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json','Cargo.toml','Cargo.lock','crates/openusage-storage/Cargo.toml','crates/openusage-import/Cargo.toml','apps/desktop/src-tauri/Cargo.toml','apps/desktop/src-tauri/tauri.conf.json','apps/desktop/src-tauri/build.rs','apps/desktop/src-tauri/capabilities/main-window.json'])evidence.sourceHashes[path]=await hash(join(repository,path));
 evidence.harnessHashes={};for(const path of ['restore-checkpoint.mjs','x11-close.py','private-fixture.mjs','webdriver.mjs','backup-verifier/Cargo.toml','backup-verifier/Cargo.lock','backup-verifier/src/bin/openusage-native-checkpoint-verifier.rs','backup-verifier/src/bin/openusage-native-live-verifier.rs','backup-verifier/src/bin/openusage-native-backup-fixtures.rs'])evidence.harnessHashes[path]=await hash(join(harness,path));
 fixture=await createFixture('checkpoint-recovery');workspace=dirname(fixture.database);checkpointDirectory=join(workspace,'restore-checkpoints');await open();
 await rejected('list_restore_checkpoints',{},'workspace_locked');
 await rejected('prepare_checkpoint_restore_preview',{checkpointId:randomUUID()},'workspace_locked');
 for(const command of ['list_restore_checkpoints','prepare_checkpoint_restore_preview'])await rejected(command,[],'invalid_arguments');
 await rejected('list_restore_checkpoints',{unexpected:true},'invalid_arguments');
 await rejected('prepare_checkpoint_restore_preview',{checkpointId:'../unsafe'},'invalid_arguments');
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

 await rejected('prepare_checkpoint_restore_preview',{checkpointId:randomUUID()},'checkpoint_not_found');
 assert.deepEqual(await driver.invoke('list_restore_checkpoints'),{ok:true,value:{checkpoints:[],truncated:false}});
 await inspect();const importedPreview=await prepareGui();await verifyCheckpoint(importedPreview.checkpointId,expectedB);const sourceCheckpointId=importedPreview.checkpointId;
 await driver.click('#restore-confirmed');await driver.click('.restore-confirmation-form button[type=submit]');await waitFor(async()=>Boolean(await driver.execute('return document.activeElement?.matches(".restore-result")')),'initial import result',30000);await unchanged(expectedA);await verifyLive();
 const homeSetAfterImport=await allHomes();assert.equal(homeSetAfterImport.length,5);
 const pendingPreview=await driver.invoke('prepare_restore_preview',{fileName,password});assert.equal(pendingPreview.ok,true);assert.equal(pendingPreview.value.restoreAvailable,true);await verifyCheckpoint(pendingPreview.value.checkpoint.id,expectedA);evidence.pendingArchivePreviewBeforeClose=pendingPreview.value;
 await closeNormally();await open();await unlock();await unchanged(expectedA);const reopenedHashes=await liveHashes();await rejected('apply_restore',{previewId:pendingPreview.value.previewId,confirmed:true},'stale_restore_preview');assert.deepEqual(await liveHashes(),reopenedHashes);assert.deepEqual(await allHomes(),homeSetAfterImport);assert.equal(await hash(join(workspace,'database-key.v1')),keyHash);await unchanged(expectedA);await verifyLive();evidence.steps.push({pendingAuthenticatedArchivePreviewRejectedAfterGenuineNativeClose:true,staleRefusalPreservesReopenedDatabaseWalKeyAndHomes:true});
 const listed=await driver.invoke('list_restore_checkpoints');assert.equal(listed.ok,true);assert.deepEqual(Object.keys(listed.value).sort(),['checkpoints','truncated']);assert.equal(typeof listed.value.truncated,'boolean');
 for(const item of listed.value.checkpoints){assert.deepEqual(Object.keys(item).sort(),['fileBytes','id','modifiedAt']);assert.match(item.id,/^[0-9a-f-]{36}$/);assert.match(item.fileBytes,/^[1-9][0-9]*$/);if(item.modifiedAt!==null)assert.equal(new Date(item.modifiedAt).toISOString(),item.modifiedAt);}
 assert.ok(listed.value.checkpoints.some(item=>item.id===sourceCheckpointId));evidence.catalog=listed.value;
 async function prepareRecovery(){const result=await driver.invoke('prepare_checkpoint_restore_preview',{checkpointId:sourceCheckpointId});assert.equal(result.ok,true);assert.deepEqual(Object.keys(result.value).sort(),['checkpoint','checkpointId','current','incoming','previewId','restoreAvailable']);assert.equal(result.value.checkpointId,sourceCheckpointId);assert.equal(result.value.restoreAvailable,true);assert.deepEqual(result.value.current,{activeProfiles:1,archivedProfiles:'1',accounts:'0'});assert.deepEqual(result.value.incoming,{activeProfiles:3,archivedProfiles:'0',accounts:'0'});return result.value;}
 async function refuse(preview,error){const hashes=await liveHashes(),homes=await allHomes();await rejected('apply_restore',{previewId:preview.previewId,confirmed:true},error);assert.deepEqual(await liveHashes(),hashes);assert.deepEqual(await allHomes(),homes);assert.deepEqual(await state(),expectedA);}
 let preview=await prepareRecovery();await driver.invoke('set_theme',{theme:'dark'});await driver.invoke('set_theme',{theme:'light'});await refuse(preview,'stale_restore_preview');await markersUnchanged();
 const sourceCheckpoint=join(checkpointDirectory,`${sourceCheckpointId}.sqlite`);const checkpointBytes=await readFile(sourceCheckpoint),corrupt=Buffer.from(checkpointBytes);corrupt[corrupt.length-1]^=1;const checkpointCount=(await checkpoints()).length;await writeFile(sourceCheckpoint,corrupt);await rejected('prepare_checkpoint_restore_preview',{checkpointId:sourceCheckpointId},'restore_checkpoint_failed');assert.equal((await checkpoints()).length,checkpointCount);await writeFile(sourceCheckpoint,checkpointBytes);await unchanged(expectedA);
 preview=await prepareRecovery();const originalHome=join(workspace,'profiles',oldHomes[0]),heldHome=`${originalHome}.held`;await rename(originalHome,heldHome);await mkdir(originalHome,{mode:0o700});try{await refuse(preview,'restore_home_unavailable');}finally{await rmdir(originalHome);await rename(heldHome,originalHome);}await markersUnchanged();assert.deepEqual(await allHomes(),homeSetAfterImport);
 evidence.steps.push({durableCatalogAfterRestart:true,metadataOnlyCatalog:true,staleThemeRejected:true,corruptSourceCheckpointRejectedBeforePublication:true,replacedOriginalHomeRejected:true,refusalsPreserveCurrentSelectedStateAndKey:true});
 await click('Restore a checkpoint');await waitFor(async()=>Boolean(await driver.execute('return document.querySelector("#restore-checkpoint")&&!document.querySelector("#restore-checkpoint").disabled')),'checkpoint catalog UI');await driver.click(`#restore-checkpoint option[value="${sourceCheckpointId}"]`);await click('Preview checkpoint');await waitFor(async()=>Boolean(await driver.execute('return document.activeElement?.matches(".restore-preview")')),'checkpoint recovery preview',30000);
 assert.deepEqual(await driver.execute('return [...document.querySelectorAll(".restore-comparison tbody tr")].map(row=>[row.firstElementChild.textContent,...[...row.querySelectorAll("[data-restore-count]")].map(node=>node.textContent)])'),[['Active profiles','1','3'],['Archived profiles','1','0'],['Accounts','0','0']]);
 const recovery=await driver.execute('const panel=document.querySelector(".restore-preview");return{previewId:panel.dataset.previewId,checkpointId:panel.dataset.checkpointId};');await verifyCheckpoint(recovery.checkpointId,expectedA);assert.notEqual(recovery.checkpointId,sourceCheckpointId);
 assert.equal(await driver.execute('return Boolean(document.querySelector("#checkpoint-recovery input[type=password]"))'),false);
 assert.deepEqual(await driver.execute('return{checked:document.querySelector("#restore-confirmed").checked,disabled:document.querySelector(".restore-confirmation-form button[type=submit]").disabled}'),{checked:false,disabled:true});
 await capture('checkpoint-confirmation-light-desktop.png','.restore-preview');await capture('checkpoint-confirmation-light-360.png','.restore-preview',360,800);await enlargedText();await driver.click('#restore-confirmed');const started=performance.now();await driver.click('.restore-confirmation-form button[type=submit]');await waitFor(async()=>Boolean(await driver.execute('return document.activeElement?.matches(".restore-result")')),'checkpoint recovery result',30000);evidence.recoveryToFocusMilliseconds=performance.now()-started;
 await waitFor(async()=>await driver.execute('return document.documentElement.dataset.theme==="dark"'),'recovered appearance');await unchanged(expectedB);assert.deepEqual(await allHomes(),homeSetAfterImport);assert.equal(await hash(join(workspace,'database-key.v1')),keyHash);assert.equal(await hash(sourceCheckpoint),createHash('sha256').update(checkpointBytes).digest('hex'));
 await capture('checkpoint-result-dark-desktop.png','.restore-result');await capture('checkpoint-result-dark-360.png','.restore-result',360,800);await verifyLive(false,expectedB,'existing');await closeNormally();evidence.closedLiveVerification=await verifyLive(true,expectedB,'existing');await open();await unlock();await unchanged(expectedB);await markersUnchanged();await verifyCheckpoint(recovery.checkpointId,expectedA);assert.deepEqual(await allHomes(),homeSetAfterImport);for(const id of homeSetAfterImport.filter(id=>!oldHomes.includes(id))){assert.deepEqual(await readdir(join(workspace,'profiles',id)),[]);assert.equal((await lstat(join(workspace,'profiles',id))).mode&0o777,0o700);}assert.equal(await hash(archive),evidence.originalArchiveSha256);assert.equal(await hash(join(workspace,'database-key.v1')),keyHash);
 evidence.steps.push({explicitRecoveryConfirmation:true,newReversibleCheckpointBeforeRecovery:true,selectedSemanticStateBAfterRestart:true,originalThreeManagedHomeMappingsRestored:true,allOriginalMarkersUnchanged:true,importedFreshHomesRetained:true,originalProtectedKeyUnchanged:true,noPasswordOrCredentialReadsForRecovery:true,monitoringStopped:true,performanceScope:'Debug native lab timing only. No release performance qualification.'});
 for(const key of ['application','verifier','generator','liveVerifier'])assert.equal(await hash(evidence[key].path),evidence[key].sha256);
 for(const[path,digest]of Object.entries(evidence.sourceHashes))assert.equal(await hash(join(repository,path)),digest,`Source drift: ${path}`);
 for(const[path,digest]of Object.entries(evidence.harnessHashes))assert.equal(await hash(join(harness,path)),digest,`Harness drift: ${path}`);
 evidence.result='passed';
} catch(error){evidence.error=error.stack??String(error);process.exitCode=1;if(driver?.session)try{await driver.screenshot(join(output,'failure.png'));evidence.screenshots.push('failure.png');}catch{}}
finally{try{await driver?.stop();}catch(error){evidence.cleanupError=error.message;evidence.result='failed';process.exitCode=1;}if(fixture){stopGroup(fixture.bus,'SIGKILL');await rm(fixture.root,{recursive:true,force:true});}evidence.finishedAt=new Date().toISOString();await writeFile(join(output,'result.json'),JSON.stringify(evidence,null,2)+'\n',{mode:0o600});console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);if(evidence.error)console.error(evidence.error);}
