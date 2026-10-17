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

test('restore previews require safe opaque IDs and exact counts without approving replacement', async () => {
  const preview = {
    previewId: '00000001-0000-4000-8000-000000000000',
    fileName: 'openusage-backup-0123456789abcdef.oubackup',
    current: { activeProfiles: 3, archivedProfiles: '1', accounts: '0' },
    incoming: { activeProfiles: 2, archivedProfiles: '9007199254740993', accounts: '0' },
    checkpoint: { id: '00000002-0000-4000-8000-000000000000', fileBytes: '12000000000' },
    restoreAvailable: false,
  };
  const args = { fileName: preview.fileName, password: 'x' };
  desktopResponse(preview);
  assert.deepEqual(await invoke('prepare_restore_preview', args), preview);
  for (const changes of [
    { previewId: '../outside' }, { restoreAvailable: true }, { fileName: '/private/backup' },
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
