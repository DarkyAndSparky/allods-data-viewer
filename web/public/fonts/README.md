# Optional interface font

The UI prefers the game's own font ("AllodsWest"), but that font is
© Nival Online, all rights reserved, so it is **not** part of this repository
and CI builds do not contain it.

To use it on your machine, do either of these:

- install `Allods.ttf` into your operating system, or
- copy it to `web/public/fonts/Allods.ttf` (the file is git-ignored).

If it is missing the UI falls back to the system font. Do not publish builds
that contain the font file.
