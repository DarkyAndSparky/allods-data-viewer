# Architecture

```
          +-----------+     +-----------------+
          |  crates/  |     |  web/ (TS UI)   |
          |   cli     |     |  Vite, Three.js |
          +-----+-----+     +--------+--------+
                |                    |   (M5: Tauri commands,
                |                    |    later optional WASM)
                +--------+  +--------+
                         v  v
                  +---------------+
                  | crates/core   |  format readers, no UI, no policy
                  +---------------+
```

## Principles

- **`core` knows nothing about UI or the file system layout.** Readers take a byte
  source (anything that is `Read + Seek`) and return plain data. This keeps them
  testable and reusable in the CLI, in the desktop app and in the browser.
- **Read only.** Nothing in the project writes to game files.
- **Errors are values.** Readers return `Result`, they never panic on bad input.
  Broken files are reported and skipped, the rest keeps working.
- **No unsafe code** (`unsafe_code = "forbid"` for the whole workspace).
- **Tests use synthetic data** built in code, never real game files.

## Stack

| Layer | Choice |
|---|---|
| Core and CLI | Rust (stable, edition 2024) |
| UI | TypeScript, Vite, Three.js (from M6) |
| Desktop shell | Tauri (from M5) |
| Tests | `cargo test`, Vitest |
| CI | GitHub Actions on Linux and Windows |
