// postinstall script: downloads the correct loci binary for the current platform.
// Called automatically by npm after `npm install loci`.

const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// ── Platform detection ───────────────────────────────────────────────
const platform = process.platform;  // 'win32' | 'darwin' | 'linux'
const arch = process.arch;          // 'x64'  | 'arm64'

const TARGET_MAP = {
    'win32-x64':   { artifact: 'loci-windows-x86_64.exe',   binary: 'loci.exe' },
    'darwin-x64':  { artifact: 'loci-macos-x86_64',         binary: 'loci' },
    'darwin-arm64':{ artifact: 'loci-macos-aarch64',        binary: 'loci' },
    'linux-x64':   { artifact: 'loci-linux-x86_64',         binary: 'loci' },
    'linux-arm64': { artifact: 'loci-linux-aarch64',        binary: 'loci' },
};

const key = `${platform}-${arch}`;
const target = TARGET_MAP[key];

if (!target) {
    console.error(`loci: unsupported platform ${key}`);
    console.error('loci supports: win32-x64, darwin-x64, darwin-arm64, linux-x64, linux-arm64');
    process.exit(1);
}

// ── Version & URL ────────────────────────────────────────────────────
// Uses npm_package_version env var (set by npm during postinstall),
// falling back to reading package.json directly.
const VERSION = 'v' + (process.env.npm_package_version || require('./package.json').version);
const RELEASE_URL = `https://github.com/Yaemikoreal/CliLoci/releases/download/${VERSION}/${target.artifact}`;

const binDir = __dirname; // npm/ directory
const destPath = path.join(binDir, target.binary);

// ── Skip if already installed ────────────────────────────────────────
if (fs.existsSync(destPath)) {
    console.log(`loci: binary already installed at ${destPath}`);
    process.exit(0);
}

// ── Download ─────────────────────────────────────────────────────────
console.log(`loci: downloading ${RELEASE_URL} ...`);

https.get(RELEASE_URL, (res) => {
    if (res.statusCode === 302 || res.statusCode === 301) {
        // Follow redirect
        https.get(res.headers.location, onResponse).on('error', onError);
        return;
    }
    onResponse(res);
}).on('error', onError);

function onResponse(res) {
    if (res.statusCode !== 200) {
        console.error(`loci: download failed (HTTP ${res.statusCode})`);
        console.error(`loci: expected binary at ${RELEASE_URL}`);
        console.error('loci: please check that the release exists on GitHub.');
        process.exit(1);
    }

    const file = fs.createWriteStream(destPath, { mode: 0o755 });
    res.pipe(file);

    file.on('finish', () => {
        file.close();
        // Ensure executable permissions on Unix
        if (process.platform !== 'win32') {
            try { execSync(`chmod +x "${destPath}"`); } catch (_) {}
        }
        console.log(`loci: installed to ${destPath}`);
    });

    file.on('error', (err) => {
        fs.unlink(destPath, () => {});
        console.error(`loci: write error — ${err.message}`);
        process.exit(1);
    });
}

function onError(err) {
    console.error(`loci: download error — ${err.message}`);
    console.error('loci: if you are behind a proxy, set HTTPS_PROXY environment variable.');
    process.exit(1);
}
