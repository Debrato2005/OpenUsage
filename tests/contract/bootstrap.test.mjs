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
