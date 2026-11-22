/** Geometry of a planar YCbCr frame as the app sends it. */
export interface YuvLayout {
  width: number;
  height: number;
  chroma_shift_x: number;
  chroma_shift_y: number;
}

const clamp = (v: number): number => (v < 0 ? 0 : v > 255 ? 255 : v);

/**
 * Converts one frame (luma plane, then Cb and Cr planes, no row padding) to RGBA with the
 * BT.601 limited-range matrix Theora uses. `out` must hold `width * height * 4` bytes.
 */
export function yuvToRgba(frame: Uint8Array, layout: YuvLayout, out: Uint8ClampedArray): void {
  const { width, height, chroma_shift_x: sx, chroma_shift_y: sy } = layout;
  const cw = (width + (1 << sx) - 1) >> sx;
  const ch = (height + (1 << sy) - 1) >> sy;
  const cbBase = width * height;
  const crBase = cbBase + cw * ch;
  let o = 0;
  for (let y = 0; y < height; y++) {
    const cRow = (y >> sy) * cw;
    for (let x = 0; x < width; x++) {
      const c = cRow + (x >> sx);
      const yy = 1.164 * ((frame[y * width + x] ?? 16) - 16);
      const cb = (frame[cbBase + c] ?? 128) - 128;
      const cr = (frame[crBase + c] ?? 128) - 128;
      out[o++] = clamp(yy + 1.596 * cr);
      out[o++] = clamp(yy - 0.392 * cb - 0.813 * cr);
      out[o++] = clamp(yy + 2.017 * cb);
      out[o++] = 255;
    }
  }
}
