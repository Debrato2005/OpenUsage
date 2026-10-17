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
