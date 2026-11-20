#!/usr/bin/env node
// One place for the project version (the pattern comes from das-forge-core).
//
//   node scripts/version.mjs get         prints the version
//   node scripts/version.mjs set 0.1.38  writes it to every file that repeats it
//   node scripts/version.mjs check       exits 1 when the files disagree
//
// Only the project's own entries are touched: in Cargo.lock just the `adv-*` packages, so a
// third-party crate that happens to have the same number is never rewritten.
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const SEMVER = /^\d+\.\d+\.\d+$/;
const LOCK_PACKAGES = ['adv-cli', 'adv-core', 'adv-desktop', 'adv-session'];

/** Every place that holds the version, as [file, read(text) -> string[], write(text, v) -> text]. */
export const targets = [
  {
    file: 'Cargo.toml',
    read: (s) => [
      /^\[workspace\.package\][^[]*?^version = "([^"]+)"/m.exec(s)?.[1],
      ...[...s.matchAll(/^adv-[a-z]+ = \{ path = "[^"]+", version = "([^"]+)" \}/gm)].map((m) => m[1]),
    ],
    write: (s, v) =>
      s
        .replace(/(^\[workspace\.package\][^[]*?^version = ")[^"]+(")/m, `$1${v}$2`)
        .replace(/(^adv-[a-z]+ = \{ path = "[^"]+", version = ")[^"]+(" \})/gm, `$1${v}$2`),
  },
  {
    file: 'Cargo.lock',
    read: (s) =>
      LOCK_PACKAGES.map((n) => new RegExp(`name = "${n}"\\nversion = "([^"]+)"`).exec(s)?.[1]),
    write: (s, v) =>
      LOCK_PACKAGES.reduce(
        (acc, n) => acc.replace(new RegExp(`(name = "${n}"\\nversion = ")[^"]+(")`), `$1${v}$2`),
        s,
      ),
  },
  {
    file: 'web/package.json',
    read: (s) => [JSON.parse(s).version],
    write: (s, v) => s.replace(/("version": ")[^"]+(")/, `$1${v}$2`),
  },
  {
    file: 'web/package-lock.json',
    read: (s) => {
      const j = JSON.parse(s);
      return [j.version, j.packages?.['']?.version];
    },
    write: (s, v) => {
      const j = JSON.parse(s);
      j.version = v;
      if (j.packages?.['']) j.packages[''].version = v;
      return `${JSON.stringify(j, null, 2)}\n`;
    },
  },
  {
    file: 'crates/desktop/tauri.conf.json',
    read: (s) => [JSON.parse(s).version],
    write: (s, v) => s.replace(/("version": ")[^"]+(")/, `$1${v}$2`),
  },
  {
    file: 'docs/BUILD-WINDOWS.md',
    read: (s) => [/^## 3а\. Окно приложения \((\d+\.\d+\.\d+)\)/m.exec(s)?.[1]],
    write: (s, v) => s.replace(/(^## 3а\. Окно приложения \()\d+\.\d+\.\d+(\))/m, `$1${v}$2`),
  },
];

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const load = (t) => readFileSync(path.join(root, t.file), 'utf8');

/** The version in Cargo.toml (the reference). */
export function current() {
  return targets[0].read(load(targets[0]))[0];
}

/** Lists every place whose version differs from the reference (or is missing). */
export function problems() {
  const want = current();
  return targets.flatMap((t) =>
    t.read(load(t)).flatMap((got) => (got === want ? [] : [`${t.file}: ${got ?? 'not found'} (expected ${want})`])),
  );
}

function main(argv) {
  const [cmd, arg] = argv;
  if (cmd === 'get') return console.log(current());
  if (cmd === 'check') {
    const bad = problems();
    if (bad.length > 0) {
      console.error(`version mismatch:\n${bad.join('\n')}`);
      process.exit(1);
    }
    return console.log(`version ${current()}: all files agree`);
  }
  if (cmd === 'set') {
    if (!SEMVER.test(arg ?? '')) {
      console.error('usage: node scripts/version.mjs set X.Y.Z');
      process.exit(2);
    }
    for (const t of targets) {
      const before = load(t);
      const after = t.write(before, arg);
      if (after !== before) writeFileSync(path.join(root, t.file), after);
    }
    const bad = problems();
    if (bad.length > 0) {
      console.error(`set did not reach every place:\n${bad.join('\n')}`);
      process.exit(1);
    }
    return console.log(`version ${arg} written`);
  }
  console.error('usage: node scripts/version.mjs get | check | set X.Y.Z');
  process.exit(2);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) main(process.argv.slice(2));
