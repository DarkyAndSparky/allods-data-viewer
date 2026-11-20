import { describe, expect, it } from "vitest";
import type { Item } from "./api";
import { filterItems, sortItems, withKinds } from "./view";

const item = (name: string, kind: Item["kind"], size: number | null, is_dir = false): Item => ({
  name,
  kind,
  size,
  is_dir,
  is_pak: false,
  files: null,
});

const items = [
  item("b.bin", "other", 10),
  item("A.txt", "text", 300),
  item("dir", "other", null, true),
  item("c.dds", "texture", 50),
  item("a2.bin", "other", 10),
];

describe("filterItems", () => {
  it("keeps directories and matching files", () => {
    expect(filterItems(items, "text").map((i) => i.name)).toEqual(["A.txt", "dir"]);
    expect(filterItems(items, "all")).toHaveLength(5);
  });
});

describe("sortItems", () => {
  it("puts directories first and sorts by name", () => {
    expect(sortItems(items, "name").map((i) => i.name)).toEqual(["dir", "A.txt", "a2.bin", "b.bin", "c.dds"]);
  });
  it("sorts by size, biggest first, then name", () => {
    expect(sortItems(items, "size").map((i) => i.name)).toEqual(["dir", "A.txt", "c.dds", "a2.bin", "b.bin"]);
  });
  it("sorts by kind, then name", () => {
    expect(sortItems(items, "kind").map((i) => i.name)).toEqual(["dir", "a2.bin", "b.bin", "A.txt", "c.dds"]);
  });
  it("does not change the input", () => {
    const copy = [...items];
    sortItems(items, "size");
    expect(items).toEqual(copy);
  });
});

describe("withKinds", () => {
  it("replaces the kind of files only", () => {
    const m = new Map([["x/b.bin", "model" as const], ["x/dir", "model" as const]]);
    const r = withKinds(items, m, "x");
    expect(r.find((i) => i.name === "b.bin")?.kind).toBe("model");
    expect(r.find((i) => i.name === "dir")?.kind).toBe("other");
  });
});
