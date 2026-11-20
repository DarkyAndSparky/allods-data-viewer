/** Joins virtual path parts with `/`, ignoring empty parts. */
export function joinPath(...parts: string[]): string {
  return parts.filter((p) => p !== "").join("/");
}

/** Last component of a virtual path. */
export function baseName(path: string): string {
  const i = path.lastIndexOf("/");
  return i < 0 ? path : path.slice(i + 1);
}

/** Maps an app error code to its translation key. */
export function errorKey(
  code: string,
):
  | "errBadPath"
  | "errNotFound"
  | "errWrongKind"
  | "errFailed"
  | "errNoSession"
  | "errUnknown" {
  switch (code) {
    case "bad_path":
      return "errBadPath";
    case "not_found":
      return "errNotFound";
    case "wrong_kind":
      return "errWrongKind";
    case "failed":
      return "errFailed";
    case "no_session":
      return "errNoSession";
    default:
      return "errUnknown";
  }
}

/** Seconds as `m:ss`. */
export function formatDuration(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

/** Longest common start of `items`, compared ignoring case; the casing of the first item is kept. */
export function commonPrefix(items: string[]): string {
  const first = items[0];
  if (first === undefined) return "";
  let n = first.length;
  for (const it of items) {
    let i = 0;
    while (i < n && i < it.length && it[i]?.toLowerCase() === first[i]?.toLowerCase()) i++;
    n = i;
  }
  return first.slice(0, n);
}

/** Folder names that hold user add-ons and mods, which can clutter the tree and the search. */
const JUNK_FOLDERS = new Set(["mods", "addons"]);

/** True when a folder name is a mods or add-ons folder. */
export function isJunkName(name: string): boolean {
  return JUNK_FOLDERS.has(name.toLowerCase());
}

/** True when any folder of a virtual path (not the file name itself) is a mods or add-ons folder. */
export function isJunkPath(path: string): boolean {
  const parts = path.split(/[\\/]/);
  return parts.slice(0, -1).some(isJunkName);
}
