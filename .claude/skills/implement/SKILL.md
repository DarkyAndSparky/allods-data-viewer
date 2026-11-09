---
name: implement
description: Implement a feature or bug fix with tests and verification.
---
Inspect git status, rules, the format note, callers and tests. For non-trivial work write a task contract (docs/engineering/templates/TASK_CONTRACT.md). Plan the smallest change. Implement incrementally; add synthetic-input tests for success, truncated and corrupted input. Run the checks listed in CLAUDE.md, inspect the full diff, report with evidence labels. Never commit, push, add dependencies or touch CI without approval.
