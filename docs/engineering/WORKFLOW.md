# Engineering workflow

Rules for contributors and AI agents. Adapted from the "AI Agent Engineering Pack" v2.0 and from the das-forge-core security documents; trimmed to what a offline desktop/CLI tool needs. Pack rules about money, databases, retries and production deployments were dropped: this project has none of those.

## Loop

Inspect, contract, plan, authorize, execute, verify, review the diff, report.

## Risk classes

| Class | Meaning | Rule |
|---|---|---|
| R0 | read-only inspection | free |
| R1 | reversible local edits | free; review the diff |
| R2 | new dependency, lockfile, CI, toolchain, bulk change | approval; explain why |
| R3 | commit, push, tag, publish, release, delete | explicit approval naming the action |
| R4 | harmful, unlawful, license-violating (copying AGPL code, publishing game assets) | refuse |

## Evidence labels in reports

VERIFIED (command ran and passed), PARTIALLY VERIFIED, NOT VERIFIED, FAILED, BLOCKED. The Windows build counts as NOT VERIFIED until the maintainer reports it.

## Parser rules

1. Every length, count and offset read from a file is untrusted: check against the remaining size, use checked arithmetic, cap allocations (limits in `limits.rs`).
2. Decompression has an output cap and a ratio cap; nested layers have a depth cap.
3. Extraction: reject absolute paths, `..`, drive letters, reserved Windows names; never follow links; report collisions (case-insensitive) instead of overwriting.
4. No panic on any input. Fuzz targets (`cargo fuzz`) are added per reader once it exists.
5. Errors carry file, offset and expectation.
6. Output formats (JSON) carry a `schema` number; fields are only added within one number.

## Tests

Synthetic inputs built in code; truncated, corrupted and oversized variants for each parser; snapshot tests for CLI output; samples from the maintainer run only when `ADV_SAMPLES` is set and are never committed. A bug fix gets a regression test.

## Dependencies

Direct dependencies and tooling are releases of the current calendar year (verify the real date). New ones need approval and a line in `docs/ROADMAP.md` or an ADR. `deny.toml` lists allowed licenses; copyleft crates are not embedded.

## Decisions

Record decisions in `docs/adr/` using the template. Open gaps live in `docs/GAPS.md` with an ID, severity and verification status.

## Guard hook

`scripts/guard.mjs` blocks obviously destructive shell commands for agents (Claude Code `PreToolUse`). It is a safety net, not a sandbox.
