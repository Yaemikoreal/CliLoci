#!/usr/bin/env node
// Wrapper script: invokes the downloaded loci binary.
// npm links this as the `loci` command; it spawns the real binary.

const { spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const binDir = path.join(__dirname, '..');
const isWin = process.platform === 'win32';
const binaryName = isWin ? 'loci.exe' : 'loci';
const binaryPath = path.join(binDir, binaryName);

if (!fs.existsSync(binaryPath)) {
    console.error(
        'loci: binary not found at %s.\n' +
        'Run "npm rebuild loci" or reinstall the package.',
        binaryPath,
    );
    process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), { stdio: 'inherit' });
process.exit(result.status ?? result.signal != null ? 1 : 0);
