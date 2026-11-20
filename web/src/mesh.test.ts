import { describe, expect, it } from "vitest";
import { parseMesh } from "./mesh";

function pack(positions: number[], uvs: number[], indices: number[]): ArrayBuffer {
  const v = positions.length / 3;
  const buf = new ArrayBuffer(8 + v * 20 + indices.length * 4);
  const view = new DataView(buf);
  view.setUint32(0, v, true);
  view.setUint32(4, indices.length, true);
  new Float32Array(buf, 8, v * 3).set(positions);
  new Float32Array(buf, 8 + v * 12, v * 2).set(uvs);
  new Uint32Array(buf, 8 + v * 20, indices.length).set(indices);
  return buf;
}

describe("parseMesh", () => {
  it("reads buffers and bounds", () => {
    const m = parseMesh(pack([0, 0, 0, 2, 1, -1, 1, 3, 0], [0, 0, 1, 0, 0, 1], [0, 1, 2]));
    expect(Array.from(m.indices)).toEqual([0, 1, 2]);
    expect(m.bounds).toEqual([0, 0, -1, 2, 3, 0]);
    expect(m.uvs.length).toBe(6);
  });

  it("rejects damaged data", () => {
    expect(() => parseMesh(new ArrayBuffer(4))).toThrow(RangeError);
    const ok = pack([0, 0, 0], [0, 0], [0, 0, 0]);
    expect(() => parseMesh(ok.slice(0, ok.byteLength - 1))).toThrow(RangeError);
    expect(() => parseMesh(pack([0, 0, 0], [0, 0], [0, 0, 5]))).toThrow(RangeError);
  });
});
