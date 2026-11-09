---
paths:
  - "crates/core/**/*.rs"
---
Parsers read hostile input. Follow docs/engineering/WORKFLOW.md section "Parser rules".
- Every offset, size and count from a file is checked against the remaining length before use; use checked arithmetic.
- Never `Vec::with_capacity(n)` or allocate from an unchecked length; apply limits from `limits.rs`.
- No `unwrap`, `expect`, indexing that can panic, or `unsafe` on input-dependent paths. Return `Error` with file and offset context.
- Decompression has an output cap (bomb protection). Extraction rejects absolute paths and `..`.
- A reader takes `Read + Seek`, not a path. No printing, no process exit, no global state.
- New behavior: a synthetic-input test for success, a truncated-input test, a corrupted-field test.
- Update the matching note in docs/formats/ and keep its status honest (`draft`, `verified`, `unknown`).
