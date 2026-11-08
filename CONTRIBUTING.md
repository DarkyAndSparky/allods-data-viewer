# Contributing

## Where the code and the knowledge come from

This project is MIT licensed. To keep it that way:

1. **Do not copy code** from projects whose license is not MIT-compatible (GPL, AGPL),
   and do not copy code from projects that have no license at all (no license means all
   rights reserved). This includes decompiled code. Code from MIT and Apache-2.0 projects
   may be ported if the original copyright notice goes into `NOTICE`. See
   [docs/REFERENCES.md](docs/REFERENCES.md) for the status of each project.
2. **File formats are facts, code is not.** Describe a format in your own words in
   `docs/formats/`, then implement it from that description.
3. Mark every format note with its status: `draft` (unverified guess), `verified`
   (checked against real files) or `unknown`.
4. **No game assets in the repository**: no archives, textures, models or extracted
   text. Tests build small synthetic inputs in code.

## Dependency policy

- **Direct dependencies and tooling must be releases of the current calendar year**
  (check the real release date on crates.io / npm before adding or bumping).
- Transitive dependencies are whatever the newest compatible version is. A package
  that had no release this year is accepted when it is the latest one.
- `rust-version` follows a Rust release of the current year.
- Dependabot opens weekly update pull requests for Cargo, npm and GitHub Actions.

## Before you open a pull request

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd web && npm test && npm run typecheck
```

CI runs the same checks on Linux and Windows.
