// User intentions are handled by the existing per-account provider controls.
export type AccountAction = { kind: 'add' } | { kind: 'signin' | 'refresh' | 'usage' | 'manage'; profileId: string };
export type AccountActionRequest = AccountAction & { sequence: number };
