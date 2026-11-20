import { describe, expect, it } from "vitest";

/**
 * Source-level guards for the web code (the pattern comes from das-forge-core): constructs that
 * must not appear, or appear only where reviewed. A new occurrence fails the test; to accept it,
 * review it and add it to the allowlist with the reason.
 */
const modules = import.meta.glob<string>(["./*.ts", "!./*.test.ts"], {
  query: "?raw",
  import: "default",
  eager: true,
});
const files = Object.entries(modules).map(([name, text]) => ({ name: name.slice(2), text }));

const hits = (re: RegExp): string[] =>
  [...new Set(files.filter((f) => re.test(f.text)).map((f) => f.name))].sort();

describe("dangerous constructs", () => {
  it("no eval, new Function or string timers", () => {
    expect(hits(/\beval\(|new Function\(|set(?:Timeout|Interval)\(\s*["'`]/)).toEqual([]);
  });

  it("innerHTML and friends only where the markup is built from escaped text", () => {
    const allowed: Record<string, string> = {
      "app.ts": "About dialog: renderAbout escapes every value (about.test.ts)",
    };
    expect(hits(/innerHTML|outerHTML|insertAdjacentHTML|document\.write/)).toEqual(
      Object.keys(allowed).sort(),
    );
  });

  it("no remote scripts, fonts or images", () => {
    expect(hits(/(?:src|href)\s*=\s*["']https?:\/\//)).toEqual([]);
    expect(hits(/@import\s+url\(\s*["']?https?:/)).toEqual([]);
  });

  it("every tauri command invoked from the web is a plain string name", () => {
    expect(hits(/invoke\(\s*[^"'\s]/)).toEqual([]);
  });
});
