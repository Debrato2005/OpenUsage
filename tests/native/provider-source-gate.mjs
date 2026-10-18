import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness=dirname(fileURLToPath(import.meta.url));
const repository=resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT??join(harness,'../..'));
const application=resolve(process.env.OPENUSAGE_NATIVE_BINARY??join(repository,'target/debug/openusage-desktop'));
const output=resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE??join(tmpdir(),`openusage-production-source-gate-${randomUUID()}`));
const hash=async path=>createHash('sha256').update(await readFile(path)).digest('hex');
const evidence={requirement:'NATIVE-PRODUCTION-SOURCE-GATE-001',result:'failed',startedAt:new Date().toISOString(),scope:'Default production configuration must reject the exact reviewed native-test executable fixture before any process launch. Metadata inspection and isolated synthetic workspace setup only. This does not qualify an official Codex binary, live identity, release packaging, or other platforms.'};
let fixture,driver;
await mkdir(output,{recursive:true,mode:0o700});
process.once('exit',()=>{driver?.kill('SIGKILL');stopGroup(fixture?.bus,'SIGKILL');});
try{
 evidence.application={path:application,sha256:await hash(application)};evidence.sourceCommit=execFileSync('git',['rev-parse','HEAD'],{cwd:repository,encoding:'utf8'}).trim();evidence.sourceHashes={};
 const sourceFiles=execFileSync('rg',['--files','apps/desktop/src','apps/desktop/src-tauri/src','crates/openusage-codex/src','crates/openusage-storage/src'],{cwd:repository,encoding:'utf8'}).trim().split('\n');for(const path of[...sourceFiles,'crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json','Cargo.lock','apps/desktop/src-tauri/Cargo.toml','apps/desktop/src-tauri/capabilities/main-window.json'])evidence.sourceHashes[path]=await hash(join(repository,path));
 evidence.harnessHashes={};for(const path of['provider-source-gate.mjs','fixtures/synthetic-provider.py','private-fixture.mjs','webdriver.mjs'])evidence.harnessHashes[path]=await hash(join(harness,path));
 fixture=await createFixture('production-source-gate');const executable=join(fixture.root,'synthetic-provider');await copyFile(join(harness,'fixtures/synthetic-provider.py'),executable);await chmod(executable,0o700);evidence.reviewedFixtureSha256=await hash(executable);
 driver=new NativeDriver(application,fixture.environment,join(output,'driver.log'));await driver.start();const passphrase='OpenUsage synthetic source gate workspace 2026';await driver.type('#workspace-passphrase',passphrase);await driver.type('#workspace-confirmation',passphrase);await driver.click('.unlock-form button[type=submit]');await waitFor(async()=>(await driver.invoke('bootstrap')).value?.storage==='ready','private source-gate workspace');
 const candidate=await driver.invoke('inspect_provider_executable',{path:executable});assert.equal(candidate.ok,true);assert.equal(candidate.value.sourceKind,'unverified');assert.equal(candidate.value.sourceVerified,false);assert.equal(candidate.value.versionChecked,false);assert.equal(candidate.value.sourceUrl,null);assert.equal(candidate.value.sha256,evidence.reviewedFixtureSha256);
 assert.deepEqual(await driver.invoke('approve_provider_executable',{candidateId:candidate.value.candidateId,confirmed:true}),{ok:false,error:'provider_source_unverified'});
 assert.equal((await readdir(fixture.root)).includes('synthetic-execution-observations.jsonl'),false,'The production configuration must not execute the fixture, including --version.');
 evidence.steps={productionDidNotRegisterTestFixture:true,metadataInspectionOnly:true,explicitApprovalStillRefused:true,noVersionOrProviderExecution:true};
 assert.equal(await hash(application),evidence.application.sha256);assert.equal(await hash(executable),evidence.reviewedFixtureSha256);for(const[path,digest]of Object.entries(evidence.sourceHashes))assert.equal(await hash(join(repository,path)),digest,`Source drift: ${path}`);for(const[path,digest]of Object.entries(evidence.harnessHashes))assert.equal(await hash(join(harness,path)),digest,`Harness drift: ${path}`);evidence.result='passed';
}catch(error){evidence.error=error.stack??String(error);process.exitCode=1;}
finally{try{await driver?.stop();}catch(error){evidence.cleanupError=error.message;evidence.result='failed';process.exitCode=1;}if(fixture){stopGroup(fixture.bus,'SIGKILL');await rm(fixture.root,{recursive:true,force:true});}evidence.finishedAt=new Date().toISOString();await writeFile(join(output,'result.json'),JSON.stringify(evidence,null,2)+'\n',{mode:0o600});console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);if(evidence.error)console.error(evidence.error);}
