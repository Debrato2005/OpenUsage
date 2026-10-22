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

test('application export returns metadata only and keeps counts exact', async () => {
  const receipt = { fileName: 'openusage-application-AbCd1234EfGh5678.json', directory: '/synthetic/Downloads/OpenUsage', format: 'json', schemaVersion: 8, tableCount: 30, recordCount: '9007199254740993', byteCount: '1048576', exportedAt: '2026-10-22T08:00:00.000Z' };
  desktopResponse(receipt);
  assert.deepEqual(await invoke('export_application_data', { confirmed: true }), receipt);
  for (const fields of [{ recordCount: '0' }, { byteCount: '5368709120' }]) {
    desktopResponse({ ...receipt, ...fields });
    assert.deepEqual(await invoke('export_application_data', { confirmed: true }), { ...receipt, ...fields });
  }
  for (const fields of [{ recordCount: 9007199254740992 }, { recordCount: '18446744073709551616' }, { recordCount: '01' }, { byteCount: '5368709121' }, { byteCount: '0' }, { format: 'csv' }, { tableCount: 0 }, { tableCount: 30.5 }, { schemaVersion: 0 }, { fileName: '../private.json' }, { directory: 'private\nlocation' }, { exportedAt: '2026-02-30T08:00:00.000Z' }, { records: [] }, { credentials: 'synthetic-only' }]) {
    desktopResponse({ ...receipt, ...fields });
    await assert.rejects(invoke('export_application_data', { confirmed: true }), /desktop response is incompatible/);
  }
});

test('local deletion accepts only an empty setup result and keeps pending recovery explicit', async () => {
  const fresh = { ...ready, storage: 'setup_required', schemaVersion: null, activeProfiles: null };
  desktopResponse(fresh); assert.deepEqual(await invoke('delete_all_local_data', { confirmed: true }), fresh);
  const pending = { ...fresh, storage: 'recovery_required', keyProtection: 'unavailable', storageIssue: 'local_deletion_incomplete' };
  desktopResponse(pending); assert.deepEqual(await invoke('bootstrap'), pending);
  for (const value of [ready, pending, { ...fresh, activeProfiles: 1 }, { ...fresh, schemaVersion: 8 }, { ...fresh, monitoring: 'running' }, { ...fresh, secret: 'synthetic-private-value' }]) {
    desktopResponse(value); await assert.rejects(invoke('delete_all_local_data', { confirmed: true }));
  }
});

test('alert history accepts saved reset evidence and refuses leaked fields or false delivery claims', async () => {
  const event = { id: '11111111-1111-4111-8111-111111111111', profileId: '22222222-2222-4222-8222-222222222222', profileLabel: 'Account A', kind: 'quota_reset', thresholdPercent: null, evidenceRunId: '33333333-3333-4333-8333-333333333333', limitId: 'codex', windowRole: 'primary', windowDurationMins: 300, evidenceAtUtc: '2026-10-22T08:00:00.000Z', sourceKind: 'official_artifact', deliveryState: 'recorded' };
  const valid = { events: [event], truncated: false };
  desktopResponse(valid); assert.deepEqual(await invoke('load_alert_history'), valid);
  desktopResponse({ events: [], truncated: false }); assert.deepEqual(await invoke('load_alert_history'), { events: [], truncated: false });
  for (const fields of [{ id: '../unsafe' }, { profileId: [] }, { profileLabel: '' }, { profileLabel: 'private\nlabel' }, { kind: 'usage_threshold' }, { kind: 'quota_threshold', thresholdPercent: null }, { kind: 'quota_threshold', thresholdPercent: 85 }, { kind: 'quota_threshold', thresholdPercent: 100, deliveryState: 'covered' }, { thresholdPercent: 80 }, { deliveryState: 'covered' }, { evidenceRunId: '../unsafe' }, { limitId: '' }, { windowRole: ['primary'] }, { windowDurationMins: 0 }, { windowDurationMins: Number.MAX_SAFE_INTEGER + 1 }, { evidenceAtUtc: '2026-02-30T08:00:00.000Z' }, { sourceKind: 'unknown' }, { deliveryState: 'delivered' }, { eventKey: 'private identity' }]) {
    desktopResponse({ events: [{ ...event, ...fields }], truncated: false });
    await assert.rejects(invoke('load_alert_history'), /desktop response is incompatible/);
  }
  for (const thresholdPercent of [80, 90, 100]) {
    const threshold = { ...event, kind: 'quota_threshold', thresholdPercent, deliveryState: thresholdPercent === 100 ? 'submitted' : 'covered' };
    desktopResponse({ events: [threshold], truncated: false });
    assert.deepEqual((await invoke('load_alert_history')).events, [threshold]);
  }
  for (const value of [{ ...valid, credentials: 'must never reach the renderer' }, { ...valid, truncated: true }, { events: [event, event], truncated: false }, { events: Array.from({ length: 101 }, () => event), truncated: true }]) {
    desktopResponse(value); await assert.rejects(invoke('load_alert_history'), /desktop response is incompatible/);
  }
});

test('operational alerts expose only the selected account and safe evidence', async () => {
  for (const kind of ['authentication_required', 'collection_failure']) {
    const event = { id: '11111111-1111-4111-8111-111111111111', profileId: '22222222-2222-4222-8222-222222222222', profileLabel: 'Account A', kind, evidenceAtUtc: '2026-10-22T08:00:00.000Z', sourceKind: 'official_artifact', deliveryState: 'recorded' };
    desktopResponse({ events: [event], truncated: false });
    assert.deepEqual((await invoke('load_alert_history')).events, [event]);
    for (const extra of [{ evidenceRunId: event.id }, { thresholdPercent: null }, { error: 'raw provider error' }, { deliveryState: 'covered' }, { kind: 'invalid_credentials' }]) {
      desktopResponse({ events: [{ ...event, ...extra }], truncated: false });
      await assert.rejects(invoke('load_alert_history'), /desktop response is incompatible/);
    }
  }
});

test('activity history keeps nullable exact summaries, cursor scope and observation ordering', async () => {
  const profileId = '11111111-1111-4111-8111-111111111111';
  const entry = { profileId, bindingId: '22222222-2222-4222-8222-222222222222', collectionRunId: '33333333-3333-4333-8333-333333333333', observedAtUtc: '2026-10-22T08:00:00.000Z', sourceKind: 'official_artifact', providerVersion: '0.163.0', sourceScope: 'account_activity_with_unknown_application_scope', coverage: 'unknown', summary: { lifetimeTokens: '9223372036854775807', peakDailyTokens: '0', longestRunningTurnSec: null, currentStreakDays: null, longestStreakDays: '9007199254740993' }, dailyBucketsState: 'unavailable' };
  const args = { profileId, beforeRunId: null };
  const page = { profileId, entries: [entry], nextBeforeRunId: null };
  desktopResponse(page); assert.deepEqual(await invoke('load_profile_activity_history', args), page);
  for (const fields of [{ profileId: '44444444-4444-4444-8444-444444444444' }, { bindingId: 'bad' }, { collectionRunId: 'bad' }, { observedAtUtc: '2026-02-30T08:00:00.000Z' }, { sourceKind: 'unknown' }, { sourceScope: 'inference_requests' }, { coverage: 'complete' }, { email: 'private@example.test' }, { dailyBucketsState: 'missing' }, { dailyUsageBuckets: [] }, { summary: { ...entry.summary, lifetimeTokens: 9007199254740993 } }, { summary: { ...entry.summary, lifetimeTokens: '9223372036854775808' } }, { summary: { ...entry.summary, lifetimeTokens: '01' } }]) {
    desktopResponse({ ...page, entries: [{ ...entry, ...fields }] });
    await assert.rejects(invoke('load_profile_activity_history', args), /desktop response is incompatible/);
  }
  for (const invalid of [{ ...page, entries: [entry, entry] }, { ...page, nextBeforeRunId: entry.collectionRunId }, { ...page, token: 'private' }, { ...page, entries: [{ ...entry, observedAtUtc: '2026-10-21T08:00:00.000Z' }, entry] }]) {
    desktopResponse(invalid); await assert.rejects(invoke('load_profile_activity_history', args), /desktop response is incompatible/);
  }
});

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

test('saved-readings export accepts only bounded exact receipts with matching format and valid UTC time', async () => {
  const receipt = { fileName: 'openusage-readings-0123456789abcdef.csv', directory: '/synthetic/Downloads/OpenUsage', format: 'csv', profileCount: 2, rowCount: 0, identifiersIncluded: false, exportedAt: '2026-10-20T00:00:00.000Z' };
  const args = { profileIds: ['00000001-0000-4000-8000-000000000000'], format: 'csv', includeIdentifiers: false, confirmed: true };
  for (const format of ['csv', 'json']) {
    const valid = { ...receipt, format, fileName: `openusage-readings-0123456789abcdef.${format}`, profileCount: 10, rowCount: 50000 };
    desktopResponse(valid);
    assert.deepEqual(await invoke('export_saved_readings', { ...args, format }), valid);
  }
  for (const changes of [
    { fileName: '../openusage-readings-0123456789abcdef.csv' },
    { fileName: 'openusage-readings-short.csv' }, { fileName: 'openusage-readings-0123456789abcdef.json' },
    { fileName: 'openusage-readings-0123456789abcdef.csv\n' },
    { fileName: `openusage-readings-${'a'.repeat(65)}.csv` }, { format: 'xml' }, { format: ['csv'] },
    { directory: '' }, { directory: '/invalid\0path' }, { directory: '/invalid\npath' },
    { profileCount: 0 }, { profileCount: 11 }, { profileCount: '2' }, { profileCount: 1.5 },
    { rowCount: -1 }, { rowCount: 50001 }, { rowCount: '0' }, { rowCount: 0.5 },
    { identifiersIncluded: 'false' }, { exportedAt: '2026-02-30T00:00:00.000Z' },
    { exportedAt: '2026-10-20' }, { exportedAt: 'not-a-time' }, { payload: 'private readings' },
  ]) {
    desktopResponse({ ...receipt, ...changes });
    await assert.rejects(invoke('export_saved_readings', args), /desktop response is incompatible/);
  }
  for (const key of Object.keys(receipt)) {
    const missing = { ...receipt }; delete missing[key]; desktopResponse(missing);
    await assert.rejects(invoke('export_saved_readings', args), /desktop response is incompatible/);
  }
});

for (const command of ['export_quota_history', 'export_activity_history']) test(`${command} accepts only one-profile receipts and rejects payload leakage`, async () => {
  const args = { profileId: '11111111-1111-4111-8111-111111111111', format: 'json', includeIdentifiers: false, confirmed: true };
  const valid = { fileName: 'openusage-readings-0123456789abcdef.json', directory: '/synthetic/Downloads/OpenUsage', format: 'json', profileCount: 1, rowCount: 50000, identifiersIncluded: false, exportedAt: '2026-10-21T00:00:00.000Z' };
  desktopResponse(valid);
  assert.deepEqual(await invoke(command, args), valid);
  for (const change of [{ profileCount: 2 }, { rowCount: 50001 }, { records: [] }, { directory: '' }, { fileName: '../records.json' }]) {
    desktopResponse({ ...valid, ...change });
    await assert.rejects(invoke(command, args), /desktop response is incompatible/);
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

test('normal connection preparation accepts only pinned official source receipts', async () => {
  const approval = { approvalId: '00000002-0000-4000-8000-000000000000', path: '/synthetic/openusage-codex', fileBytes: '296865480', sha256: 'a'.repeat(64), testedVersion: '0.163.0', versionChecked: false, sourceVerified: true, sourceKind: 'official_artifact', sourceUrl: 'https://github.com/openai/codex/releases/download/rust-v0.163.0/codex-x86_64-unknown-linux-musl.tar.gz' };
  desktopResponse(approval);
  assert.deepEqual(await invoke('prepare_provider_connection'), approval);
  for (const changes of [
    { sourceVerified: false, sourceKind: 'unverified', sourceUrl: null },
    { sourceKind: 'test_fixture', sourceUrl: null },
    { sourceUrl: 'https://example.invalid/companion' },
    { home: '/private/profile' }, { versionChecked: true },
  ]) {
    desktopResponse({ ...approval, ...changes });
    await assert.rejects(invoke('prepare_provider_connection'), /desktop response is incompatible/);
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
  const diagnostic = { profileId: '00000001-0000-4000-8000-000000000000', approvalId: '00000002-0000-4000-8000-000000000000', providerVersion: '0.163.0', authentication: 'chatgpt', identityConfidence: 'provisional', requiresOpenaiAuth: true, checkedAt: '2026-10-18T04:00:00.123Z', monitoring: 'stopped', persisted: true, sourceKind: 'test_fixture' };
  const value = { attemptId: '00000003-0000-4000-8000-000000000000', diagnostic };
  const args = { profileId: diagnostic.profileId, attemptId: value.attemptId };
  desktopResponse(value); assert.deepEqual(await invoke('complete_profile_login', args), value);
  desktopResponse({ ...value, diagnostic: { ...diagnostic, identityConfidence: 'unknown' } });
  assert.equal((await invoke('complete_profile_login', args)).diagnostic.identityConfidence, 'unknown');
  for (const changes of [{ diagnostic: { ...diagnostic, authentication: 'authentication_required' } }, { diagnostic: { ...diagnostic, identityConfidence: 'verified' } }, { diagnostic: { ...diagnostic, persisted: false } }, { diagnostic: { ...diagnostic, checkedAt: null } }, { diagnostic: { ...diagnostic, checkedAt: '2026-10-18T04:00:00Z' } }, { diagnostic: { ...diagnostic, rawAccount: {} } }, { providerLoginId: 'private' }]) {
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
    { accepted: false }, { monitoring: 'active' }, { coverage: 'complete' }, { scope: 'codex_only' }, { verifiedIdentityKey: 'account' },
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
  return { ...value, freshness: 'unknown', dailyState: 'returned', dailyTruncated: false, dailyChart: { maxTokens: '9223372036854775807', bars: [{ date: '2024-02-29', heightBasisPoints: 10000 }, { date: '0001-01-01', heightBasisPoints: 0 }] }, dailyUsageBuckets: dailyUsageBuckets.map(row => ({ date: row.startDate, tokens: row.tokens, observedAtUtc: value.observedAtUtc, freshness: 'stale' })) };
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


test('saved activity chart validates matching bounded geometry and preserves exact zero and maximum strings', async () => {
  const empty = { ...savedActivity, dailyUsageBuckets: [], dailyChart: { maxTokens: null, bars: [] } };
  const zero = { ...savedActivity, dailyUsageBuckets: [{ ...savedActivity.dailyUsageBuckets[0], tokens: '0' }], dailyChart: { maxTokens: '0', bars: [{ date: '2024-02-29', heightBasisPoints: 0 }] } };
  for (const value of [savedActivity, empty, zero]) {
    desktopResponse(value);
    assert.deepEqual(await invoke('load_profile_activity', { profileId: activityRequest.profileId }), value);
  }
  const first = savedActivity.dailyChart.bars[0];
  for (const dailyChart of [
    undefined, null, [], {}, { ...savedActivity.dailyChart, extra: true },
    { maxTokens: '9223372036854775808', bars: savedActivity.dailyChart.bars },
    { ...savedActivity.dailyChart, maxTokens: 9223372036854775807 },
    { ...savedActivity.dailyChart, maxTokens: null }, { ...savedActivity.dailyChart, maxTokens: '1' },
    { ...savedActivity.dailyChart, bars: [] }, { ...savedActivity.dailyChart, bars: [...savedActivity.dailyChart.bars, first] },
    { ...savedActivity.dailyChart, bars: [first, first] },
    { ...savedActivity.dailyChart, bars: [...savedActivity.dailyChart.bars].reverse() },
    ...[{ heightBasisPoints: -1 }, { heightBasisPoints: 10001 }, { heightBasisPoints: 0.5 }, { heightBasisPoints: '10000' },
      { date: '2024-02-30' }, { date: '2024-03-01' }, { extra: true }, { heightBasisPoints: 9999 }]
      .map(change => ({ ...savedActivity.dailyChart, bars: [{ ...first, ...change }, savedActivity.dailyChart.bars[1]] })),
    { ...savedActivity.dailyChart, bars: [first, { ...savedActivity.dailyChart.bars[1], heightBasisPoints: 1 }] },
  ]) {
    desktopResponse({ ...savedActivity, dailyChart });
    await assert.rejects(invoke('load_profile_activity', { profileId: activityRequest.profileId }), /desktop response is incompatible/);
  }
  desktopResponse({ ...zero, dailyChart: { maxTokens: '0', bars: [{ date: '2024-02-29', heightBasisPoints: 1 }] } });
  await assert.rejects(invoke('load_profile_activity', { profileId: activityRequest.profileId }), /desktop response is incompatible/);
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
  const window = { usedPercent: 100, derivedRemainingPercent: 0, windowDurationMins: 300, resetsAtUnix: 0, label: 'Five-hour', resetState: 'due' };
  const withWindow = primary => ({ ...value, limits: [{ limitId: 'codex', limitName: null, primary, secondary: null, reachedType: null, monthlyCreditLimit: null, spendControlReached: null }] });
  for (const resetState of ['expected', 'due', 'unconfirmed', 'confirmed']) {
    desktopResponse(withWindow({ ...window, resetState }));
    assert.deepEqual(await invoke('load_profile_quota', args), withWindow({ ...window, resetState }));
  }
  // A retained prior deadline can be due even when the latest provider deadline is absent.
  desktopResponse(withWindow({ ...window, resetsAtUnix: null }));
  assert.deepEqual(await invoke('load_profile_quota', args), withWindow({ ...window, resetsAtUnix: null }));
  for (const change of [{ resetState: undefined }, { resetState: null }, { resetState: true }, { resetState: 'complete' }, { resetDue: true }, { generationRunId: 'private' }]) {
    desktopResponse(withWindow({ ...window, ...change })); await assert.rejects(invoke('load_profile_quota', args), /desktop response is incompatible/);
  }
  for (const change of [{ accepted: false }, { freshness: 'expired' }, { bindingId: '../private' }, { collectionRunId: null }, { coverage: 'complete' }, { scope: 'account_activity_with_unknown_application_scope' }, { verifiedIdentityKey: 'secret' }, { identityConfidence: 'verified' }, { requestId: args.profileId }, { email: 'private@invalid' }, { availableResetCredits: '9223372036854775808' }, { resetCreditDetails: [{}] }, { limits: [{}] }]) {
    desktopResponse({ ...value, ...change }); await assert.rejects(invoke('load_profile_quota', args), /desktop response is incompatible/);
  }
  for (const key of Object.keys(value)) { const missing = { ...value }; delete missing[key]; desktopResponse(missing); await assert.rejects(invoke('load_profile_quota', args), /desktop response is incompatible/); }
});

test('combined collections accept only bound committed references and explicit unsupported capabilities', async () => {
  const profileId = '11111111-1111-4111-8111-111111111111', approvalId = '22222222-2222-4222-8222-222222222222';
  const requestId = '33333333-3333-4333-8333-333333333333';
  const quota = { bindingId: '44444444-4444-4444-8444-444444444444', collectionRunId: '55555555-5555-4555-8555-555555555555', observedAtUtc: '2026-10-20T00:00:00.000Z' };
  const activity = { ...quota, collectionRunId: '66666666-6666-4666-8666-666666666666', observedAtUtc: '2026-10-20T00:00:01.000Z' };
  const receipt = { profileId, approvalId, requestId, sourceKind: 'test_fixture', monitoring: 'stopped', accepted: false };
  for (const command of ['begin_profile_collection_read', 'begin_profile_collection_reassociation']) {
    desktopResponse(receipt); assert.deepEqual(await invoke(command, { profileId, approvalId, reviewId: requestId, confirmed: true }), receipt);
  }
  const result = { ...receipt, providerVersion: '0.163.0', identityConfidence: 'provisional', verifiedIdentityKey: null, accepted: true, quota, activity };
  for (const value of [result, { ...result, quota: null }, { ...result, activity: null }, { ...result, quota: null, activity: null, accepted: false }]) {
    desktopResponse(value); assert.deepEqual(await invoke('complete_profile_collection_read', { profileId, requestId }), value);
  }
  for (const value of [
    { ...result, extra: true }, { ...result, quota: undefined }, { ...result, activity: undefined },
    { ...result, accepted: false }, { ...result, quota: null, activity: null },
    { ...result, activity: quota }, { ...result, activity: { ...activity, bindingId: requestId } },
    { ...result, quota: { ...quota, profileId } }, { ...result, quota: { ...quota, observedAtUtc: '2026-02-30T00:00:00.000Z' } },
    { ...result, activity: [] }, { ...result, quota: false }, { ...result, verifiedIdentityKey: profileId },
    { ...result, sourceKind: 'unverified' }, { ...result, monitoring: 'active' }, { ...result, identityConfidence: 'verified' },
  ]) {
    desktopResponse(value); await assert.rejects(invoke('complete_profile_collection_read', { profileId, requestId }), /desktop response is incompatible/);
  }
  desktopResponse({ cancelled: true }); assert.deepEqual(await invoke('cancel_profile_collection_read', { profileId, requestId }), { cancelled: true });
  desktopResponse({ cancelled: true, accepted: true }); await assert.rejects(invoke('cancel_profile_collection_read', { profileId, requestId }), /desktop response is incompatible/);
});

test('monitoring status is exact, bounded, unique and preserves native runtime eligibility', async () => {
  const profile = { profileId: activityRequest.profileId, state: 'stopped', eligible: false, issue: 'sign_in_required', nextReadInSeconds: null, readingActive: false };
  const value = { state: 'stopped', readingRevision: activityRequest.requestId, profiles: [profile] };
  for (const state of ['stopped', 'running', 'paused']) {
    const snapshot = { ...value, state, profiles: [{ ...profile, state, eligible: true, issue: null, nextReadInSeconds: 4320, readingActive: true }] };
    for (const command of ['load_monitoring', 'start_profile_monitoring', 'pause_profile_monitoring']) {
      desktopResponse(snapshot); assert.deepEqual(await invoke(command, { profileId: profile.profileId, approvalId: activityRequest.approvalId }), snapshot);
    }
  }
  for (const change of [
    { extra: true }, { state: 'active' }, { readingRevision: '0' }, { profiles: undefined }, { profiles: [profile, profile] },
    { profiles: Array.from({ length: 11 }, (_, n) => ({ ...profile, profileId: `${String(n).padStart(8, '0')}-1111-4111-8111-111111111111` })) },
    ...[{ extra: true }, { state: 'active' }, { eligible: 'true' }, { readingActive: 1 }, { issue: 'credentials' }, { issue: undefined }, { nextReadInSeconds: -1 }, { nextReadInSeconds: 4321 }, { nextReadInSeconds: 0.5 }, { nextReadInSeconds: '300' }].map(fields => ({ profiles: [{ ...profile, ...fields }] })),
  ]) {
    desktopResponse({ ...value, ...change }); await assert.rejects(invoke('load_monitoring'), /desktop response is incompatible/);
  }
});

test('legacy provider and cache responses accept actual monitoring modes while restore still guarantees stopped', async () => {
  for (const monitoring of ['stopped', 'running', 'paused']) {
    desktopResponse({ ...ready, monitoring }); assert.equal((await invoke('bootstrap')).monitoring, monitoring);
    desktopResponse({ ...activityRequest, monitoring }); assert.equal((await invoke('begin_profile_collection_read', { profileId: activityRequest.profileId, approvalId: activityRequest.approvalId })).monitoring, monitoring);
    desktopResponse({ ...activityPreview, monitoring }); assert.equal((await invoke('complete_profile_activity_read', activityArgs)).monitoring, monitoring);
    const { requestId: _request, approvalId: _approval, ...cached } = activityPreview;
    desktopResponse({ ...cached, monitoring, freshness: 'fresh', dailyState: 'empty', dailyUsageBuckets: [], dailyTruncated: false, dailyChart: { maxTokens: null, bars: [] } });
    assert.equal((await invoke('load_profile_activity', { profileId: activityRequest.profileId })).monitoring, monitoring);
  }
});

test('device scale and saved plan IPC refuse extra fields, invalid scales, and identity data', async () => {
  for (const percent of [90, 100, 110, 125, 150, 175, 200]) {
    desktopResponse({ percent }); assert.deepEqual(await invoke('get_display_scale'), { percent });
  }
  for (const value of [{ percent: '125' }, { percent: 0 }, { percent: 125.5 }, { percent: 125, path: '/tmp/arbitrary' }, null]) {
    desktopResponse(value); await assert.rejects(invoke('set_display_scale', { percent: 125 }), /desktop response is incompatible/);
  }
  const args = { profileId: '11111111-1111-4111-8111-111111111111', bindingId: '22222222-2222-4222-8222-222222222222' };
  for (const plan of [null, 'plus', 'future-plan']) {
    const value = { ...args, plan }; desktopResponse(value); assert.deepEqual(await invoke('load_profile_plan', args), value);
  }
  for (const fields of [{ profileId: 'bad' }, { bindingId: 'bad' }, { plan: undefined }, { plan: '' }, { plan: 'a'.repeat(65) }, { plan: 'plus\n' }, { email: 'synthetic@example.invalid' }, { token: 'synthetic-never-a-real-token' }]) {
    desktopResponse({ ...args, plan: 'plus', ...fields }); await assert.rejects(invoke('load_profile_plan', args), /desktop response is incompatible/);
  }
});

test('history IPC keeps profiles, ordering, cursor bounds and missing windows exact', async () => {
  const profileId = '11111111-1111-4111-8111-111111111111';
  const entry = { profileId, bindingId: '22222222-2222-4222-8222-222222222222', collectionRunId: '33333333-3333-4333-8333-333333333333', observedAtUtc: '2026-10-21T00:00:00.000Z', sourceKind: 'test_fixture', providerVersion: '0.163.0', limits: [{ limitId: 'codex', limitName: null, primary: { usedPercent: 0, derivedRemainingPercent: 100, windowDurationMins: 300, resetsAtUnix: null, label: 'Five-hour' }, secondary: null }] };
  const page = { profileId, entries: [entry], nextBeforeRunId: null };
  const args = { profileId, beforeRunId: null };
  for (const value of [page, { ...page, entries: [] }, { ...page, entries: [{ ...entry, limits: [] }] }, { ...page, entries: [{ ...entry, limits: [{ ...entry.limits[0], primary: null }] }] }]) {
    desktopResponse(value); assert.deepEqual(await invoke('load_profile_quota_history', args), value);
  }
  for (const value of [
    { ...page, extra: true }, { ...page, nextBeforeRunId: entry.collectionRunId }, { ...page, entries: Array(21).fill(entry) },
    { ...page, entries: [entry, entry] }, { ...page, entries: [{ ...entry, profileId: entry.bindingId }] },
    ...[{ token: 'synthetic-secret' }, { sourceKind: ['test_fixture'] }, { observedAtUtc: '2026-02-30T00:00:00.000Z' }, { bindingId: 'bad' }, { providerVersion: '' }, { freshness: 'fresh' }, { verifiedIdentityKey: profileId }].map(fields => ({ ...page, entries: [{ ...entry, ...fields }] })),
    { ...page, entries: [{ ...entry, limits: [{ ...entry.limits[0], primary: { ...entry.limits[0].primary, resetState: 'confirmed' } }] }] },
    { ...page, entries: [entry, { ...entry, collectionRunId: '44444444-4444-4444-8444-444444444444' }] },
  ]) {
    desktopResponse(value); await assert.rejects(invoke('load_profile_quota_history', args), /desktop response is incompatible/);
  }
});

test('heatmap IPC keeps missing, zero and exact aggregate counts distinct and rejects malformed cells', async () => {
  const profileId = '11111111-1111-4111-8111-111111111111';
  const args = { profileId, mode: 'daily', periodDays: 90 };
  const cell = { startDate: '2026-10-20', endDate: '2026-10-20', tokens: '9007199254740993', level: 4, knownDays: 1, expectedDays: 1, observedAtUtc: '2026-10-21T00:00:00.000Z', freshness: 'unknown' };
  const value = { profileId, bindingId: '22222222-2222-4222-8222-222222222222', collectionRunId: '33333333-3333-4333-8333-333333333333', sourceKind: 'test_fixture', scope: 'account_activity_with_unknown_application_scope', coverage: 'unknown', observedAtUtc: cell.observedAtUtc, mode: 'daily', periodStart: '2026-10-20', periodEnd: '2026-10-21', scaleMaximum: cell.tokens, cells: [cell] };
  for (const tokens of [null, '0', '9007199254740993', '11670000']) {
    const entry = { ...cell, tokens, level: tokens === null || tokens === '0' ? 0 : 4, knownDays: tokens === null ? 0 : 1, observedAtUtc: tokens === null ? null : cell.observedAtUtc };
    desktopResponse({ ...value, cells: [entry] });
    assert.equal((await invoke('load_profile_heatmap', args)).cells[0].tokens, tokens);
  }
  for (const invalid of [{ ...cell, tokens: 9007199254740992 }, { ...cell, tokens: '01' }, { ...cell, tokens: '-1' }, { ...cell, tokens: null }, { ...cell, level: 5 }, { ...cell, startDate: '2026-02-30' }, { ...cell, knownDays: 2 }, { ...cell, extra: true }]) {
    desktopResponse({ ...value, cells: [invalid] });
    await assert.rejects(invoke('load_profile_heatmap', args), /desktop response is incompatible/);
  }
  desktopResponse({ ...value, cells: [cell, cell] });
  await assert.rejects(invoke('load_profile_heatmap', args), /desktop response is incompatible/);
  desktopResponse({ ...value, mode: 'monthly' });
  await assert.rejects(invoke('load_profile_heatmap', args), /desktop response is incompatible/);
});

test('reset notification preferences accept only typed bounded records and keep OS dispatch Rust-only', async () => {
  const preferences = { enabled: false, timeZone: 'America/New_York', quietEnabled: true, quietStart: '22:00', quietEnd: '08:00', profileOverrides: [{ profileId: '11111111-1111-4111-8111-111111111111', enabled: true }] };
  for (const command of ['load_reset_notification_preferences', 'save_reset_notification_preferences']) {
    const args = command.startsWith('save') ? { preferences } : undefined;
    desktopResponse(preferences); assert.deepEqual(await invoke(command, args), preferences);
    for (const change of [{ enabled: ['false'] }, { timeZone: '../private\nzone' }, { quietEnabled: 1 }, { quietStart: '24:00' }, { quietEnd: '22:00' }, { profileOverrides: [preferences.profileOverrides[0], preferences.profileOverrides[0]] }, { profileOverrides: [{ ...preferences.profileOverrides[0], path: '/external' }] }, { secret: 'must never reach UI' }]) {
      desktopResponse({ ...preferences, ...change }); await assert.rejects(invoke(command, args), /desktop response is incompatible/);
    }
    for (const field of Object.keys(preferences)) { const missing = { ...preferences }; delete missing[field]; desktopResponse(missing); await assert.rejects(invoke(command, args), /desktop response is incompatible/); }
  }
});


test('combined notification preferences reject malformed usage settings and private fields', async () => {
  const reset = { enabled: false, timeZone: 'Etc/UTC', quietEnabled: true, quietStart: '22:00', quietEnd: '08:00', profileOverrides: [] };
  const usage = { enabled: true, profileOverrides: [{ profileId: '11111111-1111-4111-8111-111111111111', enabled: false }] };
  const preferences = { reset, usage, authentication: { enabled: false, profileOverrides: [] }, collectionFailure: { enabled: false, profileOverrides: [] } };
  for (const command of ['load_notification_preferences', 'save_notification_preferences']) {
    const args = command.startsWith('save') ? { preferences } : undefined;
    desktopResponse(preferences); assert.deepEqual(await invoke(command, args), preferences);
    for (const invalid of [
      { reset }, { usage }, { ...preferences, authentication: null }, { ...preferences, collectionFailure: { enabled: true, profileOverrides: [], token: 'private' } }, { ...preferences, secret: 'private' },
      { ...preferences, usage: { ...usage, enabled: 'true' } },
      { ...preferences, usage: { ...usage, threshold: 85 } },
      { ...preferences, usage: { ...usage, profileOverrides: [...usage.profileOverrides, ...usage.profileOverrides] } },
      { ...preferences, usage: { ...usage, profileOverrides: [{ profileId: '../unsafe', enabled: true }] } },
      { ...preferences, reset: { ...reset, quietStart: '24:00' } },
    ]) {
      desktopResponse(invalid); await assert.rejects(invoke(command, args), /desktop response is incompatible/);
    }
  }
});
