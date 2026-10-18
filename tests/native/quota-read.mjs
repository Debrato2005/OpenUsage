import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readdir, readlink, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-native-quota-${randomUUID()}`));
const fixtureDigest = 'fecabb5cc4f5f799c3b7ab3e5475926551d7be5862413d1bbb650313abe66616';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const password = 'OpenUsage synthetic quota workspace 2026';
const backupPassword = 'OpenUsage synthetic quota backup 2026';
const evidence = { requirement: 'NATIVE-QUOTA-PREVIEW-001', result: 'failed', startedAt: new Date().toISOString(), scope: 'Actual native one-shot quota preview using a reviewed static synthetic provider under native-test. These are controlled fixture values, not production usage. No real OAuth, existing credential homes, verified identity, accepted readings, combined totals, credit spending, accepted credit history, reset confirmation, monitoring, release performance, or cross-platform qualification. Short timeout behavior remains core-qualified only.', screenshots: [], views: [], cases: [], steps: [] };
let fixture, driver, workspace, executable, executionLog, first, other, archived, approvalId, home, freshHome;
const active = new Map();
const readSelector = id => `[data-quota-read="${id}"]`;
const cancelSelector = id => `[data-quota-cancel="${id}"]`;
const resultSelector = id => `.quota-result[data-quota-profile="${id}"]`;
async function observations() { try { return (await readFile(executionLog, 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); } catch (error) { if (error.code === 'ENOENT') return []; throw error; } }
async function config(path, selectedMode) {
  const file = join(fixture.root, 'synthetic-quota-provider-config.json'); let value = { defaultMode: 'multi', modesByHome: {} };
  try { value = JSON.parse(await readFile(file, 'utf8')); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  value.modesByHome[path] = selectedMode; await writeFile(`${file}.next`, JSON.stringify(value) + '\n', { mode: 0o600 }); await rename(`${file}.next`, file);
}
async function liveHashes() { const value = {}; for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) try { value[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; } return value; }
async function files(path) {
  const value = {}; async function visit(directory, relative) { for (const item of await readdir(directory, { withFileTypes: true })) { assert.equal(item.isSymbolicLink(), false); const full = join(directory, item.name), name = join(relative, item.name); if (item.isDirectory()) { value[name] = { directory: true, mode: (await lstat(full)).mode & 0o777 }; await visit(full, name); } else { assert.equal(item.isFile(), true); value[name] = { sha256: await hash(full), mode: (await lstat(full)).mode & 0o777 }; } } } await visit(path, ''); return value;
}
async function profiles() { const response = await driver.invoke('list_profiles'); assert.equal(response.ok, true); return response.value.profiles; }
async function create(label) { await waitFor(async () => await driver.execute('return Boolean(document.querySelector("#profile-label")&&!document.querySelector("#profile-label").disabled);'), 'enabled profile form'); await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]'); await waitFor(async () => await driver.execute('const field=document.querySelector("#profile-label");return !field.disabled&&field.value==="";'), 'profile created'); return (await profiles()).find(profile => profile.label === label); }
async function rejected(command, args, error) { assert.deepEqual(await driver.invoke(command, args), { ok: false, error }); }
async function stage(kind, path, offset = 0) { let found; await waitFor(async () => { const rows = await observations(); const starts = new Map(rows.filter(row => row.kind === 'start').map(row => [row.pid, row])); found = rows.slice(offset).find(row => row.kind === kind && starts.get(row.pid)?.environment.CODEX_HOME === path); return Boolean(found); }, `${kind} in selected home`); return found; }
async function reaped(row) { await waitFor(() => { for (const target of [row.pid, -row.processGroup]) try { process.kill(target, 0); return false; } catch (error) { if (error.code !== 'ESRCH') throw error; } return true; }, 'owned quota process and group reaped'); }
async function begin(profile = first) {
  const result = await driver.invoke('begin_profile_quota_read', { profileId: profile.id, approvalId }); assert.equal(result.ok, true);
  assert.deepEqual(Object.keys(result.value).sort(), ['accepted', 'approvalId', 'monitoring', 'profileId', 'requestId', 'sourceKind']); assert.equal(result.value.profileId, profile.id); assert.equal(result.value.approvalId, approvalId); assert.equal(result.value.sourceKind, 'test_fixture'); assert.equal(result.value.accepted, false); assert.equal(result.value.monitoring, 'stopped'); assert.match(result.value.requestId, /^[0-9a-f-]{36}$/); active.set(result.value.requestId, profile.id); return result.value;
}
function validateOutcome(value, request) {
  assert.deepEqual(Object.keys(value).sort(), ['accepted', 'approvalId', 'availableResetCredits', 'identityConfidence', 'limits', 'monitoring', 'observedAtUtc', 'profileId', 'providerVersion', 'requestId', 'resetCreditDetails', 'sourceKind', 'verifiedIdentityKey']);
  assert.equal(value.requestId, request.requestId); assert.equal(value.profileId, request.profileId); assert.equal(value.approvalId, approvalId); assert.equal(value.sourceKind, 'test_fixture'); assert.equal(value.providerVersion, '0.163.0'); assert.equal(value.identityConfidence, 'provisional'); assert.equal(value.verifiedIdentityKey, null); assert.equal(value.accepted, false); assert.equal(value.monitoring, 'stopped'); assert.ok(Number.isFinite(Date.parse(value.observedAtUtc)));
  assert.ok(value.availableResetCredits === null || (typeof value.availableResetCredits === 'string' && value.availableResetCredits.trim() === value.availableResetCredits && /^(0|[1-9][0-9]{0,18})$/.test(value.availableResetCredits) && (value.availableResetCredits.length < 19 || value.availableResetCredits <= '9223372036854775807')));
  assert.ok(value.resetCreditDetails === null || (Array.isArray(value.resetCreditDetails) && value.resetCreditDetails.length <= 32));
  for (const row of value.resetCreditDetails ?? []) {
    assert.deepEqual(Object.keys(row).sort(), ['expiresAtUnix', 'grantedAtUnix', 'status', 'title']);
    assert.ok(row.title === null || typeof row.title === 'string' && row.title.trim() && Buffer.byteLength(row.title, 'utf8') <= 256);
    assert.ok(['available', 'redeeming', 'redeemed', 'unknown'].includes(row.status));
    assert.ok(Number.isSafeInteger(row.grantedAtUnix) && row.grantedAtUnix >= -62135596800 && row.grantedAtUnix <= 253402300799);
    assert.ok(row.expiresAtUnix === null || Number.isSafeInteger(row.expiresAtUnix) && row.expiresAtUnix >= -62135596800 && row.expiresAtUnix <= 253402300799);
  }
  for (const limit of value.limits) {
    assert.deepEqual(Object.keys(limit).sort(), ['limitId', 'limitName', 'monthlyCreditLimit', 'primary', 'reachedType', 'secondary', 'spendControlReached']);
    assert.ok(limit.monthlyCreditLimit === null || typeof limit.monthlyCreditLimit === 'string' && limit.monthlyCreditLimit.trim().length > 0 && Buffer.byteLength(limit.monthlyCreditLimit, 'utf8') <= 256 && !/[\u0000-\u001f\u007f-\u009f]/u.test(limit.monthlyCreditLimit));
    assert.ok(limit.spendControlReached === null || typeof limit.spendControlReached === 'boolean');
  }
  for (const forbidden of ['synthetic-private-account-id', 'synthetic-quota@example.invalid', 'accountId', 'rateLimitResetCredits', 'syntheticPrivateCredit', 'legacy-must-not-be-counted', 'synthetic-private-credit-id', 'synthetic-private-reset-type', 'synthetic-private-credit-description', 'synthetic-private-future-status', 'ordinaryUsageAllowed', '987654321.98765', 'remainingPercent', 'balance']) assert.equal(JSON.stringify(value).includes(forbidden), false);
}
async function complete(request, expectedError) {
  const result = await driver.invoke('complete_profile_quota_read', { profileId: request.profileId, requestId: request.requestId }); active.delete(request.requestId);
  if (expectedError) assert.deepEqual(result, { ok: false, error: expectedError }); else { assert.equal(result.ok, true); validateOutcome(result.value, request); } return result.value;
}
async function guiRead(mode = 'multi') {
  await config(home, mode); const offset = (await observations()).length;
  await waitFor(async () => await driver.execute('return Boolean(document.querySelector(arguments[0])&&!document.querySelector(arguments[0]).disabled);', [readSelector(first.id)]), 'enabled Read quotas action');
  await driver.click(readSelector(first.id));
  await waitFor(async () => await driver.execute('return document.activeElement?.matches(arguments[0])&&!document.querySelector(arguments[1]).disabled;', [resultSelector(first.id), readSelector(first.id)]), 'completed quota result focus');
  const preview = await driver.execute('const root=document.querySelector(arguments[0]);return {role:root.getAttribute("role"),heading:root.querySelector("h3")?.textContent.trim(),text:root.textContent};', [resultSelector(first.id)]);
  assert.equal(preview.role, 'region', 'A failed terminal result must not count as a successful read.'); assert.equal(preview.heading, 'Quota preview');
  const rows = await observations(), starts = new Map(rows.filter(row => row.kind === 'start').map(row => [row.pid, row]));
  const finalRequest = rows.slice(offset).find(row => row.kind === 'request' && row.request.id === 4 && row.request.method === 'account/read' && starts.get(row.pid)?.environment.CODEX_HOME === home);
  assert.ok(finalRequest, 'This selected-home read must reach a new final account request.');
  assert.deepEqual(finalRequest.request.params, { refreshToken: false });
  assert.deepEqual(rows.filter(row=>row.kind==='request'&&row.pid===finalRequest.pid).map(row=>({method:row.request.method,id:row.request.id??null})), [{method:'initialize',id:1},{method:'initialized',id:null},{method:'account/read',id:2},{method:'account/rateLimits/read',id:3},{method:'account/read',id:4}]);
  // Native accepts the final response and reaps its child before the fixture's later log can run.
  (evidence.guiReadBarriers ??= []).push({ mode, processId: finalRequest.pid, selectedHomeFinalAccountRequest: true, focusedSuccessfulPreview: true, postResponseMarkerObserved: rows.some(row=>row.kind==='final-account-emitted'&&row.pid===finalRequest.pid), markerScope: 'Optional post-response fixture log; it follows stdout flush and may lose a race with owned-child cleanup.' });
  return preview.text;
}
async function disclosureState(selector, index = 0) {
  return driver.execute('const root=document.querySelectorAll(arguments[0])[arguments[1]],trigger=root&&[...root.children].find(node=>node.matches("[data-slot=collapsible-trigger]")),panel=trigger&&document.getElementById(trigger.getAttribute("aria-controls"));const visible=node=>Boolean(node&&node.getBoundingClientRect().width>0&&node.getBoundingClientRect().height>0&&getComputedStyle(node).visibility!=="hidden"&&getComputedStyle(node).display!=="none");return{tag:trigger?.tagName,label:trigger?.textContent.trim(),expanded:trigger?.getAttribute("aria-expanded"),triggerVisible:visible(trigger),disabled:trigger?.disabled,panelVisible:visible(panel),ownsPanel:trigger?.getAttribute("aria-expanded")!=="true"||Boolean(panel&&panel.parentElement===root&&panel.matches("[data-slot=collapsible-panel]")),focused:document.activeElement===trigger};', [selector,index]);
}
async function waitDisclosure(selector, open, index = 0) {
  await waitFor(async () => { const value=await disclosureState(selector,index);return value.expanded===String(open)&&value.panelVisible===open&&value.ownsPanel; }, `${open?'open':'closed'} owned quota disclosure`);
  const value=await disclosureState(selector,index);assert.equal(value.tag,'BUTTON');assert.equal(value.triggerVisible,true);assert.equal(value.disabled,false);return value;
}
async function setDisclosure(selector, open, index = 0) {
  const before=await disclosureState(selector,index);assert.equal(before.tag,'BUTTON');assert.equal(before.triggerVisible,true);assert.equal(before.disabled,false);
  if(before.expanded!==String(open)){
    const trigger=await driver.execute('return [...document.querySelectorAll(arguments[0])[arguments[1]].children].find(node=>node.matches("[data-slot=collapsible-trigger]"));',[selector,index]);
    await driver.request('POST',`/session/${driver.session}/element/${trigger['element-6066-11e4-a52e-4f735466cecf']}/click`,{});
  }
  return waitDisclosure(selector,open,index);
}
async function previewDisclosure() {
  const selector=resultSelector(first.id),limits=`${selector} .quota-limit`,notesSelector=`${selector} .quota-preview-notes`,sourceSelector=`${selector} .quota-source-details`;
  const observationCount=(await observations()).length;
  const summary=await driver.execute('const panel=document.querySelector(arguments[0]);const visible=node=>Boolean(node&&!node.closest("details:not([open]),[data-slot=collapsible-panel][data-closed]")&&node.getBoundingClientRect().width>0&&node.getBoundingClientRect().height>0&&getComputedStyle(node).visibility!=="hidden");const heading=panel.querySelector("h3"),fixture=panel.querySelector("[data-provider-fixture]"),identity=panel.querySelector(".quota-preview-meta > p"),time=panel.querySelector(".quota-observed time");return {heading:heading.textContent.trim(),fixture:fixture.textContent.trim(),identity:identity.textContent.trim(),headingVisible:visible(heading),fixtureVisible:visible(fixture),identityVisible:visible(identity),freshnessVisible:visible(time)};',[selector]);
  assert.deepEqual(summary,{heading:'Quota preview',fixture:'Synthetic test data',identity:'Unverified identity · Not saved',headingVisible:true,fixtureVisible:true,identityVisible:true,freshnessVisible:true});
  const initialLimits=[await waitDisclosure(limits,true),await waitDisclosure(limits,false,1)];
  assert.equal((await waitDisclosure(notesSelector,false)).label,'About this preview');
  assert.equal((await waitDisclosure(sourceSelector,false)).label,'Reading details');
  assert.equal(await driver.execute('return document.activeElement?.matches(arguments[0]);',[selector]),true);
  await key('\uE004');assert.equal((await disclosureState(limits)).focused,true,'Tab must reach the first limit trigger.');
  await key(' ');await waitDisclosure(limits,false);await key('\uE007');await waitDisclosure(limits,true);
  await key('\uE004');assert.equal((await disclosureState(sourceSelector)).focused,true,'Tab must reach Reading details.');
  await key('\uE007');await waitDisclosure(sourceSelector,true);
  const reading=await driver.execute('const root=document.querySelector(arguments[0]),panel=document.getElementById(root.querySelector("[data-slot=collapsible-trigger]").getAttribute("aria-controls"));return{rows:[...panel.querySelectorAll("dt")].map(node=>[node.textContent,node.nextElementSibling.textContent]),visible:panel.getBoundingClientRect().height>0};',[sourceSelector]);
  assert.deepEqual(reading,{rows:[['Provider limit ID','codex'],['Provider status','rate_limit_reached'],['Monthly credit limit','Not supplied'],['Spend control','Not supplied']],visible:true});
  await key(' ');await waitDisclosure(sourceSelector,false);
  await key('\uE004');assert.equal((await disclosureState(limits,1)).focused,true,'Tab must skip closed Reading details and reach the second limit.');
  await key('\uE007');await waitDisclosure(limits,true,1);await key(' ');await waitDisclosure(limits,false,1);
  await setDisclosure(notesSelector,true);
  const notes=await driver.execute('const root=document.querySelector(arguments[0]),panel=document.getElementById(root.querySelector("[data-slot=collapsible-trigger]").getAttribute("aria-controls"));return{paragraphs:[...panel.querySelectorAll("p")].map(node=>({text:node.textContent.trim(),visible:node.getBoundingClientRect().width>0&&node.getBoundingClientRect().height>0&&getComputedStyle(node).visibility!=="hidden"}))};',[notesSelector]);
  assert.ok(notes.paragraphs.every(node=>node.visible));
  const text=notes.paragraphs.map(node=>node.text).join(' ');
  assert.match(text,/not saved or included in combined totals/);assert.match(text,/Monitoring is stopped/);assert.match(text,/Remaining quota is calculated from reported usage/);assert.match(text,/Expected reset times do not confirm that a reset occurred/);assert.match(text,/Activity history is not available/);assert.match(text,/separate from subscription payments and OpenAI Platform API billing/);
  await key(' ');await waitDisclosure(notesSelector,false);await key('\uE007');await waitDisclosure(notesSelector,true);await setDisclosure(notesSelector,false);
  assert.equal((await observations()).length,observationCount,'Disclosure actions must execute no provider.');
  evidence.previewDisclosure={summary,initialLimits,expandedReading:reading,expandedNotes:notes,closedBeforeCaptures:true,actualTabSpaceEnter:true,ownedVisibleTriggerPanels:true};
}
async function creditDetails(expected, expectedRows = null) {
  const selector = resultSelector(first.id), notes = `${selector} .quota-preview-notes`;
  const before = (await observations()).length;
  await waitDisclosure(notes, false);
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).querySelectorAll("[data-earned-reset-credits]").length;', [selector]), 0, 'Closed About content must remain unmounted.');
  await setDisclosure(notes, true);
  const displayed = await driver.execute('const root=document.querySelector(arguments[0]),value=root.querySelector("[data-earned-reset-credits]"),panel=value.closest("[data-slot=collapsible-panel]");return {label:value.previousElementSibling.textContent.trim(),text:value.textContent.trim(),count:root.querySelectorAll("[data-earned-reset-credits]").length,visible:value.getBoundingClientRect().width>0&&value.getBoundingClientRect().height>0&&getComputedStyle(value).visibility!=="hidden",explanation:panel.textContent,actions:panel.querySelectorAll("button,input,select,textarea,form").length};', [selector]);
  assert.equal(displayed.label, 'Earned reset credits');
  assert.equal(displayed.text, expected ?? 'Not supplied');
  assert.equal(displayed.count, 1, 'The response must show its count once, independent of quota-limit count.');
  assert.equal(displayed.visible, true);
  const viewport = await creditViewport(); assert.equal(viewport.lineCount, 1); assert.ok(viewport.scrollWidth <= viewport.clientWidth + 1, 'Normal-size counts must remain fully visible without scrolling.');
  assert.equal(displayed.actions, 0, 'Credit observation must offer no spending or configuration control.');
  assert.match(displayed.explanation, /separate from subscription payments and OpenAI Platform API billing/);
  assert.match(displayed.explanation, /Activity history is not available/);
  const detailState = await driver.execute('const root=document.querySelector(arguments[0]),state=root.querySelector("[data-credit-detail-state]"),list=root.querySelector(".quota-credit-details");return {state:state?.getAttribute("data-credit-detail-state")??null,message:state?.textContent.trim()??null,label:list?.getAttribute("aria-label")??null,rows:[...root.querySelectorAll("[data-credit-detail]")].map(row=>({title:row.querySelector(".quota-credit-title").textContent.trim(),fields:[...row.querySelectorAll("dt")].map(node=>({label:node.textContent.trim(),text:node.nextElementSibling.textContent.trim(),dateTime:node.nextElementSibling.querySelector("time")?.dateTime??null}))})),text:root.textContent};', [selector]);
  if (expectedRows === null || expectedRows.length === 0) {
    assert.equal(detailState.state, expectedRows === null ? 'unavailable' : 'empty');
    assert.equal(detailState.message, expectedRows === null ? 'Credit details were not supplied.' : 'No credit rows were returned.');
    assert.deepEqual(detailState.rows, []); assert.equal(detailState.label, null);
  } else {
    assert.equal(detailState.state, null); assert.equal(detailState.label, 'Credit details');
    const status = { available: 'Available', redeeming: 'Being redeemed', redeemed: 'Redeemed', unknown: 'Unknown' };
    const date = seconds => new Date(seconds * 1000).toISOString();
    assert.equal(detailState.rows.length, expectedRows.length);
    for (const [index, row] of expectedRows.entries()) {
      const actual = detailState.rows[index]; assert.equal(actual.title, row.title ?? 'Title not supplied');
      assert.deepEqual(actual.fields.map(field=>field.label), ['Status', 'Granted', 'Expires']);
      assert.equal(actual.fields[0].text, status[row.status]); assert.equal(actual.fields[0].dateTime, null);
      assert.equal(actual.fields[1].dateTime, date(row.grantedAtUnix));
      assert.equal(actual.fields[2].dateTime, row.expiresAtUnix === null ? null : date(row.expiresAtUnix));
      if (row.expiresAtUnix === null) assert.equal(actual.fields[2].text, 'No expiry');
      assert.ok(actual.fields[1].text.length > 0 && actual.fields[2].text.length > 0);
    }
    assert.match(detailState.text, /These rows may not include every credit. The count above is separate./);
  }
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).querySelectorAll(".quota-credit-details script,.quota-credit-details img").length;', [selector]), 0, 'Provider titles must remain plain text.');
  assert.equal((await observations()).length, before, 'Credit disclosure must execute no provider.');
  displayed.details = detailState;
  return displayed;
}
async function monthlyDetails(expected, keepFirstOpen = false) {
  const selector = resultSelector(first.id), limits = `${selector} .quota-limit`, sources = `${selector} .quota-source-details`;
  const observationCount = (await observations()).length, displayed = [];
  for (const [index, limit] of expected.entries()) {
    await setDisclosure(limits, true, index);
    await waitDisclosure(sources, false, index); await setDisclosure(sources, true, index);
    const value = await driver.execute('const root=document.querySelectorAll(arguments[0])[arguments[1]],panel=document.getElementById(root.querySelector("[data-slot=collapsible-trigger]").getAttribute("aria-controls")),monthly=panel.querySelector("[data-monthly-credit-limit]"),spend=panel.querySelector("[data-spend-control]");const visible=node=>node.getBoundingClientRect().width>0&&node.getBoundingClientRect().height>0&&getComputedStyle(node).visibility!=="hidden";return {id:monthly.getAttribute("data-monthly-credit-limit"),monthlyLabel:monthly.previousElementSibling.textContent,monthly:monthly.querySelector("bdi").textContent,whiteSpace:getComputedStyle(monthly.querySelector("bdi")).whiteSpace,spendLabel:spend.previousElementSibling.textContent,spend:spend.textContent,visible:visible(monthly)&&visible(spend),actions:panel.querySelectorAll("button,input,select,textarea,form").length};', [sources, index]);
    assert.deepEqual(value, { id: limit.limitId, monthlyLabel: 'Monthly credit limit', monthly: limit.monthlyCreditLimit ?? 'Not supplied', whiteSpace: 'pre', spendLabel: 'Spend control', spend: limit.spendControlReached === null ? 'Not supplied' : limit.spendControlReached ? 'Reached' : 'Not reached', visible: true, actions: 0 });
    displayed.push(value);
    if (!keepFirstOpen || index !== 0) await setDisclosure(sources, false, index);
    if (index !== 0) await setDisclosure(limits, false, index);
  }
  assert.equal((await observations()).length, observationCount, 'Reading details must execute no provider.');
  return displayed;
}
async function approveGui() {
  if (await driver.execute('return !document.querySelector("[data-provider-settings]").closest("details").open;')) await driver.click('[data-provider-settings]');
  if (await driver.execute('const manual=document.querySelector("[data-provider-manual]");return Boolean(manual&&!manual.closest("details").open);')) await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(async () => await driver.execute('return Boolean(document.querySelector("#provider-executable-approved")&&!document.querySelector("#provider-executable-approved").disabled);'), 'reviewed quota fixture');
  assert.deepEqual(await driver.execute('return {tag:document.querySelector("#provider-executable-approved").tagName,role:document.querySelector("#provider-executable-approved").getAttribute("role"),checked:document.querySelector("#provider-executable-approved").getAttribute("aria-checked"),disabled:document.querySelector(".provider-approval-form button[type=submit]").disabled};'), { tag: 'BUTTON', role: 'checkbox', checked: 'false', disabled: true });
  await driver.click('#provider-executable-approved'); await waitFor(async () => await driver.execute('return document.querySelector("#provider-executable-approved").getAttribute("aria-checked")==="true"&&!document.querySelector(".provider-approval-form button[type=submit]").disabled;'), 'explicit visible source checkbox checked'); await driver.click('.provider-approval-form button[type=submit]'); await waitFor(async () => await driver.execute('return document.activeElement?.matches(".provider-approval-result");'), 'explicit fixture approval');
  const check = await driver.invoke('check_profile_connection', { profileId: first.id }); assert.equal(check.ok, true); assert.equal(check.value.sourceKind, 'test_fixture'); approvalId = check.value.approvalId;
}
async function theme(value) { await driver.selectVisibleOption('#theme', { system: 'System', light: 'Light', dark: 'Dark' }[value]); await waitFor(async () => await driver.execute('return document.documentElement.dataset.theme===arguments[0]&&!document.querySelector("#theme").disabled;', [value]), 'saved appearance'); }
async function creditViewport(selector = '[data-earned-reset-credits]') {
  return driver.execute('const node=document.querySelector(arguments[0]),range=document.createRange();range.selectNodeContents(node);const style=getComputedStyle(node),rect=node.getBoundingClientRect();return {text:node.querySelector("bdi").textContent,label:node.getAttribute("aria-label"),tabIndex:node.tabIndex,focused:document.activeElement===node,whiteSpace:style.whiteSpace,overflowX:style.overflowX,outlineWidth:parseFloat(style.outlineWidth),outlineStyle:style.outlineStyle,outlineColor:style.outlineColor,clientWidth:node.clientWidth,scrollWidth:node.scrollWidth,scrollLeft:node.scrollLeft,left:rect.left,right:rect.right,lineCount:new Set([...range.getClientRects()].filter(rect=>rect.width>0&&rect.height>0).map(rect=>Math.round(rect.top))).size,pageWidth:document.scrollingElement.scrollWidth,viewportWidth:innerWidth};', [selector]);
}
async function capture(name, selector, width = 1280, scale = 1, checkCreditScroll = false) {
  const scrollCheck = checkCreditScroll === true ? { selector: '[data-earned-reset-credits]', trigger: '.quota-preview-notes > [data-slot=collapsible-trigger]', label: 'Earned reset credits', exactText: '9223372036854775807', endName: 'quota-credits-exact-end-light-text200-360.png', receiptKey: 'creditCountKeyboardScroll' } : checkCreditScroll;
  for (let attempt = 0; attempt < 3; attempt++) { const actual = await driver.execute('return {width:innerWidth,height:innerHeight}'); if (actual.width === width && actual.height === 850) break; const rect = await driver.request('GET', `/session/${driver.session}/window/rect`); await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - actual.width, height: rect.height + 850 - actual.height }); }
  if (scale !== 1) await driver.execute('window.__quotaFonts=[...document.querySelectorAll("body *")].map(node=>({node,style:node.getAttribute("style"),size:getComputedStyle(node).fontSize,inlineSize:node.style.getPropertyValue("font-size"),inlinePriority:node.style.getPropertyPriority("font-size")}));for(const item of window.__quotaFonts)item.node.style.fontSize=`${parseFloat(item.size)*arguments[0]}px`;return true;', [scale]);
  try {
    await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>done(true))));', args: [] });
    const typography = await driver.execute(`const panel=document.querySelector(arguments[0]);const family=value=>value.replaceAll('"','').replaceAll("'",'');return {status:document.fonts.status,checks:[400,500].map(weight=>({weight,loaded:document.fonts.check(weight+' 14px "JetBrains Mono"','OpenUsage 75%')})),faces:[...document.fonts].filter(face=>family(face.family)==='JetBrains Mono').map(face=>({family:face.family,weight:face.weight,status:face.status})),body:getComputedStyle(document.body).fontFamily,targets:[panel,...panel.querySelectorAll('h3,button,input,textarea,[data-slot=meter-value],.quota-credit-count')].filter(node=>!node.closest('details:not([open]),[data-slot=collapsible-panel][data-closed]')&&node.getBoundingClientRect().width>0&&node.getBoundingClientRect().height>0).map(node=>({tag:node.tagName,family:getComputedStyle(node).fontFamily}))};`, [selector]);
    assert.equal(typography.status, 'loaded'); assert.ok(typography.checks.every(check => check.loaded));
    for (const weight of [400, 500]) assert.ok(typography.faces.some(face => face.weight === String(weight) && face.status === 'loaded'), `JetBrains Mono weight ${weight} must load.`);
    for (const family of [typography.body, ...typography.targets.map(target => target.family)]) assert.equal(family.split(',')[0].replaceAll('"', '').replaceAll("'", '').trim(), 'JetBrains Mono');
    evidence.typography ??= []; evidence.typography.push({ name, ...typography });
    await driver.execute('document.querySelector(arguments[0]).scrollIntoView({block:"center"});return true;', [selector]);
    let initialCredit;
    if (scrollCheck) {
      assert.equal(await driver.execute('return document.activeElement===document.querySelector(arguments[0]);', [scrollCheck.trigger]), true, 'The disclosure trigger must own focus before the real Tab.');
      await key('\uE004');
      initialCredit = await creditViewport(scrollCheck.selector);
      assert.equal(initialCredit.focused, true, 'Tab must reach the count viewport.');
      assert.equal(initialCredit.label, scrollCheck.label); assert.equal(initialCredit.tabIndex, 0);
      assert.equal(initialCredit.text, scrollCheck.exactText); assert.equal(initialCredit.lineCount, 1);
      assert.equal(initialCredit.whiteSpace, 'nowrap'); assert.equal(initialCredit.overflowX, 'auto');
      assert.equal(initialCredit.scrollLeft, 0); assert.ok(initialCredit.scrollWidth > initialCredit.clientWidth);
      assert.ok(initialCredit.left >= 0 && initialCredit.right <= width + 1); assert.ok(initialCredit.pageWidth <= width + 1);
      assert.ok(initialCredit.outlineWidth >= 2); assert.equal(initialCredit.outlineStyle, 'solid'); assert.notEqual(initialCredit.outlineColor, 'rgba(0, 0, 0, 0)');
    }
    const view = await driver.execute('const panel=document.querySelector(arguments[0]),result=panel.closest(".quota-result")??panel;return {theme:document.documentElement.dataset.theme,width:innerWidth,height:innerHeight,scrollWidth:document.scrollingElement.scrollWidth,meterValues:[...result.querySelectorAll("[data-slot=meter-value]")].filter(node=>!node.closest("details:not([open]),[data-slot=collapsible-panel][data-closed]")&&[...node.getClientRects()].some(rect=>rect.width>0&&rect.height>0)).map(node=>{const range=document.createRange();range.selectNodeContents(node);const style=getComputedStyle(node);return {text:node.textContent.trim(),lineCount:new Set([...range.getClientRects()].filter(rect=>rect.width>0&&rect.height>0).map(rect=>Math.round(rect.top))).size,whiteSpace:style.whiteSpace,flexShrink:style.flexShrink};}),overflow:[panel,...panel.querySelectorAll("*")].filter(node=>{const rect=node.getBoundingClientRect();const count=node.parentElement,ownedScrollText=node.matches("bdi")&&count?.matches(".quota-credit-count")&&getComputedStyle(count).overflowX==="auto";return !ownedScrollText&&rect.width>1&&rect.height>1&&(rect.left< -1||rect.right>innerWidth+1)}).map(node=>node.tagName)};', [selector]);
    assert.equal(view.width, width); assert.ok(view.scrollWidth <= width + 1); assert.deepEqual(view.overflow, []); if (scale === 2) { assert.equal(view.meterValues.length, 2); for (const value of view.meterValues) { assert.match(value.text, /^\d{1,3}%$/); assert.equal(value.lineCount, 1, "A percentage must stay on one text line at 200 percent."); assert.equal(value.whiteSpace, "nowrap"); assert.equal(value.flexShrink, "0"); } } await driver.screenshot(join(output, name)); evidence.screenshots.push(name); evidence.views.push({ name, textScale: scale, scalingScope: 'Test-side CSS text size changes, not OS zoom or full accessibility acceptance.', ...view });
    if (scrollCheck) {
      let endCredit = initialCredit, arrowPresses = 0;
      while (endCredit.scrollLeft < endCredit.scrollWidth - endCredit.clientWidth - 1 && arrowPresses < 20) {
        await key('\uE014'); arrowPresses += 1;
        await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];requestAnimationFrame(()=>requestAnimationFrame(()=>done(true)));', args: [] });
        endCredit = await creditViewport(scrollCheck.selector);
      }
      assert.ok(arrowPresses > 0); assert.ok(endCredit.scrollLeft > 0); assert.ok(endCredit.scrollLeft >= endCredit.scrollWidth - endCredit.clientWidth - 1, 'Real ArrowRight must expose the final digits.');
      assert.equal(endCredit.focused, true); assert.equal(endCredit.text, initialCredit.text); assert.equal(endCredit.lineCount, 1); assert.ok(endCredit.left >= 0 && endCredit.right <= width + 1); assert.ok(endCredit.pageWidth <= width + 1);
      const endName = scrollCheck.endName;
      await driver.screenshot(join(output, endName)); evidence.screenshots.push(endName); evidence.views.push({ name: endName, textScale: scale, scalingScope: 'Test-side CSS text size changes, not OS zoom or full accessibility acceptance.', ...view, scrollWidth: endCredit.pageWidth, creditScrollPosition: 'end' });
      evidence[scrollCheck.receiptKey] = { initial: initialCredit, end: endCredit, arrowPresses, actualTabAndArrowRight: true, contentOutsideCountViewportIsExpectedAndNativeScrollable: true };
    }
  } finally { if (scale !== 1) {
    const restoration=await driver.execute('for(const item of window.__quotaFonts){if(item.inlineSize)item.node.style.setProperty("font-size",item.inlineSize,item.inlinePriority);else item.node.style.removeProperty("font-size");if(item.style===null&&item.node.style.length===0)item.node.removeAttribute("style");}const connected=window.__quotaFonts.filter(item=>item.node.isConnected);const mismatches=connected.filter(item=>item.node.style.getPropertyValue("font-size")!==item.inlineSize||item.node.style.getPropertyPriority("font-size")!==item.inlinePriority).map(item=>item.node.tagName);const critical=connected.filter(item=>item.node.matches(".quota-disclosure-trigger,[data-slot=meter-value],.quota-credit-count")).map(item=>({tag:item.node.tagName,before:item.size,after:getComputedStyle(item.node).fontSize}));delete window.__quotaFonts;return{connected:connected.length,inlineMismatches:mismatches,critical};');
    assert.deepEqual(restoration.inlineMismatches,[],'Test scaling must restore every surviving inline font-size property.');
    for(const value of restoration.critical)assert.equal(value.after,value.before,'Quota control/value font sizes must return to their baseline.');
    (evidence.fontRestoration??=[]).push({name,method:'Inverse individual font-size CSSOM property changes under unchanged CSP.',...restoration});
  } }

}
async function desktopPid() {
  const queue = [driver.child.pid]; for (let count = 0; queue.length && count < 64; count++) { const pid = queue.shift(); try { if (await readlink(`/proc/${pid}/exe`) === application) return pid; queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); } catch (error) { if (error.code !== 'ENOENT') throw error; } } assert.fail('Owned native application not found.');
}
async function key(value) { if (!await driver.execute('return document.hasFocus();')) { const pid = await desktopPid(); evidence.nativeWindowFocus = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid), 'focus'], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })); await waitFor(async () => await driver.execute('return document.hasFocus();'), 'native window focus'); } await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'quota-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] }); }
async function closeNormally() { const pid = await desktopPid(); evidence.nativeClose = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })); await waitFor(async () => { try { await readlink(`/proc/${pid}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native window exit'); driver.session = undefined; await driver.stop(); driver = undefined; }
async function button(text) { const found = await driver.execute('return [...document.querySelectorAll("button")].find(node=>node.textContent.trim()===arguments[0]);', [text]); assert.ok(found); await driver.request('POST', `/session/${driver.session}/element/${found['element-6066-11e4-a52e-4f735466cecf']}/click`, {}); }

await mkdir(output, { recursive: true, mode: 0o700 }); process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await hash(application) }; evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim(); evidence.sourceHashes = {};
  const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'apps/desktop/src-tauri/permissions', 'crates/openusage-codex/src', 'crates/openusage-storage/src'], { cwd: repository, encoding: 'utf8' }).trim().split('\n'); for (const path of [...sources, 'package.json', 'package-lock.json', 'Cargo.toml', 'Cargo.lock', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/capabilities/main-window.json', 'crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json', ...['Regular', 'Medium', 'SemiBold'].map(weight => `apps/desktop/public/fonts/JetBrainsMono-${weight}.woff2`), 'apps/desktop/public/fonts/JetBrainsMono-OFL.txt']) evidence.sourceHashes[path] = await hash(join(repository, path));
  evidence.harnessHashes = {}; for (const path of ['quota-read.mjs', 'fixtures/synthetic-quota-provider.py', 'private-fixture.mjs', 'webdriver.mjs', 'x11-close.py']) evidence.harnessHashes[path] = await hash(join(harness, path));
  fixture = await createFixture('quota-read'); fixture.environment.OPENAI_API_KEY = 'synthetic-parent-key-must-not-reach-child'; fixture.environment.OPENAI_ACCESS_TOKEN = 'synthetic-parent-token-must-not-reach-child'; workspace = dirname(fixture.database); executable = join(fixture.root, 'synthetic-quota-provider.py'); executionLog = join(fixture.root, 'synthetic-quota-observations.jsonl'); await copyFile(join(harness, 'fixtures/synthetic-quota-provider.py'), executable); await chmod(executable, 0o700); assert.equal(await hash(executable), fixtureDigest);
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log')); await driver.start(); await waitFor(async () => await driver.execute('return ["#workspace-passphrase","#workspace-confirmation"].every(selector=>{const field=document.querySelector(selector);return Boolean(field&&!field.disabled);});'), 'enabled setup fields'); for (const command of ['begin_profile_quota_read', 'complete_profile_quota_read', 'cancel_profile_quota_read']) await rejected(command, [], 'invalid_arguments'); await rejected('begin_profile_quota_read', { profileId: randomUUID(), approvalId: randomUUID() }, 'workspace_locked'); await assert.rejects(lstat(fixture.database), { code: 'ENOENT' }); await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password); await driver.click('.unlock-form button[type=submit]'); await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'fresh SQLCipher workspace');
  first = await create('Quota review profile'); other = await create('Other synthetic profile'); archived = await create('Archived synthetic profile'); await driver.openProfileOptions(archived.id); await driver.click(`[data-profile-id="${archived.id}"] [data-profile-action="archive"]`); await waitFor(async () => (await profiles()).find(profile => profile.id === archived.id)?.archived, 'archived fixture profile');
  home = join(workspace, 'profiles', first.id); const otherHome = join(workspace, 'profiles', other.id), archivedHome = join(workspace, 'profiles', archived.id);
  for (const path of [home, otherHome, archivedHome]) await writeFile(join(path, 'synthetic-preserved-marker.txt'), 'Owned synthetic profile marker. No credentials.\n', { mode: 0o600 }); const originalHome = await files(home), otherBefore = await files(otherHome), archivedBefore = await files(archivedHome), keyHash = await hash(join(workspace, 'database-key.v1'));
  await rejected('begin_profile_quota_read', { profileId: first.id, approvalId: randomUUID() }, 'provider_executable_not_approved'); assert.deepEqual(await observations(), []); await approveGui();
  for (const args of [{ profileId: first.id }, { profileId: first.id, approvalId, unexpected: true }, { profileId: first.id, approvalId: true }]) await rejected('begin_profile_quota_read', args, 'invalid_arguments');
  await rejected('begin_profile_quota_read', { profileId: archived.id, approvalId }, 'provider_profile_archived');
  const oldApproval = approvalId; await approveGui(); const noExecution = (await observations()).length; await rejected('begin_profile_quota_read', { profileId: first.id, approvalId: oldApproval }, 'stale_provider_approval'); assert.equal((await observations()).length, noExecution);
  await button('Create a backup'); await driver.type('#backup-password', backupPassword); await driver.type('#backup-confirmation', backupPassword); await driver.click('#backup-records'); await driver.click('.backup-form button[type=submit]'); await waitFor(async () => await driver.execute('return [...document.querySelectorAll(".backup-receipt")].some(node=>node.textContent.includes("Backup saved and authenticated."));'), 'authenticated backup publication'); const archives = (await readdir(join(fixture.dirs.downloads, 'OpenUsage'))).filter(name => name.endsWith('.oubackup')); assert.equal(archives.length, 1);
  await theme('dark'); let before = await liveHashes(); const guiText = await guiRead(); await previewDisclosure(); evidence.defaultCreditDisplay = await creditDetails('3'); await capture('quota-earned-credits-dark-desktop.png', `${resultSelector(first.id)} .quota-preview-notes`); await setDisclosure(`${resultSelector(first.id)} .quota-preview-notes`, false); assert.equal(guiText.includes('legacy-must-not-be-counted'), false); assert.deepEqual(await liveHashes(), before);
  assert.deepEqual(await driver.execute('const panel=document.querySelector(arguments[0]);return {fixture:Boolean(panel.querySelector("[data-provider-fixture]")),limits:panel.querySelectorAll(".quota-limit").length,meters:[...panel.querySelectorAll("[role=meter]")].map(node=>({value:node.getAttribute("aria-valuenow"),label:node.getAttribute("aria-label")}))};', [resultSelector(first.id)]), { fixture: true, limits: 2, meters: [{ value: '75', label: 'Five-hour: 75% remaining (derived)' }, { value: '0', label: 'Seven-day: 0% remaining (derived)' }] });
  await setDisclosure(`${resultSelector(first.id)} .quota-limit`,true,1);
  assert.deepEqual(await driver.execute('return [...document.querySelector(arguments[0]).querySelectorAll("[role=meter]")].map(node=>({value:node.getAttribute("aria-valuenow"),label:node.getAttribute("aria-label")}));',[resultSelector(first.id)]),[{value:'75',label:'Five-hour: 75% remaining (derived)'},{value:'0',label:'Seven-day: 0% remaining (derived)'},{value:'100',label:'Primary window: 100% remaining (derived)'}]);
  const geometry = await driver.execute('return [...document.querySelector(arguments[0]).querySelectorAll("[role=meter]")].map(node=>{const track=node.querySelector("[data-slot=meter-track]"),indicator=node.querySelector("[data-slot=meter-indicator]");return {value:node.getAttribute("aria-valuenow"),min:node.getAttribute("aria-valuemin"),max:node.getAttribute("aria-valuemax"),text:node.getAttribute("aria-valuetext"),trackWidth:track.getBoundingClientRect().width,indicatorWidth:indicator.getBoundingClientRect().width,trackBackground:getComputedStyle(track).backgroundImage,indicatorBackground:getComputedStyle(indicator).backgroundImage};});', [resultSelector(first.id)]);
  assert.equal(geometry.length, 3); for (const [index, value] of geometry.entries()) { const remaining = [75, 0, 100][index]; assert.equal(value.value, String(remaining)); assert.equal(value.min, '0'); assert.equal(value.max, '100'); assert.equal(value.text, `${remaining}% remaining (derived)`); assert.ok(value.trackWidth > 0); assert.ok(Math.abs(value.indicatorWidth - value.trackWidth * remaining / 100) <= 1, 'The visible coss indicator must match derived remaining capacity.'); assert.equal(value.trackBackground, 'none'); assert.equal(value.indicatorBackground, 'none'); } evidence.meterGeometry = geometry;
  await setDisclosure(`${resultSelector(first.id)} .quota-limit`,false,1);
  await capture('quota-preview-dark-desktop.png', resultSelector(first.id)); await theme('light'); await capture('quota-preview-light-360.png', resultSelector(first.id), 360); await capture('quota-secondary-light-360.png', `${resultSelector(first.id)} .quota-window:nth-of-type(2)`, 360); await capture('quota-primary-light-text200-360.png', `${resultSelector(first.id)} .quota-window`, 360, 2);
  await config(home, 'multi'); before = await liveHashes(); const request = await begin(), full = await complete(request); assert.equal(full.availableResetCredits, '3'); assert.equal(full.resetCreditDetails, null); assert.equal(full.limits.length, 2); assert.deepEqual(full.limits[0], { limitId: 'codex', limitName: null, monthlyCreditLimit: null, spendControlReached: null, primary: { usedPercent: 25, derivedRemainingPercent: 75, windowDurationMins: 300, resetsAtUnix: 1893456000, label: 'Five-hour' }, secondary: { usedPercent: 100, derivedRemainingPercent: 0, windowDurationMins: 10080, resetsAtUnix: null, label: 'Seven-day' }, reachedType: 'rate_limit_reached' }); assert.deepEqual(full.limits[1], { limitId: 'codex_other', limitName: 'Synthetic returned bucket', monthlyCreditLimit: null, spendControlReached: null, primary: { usedPercent: 0, derivedRemainingPercent: 100, windowDurationMins: null, resetsAtUnix: null, label: 'Primary window' }, secondary: null, reachedType: null }); assert.deepEqual(await liveHashes(), before); evidence.sampleDiagnostic = full;
  for (const mode of ['legacy', 'empty', 'null_windows', 'sparse_event']) { await config(home, mode); before = await liveHashes(); const attempt = await begin(), value = await complete(attempt); if (mode === 'empty') assert.deepEqual(value.limits, []); if (mode === 'legacy') { assert.equal(value.limits.length, 1); assert.equal(value.limits[0].limitId, 'returned-legacy'); assert.equal(value.limits[0].primary.usedPercent, 42); assert.equal(value.limits[0].primary.label, '1440 minutes'); } if (mode === 'null_windows') { assert.equal(value.limits[0].primary, null); assert.equal(value.limits[0].secondary, null); } if (mode === 'sparse_event') assert.deepEqual(value.limits, full.limits); assert.deepEqual(await liveHashes(), before); evidence.cases.push({ mode, passed: true }); }
  for (const [mode, error] of [['missing', 'provider_quota_failed'], ['invalid', 'provider_quota_failed'], ['fractional', 'provider_quota_failed'], ['string_percent', 'provider_quota_failed'], ['mismatch', 'provider_incompatible_quota_scope'], ['unknown_identity', 'provider_account_identity_unavailable'], ['signed_out', 'provider_quota_failed'], ['unsupported', 'provider_quota_failed'], ['changed_email', 'provider_account_changed'], ['changed_plan', 'provider_account_changed'], ['changed_auth', 'provider_account_changed'], ['post_unknown', 'provider_account_changed'], ['early_auth', 'provider_account_changed'], ['auth_notice', 'provider_account_changed'], ['late_auth', 'provider_account_changed']]) { await config(home, mode); before = await liveHashes(); const attempt = await begin(); await complete(attempt, error); assert.deepEqual(await liveHashes(), before); evidence.cases.push({ mode, error, passed: true }); }
  for (const [mode, expected] of [['credits_absent', null], ['credits_null', null], ['credits_zero', '0'], ['credits_nonzero', '3'], ['credits_null_details', '3'], ['credits_empty_details', '7'], ['credits_above_safe', '9007199254740993'], ['credits_max', '9223372036854775807']]) {
    await config(home, mode); before = await liveHashes();
    const attempt = await begin(), value = await complete(attempt);
    assert.equal(value.availableResetCredits, expected);
    assert.deepEqual(value.limits, full.limits);
    const expectedRows = mode === 'credits_empty_details' ? [] : null; assert.deepEqual(value.resetCreditDetails, expectedRows);
    await guiRead(mode);
    const display = await creditDetails(expected, expectedRows);
    if (mode === 'credits_null') await capture('quota-credits-unavailable-light-360.png', `${resultSelector(first.id)} .quota-preview-notes`, 360);
    if (mode === 'credits_max') await capture('quota-credits-exact-light-text200-360.png', `${resultSelector(first.id)} .quota-preview-notes`, 360, 2, true);
    await setDisclosure(`${resultSelector(first.id)} .quota-preview-notes`, false);
    assert.deepEqual(await liveHashes(), before);
    evidence.cases.push({ mode, availableResetCredits: expected, displayed: display, guiAndRustTypedCount: true, passed: true });
  }
  for (const mode of ['credits_negative', 'credits_fractional', 'credits_out_of_range', 'credits_missing_count', 'credits_null_count']) {
    await config(home, mode); before = await liveHashes();
    await complete(await begin(), 'provider_quota_failed');
    assert.deepEqual(await liveHashes(), before);
    evidence.cases.push({ mode, error: 'provider_quota_failed', passed: true });
  }
  const orderedCreditRows = [
    { title: 'Synthetic available credit', status: 'available', grantedAtUnix: 0, expiresAtUnix: null },
    { title: 'Synthetic redeeming credit', status: 'redeeming', grantedAtUnix: -62135596800, expiresAtUnix: 253402300799 },
    { title: null, status: 'redeemed', grantedAtUnix: 0, expiresAtUnix: 0 },
    { title: '<Synthetic future credit>', status: 'unknown', grantedAtUnix: 1893456000, expiresAtUnix: null },
  ];
  for (const mode of ['details_rows', 'details_bound']) {
    const expectedRows = mode === 'details_rows' ? orderedCreditRows : Array.from({ length: 32 }, (_, index) => ({ title: `Synthetic returned credit ${String(index + 1).padStart(2, '0')}`, status: 'available', grantedAtUnix: 1704067200, expiresAtUnix: null }));
    await config(home, mode); before = await liveHashes(); const request = await begin(), value = await complete(request);
    assert.equal(value.availableResetCredits, '9'); assert.deepEqual(value.resetCreditDetails, expectedRows); assert.deepEqual(await liveHashes(), before);
    await guiRead(mode); const displayed = await creditDetails('9', expectedRows);
    assert.deepEqual(await liveHashes(), before);
    if (mode === 'details_rows') {
      await theme('dark'); const darkAppearance = await liveHashes(); await capture('quota-credit-rows-dark-desktop.png', `${resultSelector(first.id)} .quota-credit-details`); assert.deepEqual(await liveHashes(), darkAppearance);
      await theme('light'); const lightAppearance = await liveHashes(); await capture('quota-credit-rows-light-360.png', `${resultSelector(first.id)} [data-credit-detail]`, 360);
      await capture('quota-credit-rows-light-text200-360.png', `${resultSelector(first.id)} [data-credit-detail]:nth-child(2)`, 360, 2); assert.deepEqual(await liveHashes(), lightAppearance);
      before = await liveHashes();
    }
    await setDisclosure(`${resultSelector(first.id)} .quota-preview-notes`, false); assert.deepEqual(await liveHashes(), before);
    evidence.cases.push({ mode, availableResetCredits: '9', rowCount: expectedRows.length, displayed, providerOrderAndCountIndependence: true, passed: true });
  }
  for (const mode of ['details_too_many', 'details_missing_expiry', 'details_invalid_time', 'details_invalid_title']) {
    await config(home, mode); before = await liveHashes(); const request = await begin(); await complete(request, 'provider_quota_failed');
    assert.deepEqual(await liveHashes(), before); evidence.cases.push({ mode, error: 'provider_quota_failed', passed: true });
  }
  const monthlyCases = [
    { mode: 'monthly_exact', expected: [['codex', '9007199254740993.12500', true], ['codex_other', null, null]] },
    { mode: 'monthly_zero', expected: [['codex', '0', false], ['codex_other', null, null]] },
    { mode: 'spend_without_monthly', expected: [['codex', null, true], ['codex_other', null, null]] },
    { mode: 'monthly_without_spend', expected: [['codex', '  37.5000  ', null], ['codex_other', null, null]] },
    { mode: 'monthly_fullmap', expected: [['codex', '12.50', false], ['codex_other', '5.125', true]] },
    { mode: 'monthly_legacy', expected: [['returned-legacy', '25.50', false]] },
  ];
  for (const { mode, expected } of monthlyCases) {
    await config(home, mode); before = await liveHashes(); const value = await complete(await begin());
    const returned = value.limits.map(({ limitId, monthlyCreditLimit, spendControlReached }) => ({ limitId, monthlyCreditLimit, spendControlReached }));
    assert.deepEqual(returned, expected.map(([limitId, monthlyCreditLimit, spendControlReached]) => ({ limitId, monthlyCreditLimit, spendControlReached })));
    assert.equal(value.availableResetCredits, '3'); assert.equal(value.resetCreditDetails, null); assert.deepEqual(await liveHashes(), before);
    await guiRead(mode); const displayed = await monthlyDetails(returned, mode === 'monthly_exact'); assert.deepEqual(await liveHashes(), before);
    if (mode === 'monthly_exact') {
      const source = `${resultSelector(first.id)} .quota-source-details`;
      await theme('dark'); const darkAppearance = await liveHashes(); await capture('quota-monthly-credit-dark-desktop.png', source); assert.deepEqual(await liveHashes(), darkAppearance);
      await theme('light'); const lightAppearance = await liveHashes(); await capture('quota-monthly-credit-light-360.png', source, 360);
      await setDisclosure(source, false); await setDisclosure(source, true);
      await capture('quota-monthly-credit-exact-light-text200-360.png', source, 360, 2, { selector: '[data-monthly-credit-limit="codex"]', trigger: '.quota-source-details > [data-slot=collapsible-trigger]', label: 'Monthly credit limit for codex', exactText: '9007199254740993.12500', endName: 'quota-monthly-credit-exact-end-light-text200-360.png', receiptKey: 'monthlyCreditKeyboardScroll' });
      assert.deepEqual(await liveHashes(), lightAppearance); before = await liveHashes(); await setDisclosure(source, false);
    }
    assert.deepEqual(await liveHashes(), before);
    evidence.cases.push({ mode, limits: returned, displayed, exactStringsAndIndependentNullableState: true, passed: true });
  }
  for (const mode of ['monthly_limit_number', 'monthly_limit_control', 'monthly_spend_string']) {
    await config(home, mode); before = await liveHashes(); await complete(await begin(), 'provider_quota_failed');
    assert.deepEqual(await liveHashes(), before); evidence.cases.push({ mode, error: 'provider_quota_failed', passed: true });
  }
  assert.match(await guiRead('empty'), /Missing readings do not mean zero usage/); assert.equal(await driver.execute('return document.querySelector(arguments[0]).querySelectorAll("[role=meter]").length;', [resultSelector(first.id)]), 0); await capture('quota-empty-light-360.png', resultSelector(first.id), 360);
  assert.match(await guiRead('null_windows'), /No quota windows were supplied/); assert.equal(await driver.execute('return document.querySelector(arguments[0]).querySelectorAll("[role=meter]").length;', [resultSelector(first.id)]), 0); await capture('quota-missing-windows-light-360.png', resultSelector(first.id), 360);
  await config(home, 'pending'); let offset = (await observations()).length; await driver.click(readSelector(first.id)); const guiPending = await stage('quota-read-pending', home, offset); await waitFor(async () => await driver.execute('return document.activeElement?.matches(arguments[0])&&!document.activeElement.disabled;', [cancelSelector(first.id)]), 'focused enabled Cancel read'); await key('\uE007'); await waitFor(async () => await driver.execute('return document.activeElement?.matches(arguments[0])&&document.activeElement.textContent.includes("Quota read stopped.");', [resultSelector(first.id)]), 'cancelled result focus'); await reaped(guiPending); await capture('quota-cancelled-light-360.png', resultSelector(first.id), 360);
  await config(home, 'sparse_only'); offset = (await observations()).length; const stale = await begin(); await stage('notification-emitted', home, offset); const sparseChild = await stage('quota-read-pending', home, offset); await rejected('begin_profile_quota_read', { profileId: first.id, approvalId }, 'provider_quota_in_progress'); await rejected('check_profile_connection', { profileId: first.id }, 'provider_quota_in_progress'); await rejected('cancel_profile_check', { profileId: first.id }, 'provider_quota_in_progress'); assert.deepEqual(await driver.invoke('cancel_profile_quota_read', { profileId: other.id, requestId: stale.requestId }), { ok: true, value: { cancelled: false } }); assert.deepEqual(await driver.invoke('cancel_profile_quota_read', { profileId: first.id, requestId: stale.requestId }), { ok: true, value: { cancelled: true } }); active.delete(stale.requestId); await reaped(sparseChild); await rejected('complete_profile_quota_read', { profileId: first.id, requestId: stale.requestId }, 'provider_quota_not_found');
  await config(home, 'pending'); offset = (await observations()).length; const successor = await begin(), successorChild = await stage('quota-read-pending', home, offset); assert.deepEqual(await driver.invoke('cancel_profile_quota_read', { profileId: first.id, requestId: stale.requestId }), { ok: true, value: { cancelled: false } }); process.kill(successorChild.pid, 0);
  const beforeHomes = await readdir(join(workspace, 'profiles')); const preview = await driver.invoke('prepare_restore_preview', { fileName: archives[0], password: backupPassword }); assert.equal(preview.ok, true); assert.equal((await driver.invoke('apply_restore', { previewId: preview.value.previewId, confirmed: true })).ok, true); await reaped(successorChild); active.delete(successor.requestId); await rejected('complete_profile_quota_read', { profileId: first.id, requestId: successor.requestId }, 'provider_quota_not_found'); assert.deepEqual(await files(home), originalHome); assert.deepEqual(await files(otherHome), otherBefore); assert.deepEqual(await files(archivedHome), archivedBefore); assert.equal(await hash(join(workspace, 'database-key.v1')), keyHash);
  const newHomes = (await readdir(join(workspace, 'profiles'))).filter(name => !beforeHomes.includes(name)); assert.equal(newHomes.length, 3); offset = (await observations()).length; const remapped = await begin(); await complete(remapped); const newStart = (await observations()).slice(offset).find(row => row.kind === 'start'); freshHome = newStart.environment.CODEX_HOME; assert.ok(newHomes.includes(freshHome.split('/').at(-1))); assert.notEqual(freshHome, home);
  await config(freshHome, 'pending'); offset = (await observations()).length; const exiting = await begin(), exitChild = await stage('quota-read-pending', freshHome, offset); await closeNormally(); active.delete(exiting.requestId); await reaped(exitChild);
  const rows = await observations(); const starts = new Map(rows.filter(row => row.kind === 'start').map(row => [row.pid, row]));
  for (const row of starts.values()) { assert.ok([home, freshHome].includes(row.environment.CODEX_HOME)); assert.deepEqual(row.unexpectedEnvironmentKeys, []); assert.equal(row.environment.PATH, '/usr/bin:/bin'); assert.equal(row.environment.LANG, 'C.UTF-8'); const runtime = dirname(row.environment.HOME); assert.ok(runtime.startsWith(row.environment.CODEX_HOME + '/openusage-provider-')); for (const key of ['XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_CACHE_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR', 'TMPDIR']) assert.ok(row.environment[key].startsWith(runtime + '/')); await reaped(row); await assert.rejects(lstat(runtime), { code: 'ENOENT' }); }
  for (const row of rows.filter(row => row.kind === 'request')) { assert.ok(['initialize', 'initialized', 'account/read', 'account/rateLimits/read'].includes(row.request.method)); if (row.request.method === 'account/read') assert.deepEqual(row.request.params, { refreshToken: false }); if (row.request.method === 'account/rateLimits/read') assert.deepEqual(row.request, { id: 3, method: 'account/rateLimits/read', params: {} }); }
  for (const row of rows.filter(row => row.kind === 'full-quota-emitted' && (['multi', 'legacy', 'empty', 'null_windows', 'sparse_event', 'details_rows', 'details_bound', 'monthly_exact', 'monthly_zero', 'spend_without_monthly', 'monthly_without_spend', 'monthly_fullmap', 'monthly_legacy'].includes(row.mode) || row.mode.startsWith('credits_') && !['credits_negative', 'credits_fractional', 'credits_out_of_range', 'credits_missing_count', 'credits_null_count'].includes(row.mode)))) { const requests = rows.filter(item => item.kind === 'request' && item.pid === row.pid).map(item => ({ method: item.request.method, id: item.request.id ?? null })); assert.deepEqual(requests, [{ method: 'initialize', id: 1 }, { method: 'initialized', id: null }, { method: 'account/read', id: 2 }, { method: 'account/rateLimits/read', id: 3 }, { method: 'account/read', id: 4 }]); }
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']); evidence.observations = rows; evidence.steps.push({ explicitGuiSourceApproval: true, fullReadBracketedByFreshAccountReads: true, exactIntegerDerivedValuesAndNulls: true, exactNullableResetCreditCount: true, nullOrEmptyCreditDetailsNeverReplaceCount: true, noCreditSpendingControl: true, typedOrderedCreditDetails: true, explicitNoExpiryWithoutTimeInference: true, resourceBoundIsNotProviderMaximumOrCompleteness: true, privateCreditFieldsExcluded: true, exactRawMonthlyCreditStrings: true, independentNullableSpendControl: true, balanceAndUsagePermissionNeverSubstitute: true, missingIsNotZero: true, sparseEventsNotMerged: true, allReadingsProvisionalAndUnaccepted: true, noApplicationWritesFromQuotaReads: true, nativeKeyboardCancelReaps: true, staleCancellationCannotAffectSuccessor: true, profileOnlyCheckCancellationCannotInterruptQuota: true, restoreQuiescesBeforeFreshHomeRemap: true, genuineWindowExitReaps: true, otherAndArchivedHomesPreserved: true, isolatedEnvironmentAndDocumentedOperationsOnly: true });
  assert.equal(await hash(executable), fixtureDigest); assert.equal(await hash(application), evidence.application.sha256); for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest); for (const [path, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, path)), digest); evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; try { evidence.observations = await observations(); } catch {} if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {} }
finally {
  if (evidence.result !== 'passed' && driver?.session) for (const [requestId, profileId] of active) try { await driver.invoke('cancel_profile_quota_read', { profileId, requestId }); } catch {}
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) { if (evidence.result !== 'passed') { evidence.failedFixtureCleanup = []; for (const row of evidence.observations ?? []) if (row.kind === 'start') try { const command = await readFile(`/proc/${row.pid}/cmdline`); if (command.includes(Buffer.from(executable + '\0')) && row.pid === row.processGroup) { process.kill(-row.processGroup, 'SIGTERM'); evidence.failedFixtureCleanup.push({ pid: row.pid, processGroup: row.processGroup, scope: 'Confirmed owned synthetic fixture cleanup after failed acceptance only.' }); } } catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) throw error; } } stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); }
  evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); if (evidence.error) console.error(evidence.error);
}
