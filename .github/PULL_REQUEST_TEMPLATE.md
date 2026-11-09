## What and why

## Checks
- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
- [ ] `cd web && npm run typecheck && npm test && npm run build`
- [ ] `node scripts/check-repo.mjs` (no game assets)
- [ ] New or changed parser: tests for truncated and corrupted input; `docs/formats/` updated with honest status
- [ ] New dependency: released this calendar year, license allowed by `deny.toml`
- [ ] No code copied from AGPL/GPL or unlicensed projects; MIT/Apache code listed in `NOTICE`
