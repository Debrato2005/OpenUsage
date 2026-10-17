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

test('inspection metadata preserves unknown database validity and keeps restore unavailable', async () => {
  const metadata = {
    fileName: 'openusage-backup-0123456789abcdef.oubackup', archiveVersion: 1,
    manifestSchemaVersion: 77, entryCount: 1, totalBytes: '51',
    entries: [{ name: 'records.sqlite', bytes: '51' }],
    databaseValidation: 'not_checked', restoreAvailable: false,
  };
  desktopResponse(metadata);
  assert.deepEqual(await invoke('inspect_backup', { fileName: metadata.fileName, password: 'x' }), metadata);
  for (const changes of [
    { restoreAvailable: true }, { databaseValidation: 'verified' }, { archiveVersion: 2 },
    { entryCount: 2 }, { totalBytes: 51 }, { entries: [] },
    { fileName: '/private/archive.oubackup' }, { path: '/private/archive.oubackup' },
  ]) {
    desktopResponse({ ...metadata, ...changes });
    await assert.rejects(invoke('inspect_backup', { fileName: metadata.fileName, password: 'x' }), /desktop response is incompatible/);
  }
});
