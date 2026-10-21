// Verify published release evidence. This script never executes the Codex artifact.
const assert = require('node:assert/strict');
const { readFile, writeFile } = require('node:fs/promises');
const { createReadStream } = require('node:fs');
const { createHash, X509Certificate } = require('node:crypto');
const { join } = require('node:path');

async function sha256(path) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest('hex');
}

(async () => {
  const [directory, npmModules] = process.argv.slice(2);
  assert.ok(directory && npmModules, 'Supply the artifact folder and the existing npm module folder.');
  const sigstore = require(join(npmModules, 'sigstore'));
  const release = JSON.parse(await readFile(join(directory, 'codex-release-0.163.0.json'), 'utf8'));
  assert.equal(release.tag_name, 'rust-v0.163.0');
  assert.equal(release.draft, false);
  assert.equal(release.prerelease, false);
  const asset = (name) => release.assets.find((entry) => entry.name === name);
  const archiveSha256 = await sha256(join(directory, 'codex-0.163.0-x86_64-unknown-linux-musl.tar.gz'));
  assert.equal(`sha256:${archiveSha256}`, asset('codex-x86_64-unknown-linux-musl.tar.gz').digest);
  const signaturePath = join(directory, 'codex-0.163.0-x86_64-unknown-linux-musl.sigstore');
  assert.equal(`sha256:${await sha256(signaturePath)}`, asset('codex-x86_64-unknown-linux-musl.sigstore').digest);
  const legacy = JSON.parse(await readFile(signaturePath, 'utf8'));
  const payload = legacy.rekorBundle.Payload;
  const body = JSON.parse(Buffer.from(payload.body, 'base64'));
  assert.equal(body.kind, 'hashedrekord');
  assert.equal(body.apiVersion, '0.0.1');
  assert.equal(body.spec.data.hash.algorithm, 'sha256');
  assert.equal(body.spec.signature.content, legacy.base64Signature);
  assert.equal(body.spec.signature.publicKey.content, legacy.cert);
  const executablePath = join(directory, 'codex-0.163.0-source-verified', 'codex');
  const executableSha256 = await sha256(executablePath);
  assert.equal(executableSha256, body.spec.data.hash.value);
  const certificate = new X509Certificate(Buffer.from(legacy.cert, 'base64'));
  const bundle = {
    mediaType: 'application/vnd.dev.sigstore.bundle+json;version=0.1',
    verificationMaterial: {
      x509CertificateChain: { certificates: [{ rawBytes: certificate.raw.toString('base64') }] },
      tlogEntries: [{
        logIndex: String(payload.logIndex),
        logId: { keyId: Buffer.from(payload.logID, 'hex').toString('base64') },
        kindVersion: { kind: body.kind, version: body.apiVersion },
        integratedTime: String(payload.integratedTime),
        inclusionPromise: { signedEntryTimestamp: legacy.rekorBundle.SignedEntryTimestamp },
        canonicalizedBody: payload.body,
      }],
    },
    messageSignature: {
      messageDigest: { algorithm: 'SHA2_256', digest: Buffer.from(executableSha256, 'hex').toString('base64') },
      signature: legacy.base64Signature,
    },
  };
  const identity = 'https://github.com/openai/codex/.github/workflows/rust-release.yml@refs/tags/rust-v0.163.0';
  const issuer = 'https://token.actions.githubusercontent.com';
  const options = {
    certificateIdentityURI: identity,
    certificateIssuer: issuer,
    tufMirrorURL: 'https://tuf-repo-cdn.sigstore.dev',
    tufCachePath: join(directory, 'sigstore-tuf-cache'),
    ctLogThreshold: 1,
    tlogThreshold: 1,
  };
  const verifier = await sigstore.createVerifier(options);
  const artifact = await readFile(executablePath);
  verifier.verify(bundle, artifact);
  artifact[0] ^= 1;
  assert.throws(() => verifier.verify(bundle, artifact), /signature verification failed/);
  artifact[0] ^= 1;
  const wrongIdentity = await sigstore.createVerifier({ ...options, certificateIdentityURI: identity.replace('/openai/', '/untrusted-owner/') });
  assert.throws(() => wrongIdentity.verify(bundle, artifact));
  await writeFile(join(directory, 'codex-0.163.0-converted-sigstore-bundle.json'), JSON.stringify(bundle, null, 2) + '\n');
  const result = {
    result: 'passed', archiveSha256, executableSha256,
    verifier: { name: 'sigstore', version: require(join(npmModules, 'sigstore/package.json')).version },
    signingIdentity: identity, oidcIssuer: issuer,
    certificateSha256: createHash('sha256').update(certificate.raw).digest('hex'),
    certificateTransparencyThreshold: 1, rekorThreshold: 1,
    rekorLogId: payload.logID, rekorLogIndex: String(payload.logIndex), rekorIntegratedTime: payload.integratedTime,
    tufMirror: options.tufMirrorURL, artifactExecuted: false,
    tamperedArtifactRejected: true, incorrectSigningIdentityRejected: true,
  };
  await writeFile(join(directory, 'codex-0.163.0-official-verification.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result));
})().catch((error) => { console.error(error); process.exitCode = 1; });
