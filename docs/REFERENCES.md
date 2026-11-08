# Reference projects

Projects we studied. What we may take from each depends on its license.

| Project | License | Language | What it gives us | How we use it |
|---|---|---|---|---|
| AllodsOnlineEditorTools (Paulus) | MIT | C# | Documented `.bin` database format (V1, V2 x86/x64), ImHex patterns, per-version struct layouts, texture `.bin` to DDS/PNG, JSON/XML export | Format knowledge; code may be ported with attribution |
| AOParser (koronabora) | MIT | C++ | Parser for `.xdb` data into spreadsheets | Format knowledge; code may be ported with attribution |
| SarnautCore tools | Apache-2.0 | Rust | Content addressed asset store (BLAKE3), XDB to YAML extractors, ideas for a manifest | Code may be reused with attribution and a NOTICE entry |
| SarnautCore client | AGPL-3.0 | C# (Godot) | Converted-asset viewer, zone and lighting notes | **No code is copied.** Only facts about formats and behavior |
| AODataViewer (mrkriv) | none (all rights reserved) | C# | Idea of the viewer, the `.loc` reader behavior | **No code is copied.** Formats re-derived from samples |

Notes:

- The two copies of AllodsOnlineEditorTools that were supplied are identical.
- None of these projects covers `.loc` files, FMOD audio, video or model geometry.
  Those parts come from our own analysis of samples, see [formats](formats/README.md).
- When code is ported, add the original copyright notice to a `NOTICE` file in the same
  commit.
