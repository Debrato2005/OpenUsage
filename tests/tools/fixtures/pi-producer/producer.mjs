import fs from 'node:fs';
import path from 'node:path';
import { syncBuiltinESMExports } from 'node:module';

const root = process.cwd();
const folder = path.join(root, 'sessions');
const statePath = path.join(root, 'writer-state.json');
let clock = Date.parse('2026-05-28T20:26:40.000Z');
const SystemDate = Date;
globalThis.Date = class extends SystemDate {
  constructor(...args) { super(...(args.length ? args : [clock])); }
  static now() { return clock; }
};

// The schedule pauses after an unchanged producer write reaches the filesystem.
const writeFile = fs.writeFileSync;
const write = fs.writeSync;
const read = fs.readSync;
let paused = false;
fs.writeFileSync = function (file, data, ...args) {
  const result = writeFile(file, data, ...args);
  const mode = process.env.PI_PRODUCER_PAUSE;
  if (!paused && typeof file === 'number' && typeof data === 'string' && mode) {
    const record = JSON.parse(data);
    if ((mode === 'header' && record.type === 'session') || (mode === 'usage' && record.type === 'usage')) {
      paused = true;
      write(1, JSON.stringify({ paused: mode }) + '\n');
      if (read(0, Buffer.alloc(1), 0, 1, null) !== 1) throw new Error('Schedule ended before resume');
    }
  }
  return result;
};
syncBuiltinESMExports();

const { SessionManager } = await import('/producer/packages/coding-agent/src/core/session-manager.ts');
const usage = total => ({ input: Math.max(total - 5, 0), output: Math.min(total, 5),
  cacheRead: 0, cacheWrite: 0, totalTokens: total,
  cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } });
const assistant = total => ({ role: 'assistant', content: [{ type: 'text', text: 'Synthetic.' }],
  api: 'openai-responses', provider: 'openai', model: 'gpt-5', timestamp: clock,
  stopReason: 'stop', usage: usage(total) });
const user = () => ({ role: 'user', content: 'Synthetic.', timestamp: clock });
const create = () => SessionManager.create(path.join(root, 'project'), folder, { id: 'root' });
const open = () => SessionManager.open(JSON.parse(fs.readFileSync(statePath, 'utf8')).file, folder);
const action = process.argv[2];
let session;
if (['append', 'compact', 'fork', 'branch', 'branched', 'optional_summary'].includes(action)) {
  session = open();
} else {
  session = create();
  session.appendModelChange('openai', 'gpt-5');
  session.appendThinkingLevelChange('off');
}
switch (action) {
  case 'setup_only': break;
  case 'user_only': session.appendMessage(user()); break;
  case 'seed': session.appendMessage(user()); session.appendMessage(assistant(120)); break;
  case 'zero': session.appendMessage(assistant(0)); break;
  case 'total_only': {
    const message = assistant(162);
    message.usage = { totalTokens: 162 };
    session.appendMessage(message);
    break;
  }
  case 'flush': session.appendUsage('synthetic-preflight', 'openai', 'gpt-5', usage(17));
    session.appendMessage(assistant(120)); break;
  case 'append': clock += 1000; session.appendMessage(assistant(17)); break;
  case 'compact': {
    const first = session.getEntries().find(e => e.type === 'message').id;
    session.appendCompaction('Synthetic.', first, 999, undefined, false, usage(49));
    session.branchWithSummary(first, 'Synthetic.', undefined, false, usage(23));
    session.appendUsage('synthetic-cache', 'openai', 'gpt-5', usage(11));
    break;
  }
  case 'optional_summary': {
    const first = session.getEntries().find(e => e.type === 'message').id;
    session.appendCompaction('Synthetic.', first, 999);
    session.branchWithSummary(first, 'Synthetic.');
    break;
  }
  case 'branch':
  case 'branched': {
    const start = session.getLeafId();
    const older = session.appendMessage(assistant(17));
    session.branch(start);
    session.appendMessage(assistant(23));
    if (action === 'branched') {
      session.createBranchedSession(older);
      session.appendMessage(assistant(31));
    }
    break;
  }
  case 'fork':
    session = SessionManager.forkFrom(session.getSessionFile(), path.join(root, 'other-project'), folder, { id: 'fork' });
    session.appendMessage(assistant(31));
    break;
  default: throw new Error('Unsupported fixture action');
}
const state = { file: session.getSessionFile(), id: session.getSessionId(), entries: session.getEntries().length };
writeFile(statePath, JSON.stringify(state));
write(1, JSON.stringify(state) + '\n');
