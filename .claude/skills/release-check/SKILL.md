---
name: release-check
description: Read-only release readiness assessment; does not tag, publish or push.
---
Check git state, version consistency (Cargo.toml, web/package.json, tauri config), changelog, all checks in CLAUDE.md, `node scripts/check-repo.mjs`, dependency audit (cargo deny, cargo audit, npm audit) if available, license list, NOTICE, SBOM, and that no game asset or font is in the tree or the installer. Return READY, NOT READY or BLOCKED with evidence per item. No tag, push or publish.
