// Realistic synthetic local usage for renderer tests. It is not provider or log data.
export function localUsage(days = 30, end = '2026-11-08') {
  const endDate = new Date(`${end}T00:00:00Z`);
  const buckets: { startDate: string; tool: 'claude' | 'codex'; totalTokens: string }[] = [];
  let claude = 0n, codex = 0n;
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(endDate); d.setUTCDate(d.getUTCDate() - i);
    const date = d.toISOString().slice(0, 10);
    if (i % 7 === 3) continue;
    const c = BigInt(2_000_000 + ((i * 7919) % 9_000_000)); claude += c; buckets.push({ startDate: date, tool: 'claude', totalTokens: String(c) });
    if (i % 3 !== 0) { const x = BigInt(800_000 + ((i * 104729) % 4_000_000)); codex += x; buckets.push({ startDate: date, tool: 'codex', totalTokens: String(x) }); }
  }
  const start = new Date(endDate); start.setUTCDate(start.getUTCDate() - (days - 1));
  const split = (total: bigint) => { const input = total / 10n, output = total / 20n, write = total / 25n; return { inputTokens: String(input), outputTokens: String(output), cacheWriteTokens: String(write), cacheReadTokens: String(total - input - output - write), totalTokens: String(total) }; };
  return {
    startDate: start.toISOString().slice(0, 10), endDate: end, timeZone: 'Asia/Kolkata', scannerVersions: ['20.0.48'],
    tools: [
      { tool: 'claude', ...split(claude), estimatedValueUsdMicros: '184320000', estimateComplete: false, logsGone: true },
      { tool: 'codex', ...split(codex), estimatedValueUsdMicros: '61875000', estimateComplete: true, logsGone: false },
    ],
    models: [
      { tool: 'claude', model: 'claude-sonnet-4-5-20250929', totalTokens: String(claude * 7n / 10n), estimatedValueUsdMicros: '120000000' },
      { tool: 'codex', model: 'gpt-5-codex', totalTokens: String(codex), estimatedValueUsdMicros: '61875000' },
      { tool: 'claude', model: 'claude-opus-4-1-20250805', totalTokens: String(claude * 3n / 10n - 150n), estimatedValueUsdMicros: '64320000' },
      { tool: 'claude', model: 'totally-unknown-model-x', totalTokens: '150', estimatedValueUsdMicros: null },
    ],
    buckets,
    differences: [{ date: '2026-10-20', tool: 'claude', model: 'claude-sonnet-4-5-20250929', savedTotalTokens: '5120000', newTotalTokens: '0', scanAtUtc: '2026-11-08T08:00:00.000Z' }],
    activeDays: 26, longestStreakDays: 6, busiestDay: { date: '2026-10-10', totalTokens: '6912345' },
    // Claude Code reports a project and a start, and Codex reports reasoning (SC-021).
    sessions: [
      { key: '2026/11/08/rollout-2026-11-08T09-00-00-22222222-bbbb-4bbb-8bbb-222222222222', tool: 'codex', project: null, startUtc: null, endUtc: '2026-11-08T04:00:05.000Z', totalTokens: '1250000', reasoningTokens: '48000', estimatedValueUsdMicros: '2100000' },
      { key: '11111111-aaaa-4aaa-8aaa-111111111111', tool: 'claude', project: '-home-dev-alpha', startUtc: '2026-11-07T09:30:00.000Z', endUtc: '2026-11-07T11:45:00.000Z', totalTokens: '3400000', reasoningTokens: null, estimatedValueUsdMicros: '5120000' },
      { key: '33333333-cccc-4ccc-8ccc-333333333333', tool: 'claude', project: '-home-dev-beta', startUtc: '2026-11-06T13:00:00.000Z', endUtc: '2026-11-06T13:20:00.000Z', totalTokens: '150', reasoningTokens: null, estimatedValueUsdMicros: null },
    ],
    projects: [
      { tool: 'claude', project: '-home-dev-alpha', sessions: 1, totalTokens: '3400000' },
      { tool: 'claude', project: '-home-dev-beta', sessions: 1, totalTokens: '150' },
    ],
  };
}
