#!/usr/bin/env node
// Bump the application version across every file that hard-codes it.
//
// Usage:
//   node scripts/bump-version.mjs <x.y.z>
//
// Updates (and fails loudly if any expected version string is missing, so a
// silent miss can never ship a half-bumped release):
//   - src-tauri/tauri.conf.json   ("version")
//   - frontend/package.json       ("version")
//   - src-tauri/Cargo.toml        ([package] version)
//   - backend-rust/Cargo.toml     ([package] version)
//   - src-tauri/Cargo.lock        (video-manager-tauri, video-manager-backend)
//   - backend-rust/Cargo.lock     (video-manager-backend)
//
// Pure Node, no dependencies — runs on any CI runner with Node installed.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+([-+].+)?$/.test(version)) {
  console.error(`Usage: node scripts/bump-version.mjs <x.y.z>  (got: ${version ?? '<none>'})`);
  process.exit(1);
}

/** Read a file, apply `fn`, and write it back only if it changed. */
function patch(relPath, fn) {
  const abs = join(repoRoot, relPath);
  const before = readFileSync(abs, 'utf8');
  const after = fn(before, relPath);
  if (after === before) {
    console.log(`  = ${relPath} (already ${version})`);
    return;
  }
  writeFileSync(abs, after);
  console.log(`  ✓ ${relPath}`);
}

/** Replace the top-level JSON "version" field. */
function jsonVersion(content, relPath) {
  const re = /("version"\s*:\s*")[^"]+(")/;
  if (!re.test(content)) throw new Error(`No "version" field found in ${relPath}`);
  return content.replace(re, `$1${version}$2`);
}

/** Replace the `version = "..."` line inside the [package] section of a Cargo.toml. */
function cargoTomlVersion(content, relPath) {
  const lines = content.split('\n');
  let inPackage = false;
  let done = false;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const header = line.match(/^\s*\[([^\]]+)\]\s*$/);
    if (header) {
      inPackage = header[1] === 'package';
      continue;
    }
    if (inPackage && /^\s*version\s*=\s*"/.test(line)) {
      lines[i] = line.replace(/("|')[^"']+("|')/, `"${version}"`);
      done = true;
      break;
    }
  }
  if (!done) throw new Error(`No [package] version found in ${relPath}`);
  return lines.join('\n');
}

/** Replace the version of one or more local packages inside a Cargo.lock. */
function cargoLockVersion(pkgNames) {
  return (content, relPath) => {
    let out = content;
    for (const name of pkgNames) {
      // Lock files may use CRLF (Windows) or LF — match either. In a Cargo.lock
      // the `version` line always immediately follows the `name` line.
      const re = new RegExp(`(name = "${name}"\\r?\\nversion = ")[^"]+(")`);
      if (!re.test(out)) throw new Error(`Package "${name}" not found in ${relPath}`);
      out = out.replace(re, `$1${version}$2`);
    }
    return out;
  };
}

console.log(`Bumping version → ${version}`);
patch('src-tauri/tauri.conf.json', jsonVersion);
patch('frontend/package.json', jsonVersion);
patch('src-tauri/Cargo.toml', cargoTomlVersion);
patch('backend-rust/Cargo.toml', cargoTomlVersion);
patch('src-tauri/Cargo.lock', cargoLockVersion(['video-manager-tauri', 'video-manager-backend']));
patch('backend-rust/Cargo.lock', cargoLockVersion(['video-manager-backend']));
console.log('Done.');
