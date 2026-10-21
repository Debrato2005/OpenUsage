import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness=dirname(fileURLToPath(import.meta.url));
const repository=resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT??join(harness,'../..'));
const application=resolve(process.env.OPENUSAGE_NATIVE_BINARY??join(repository,'target/debug/openusage-desktop'));
const output=resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE??join(tmpdir(),`openusage-production-source-gate-${randomUUID()}`));
const hash=async path=>createHash('sha256').update(await readFile(path)).digest('hex');
const evidence={requirement:'NATIVE-PRODUCTION-SOURCE-GATE-001',result:'failed',startedAt:new Date().toISOString(),scope:'Default production configuration must reject all five exact reviewed native-test executable fixtures before any process launch. Metadata inspection and isolated synthetic workspace setup only. This does not qualify an official Codex binary, live identity, release packaging, or other platforms.'};
let fixture,driver;
await mkdir(output,{recursive:true,mode:0o700});
process.once('exit',()=>{driver?.kill('SIGKILL');stopGroup(fixture?.bus,'SIGKILL');});
try{
 evidence.application={path:application,sha256:await hash(application)};evidence.sourceCommit=execFileSync('git',['rev-parse','HEAD'],{cwd:repository,encoding:'utf8'}).trim();evidence.sourceHashes={};
 const sourceFiles=execFileSync('rg',['--files','apps/desktop/src','apps/desktop/src-tauri/src','crates/openusage-codex/src','crates/openusage-storage/src'],{cwd:repository,encoding:'utf8'}).trim().split('\n');for(const path of[...sourceFiles,'crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json','Cargo.lock','apps/desktop/src-tauri/Cargo.toml','apps/desktop/src-tauri/capabilities/main-window.json','crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py'])evidence.sourceHashes[path]=await hash(join(repository,path));
 evidence.harnessHashes={};for(const path of['provider-source-gate.mjs','fixtures/synthetic-provider.py','fixtures/synthetic-login-provider.py','fixtures/synthetic-quota-provider.py','fixtures/synthetic-activity-provider.py','private-fixture.mjs','webdriver.mjs'])evidence.harnessHashes[path]=await hash(join(harness,path));
 fixture=await createFixture('production-source-gate');evidence.reviewedFixtures=[];
 const protectedHomeBefore=await readdir(fixture.dirs.codex),protectedSentinelBefore=await hash(join(fixture.dirs.codex,'synthetic-fixture.txt'));assert.deepEqual(protectedHomeBefore,['synthetic-fixture.txt']);assert.equal(await readFile(join(fixture.dirs.codex,'synthetic-fixture.txt'),'utf8'),CODEX_SENTINEL);
 driver=new NativeDriver(application,fixture.environment,join(output,'driver.log'));await driver.start();await waitFor(async()=>await driver.execute('return ["#workspace-passphrase","#workspace-confirmation"].every(selector=>{const input=document.querySelector(selector);return Boolean(input&&!input.disabled);});'),'source-gate setup fields ready');const passphrase='OpenUsage synthetic source gate workspace 2026';await driver.type('#workspace-passphrase',passphrase);await driver.type('#workspace-confirmation',passphrase);await driver.click('.unlock-form button[type=submit]');await waitFor(async()=>(await driver.invoke('bootstrap')).value?.storage==='ready','private source-gate workspace');
 const beforeStorage={};for(const path of[fixture.database,`${fixture.database}-wal`,join(dirname(fixture.database),'database-key.v1')])try{beforeStorage[path]=await hash(path);}catch(error){if(error.code!=='ENOENT')throw error;}
 for(const fileName of ['synthetic-provider.py','synthetic-login-provider.py','synthetic-quota-provider.py','synthetic-activity-provider.py','synthetic-collection-provider.py']){
  const source=fileName==='synthetic-collection-provider.py'?join(repository,'crates/openusage-codex/tests/fixtures',fileName):join(harness,'fixtures',fileName);
  const executable=join(fixture.root,fileName);await copyFile(source,executable);await chmod(executable,0o700);const digest=await hash(executable);if(fileName==='synthetic-collection-provider.py')assert.equal(digest,'a21d53d66269e8c25c2f12ce5c37ed3abfef4d63aa003288cf2f7763fc146241');
  const candidate=await driver.invoke('inspect_provider_executable',{path:executable});assert.equal(candidate.ok,true);assert.equal(candidate.value.sourceKind,'unverified');assert.equal(candidate.value.sourceVerified,false);assert.equal(candidate.value.versionChecked,false);assert.equal(candidate.value.sourceUrl,null);assert.equal(candidate.value.sha256,digest);
  assert.deepEqual(await driver.invoke('approve_provider_executable',{candidateId:candidate.value.candidateId,confirmed:true}),{ok:false,error:'provider_source_unverified'});
  assert.equal(await hash(executable),digest);evidence.reviewedFixtures.push({fileName,sha256:digest,productionRegistered:false,approvalRefused:true});
 }
 for(const marker of ['synthetic-execution-observations.jsonl','synthetic-login-observations.jsonl','synthetic-quota-observations.jsonl','synthetic-activity-observations.jsonl'])assert.equal((await readdir(fixture.root)).includes(marker),false,'The production configuration must not execute any fixture, including --version.');
 const profiles=await driver.invoke('list_profiles');assert.equal(profiles.ok,true);assert.deepEqual(profiles.value.profiles,[]);assert.equal(profiles.value.activeProfiles,0);
 async function noExecutionMarkers(directory){for(const entry of await readdir(directory,{withFileTypes:true})){assert.equal(entry.isSymbolicLink(),false,'Synthetic workspace must not contain linked execution evidence.');assert.ok(!['synthetic-collection-observations.jsonl','synthetic-child-pid','synthetic-runtime-home'].includes(entry.name),`Unexpected provider execution marker: ${entry.name}`);if(entry.isDirectory())await noExecutionMarkers(join(directory,entry.name));}}
 await noExecutionMarkers(dirname(fixture.database));
 assert.deepEqual(await readdir(fixture.dirs.codex),['synthetic-fixture.txt']);assert.equal(await readFile(join(fixture.dirs.codex,'synthetic-fixture.txt'),'utf8'),CODEX_SENTINEL);assert.deepEqual(await readdir(fixture.dirs.codex),protectedHomeBefore);assert.equal(await hash(join(fixture.dirs.codex,'synthetic-fixture.txt')),protectedSentinelBefore);for(const[path,digest]of Object.entries(beforeStorage))assert.equal(await hash(path),digest,'Source refusal must preserve encrypted workspace files.');
 evidence.steps={syntheticWorkspaceScannedForExecutionMarkers:true,noProfilesOrManagedProviderHomesSelected:true,syntheticHomeAndEncryptedStoragePreserved:true,productionDidNotRegisterTestFixture:true,metadataInspectionOnly:true,explicitApprovalStillRefused:true,noVersionOrProviderExecution:true};
 assert.equal(await hash(application),evidence.application.sha256);for(const[path,digest]of Object.entries(evidence.sourceHashes))assert.equal(await hash(join(repository,path)),digest,`Source drift: ${path}`);for(const[path,digest]of Object.entries(evidence.harnessHashes))assert.equal(await hash(join(harness,path)),digest,`Harness drift: ${path}`);evidence.result='passed';
}catch(error){evidence.error=error.stack??String(error);process.exitCode=1;if(driver?.session)try{evidence.failureView=await driver.execute('return {heading:document.querySelector("#startup-heading")?.textContent,passphrasePresent:Boolean(document.querySelector("#workspace-passphrase")),confirmationPresent:Boolean(document.querySelector("#workspace-confirmation")),profileFormPresent:Boolean(document.querySelector("#profile-label"))};');await driver.screenshot(join(output,'failure.png'));evidence.screenshots=['failure.png'];}catch{}}
finally{try{await driver?.stop();}catch(error){evidence.cleanupError=error.message;evidence.result='failed';process.exitCode=1;}if(fixture){stopGroup(fixture.bus,'SIGKILL');await rm(fixture.root,{recursive:true,force:true});}evidence.finishedAt=new Date().toISOString();await writeFile(join(output,'result.json'),JSON.stringify(evidence,null,2)+'\n',{mode:0o600});console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);if(evidence.error)console.error(evidence.error);}
