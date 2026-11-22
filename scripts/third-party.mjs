#!/usr/bin/env node
// Components and licences list.
//   node scripts/third-party.mjs generate   writes web/src/third-party.json and docs/THIRD-PARTY.md
//   node scripts/third-party.mjs check      fails when they are out of date or a direct dependency has no purpose
// Direct runtime dependencies (Rust and npm) need an entry in scripts/third-party-purpose.json (ru and en);
// code that was ported or copied into the repository is listed in EXTRA below.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const JSON_OUT = join(ROOT, 'web/src/third-party.json');
const MD_OUT = join(ROOT, 'docs/THIRD-PARTY.md');
const OK_WORDS = ['MIT', 'Apache-2.0', 'BSD-3-Clause', 'BSD-2-Clause', 'ISC', 'Zlib', 'MPL-2.0', 'Unlicense', 'CC0-1.0', 'Unicode-3.0', '0BSD', 'MIT-0', 'BSL-1.0', 'LLVM-exception', 'WITH', 'OR', 'AND'];

/** Code that lives in this repository but comes from somewhere else. */
export const EXTRA = [
  {
    name: 'libcelt 0.11.0 (Xiph.Org)', version: '2011-02-03', license: 'BSD-3-Clause', kind: 'code',
    url: 'https://gitlab.xiph.org/xiph/opus/-/tree/d922fe1c',
    ru: 'Декодер CELT (звук FSB5, режим 12): порт на Rust, crates/core/src/celt', en: 'CELT decoder (FSB5 sound, mode 12): Rust port, crates/core/src/celt',
  },
];

function sh(cmd, args, cwd = ROOT) {
  return execFileSync(cmd, args, { cwd, encoding: 'utf8', maxBuffer: 256 << 20, stdio: ['ignore', 'pipe', 'ignore'] });
}

function rustPackages() {
  const m = JSON.parse(sh('cargo', ['metadata', '--offline', '--format-version', '1']));
  const byId = new Map(m.packages.map((p) => [p.id, p]));
  const ws = new Set(m.workspace_members);
  const nodes = new Map(m.resolve.nodes.map((n) => [n.id, n]));
  const normal = (n) => n.deps.filter((d) => d.dep_kinds.some((k) => k.kind === null));
  const direct = new Set();
  for (const id of ws) for (const d of normal(nodes.get(id))) if (!ws.has(d.pkg)) direct.add(d.pkg);
  const seen = new Set();
  const stack = [...direct];
  while (stack.length) {
    const id = stack.pop();
    if (seen.has(id)) continue;
    seen.add(id);
    for (const d of normal(nodes.get(id))) if (!ws.has(d.pkg)) stack.push(d.pkg);
  }
  const row = (id) => {
    const p = byId.get(id);
    return { name: p.name, version: p.version, license: p.license ?? 'UNKNOWN', url: p.repository ?? `https://crates.io/crates/${p.name}`, kind: 'rust' };
  };
  return { direct: [...direct].map(row), all: [...seen].map(row) };
}

function npmPackages() {
  const pkg = JSON.parse(readFileSync(join(ROOT, 'web/package.json'), 'utf8'));
  // @tauri-apps/api is a dev dependency in package.json but is bundled into the page.
  const names = [...Object.keys(pkg.dependencies ?? {}), '@tauri-apps/api'];
  const info = (name) => {
    const f = join(ROOT, 'web/node_modules', name, 'package.json');
    if (!existsSync(f)) throw new Error(`${name} is not installed (run npm ci in web/)`);
    const j = JSON.parse(readFileSync(f, 'utf8'));
    const lic = typeof j.license === 'string' ? j.license : j.license?.type ?? 'UNKNOWN';
    const repo = typeof j.repository === 'string' ? j.repository : j.repository?.url ?? '';
    const url = repo.replace(/^git\+/, '').replace(/\.git$/, '').replace(/^git:/, 'https:') || `https://www.npmjs.com/package/${name}`;
    return { name, version: j.version, license: lic, url, kind: 'npm' };
  };
  return names.map(info);
}

/** An SPDX expression is fine when at least one OR alternative uses only allowed licences. */
export function licenceOk(expr) {
  if (!expr || expr === 'UNKNOWN') return false;
  return expr.replace(/[()]/g, '').split(/\s+OR\s+|\s*\/\s*/).some((alt) => alt.split(/\s+(?:AND|WITH)\s+/).every((w) => OK_WORDS.includes(w.trim())));
}

export function build(purpose = JSON.parse(readFileSync(join(ROOT, 'scripts/third-party-purpose.json'), 'utf8'))) {
  const problems = [];
  const rust = rustPackages();
  const npm = npmPackages();
  const direct = [...rust.direct, ...npm].sort((a, b) => a.kind.localeCompare(b.kind) || a.name.localeCompare(b.name));
  const components = [];
  for (const d of direct) {
    const p = purpose[`${d.kind}:${d.name}`];
    if (!p?.ru || !p?.en) problems.push(`${d.kind}:${d.name}: no purpose (ru and en) in scripts/third-party-purpose.json`);
    components.push({ ...d, ru: p?.ru ?? '', en: p?.en ?? '' });
  }
  components.push(...EXTRA);
  for (const c of [...components, ...rust.all]) {
    if (!licenceOk(c.license)) problems.push(`${c.name} ${c.version}: licence "${c.license}" is not on the allowed list`);
  }
  return { components, rustAll: rust.all.sort((a, b) => a.name.localeCompare(b.name)), problems };
}

export function markdown({ components, rustAll }) {
  const direct = new Set(components.filter((c) => c.kind === 'rust').map((c) => c.name));
  const lines = [
    '# Third-party components',
    '',
    'Generated by `node scripts/third-party.mjs generate`; do not edit by hand. The viewer shows the first table in its About dialog.',
    '',
    '## Components that are part of the program',
    '',
    '| Component | Version | Licence | Kind | Purpose |',
    '| --- | --- | --- | --- | --- |',
    ...components.map((c) => `| [${c.name}](${c.url}) | ${c.version} | ${c.license} | ${c.kind} | ${c.en} |`),
    '',
    '## All Rust packages in the build (including indirect)',
    '',
    '| Package | Version | Licence |',
    '| --- | --- | --- |',
    ...rustAll.map((c) => `| ${c.name}${direct.has(c.name) ? ' (direct)' : ''} | ${c.version} | ${c.license} |`),
    '',
  ];
  return lines.join('\n');
}

const json = (r) => `${JSON.stringify({ components: r.components }, null, 2)}\n`;

function main(mode) {
  const r = build();
  if (r.problems.length) {
    console.error(r.problems.join('\n'));
    process.exit(1);
  }
  if (mode === 'generate') {
    writeFileSync(JSON_OUT, json(r));
    writeFileSync(MD_OUT, markdown(r));
    console.log(`third-party: ${r.components.length} components, ${r.rustAll.length} Rust packages written`);
  } else if (mode === 'check') {
    const stale = [[JSON_OUT, json(r)], [MD_OUT, markdown(r)]].filter(([f, want]) => !existsSync(f) || readFileSync(f, 'utf8') !== want);
    if (stale.length) {
      console.error(`third-party: out of date: ${stale.map(([f]) => f.slice(ROOT.length + 1)).join(', ')} (run: node scripts/third-party.mjs generate)`);
      process.exit(1);
    }
    console.log('third-party: ok');
  } else {
    console.error('usage: third-party.mjs generate|check');
    process.exit(2);
  }
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) main(process.argv[2]);
