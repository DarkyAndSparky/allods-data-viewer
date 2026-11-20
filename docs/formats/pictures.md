# Ordinary pictures (`.ico`, `.dds`, JPEG, GIF, BMP)

Status: **verified** on one icon (48x48, 4/8 bit or 32 bit DIB) and one DDS (DXT, 1024x1024) from the game launcher and UI; the other paths are covered by synthetic tests only.

- `.ico`: directory of images; the one with the most pixels is shown. An image is either a PNG or a bitmap (BITMAPINFOHEADER, height doubled, palette for 1/4/8 bit, rows bottom first, 1 bit transparency mask). Only uncompressed bitmaps are read.
- `.dds`: 128 byte header, top level only. DXT1, DXT3, DXT5 and uncompressed 24/32 bit with channel masks are read. Mip levels, cube maps and other FourCC codes are not decoded (shown as a hex dump).
- JPEG, GIF and BMP are not decoded by the core; only the size is read from the header (JPEG fields are big endian) and the bytes are handed to the web view.
- Size limit: 16 million pixels.
