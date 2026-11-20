import type { Item, Kind } from "./api";

export type Show = "all" | Kind;
export type SortBy = "name" | "size" | "kind";

/** Keeps directories (they may hold matching files) and the files of the chosen kind. */
export function filterItems(items: Item[], show: Show): Item[] {
  return show === "all" ? items : items.filter((i) => i.is_dir || i.kind === show);
}

/** Directories first, then files; stable and case-insensitive by name. */
export function sortItems(items: Item[], by: SortBy): Item[] {
  const byName = (a: Item, b: Item) => a.name.localeCompare(b.name, undefined, { sensitivity: "base", numeric: true });
  return [...items].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    if (a.is_dir) return byName(a, b);
    if (by === "size") return (b.size ?? 0) - (a.size ?? 0) || byName(a, b);
    if (by === "kind") return a.kind.localeCompare(b.kind) || byName(a, b);
    return byName(a, b);
  });
}

/** Applies real kinds (found by looking into files) to items whose kind the name could not tell. */
export function withKinds(items: Item[], kinds: Map<string, Kind>, dir: string): Item[] {
  return items.map((i) => {
    const k = kinds.get(dir === "" ? i.name : `${dir}/${i.name}`);
    return k && !i.is_dir ? { ...i, kind: k } : i;
  });
}
