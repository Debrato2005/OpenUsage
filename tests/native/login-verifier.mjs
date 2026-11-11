import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';

const historicalDigest = 'a2e4a64707081a9943f3c04f8237c2b16bea673e974e96393d6a8a7d9357f975';

export function loginVerifierDigest(value = process.env.OPENUSAGE_LOGIN_VERIFIER_SHA256 ?? historicalDigest) {
  assert.match(value, /^[0-9a-f]{64}$/);
  assert.equal(value.length, 64);
  return value;
}

export async function runLoginVerifier(executable, digest, args, options) {
  loginVerifierDigest(digest);
  assert.equal(createHash('sha256').update(await readFile(executable)).digest('hex'), digest);
  return execFileSync(executable, args, options);
}
