#!/usr/bin/env node
// PreToolUse guard for Bash commands (Claude Code hook). Exit code 2 blocks the command.
// This is a safety net against mistakes, not a sandbox: a determined caller can bypass it.
// Ported from the "AI Agent Engineering Pack" guard; extended for flag order (-fr, -r -f),
// "git push -f", Windows (PowerShell, cmd) and "pipe into a shell". Uses Node, which the
// project needs anyway (no Python on a typical Windows machine).
import { pathToFileURL } from 'node:url';

const SECRET_FILE = /(^|[\\/])(\.env(\.[^\s\\/]*)?|[^\s\\/]*\.(pem|key|p12|pfx)|id_(rsa|ed25519|ecdsa))(\s|$|["'])/i;
const READERS = /^(cat|type|less|more|head|tail|bat|gc|get-content|strings|xxd|od)$/i;

/** Split a shell line into simple commands; good enough for flag checks. */
function segments(line) {
  return line.split(/&&|\|\||;|\||\n/).map((s) => s.trim()).filter(Boolean);
}

function tokens(segment) {
  return segment.match(/"[^"]*"|'[^']*'|\S+/g) ?? [];
}

function flagsOf(args) {
  return args.filter((a) => a.startsWith('-') || a.startsWith('/'));
}

/** Returns a reason string when the command must be blocked, otherwise null. */
export function check(command) {
  const text = String(command ?? '');

  if (/\|\s*(sudo\s+)?(sh|bash|zsh|dash|pwsh|powershell|iex)\b/i.test(text)) {
    return 'download-and-execute or pipe into a shell';
  }
  if (/\b(iex|invoke-expression)\b/i.test(text) && /\b(iwr|irm|invoke-webrequest|invoke-restmethod|curl|wget)\b/i.test(text)) {
    return 'download-and-execute (PowerShell)';
  }

  for (const seg of segments(text)) {
    const t = tokens(seg);
    // skip a leading sudo/env-style prefix
    while (t.length > 1 && /^(sudo|command|env|nohup|time)$/i.test(t[0])) t.shift();
    const cmd = (t[0] ?? '').toLowerCase();
    const args = t.slice(1);
    const flags = flagsOf(args).map((f) => f.toLowerCase());
    const short = flags.filter((f) => /^-[a-z]+$/.test(f)).join('').replace(/-/g, '');

    if (cmd === 'rm' || cmd === 'rmdir') {
      const recursive = /[r]/i.test(short) || flags.includes('--recursive');
      const force = /f/.test(short) || flags.includes('--force');
      if (recursive && force) return 'recursive forced delete';
    }
    if (/^(remove-item|ri|del|rd|erase)$/.test(cmd)) {
      const recurse = flags.some((f) => /^-r(ecurse)?$/.test(f) || f === '/s');
      const force = flags.some((f) => /^-fo(rce)?$/.test(f) || f === '/q' || f === '/f');
      if (recurse && force) return 'recursive forced delete (Windows)';
    }
    if (cmd === 'git') {
      const sub = args.find((a) => !a.startsWith('-')) ?? '';
      if (sub === 'reset' && flags.includes('--hard')) return 'git reset --hard';
      if (sub === 'clean' && (/f/.test(short) || flags.includes('--force'))) return 'git clean with force';
      if (sub === 'push') {
        const forced = flags.some((f) => f === '-f' || f.startsWith('--force'));
        const plus = args.some((a) => /^\+\S/.test(a));
        if (forced || plus) return 'git push with force';
      }
    }
    if ((cmd === 'cargo' || cmd === 'npm' || cmd === 'pnpm' || cmd === 'yarn') && args[0] === 'publish') {
      return 'publishing a package';
    }
    if ((cmd === 'terraform' || cmd === 'tofu') && args[0] === 'destroy') return 'terraform destroy';
    if (cmd === 'kubectl' && args[0] === 'delete') return 'kubectl delete';
    if (cmd === 'printenv' || (cmd === 'env' && args.length === 0) || (cmd === 'set' && args.length === 0)) {
      return 'dumps the environment';
    }
    if (/^(get-childitem|gci|dir|ls)$/.test(cmd) && args.some((a) => /^env:/i.test(a))) {
      return 'dumps the environment';
    }
    if (READERS.test(cmd) && args.some((a) => SECRET_FILE.test(a + ' '))) {
      return 'reads a file that may hold credentials';
    }
  }
  if (/\bDROP\s+(DATABASE|SCHEMA)\b/i.test(text)) return 'drops a database';
  return null;
}

async function main() {
  let raw = '';
  for await (const chunk of process.stdin) raw += chunk;
  let payload;
  try {
    payload = JSON.parse(raw);
  } catch {
    process.stderr.write('guard: cannot parse hook input; blocking conservatively.\n');
    return 2;
  }
  const reason = check(payload?.tool_input?.command);
  if (reason) {
    process.stderr.write(`Blocked: ${reason}. Inspect the target and ask the maintainer for explicit approval.\n`);
    return 2;
  }
  return 0;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  process.exitCode = await main();
}
