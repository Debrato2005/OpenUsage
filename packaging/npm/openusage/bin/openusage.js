#!/usr/bin/env node
// Runs the native openusage binary from the package for this platform (OU-DIST-001, OU-DIST-003).
// It downloads nothing and runs no install script. A missing package prints the fix.
'use strict';
const { spawnSync } = require('node:child_process');
const { dirname, join } = require('node:path');

const PACKAGES = { 'linux x64': '@openusage/linux-x64' };
const name = PACKAGES[`${process.platform} ${process.arch}`];
if (!name) {
  console.error(`openusage: ${process.platform} ${process.arch} has no package in this release (install_incomplete)`);
  process.exit(11);
}
let directory;
try {
  directory = dirname(require.resolve(`${name}/package.json`));
} catch {
  console.error(`openusage: The ${name} package is missing. Reinstall with: npm install -g openusage (install_incomplete)`);
  process.exit(11);
}
const binary = join(directory, 'bin', process.platform === 'win32' ? 'openusage.exe' : 'openusage');
const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
if (result.error) {
  console.error(`openusage: ${binary} could not start. Reinstall with: npm install -g openusage (install_incomplete)`);
  process.exit(11);
}
if (result.signal) process.kill(process.pid, result.signal);
process.exit(result.status ?? 1);
