import { describe, expect, it } from "vitest";
import { yuvToRgba } from "./yuv";

const layout = { width: 2, height: 2, chroma_shift_x: 1, chroma_shift_y: 1 };

describe("yuvToRgba", () => {
  it("maps limited-range black, white and a gray", () => {
    const rgba = new Uint8ClampedArray(16);
    yuvToRgba(Uint8Array.from([16, 235, 126, 16, 128, 128]), layout, rgba);
    expect([...rgba.slice(0, 4)]).toEqual([0, 0, 0, 255]);
    expect([...rgba.slice(4, 8)]).toEqual([255, 255, 255, 255]);
    expect(rgba[8]).toBeGreaterThan(120);
    expect(rgba[8]).toBeLessThan(130);
  });

  it("applies chroma to the whole 2x2 block", () => {
    const rgba = new Uint8ClampedArray(16);
    yuvToRgba(Uint8Array.from([81, 81, 81, 81, 90, 240]), layout, rgba);
    for (let p = 0; p < 4; p++) {
      expect(rgba[p * 4]).toBeGreaterThan(240);
      expect(rgba[p * 4 + 1]).toBeLessThan(20);
      expect(rgba[p * 4 + 2]).toBeLessThan(20);
    }
  });

  it("handles 4:4:4 and a truncated frame without throwing", () => {
    const rgba = new Uint8ClampedArray(8);
    yuvToRgba(Uint8Array.from([16, 235, 128, 128, 128, 128]), { width: 2, height: 1, chroma_shift_x: 0, chroma_shift_y: 0 }, rgba);
    expect(rgba[4]).toBe(255);
    yuvToRgba(new Uint8Array(1), layout, new Uint8ClampedArray(16));
  });
});
