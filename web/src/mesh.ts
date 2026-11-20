/** Mesh buffers as sent by the app (`get_mesh`): see `adv_core::preview::mesh_bytes`. */
export interface Mesh {
  positions: Float32Array;
  uvs: Float32Array;
  indices: Uint32Array;
  /** Axis-aligned bounds: `[minX, minY, minZ, maxX, maxY, maxZ]`. */
  bounds: [number, number, number, number, number, number];
}

/** Decodes the packed mesh. Throws a `RangeError` for truncated or inconsistent data. */
export function parseMesh(buf: ArrayBuffer): Mesh {
  if (buf.byteLength < 8) throw new RangeError("mesh is truncated");
  const view = new DataView(buf);
  const v = view.getUint32(0, true);
  const i = view.getUint32(4, true);
  const need = 8 + v * 12 + v * 8 + i * 4;
  if (buf.byteLength !== need) throw new RangeError("mesh size does not match its header");
  const positions = new Float32Array(buf, 8, v * 3);
  const uvs = new Float32Array(buf, 8 + v * 12, v * 2);
  const indices = new Uint32Array(buf, 8 + v * 20, i);
  for (const idx of indices) if (idx >= v) throw new RangeError("mesh index is out of range");
  const b: Mesh["bounds"] = [Infinity, Infinity, Infinity, -Infinity, -Infinity, -Infinity];
  for (let k = 0; k < positions.length; k += 3) {
    for (let a = 0; a < 3; a++) {
      const c = positions[k + a] ?? 0;
      if (c < (b[a] ?? 0)) b[a] = c;
      if (c > (b[a + 3] ?? 0)) b[a + 3] = c;
    }
  }
  return { positions, uvs, indices, bounds: b };
}
