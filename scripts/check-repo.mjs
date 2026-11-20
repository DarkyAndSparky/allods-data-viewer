#!/usr/bin/env node
// Repository hygiene check. Fails when a tracked file looks like game data or a secret.
// The project must never contain game assets (see CONTRIBUTING.md). Run in CI and before a PR:
//   node scripts/check-repo.mjs
import { execFileSync } from 'node:child_process';
import { readdirSync, statSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const FORBIDDEN_EXT = new Set([
  'pak', 'loc', 'fsb', 'bsb', 'ogv', 'dds', 'bin', 'ttf', 'otf', 'woff', 'woff2',
  'pem', 'key', 'p12', 'pfx', 'env',
]);
const ALLOWED_PATHS = new Set(['web/public/fonts/README.md']);
const MAX_BYTES = 2_000_000; // icons and docs only; anything bigger needs a reason

export function problems(files, sizeOf = (f) => statSync(f).size) {
  const out = [];
  for (const file of files) {
    if (ALLOWED_PATHS.has(file)) continue;
    const base = file.split('/').pop() ?? file;
    const ext = base.includes('.') ? base.split('.').pop().toLowerCase() : '';
    if (FORBIDDEN_EXT.has(ext) || base === '.env' || base.startsWith('.env.')) {
      out.push(`${file}: forbidden file type (game data, font or secret)`);
      continue;
    }
    if (file.startsWith('samples/') || file.startsWith('extracted/')) {
      out.push(`${file}: samples and extracted data must stay outside the repository`);
      continue;
    }
    let size = 0;
    try {
      size = sizeOf(file);
    } catch {
      continue; // deleted in the working tree but still indexed
    }
    if (size > MAX_BYTES) out.push(`${file}: ${size} bytes is larger than ${MAX_BYTES}`);
  }
  return out;
}

const SKIP_DIRS = new Set(['.git', 'target', 'node_modules', 'dist']);

/** Fallback for a source archive without `.git`: walks the working tree. */
function walk(dir = '.', prefix = '') {
  const files = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      if (!SKIP_DIRS.has(entry.name)) files.push(...walk(`${dir}/${entry.name}`, `${prefix}${entry.name}/`));
    } else {
      files.push(`${prefix}${entry.name}`);
    }
  }
  return files;
}

function trackedFiles() {
  try {
    execFileSync('git', ['rev-parse', '--is-inside-work-tree'], { stdio: 'ignore' });
  } catch {
    console.error('check-repo: not a git repository, checking the working tree instead');
    return walk();
  }
  const out = execFileSync('git', ['ls-files', '-z'], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  return out.split('\0').filter(Boolean);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const found = problems(trackedFiles());
  if (found.length) {
    console.error('check-repo: problems found\n' + found.map((p) => '  ' + p).join('\n'));
    process.exitCode = 1;
  } else {
    console.log('check-repo: ok');
  }
}
