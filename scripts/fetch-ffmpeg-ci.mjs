#!/usr/bin/env node
// Fetch FFmpeg + FFprobe sidecar binaries for the Tauri bundle, in CI.
//
// Uses the `ffmpeg-ffprobe-static` npm package (Descript fork) which ships
// *both* static binaries for the host platform/arch (darwin-arm64/x64,
// linux-x64, win32-x64). Every release runner builds host-native, so the host
// arch always matches the Rust target — no cross-arch sourcing needed.
//
// Output: src-tauri/binaries/{ffmpeg,ffprobe}-<target-triple>[.exe]
//
// Usage:
//   node scripts/fetch-ffmpeg-ci.mjs                 # auto-detect host triple
//   node scripts/fetch-ffmpeg-ci.mjs <target-triple> # explicit
//
// For local dev the per-OS scripts (fetch-ffmpeg.sh / .ps1) still work; this
// one exists so CI has a single uniform path that also covers macOS.

import { execSync } from 'node:child_process';
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  statSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const PACKAGE = 'ffmpeg-ffprobe-static@6.1.1';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const binDir = join(repoRoot, 'src-tauri', 'binaries');

function hostTriple() {
  try {
    const out = execSync('rustc -vV', { encoding: 'utf8' });
    const m = out.match(/^host:\s*(.+)$/m);
    if (m) return m[1].trim();
  } catch {
    /* rustc not on PATH — fall through */
  }
  throw new Error('Could not detect host triple; pass one as an argument.');
}

const triple = process.argv[2] || hostTriple();
const isWindows = triple.includes('windows');
const exe = isWindows ? '.exe' : '';
console.log(`Target triple: ${triple}`);

// Install the package into the repo-root node_modules (git-ignored). The
// postinstall downloads the host binaries into the package directory.
console.log(`Installing ${PACKAGE} ...`);
execSync(`npm install --no-save --no-package-lock ${PACKAGE}`, {
  cwd: repoRoot,
  stdio: 'inherit',
});

const pkgDir = join(repoRoot, 'node_modules', 'ffmpeg-ffprobe-static');
if (!existsSync(pkgDir)) {
  throw new Error(`Package directory not found: ${pkgDir}`);
}

/** Recursively find the first file whose basename matches `name`. */
function findBinary(root, name) {
  for (const entry of readdirSync(root)) {
    const full = join(root, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      const found = findBinary(full, name);
      if (found) return found;
    } else if (entry === name) {
      return full;
    }
  }
  return null;
}

mkdirSync(binDir, { recursive: true });

for (const tool of ['ffmpeg', 'ffprobe']) {
  const srcName = `${tool}${exe}`;
  const src = findBinary(pkgDir, srcName);
  if (!src) {
    throw new Error(`Could not locate ${srcName} inside ${pkgDir}`);
  }
  const dest = join(binDir, `${tool}-${triple}${exe}`);
  copyFileSync(src, dest);
  if (!isWindows) chmodSync(dest, 0o755);
  console.log(`  ✓ ${src} -> ${dest}`);
}

console.log('Sidecars ready.');
