# PakScope

Open source viewer and extractor for resources of an MMORPG game client:
archives, localization, textures and models.

> Status: **M0**, project skeleton. See [docs/ROADMAP.md](docs/ROADMAP.md).
> Русская версия: [README.ru.md](README.ru.md).

## Goals

- Read the game's archives and resources without modifying them.
- Command line tool for listing, searching and extracting files.
- Convert textures to PNG and models to glTF.
- Desktop app (Tauri) with a file tree and a 3D preview, the same UI also runs in a browser.
- Work on Windows and Linux.

## Layout

| Path | What it is |
|---|---|
| `crates/core` | Rust library with the format readers, no UI |
| `crates/cli` | the `pakscope` command line tool |
| `web/` | TypeScript UI (Vite), later wrapped by Tauri |
| `docs/` | roadmap, architecture, format notes |

## Build and test

Requirements: [Rust](https://rustup.rs) (stable) and [Node.js](https://nodejs.org) 22 LTS or newer.
On Windows Rust also needs the Microsoft C++ Build Tools.

```
cargo test --workspace
cargo run -p pakscope-cli

cd web
npm install
npm test
npm run build
```

## Legal

PakScope is an independent project. It is not affiliated with the game's developers
or publishers. The repository contains no game assets and never will, test data is
generated in the tests. Game and company names belong to their respective owners.

The file format notes were prepared by studying the behaviour of
[mrkriv/AODataViewer](https://github.com/mrkriv/AODataViewer). No code from that
project is used here, see [CONTRIBUTING.md](CONTRIBUTING.md).

Licensed under the [MIT License](LICENSE).
