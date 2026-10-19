import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import { DesktopUnavailable, invoke } from '../../apps/desktop/src/contracts.ts';

// Synthetic IPC responses. These tests do not exercise a native Tauri application.
const ready = {
  storage: 'ready', theme: 'system', schemaVersion: 1, activeProfiles: 0,
  monitoring: 'stopped', provider: 'not_configured',
  keyProtection: 'os_store', storageIssue: null,
};

function desktopResponse(value) {
  globalThis.window = { __TAURI__: { core: { invoke: async () => value } } };
}

afterEach(() => { delete globalThis.window; });

test('accepts valid native bootstrap states without changing missing values', async () => {
  for (const storage of ['setup_required', 'locked', 'ready', 'recovery_required']) {
    for (const theme of ['system', 'light', 'dark']) {
      const value = { ...ready, storage, theme, ...(storage === 'ready' ? {} : { schemaVersion: null, activeProfiles: null }) };
      desktopResponse(value);
      assert.deepEqual(await invoke('bootstrap'), value);
    }
  }
});

test('rejects array enums instead of treating them as native state strings', async () => {
  for (const value of [
    { ...ready, storage: ['ready'], schemaVersion: null, activeProfiles: null },
    { ...ready, theme: ['dark'] },
    { ...ready, keyProtection: ['os_store'] },
    { ...ready, storageIssue: ['missing_key'] },
  ]) {
    desktopResponse(value);
    await assert.rejects(invoke('bootstrap'), /desktop response is incompatible/);
  }
});

test('rejects incomplete ready state and invalid numeric metadata', async () => {
  for (const fields of [
    { schemaVersion: null }, { activeProfiles: null }, { schemaVersion: 0 },
    { activeProfiles: -1 }, { activeProfiles: 11 }, { activeProfiles: 0.5 },
    { activeProfiles: '0' }, { schemaVersion: Number.MAX_SAFE_INTEGER + 1 },
    { keyProtection: undefined }, { storageIssue: undefined },
  ]) {
    desktopResponse({ ...ready, ...fields });
    await assert.rejects(invoke('bootstrap'), /desktop response is incompatible/);
  }
});

test('reports a missing desktop bridge without fabricating local state', async () => {
  globalThis.window = {};
  await assert.rejects(invoke('bootstrap'), DesktopUnavailable);
});

test('accepts bounded backup locations and rejects incompatible native receipts', async () => {
  const receipt = { fileName: 'openusage-backup-synthetic.oubackup', directory: '/synthetic/Downloads/OpenUsage' };
  desktopResponse(receipt);
  assert.deepEqual(await invoke('create_backup', { password: 'synthetic backup password' }), receipt);
  for (const value of [ready, { ...receipt, fileName: '../records.sqlite' }, { ...receipt, directory: '' }, { ...receipt, directory: '/invalid\0path' }]) {
    desktopResponse(value);
    await assert.rejects(invoke('create_backup', { password: 'synthetic backup password' }), /desktop response is incompatible/);
  }
});

test('accepts profile records with Unicode labels and counts only active profiles', async () => {
  const profile = {
    id: '00000001-0000-4000-8000-000000000000', label: '😀'.repeat(100),
    environment: 'native', connectionState: 'authentication_required', archived: false,
  };
  const value = { profiles: [profile, { ...profile, id: '00000002-0000-4000-8000-000000000000', archived: true }], activeProfiles: 1 };
  desktopResponse(value);
  assert.deepEqual(await invoke('list_profiles'), value);
});

test('rejects incompatible profile metadata, duplicate IDs, leaked paths, and false counts', async () => {
  const profile = {
    id: '00000001-0000-4000-8000-000000000000', label: 'Synthetic profile',
    environment: 'native', connectionState: 'authentication_required', archived: false,
  };
  for (const value of [
    { profiles: [profile], activeProfiles: 0 },
    { profiles: [profile, profile], activeProfiles: 2 },
    { profiles: [{ ...profile, id: '../other-home' }], activeProfiles: 1 },
    { profiles: [{ ...profile, home: '/private/home' }], activeProfiles: 1 },
    { profiles: [{ ...profile, connectionState: ['connected'] }], activeProfiles: 1 },
    { profiles: [{ ...profile, label: 'é'.repeat(101) }], activeProfiles: 1 },
    { profiles: [{ ...profile, label: 'name\u0000hidden' }], activeProfiles: 1 },
    { profiles: [profile], activeProfiles: 11 },
  ]) {
    desktopResponse(value);
    await assert.rejects(invoke('list_profiles'), /desktop response is incompatible/);
  }
});

test('accepts bounded backup candidates and rejects duplicate or path-based selections', async () => {
  const file = { fileName: 'openusage-backup-0123456789abcdef.oubackup', fileBytes: '28785' };
  desktopResponse({ backups: [file], truncated: false });
  assert.deepEqual(await invoke('list_backups'), { backups: [file], truncated: false });
  for (const value of [
    { backups: [file, file], truncated: false },
    { backups: [{ ...file, fileName: '../openusage.db' }], truncated: false },
    { backups: [{ ...file, fileBytes: 28785 }], truncated: false },
    { backups: [file], truncated: 'false' },
  ]) {
    desktopResponse(value);
    await assert.rejects(invoke('list_backups'), /desktop response is incompatible/);
  }
});

test('inspection metadata keeps unsupported database contents distinct from valid records', async () => {
  const metadata = {
    fileName: 'openusage-backup-0123456789abcdef.oubackup', archiveVersion: 1,
    manifestSchemaVersion: 77, entryCount: 1, totalBytes: '51',
    entries: [{ name: 'records.sqlite', bytes: '51' }],
    databaseValidation: 'unsupported', contentSummary: null, restoreAvailable: false,
  };
  desktopResponse(metadata);
  assert.deepEqual(await invoke('inspect_backup', { fileName: metadata.fileName, password: 'x' }), metadata);
  for (const changes of [
    { restoreAvailable: true }, { databaseValidation: 'verified' }, { archiveVersion: 2 },
    { databaseValidation: 'valid' }, { contentSummary: { activeProfiles: 0, archivedProfiles: '0', accounts: '0' } },
    { entryCount: 2 }, { totalBytes: 51 }, { entries: [] },
    { fileName: '/private/archive.oubackup' }, { path: '/private/archive.oubackup' },
  ]) {
    desktopResponse({ ...metadata, ...changes });
    await assert.rejects(invoke('inspect_backup', { fileName: metadata.fileName, password: 'x' }), /desktop response is incompatible/);
  }
});

test('validated content counts stay exact and cannot enable restore', async () => {
  const metadata = {
    fileName: 'openusage-backup-0123456789abcdef.oubackup', archiveVersion: 1,
    manifestSchemaVersion: 1, entryCount: 1, totalBytes: '28672',
    entries: [{ name: 'records.sqlite', bytes: '28672' }],
    databaseValidation: 'valid', contentSummary: { activeProfiles: 2, archivedProfiles: '1', accounts: '0' },
    restoreAvailable: false,
  };
  desktopResponse(metadata);
  assert.deepEqual(await invoke('inspect_backup', { fileName: metadata.fileName, password: 'x' }), metadata);
  for (const contentSummary of [null, [], { activeProfiles: 11, archivedProfiles: '1', accounts: '0' },
    { activeProfiles: 2, archivedProfiles: 1, accounts: '0' },
    { activeProfiles: 2, archivedProfiles: '01', accounts: '0' },
    { activeProfiles: 2, archivedProfiles: '1', accounts: '0', home: '/private' }]) {
    desktopResponse({ ...metadata, contentSummary });
    await assert.rejects(invoke('inspect_backup', { fileName: metadata.fileName, password: 'x' }), /desktop response is incompatible/);
  }
  desktopResponse({ ...metadata, databaseValidation: 'invalid', contentSummary: null });
  assert.equal((await invoke('inspect_backup', { fileName: metadata.fileName, password: 'x' })).databaseValidation, 'invalid');
});

test('restore previews require safe opaque IDs and exact counts before confirmation', async () => {
  const preview = {
    previewId: '00000001-0000-4000-8000-000000000000',
    fileName: 'openusage-backup-0123456789abcdef.oubackup',
    current: { activeProfiles: 3, archivedProfiles: '1', accounts: '0' },
    incoming: { activeProfiles: 2, archivedProfiles: '9007199254740993', accounts: '0' },
    checkpoint: { id: '00000002-0000-4000-8000-000000000000', fileBytes: '12000000000' },
    restoreAvailable: true,
  };
  const args = { fileName: preview.fileName, password: 'x' };
  desktopResponse(preview);
  assert.deepEqual(await invoke('prepare_restore_preview', args), preview);
  for (const changes of [
    { previewId: '../outside' }, { restoreAvailable: false }, { fileName: '/private/backup' },
    { current: { ...preview.current, activeProfiles: 11 } },
    { incoming: { ...preview.incoming, archivedProfiles: 9007199254740992 } },
    { checkpoint: { ...preview.checkpoint, fileBytes: '0' } },
    { checkpoint: { ...preview.checkpoint, path: '/private/checkpoint' } },
    { password: 'must not cross the result boundary' },
  ]) {
    desktopResponse({ ...preview, ...changes });
    await assert.rejects(invoke('prepare_restore_preview', args), /desktop response is incompatible/);
  }
});

test('restore receipts require exact counts, stopped monitoring, and authentication', async () => {
  const receipt = {
    applied: true,
    restored: { activeProfiles: 1, archivedProfiles: '9007199254740993', accounts: '0' },
    checkpoint: { id: '00000002-0000-4000-8000-000000000000', fileBytes: '12000000000' },
    monitoring: 'stopped', authenticationRequired: true,
  };
  const args = { previewId: '00000001-0000-4000-8000-000000000000', confirmed: true };
  desktopResponse(receipt);
  assert.deepEqual(await invoke('apply_restore', args), receipt);
  for (const changes of [
    { applied: false }, { monitoring: 'running' }, { authenticationRequired: false },
    { restored: { ...receipt.restored, activeProfiles: 11 } },
    { restored: { ...receipt.restored, accounts: 9007199254740992 } },
    { checkpoint: { ...receipt.checkpoint, fileBytes: '0' } },
    { checkpoint: { ...receipt.checkpoint, path: '/private/checkpoint' } },
    { key: 'must not cross the result boundary' },
  ]) {
    desktopResponse({ ...receipt, ...changes });
    await assert.rejects(invoke('apply_restore', args), /desktop response is incompatible/);
  }
});

test('preview cancellation accepts already-closed state and rejects incompatible replies', async () => {
  const args = { previewId: '00000001-0000-4000-8000-000000000000' };
  for (const cancelled of [true, false]) {
    desktopResponse({ cancelled });
    assert.deepEqual(await invoke('cancel_restore_preview', args), { cancelled });
  }
  for (const value of [{}, { cancelled: 'false' }, { cancelled: true, path: '/private' }, []]) {
    desktopResponse(value);
    await assert.rejects(invoke('cancel_restore_preview', args), /desktop response is incompatible/);
  }
});

test('checkpoint catalogs reject unsafe IDs, false dates, and unexpected metadata', async () => {
  const item = { id: '00000001-0000-4000-8000-000000000000', fileBytes: '12000000000', modifiedAt: '2026-10-18T04:00:00.123Z' };
  for (const modifiedAt of [item.modifiedAt, null]) {
    const list = { checkpoints: [{ ...item, modifiedAt }], truncated: false };
    desktopResponse(list);
    assert.deepEqual(await invoke('list_restore_checkpoints'), list);
  }
  for (const changes of [{ id: '../outside' }, { fileBytes: 123 }, { fileBytes: '0' },
    { modifiedAt: '2026-02-30T04:00:00.123Z' }, { modifiedAt: '2026-10-18' },
    { modifiedAt: undefined }, { path: '/private/checkpoint' }]) {
    desktopResponse({ checkpoints: [{ ...item, ...changes }], truncated: false });
    await assert.rejects(invoke('list_restore_checkpoints'), /desktop response is incompatible/);
  }
  for (const checkpoints of [[item, item], Array.from({ length: 101 }, () => item)]) {
    desktopResponse({ checkpoints, truncated: true });
    await assert.rejects(invoke('list_restore_checkpoints'), /desktop response is incompatible/);
  }
});

test('checkpoint previews carry opaque source and recovery IDs without file paths', async () => {
  const preview = {
    checkpointId: '00000001-0000-4000-8000-000000000000',
    previewId: '00000002-0000-4000-8000-000000000000',
    current: { activeProfiles: 1, archivedProfiles: '1', accounts: '0' },
    incoming: { activeProfiles: 3, archivedProfiles: '0', accounts: '0' },
    checkpoint: { id: '00000003-0000-4000-8000-000000000000', fileBytes: '12288' },
    restoreAvailable: true,
  };
  const args = { checkpointId: preview.checkpointId };
  desktopResponse(preview);
  assert.deepEqual(await invoke('prepare_checkpoint_restore_preview', args), preview);
  for (const changes of [{ checkpointId: '../outside' }, { restoreAvailable: false },
    { fileName: 'openusage-backup-0123456789abcdef.oubackup' },
    { checkpoint: { ...preview.checkpoint, path: '/private' } },
    { incoming: { ...preview.incoming, accounts: 0 } }]) {
    desktopResponse({ ...preview, ...changes });
    await assert.rejects(invoke('prepare_checkpoint_restore_preview', args), /desktop response is incompatible/);
  }
});

test('executable reviews distinguish unknown sources, official artifacts, and test fixtures', async () => {
  const candidate = { candidateId: '00000001-0000-4000-8000-000000000000', path: '/synthetic/codex', fileBytes: '28785', sha256: 'a'.repeat(64), testedVersion: '0.163.0', versionChecked: false, sourceVerified: false, sourceKind: 'unverified', sourceUrl: null };
  for (const source of [
    {}, { sourceVerified: true, sourceKind: 'test_fixture' },
    { sourceVerified: true, sourceKind: 'official_artifact', sourceUrl: 'https://github.com/openai/codex/releases/download/rust-v0.163.0/codex-x86_64-unknown-linux-musl.tar.gz' },
  ]) {
    const value = { ...candidate, ...source };
    desktopResponse(value);
    assert.deepEqual(await invoke('inspect_provider_executable', { path: candidate.path }), value);
  }
  for (const changes of [{ sourceVerified: true }, { sourceKind: 'official_artifact', sourceVerified: true, sourceUrl: 'https://example.invalid/codex' },
    { sourceKind: 'test_fixture', sourceVerified: true, sourceUrl: 'https://example.invalid' },
    { sha256: 'invalid' }, { versionChecked: true }, { path: '/invalid\0path' }, { key: 'private' }]) {
    desktopResponse({ ...candidate, ...changes });
    await assert.rejects(invoke('inspect_provider_executable', { path: candidate.path }), /desktop response is incompatible/);
  }
});

test('approval receipts require a verified source and exclude private home paths', async () => {
  const approval = { approvalId: '00000002-0000-4000-8000-000000000000', path: '/synthetic/codex', fileBytes: '28785', sha256: 'a'.repeat(64), testedVersion: '0.163.0', versionChecked: false, sourceVerified: true, sourceKind: 'test_fixture', sourceUrl: null };
  const args = { candidateId: '00000001-0000-4000-8000-000000000000', confirmed: true };
  desktopResponse(approval);
  assert.deepEqual(await invoke('approve_provider_executable', args), approval);
  for (const changes of [{ sourceVerified: false, sourceKind: 'unverified' }, { approvalId: '../outside' }, { home: '/private/profile' }]) {
    desktopResponse({ ...approval, ...changes });
    await assert.rejects(invoke('approve_provider_executable', args), /desktop response is incompatible/);
  }
});

test('provider diagnostics cannot claim verified identity, persisted readings, or active monitoring', async () => {
  const diagnostic = { profileId: '00000001-0000-4000-8000-000000000000', approvalId: '00000002-0000-4000-8000-000000000000', providerVersion: '0.163.0', authentication: 'authentication_required', identityConfidence: 'unknown', requiresOpenaiAuth: true, checkedAt: '2026-10-18T04:00:00.123Z', monitoring: 'stopped', persisted: false, sourceKind: 'test_fixture' };
  const args = { profileId: diagnostic.profileId };
  for (const account of [{}, { authentication: 'chatgpt', identityConfidence: 'provisional' }, { authentication: 'chatgpt', identityConfidence: 'unknown' }]) {
    const value = { ...diagnostic, ...account };
    desktopResponse(value);
    assert.deepEqual(await invoke('check_profile_connection', args), value);
  }
  for (const changes of [{ identityConfidence: 'verified' }, { identityConfidence: 'provisional' }, { persisted: true }, { monitoring: 'active' },
    { sourceKind: 'unverified' }, { checkedAt: '2026-02-30T04:00:00.123Z' }, { returnedEmail: 'private@example.invalid' }, { raw: '{}' }]) {
    desktopResponse({ ...diagnostic, ...changes });
    await assert.rejects(invoke('check_profile_connection', args), /desktop response is incompatible/);
  }
});


test('sign-in attempts keep opaque bindings and stopped monitoring without provider secrets', async () => {
  const value = { profileId: '00000001-0000-4000-8000-000000000000', attemptId: '00000003-0000-4000-8000-000000000000', approvalId: '00000002-0000-4000-8000-000000000000', sourceKind: 'test_fixture', monitoring: 'stopped', applicationWaitLimitSeconds: 900 };
  const args = { profileId: value.profileId, approvalId: value.approvalId, flow: 'device_code', confirmed: true };
  desktopResponse(value); assert.deepEqual(await invoke('begin_profile_login', args), value);
  for (const changes of [{ attemptId: '../private' }, { monitoring: 'active' }, { sourceKind: 'unverified' }, { applicationWaitLimitSeconds: '900' }, { applicationWaitLimitSeconds: 0 }, { providerLoginId: 'private' }, { token: 'secret' }]) {
    desktopResponse({ ...value, ...changes });
    await assert.rejects(invoke('begin_profile_login', args), /desktop response is incompatible/);
  }
});

test('device challenges allow only the reviewed page and bounded visible code with no expiry claim', async () => {
  const value = { profileId: '00000001-0000-4000-8000-000000000000', attemptId: '00000003-0000-4000-8000-000000000000', verificationUrl: 'https://auth.openai.com/codex/device', userCode: 'SYNTH-1234', sourceKind: 'test_fixture', applicationWaitLimitSeconds: 900 };
  const args = { profileId: value.profileId, attemptId: value.attemptId };
  desktopResponse(value); assert.deepEqual(await invoke('await_profile_login_prompt', args), value);
  for (const changes of [{ verificationUrl: 'https://auth.openai.com.evil.invalid/codex/device' }, { verificationUrl: value.verificationUrl + '?redirect=elsewhere' }, { verificationUrl: 'https://auth.openai.com:443/codex/device' }, { userCode: ' ' }, { userCode: 'x'.repeat(65) }, { userCode: 'SYNTH\n1234' }, { userCode: 'SYNTH\u202e1234' }, { expiresAt: '2026-10-18T04:00:00.123Z' }, { accessToken: 'secret' }]) {
    desktopResponse({ ...value, ...changes });
    await assert.rejects(invoke('await_profile_login_prompt', args), /desktop response is incompatible/);
  }
});

test('sign-in completion requires a fresh ChatGPT diagnostic while verified identity remains unavailable', async () => {
  const diagnostic = { profileId: '00000001-0000-4000-8000-000000000000', approvalId: '00000002-0000-4000-8000-000000000000', providerVersion: '0.163.0', authentication: 'chatgpt', identityConfidence: 'provisional', requiresOpenaiAuth: true, checkedAt: '2026-10-18T04:00:00.123Z', monitoring: 'stopped', persisted: false, sourceKind: 'test_fixture' };
  const value = { attemptId: '00000003-0000-4000-8000-000000000000', diagnostic };
  const args = { profileId: diagnostic.profileId, attemptId: value.attemptId };
  desktopResponse(value); assert.deepEqual(await invoke('complete_profile_login', args), value);
  for (const changes of [{ diagnostic: { ...diagnostic, authentication: 'authentication_required' } }, { diagnostic: { ...diagnostic, identityConfidence: 'verified' } }, { diagnostic: { ...diagnostic, persisted: true } }, { diagnostic: { ...diagnostic, rawAccount: {} } }, { providerLoginId: 'private' }]) {
    desktopResponse({ ...value, ...changes });
    await assert.rejects(invoke('complete_profile_login', args), /desktop response is incompatible/);
  }
});

test('accepts bounded advisory discovery without trusting extra fields or malformed paths', async () => {
  const candidate = { path: '/synthetic/bin/codex', location: 'path' };
  const value = { candidates: [candidate], truncated: false };
  desktopResponse(value);
  assert.deepEqual(await invoke('discover_provider_executables'), value);
  for (const malformed of [
    { ...value, approved: true }, { ...value, truncated: 'false' },
    { ...value, candidates: Array(17).fill(candidate) },
    { ...value, candidates: [candidate, candidate] },
    ...[{ path: 'relative/codex' }, { path: '/bad\0path' }, { location: ['path'] },
      { location: 'unknown' }, { sourceVerified: true }].map(change => ({ ...value, candidates: [{ ...candidate, ...change }] })),
  ]) {
    desktopResponse(malformed);
    await assert.rejects(invoke('discover_provider_executables'), /desktop response is incompatible/);
  }
});

test('accepts provisional saved quota receipts without inventing missing windows', async () => {
  const request = { profileId: '11111111-1111-4111-8111-111111111111', requestId: '22222222-2222-4222-8222-222222222222', approvalId: '33333333-3333-4333-8333-333333333333', sourceKind: 'test_fixture', monitoring: 'stopped', accepted: false };
  desktopResponse(request);
  assert.deepEqual(await invoke('begin_profile_quota_read', { profileId: request.profileId, approvalId: request.approvalId }), request);
  const window = { usedPercent: 0, derivedRemainingPercent: 100, windowDurationMins: null, resetsAtUnix: null, label: 'Primary window' };
  const preview = { ...request, accepted: true, bindingId: '44444444-4444-4444-8444-444444444444', collectionRunId: '55555555-5555-4555-8555-555555555555', providerVersion: '0.163.0', observedAtUtc: '2026-10-18T07:00:00.000Z', identityConfidence: 'provisional', verifiedIdentityKey: null, availableResetCredits: null, resetCreditDetails: null, limits: [{ limitId: 'codex', limitName: null, primary: window, secondary: null, reachedType: null, monthlyCreditLimit: null, spendControlReached: null }] };
  desktopResponse(preview);
  assert.deepEqual(await invoke('complete_profile_quota_read', { profileId: request.profileId, requestId: request.requestId }), preview);
  for (const availableResetCredits of [null, '0', '9007199254740993', '9223372036854775807']) {
    desktopResponse({ ...preview, availableResetCredits });
    assert.equal((await invoke('complete_profile_quota_read', { profileId: request.profileId, requestId: request.requestId })).availableResetCredits, availableResetCredits);
  }
  const detail = { title: 'Synthetic earned credit', status: 'available', grantedAtUnix: 0, expiresAtUnix: null };
  for (const resetCreditDetails of [null, [], [detail], Array.from({ length: 32 }, () => ({ ...detail, title: 'é'.repeat(128), status: 'unknown', grantedAtUnix: -62135596800, expiresAtUnix: 253402300799 }))]) {
    desktopResponse({ ...preview, availableResetCredits: '9', resetCreditDetails });
    const result = await invoke('complete_profile_quota_read', { profileId: request.profileId, requestId: request.requestId });
    assert.deepEqual(result.resetCreditDetails, resetCreditDetails);
    assert.equal(result.availableResetCredits, '9');
  }
  for (const monthlyCreditLimit of [null, '0', '9007199254740993.012345678900', '100.00', ' 001.2500 ', 'provider-format', 'é'.repeat(128)]) {
    for (const spendControlReached of [null, false, true]) {
      const limit = { ...preview.limits[0], monthlyCreditLimit, spendControlReached };
      desktopResponse({ ...preview, limits: [limit] });
      const result = await invoke('complete_profile_quota_read', { profileId: request.profileId, requestId: request.requestId });
      assert.deepEqual(result.limits[0], limit);
    }
  }
  desktopResponse({ ...preview, limits: [] });
  assert.deepEqual((await invoke('complete_profile_quota_read', { profileId: request.profileId, requestId: request.requestId })).limits, []);
});

test('rejects malformed or secret-bearing quota payloads and unqualified identity claims', async () => {
  const window = { usedPercent: 25, derivedRemainingPercent: 75, windowDurationMins: 300, resetsAtUnix: 1792310400, label: 'Five-hour' };
  const limit = { limitId: 'codex', limitName: null, primary: window, secondary: null, reachedType: null, monthlyCreditLimit: null, spendControlReached: null };
  const value = { profileId: '11111111-1111-4111-8111-111111111111', requestId: '22222222-2222-4222-8222-222222222222', approvalId: '33333333-3333-4333-8333-333333333333', sourceKind: 'test_fixture', monitoring: 'stopped', accepted: true, bindingId: '44444444-4444-4444-8444-444444444444', collectionRunId: '55555555-5555-4555-8555-555555555555', providerVersion: '0.163.0', observedAtUtc: '2026-10-18T07:00:00.000Z', identityConfidence: 'provisional', verifiedIdentityKey: null, availableResetCredits: null, resetCreditDetails: null, limits: [limit] };
  const malformedMonthly = [
    ...[undefined, 0, 1.5, true, '', ' ', 'synthetic\nprivate', 'x'.repeat(257), 'é'.repeat(129), [], {}].map(monthlyCreditLimit => ({ ...limit, monthlyCreditLimit })),
    ...[undefined, 0, 1, 'true', [], {}].map(spendControlReached => ({ ...limit, spendControlReached })),
    { ...limit, individualLimit: { limit: '3', accountId: 'synthetic-private' } },
  ].map(limit => ({ ...value, limits: [limit] }));
  const malformedWindows = [{ usedPercent: -1 }, { usedPercent: 101 }, { usedPercent: 25.5 }, { usedPercent: Number.NaN }, { derivedRemainingPercent: Infinity }, { windowDurationMins: 0 }, { windowDurationMins: Number.MAX_SAFE_INTEGER + 1 }, { resetsAtUnix: 253402300800 }, { label: '' }, { token: 'synthetic-must-not-enter-renderer' }].map(change => ({ ...value, limits: [{ ...limit, primary: { ...window, ...change } }] }));
  const malformedCredits = [undefined, 0, -1, 1.5, true, '', '-1', '01', ' 3', '3 ', '3\n', '1.5', '1e3', '9223372036854775808', '100000000000000000000'].map(availableResetCredits => ({ ...value, availableResetCredits }));
  const detail = { title: 'Synthetic earned credit', status: 'available', grantedAtUnix: 0, expiresAtUnix: null };
  const badDetailFields = [{ title: '' }, { title: 'x'.repeat(257) }, { title: 'é'.repeat(129) }, { title: 'synthetic\nprivate' }, { title: undefined }, { status: 'future' }, { status: ['available'] }, { grantedAtUnix: null }, { grantedAtUnix: 1.5 }, { grantedAtUnix: -62135596801 }, { expiresAtUnix: 253402300800 }, { expiresAtUnix: undefined }, { id: 'synthetic-private-id' }, { description: 'synthetic-private-description' }, { rawPayload: {} }];
  const malformedDetails = [undefined, {}, 'synthetic-private', [null], [[]], Array.from({ length: 33 }, () => detail), ...badDetailFields.map(change => [{ ...detail, ...change }])].map(resetCreditDetails => ({ ...value, resetCreditDetails }));
  const missingDetails = { ...value };
  delete missingDetails.resetCreditDetails;
  const missingCredits = { ...value };
  delete missingCredits.availableResetCredits;
  for (const malformed of [...malformedMonthly, missingDetails, ...malformedDetails, missingCredits, ...malformedCredits, ...malformedWindows, ...[
    { accepted: false }, { monitoring: 'active' }, { identityConfidence: 'verified' }, { identityConfidence: 'unknown' }, { verifiedIdentityKey: 'synthetic-key' }, { observedAtUtc: '2026-02-30T07:00:00.000Z' }, { email: 'synthetic@example.invalid' }, { accountId: 'synthetic-private-source-scope' }, { limits: [limit, limit] }, { limits: Array.from({ length: 65 }, (_, id) => ({ ...limit, limitId: `limit-${id}` })) }, { limits: [{ ...limit, limitId: '' }] }, { limits: [{ ...limit, secondary: undefined }] }, { limits: [{ ...limit, rawFrame: '{}' }] },
  ].map(change => ({ ...value, ...change }))]) {
    desktopResponse(malformed);
    await assert.rejects(invoke('complete_profile_quota_read', { profileId: value.profileId, requestId: value.requestId }), /desktop response is incompatible/);
  }
});

const activityRequest = { profileId: '11111111-1111-4111-8111-111111111111', requestId: '22222222-2222-4222-8222-222222222222', approvalId: '33333333-3333-4333-8333-333333333333', sourceKind: 'test_fixture', monitoring: 'stopped', accepted: false };
const activityPreview = { ...activityRequest, accepted: true, collectionRunId: '44444444-4444-4444-8444-444444444444', bindingId: '55555555-5555-4555-8555-555555555555', providerVersion: '0.163.0', observedAtUtc: '2026-10-18T00:00:00.000Z', identityConfidence: 'provisional', verifiedIdentityKey: null, coverage: 'unknown', scope: 'account_activity_with_unknown_application_scope', summary: { lifetimeTokens: '9223372036854775807', peakDailyTokens: '9007199254740993', longestRunningTurnSec: null, currentStreakDays: '0', longestStreakDays: '1' }, dailyUsageBuckets: [{ startDate: '2024-02-29', tokens: '9223372036854775807' }, { startDate: '0001-01-01', tokens: '0' }] };
const activityArgs = { profileId: activityRequest.profileId, requestId: activityRequest.requestId };

test('accepts exact bounded activity previews without replacing null or reordering literal dates', async () => {
  desktopResponse(activityRequest);
  assert.deepEqual(await invoke('begin_profile_activity_read', { profileId: activityRequest.profileId, approvalId: activityRequest.approvalId }), activityRequest);
  for (const dailyUsageBuckets of [null, [], activityPreview.dailyUsageBuckets, Array.from({ length: 4096 }, (_, index) => ({ startDate: new Date(Date.UTC(2000, 0, index + 1)).toISOString().slice(0, 10), tokens: '0' }))]) {
    const value = { ...activityPreview, dailyUsageBuckets };
    desktopResponse(value); assert.deepEqual(await invoke('complete_profile_activity_read', activityArgs), value);
  }
  desktopResponse({ cancelled: false });
  assert.deepEqual(await invoke('cancel_profile_activity_read', activityArgs), { cancelled: false });
});

test('rejects activity completeness claims, leaked fields, malformed counts and missing summary metrics', async () => {
  const summary = activityPreview.summary;
  for (const change of [
    { accepted: false }, { monitoring: 'running' }, { coverage: 'complete' }, { scope: 'codex_only' }, { verifiedIdentityKey: 'account' },
    { accountId: 'private' }, { observedAtUtc: '2026-02-30T00:00:00.000Z' }, { summary: null },
    ...[undefined, 0, -1, '01', '-1', '1.0', '9223372036854775808', ' 1', '1\n'].map(lifetimeTokens => ({ summary: { ...summary, lifetimeTokens } })),
    { summary: { ...summary, rawPayload: 'private' } }, { summary: { ...summary, currentStreakDays: undefined } },
  ]) {
    desktopResponse({ ...activityPreview, ...change });
    await assert.rejects(invoke('complete_profile_activity_read', activityArgs), /desktop response is incompatible/);
  }
});

test('rejects oversized activity previews and invalid Gregorian days without truncation or normalization', async () => {
  for (const dailyUsageBuckets of [
    Array(4097).fill({ startDate: '2024-02-29', tokens: '0' }),
    ...['2023-02-29', '2024-02-30', '0000-01-01', '2024-13-01', '2024-1-01', '2024-01-01Z', '2024-04-31'].map(startDate => [{ startDate, tokens: '0' }]),
    ...[null, '01', '9223372036854775808', 10].map(tokens => [{ startDate: '2024-01-01', tokens }]),
    [{ startDate: '2024-01-01', tokens: '0', accountId: 'private' }],
    [{ startDate: '2024-01-01', tokens: '0' }, { startDate: '2024-01-01', tokens: '1' }],
  ]) {
    desktopResponse({ ...activityPreview, dailyUsageBuckets });
    await assert.rejects(invoke('complete_profile_activity_read', activityArgs), /desktop response is incompatible/);
  }
});

const savedActivity = (() => {
  const { requestId, approvalId, dailyUsageBuckets, ...value } = activityPreview;
  return { ...value, freshness: 'unknown', dailyState: 'returned', dailyTruncated: false, dailyUsageBuckets: dailyUsageBuckets.map(row => ({ date: row.startDate, tokens: row.tokens, observedAtUtc: value.observedAtUtc, freshness: 'stale' })) };
})();

test('accepts committed cached activity without approval fields and preserves daily cache independent of latest array state', async () => {
  desktopResponse(null);
  assert.equal(await invoke('load_profile_activity', { profileId: activityRequest.profileId }), null);
  for (const dailyState of ['unavailable', 'empty', 'returned']) {
    const value = { ...savedActivity, dailyState, summary: { ...savedActivity.summary, lifetimeTokens: null } };
    desktopResponse(value);
    assert.deepEqual(await invoke('load_profile_activity', { profileId: activityRequest.profileId }), value);
  }
});

test('refuses malformed cached provenance, uncommitted results, guessed freshness and lossy cached totals', async () => {
  for (const change of [
    { accepted: false }, { collectionRunId: undefined }, { bindingId: 'not-a-uuid' }, { freshness: ['fresh'] }, { freshness: 'online' },
    { sourceKind: ['test_fixture'] }, { dailyState: ['returned'] }, { dailyTruncated: 'false' }, { approvalId: activityRequest.approvalId },
    { summary: { ...savedActivity.summary, lifetimeTokens: 9007199254740992 } },
    ...[
      { tokens: '9223372036854775808' }, { date: '2024-02-30' }, { date: '2024-02-29Z' }, { observedAtUtc: 'yesterday' },
      { freshness: ['stale'] }, { freshness: 'current' }, { accountId: 'private' },
    ].map(change => ({ dailyUsageBuckets: [{ ...savedActivity.dailyUsageBuckets[0], ...change }] })),
    { dailyUsageBuckets: [savedActivity.dailyUsageBuckets[0], savedActivity.dailyUsageBuckets[0]] },
    { dailyUsageBuckets: Array(4097).fill(savedActivity.dailyUsageBuckets[0]) },
  ]) {
    desktopResponse({ ...savedActivity, ...change });
    await assert.rejects(invoke('load_profile_activity', { profileId: activityRequest.profileId }), /desktop response is incompatible/);
  }
});


for (const command of ['begin_profile_activity_reassociation', 'begin_profile_quota_reassociation']) test(`${command} begins with an unaccepted owned request and rejects committed or extra-field replies`, async () => {
  const args = { profileId: activityRequest.profileId, approvalId: activityRequest.approvalId, reviewId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', confirmed: true };
  desktopResponse(activityRequest);
  assert.deepEqual(await invoke(command, args), activityRequest);
  for (const value of [{ ...activityRequest, accepted: true }, { ...activityRequest, privateHome: '/private' }, { ...activityRequest, requestId: 'invalid' }]) {
    desktopResponse(value);
    await assert.rejects(invoke(command, args), /desktop response is incompatible/);
  }
});


for (const command of ['review_profile_activity_association', 'review_profile_quota_association']) test(`${command} accepts only bounded descriptive evidence and an exact provisional receipt`, async () => {
  const args = { profileId: activityRequest.profileId, approvalId: activityRequest.approvalId };
  const review = { ...args, reviewId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', observedAtUtc: '2026-10-18T00:00:00.000Z', previous: { email: 'previous@synthetic.invalid', plan: null }, current: { email: 'current@synthetic.invalid', plan: 'Synthetic plan' }, homeChanged: true, identityConfidence: 'provisional' };
  desktopResponse(review); assert.deepEqual(await invoke(command, args), review);
  const boundary = { ...review, previous: { email: 'é'.repeat(127), plan: 'é'.repeat(32) }, homeChanged: false };
  desktopResponse(boundary); assert.deepEqual(await invoke(command, args), boundary);
  for (const change of [
    { reviewId: 'invalid' }, { profileId: 'invalid' }, { approvalId: 'invalid' }, { observedAtUtc: '2026-02-30T00:00:00.000Z' },
    { identityConfidence: 'verified' }, { homeChanged: null }, { privateHome: '/private' },
    { previous: null }, { previous: { email: 'x', plan: null, token: 'private' } },
    { current: { email: '', plan: null } }, { current: { email: 'x\n', plan: null } },
    { current: { email: 'é'.repeat(128), plan: null } }, { current: { email: 'current@synthetic.invalid' } },
    { current: { email: 'current@synthetic.invalid', plan: 'é'.repeat(33) } },
    { current: { email: 'current@synthetic.invalid', plan: '' } },
  ]) {
    desktopResponse({ ...review, ...change });
    await assert.rejects(invoke(command, args), /desktop response is incompatible/);
  }
});


test('saved quota loads accept only exact bounded provisional cache values', async () => {
  const args = { profileId: '11111111-1111-4111-8111-111111111111' };
  const value = { ...args, bindingId: '44444444-4444-4444-8444-444444444444', collectionRunId: '55555555-5555-4555-8555-555555555555',
    providerVersion: '0.163.0', sourceKind: 'test_fixture', observedAtUtc: '2026-10-18T00:00:00.000Z', identityConfidence: 'provisional', verifiedIdentityKey: null,
    monitoring: 'stopped', accepted: true, coverage: 'unknown', scope: 'account_quota_with_unknown_application_scope', freshness: 'unknown', limits: [], availableResetCredits: null, resetCreditDetails: null };
  desktopResponse(null); assert.equal(await invoke('load_profile_quota', args), null);
  for (const freshness of ['unknown', 'fresh', 'stale']) { desktopResponse({ ...value, freshness }); assert.deepEqual(await invoke('load_profile_quota', args), { ...value, freshness }); }
  const window = { usedPercent: 100, derivedRemainingPercent: 0, windowDurationMins: 300, resetsAtUnix: 0, label: 'Five-hour', resetDue: true };
  const withWindow = primary => ({ ...value, limits: [{ limitId: 'codex', limitName: null, primary, secondary: null, reachedType: null, monthlyCreditLimit: null, spendControlReached: null }] });
  desktopResponse(withWindow(window)); assert.deepEqual(await invoke('load_profile_quota', args), withWindow(window));
  for (const change of [{ resetDue: undefined }, { resetDue: null }, { resetDue: 'true' }, { resetDue: true, resetsAtUnix: null }, { resetConfirmed: true }]) {
    desktopResponse(withWindow({ ...window, ...change })); await assert.rejects(invoke('load_profile_quota', args), /desktop response is incompatible/);
  }
  for (const change of [{ accepted: false }, { freshness: 'expired' }, { bindingId: '../private' }, { collectionRunId: null }, { coverage: 'complete' }, { scope: 'account_activity_with_unknown_application_scope' }, { verifiedIdentityKey: 'secret' }, { identityConfidence: 'verified' }, { requestId: args.profileId }, { email: 'private@invalid' }, { availableResetCredits: '9223372036854775808' }, { resetCreditDetails: [{}] }, { limits: [{}] }]) {
    desktopResponse({ ...value, ...change }); await assert.rejects(invoke('load_profile_quota', args), /desktop response is incompatible/);
  }
  for (const key of Object.keys(value)) { const missing = { ...value }; delete missing[key]; desktopResponse(missing); await assert.rejects(invoke('load_profile_quota', args), /desktop response is incompatible/); }
});
