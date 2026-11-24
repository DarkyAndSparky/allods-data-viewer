/** Typed access to the desktop app commands. Only this file knows the command names. */

export type Kind = "text" | "texture" | "model" | "animation" | "audio" | "video" | "database" | "other";

export interface Item {
  name: string;
  is_dir: boolean;
  is_pak: boolean;
  size: number | null;
  files: number | null;
  kind: Kind;
}

export interface Census {
  files: number;
  paks: number;
  bytes: number;
  kinds: { kind: Kind; count: number; bytes: number }[];
}

/** A match inside a text file or a `.loc` file. */
export interface TextHit {
  path: string;
  /** Entry id inside a `.loc` file; null for plain texts. */
  id: number | null;
  snippet: string;
}

export interface LocItem {
  id: number;
  key: string | null;
  text: string;
}

export interface LocPage {
  total: number;
  matched: number;
  offset: number;
  items: LocItem[];
}

export interface GameVersion {
  head: string;
  version: string;
  /** Source-control revision from a `.rev` file; empty when absent. */
  revision: string;
  /** Date of that revision (YYYY-MM-DD); empty when absent. */
  date: string;
}

export interface Stream {
  name: string;
  frequency: number | null;
  channels: number;
  seconds: number | null;
}

export type Preview =
  | { kind: "text"; text: string; truncated: boolean }
  | { kind: "bev"; version: number; project: string; languages: string[]; chunks: [string, number][]; names: string[]; paths: string[] }
  | { kind: "svg"; svg: string; source: string; removed: number; removed_names: string[] }
  | { kind: "loc"; count: number; keys: boolean }
  | {
      kind: "image";
      width: number;
      height: number;
      format: string;
      guessed: boolean;
      candidates: [number, number][];
      index: number;
      labels: string[];
    }
  | { kind: "model"; vertices: number; triangles: number; stride: number }
  | { kind: "animation"; fps: number; frames: number; seconds: number; bones: number }
  | { kind: "video"; width: number; height: number; fps: number; seconds: number | null; frames: number | null }
  | { kind: "audio"; streams: Stream[] }
  | { kind: "binary"; len: number; hex: string };

/** Error from the app: a stable code to translate plus the technical English detail. */
export interface AppError {
  code: string;
  detail: string;
}

/** Size, subsampling, rate and length of a video, from `video_open`. */
export interface VideoInfo {
  width: number;
  height: number;
  chroma_shift_x: number;
  chroma_shift_y: number;
  fps: number;
  frames: number;
  frame_bytes: number;
}

export type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

/** Normalises whatever a failed call threw into an [`AppError`]. */
export function toAppError(e: unknown): AppError {
  if (typeof e === "object" && e !== null && "code" in e && "detail" in e) {
    const { code, detail } = e as { code: unknown; detail: unknown };
    return { code: String(code), detail: String(detail) };
  }
  return { code: "unknown", detail: e instanceof Error ? e.message : String(e) };
}

/** True when the page runs inside the desktop app (and not in a plain browser). */
export function insideApp(w: object = window): boolean {
  return "__TAURI_INTERNALS__" in w;
}

/**
 * Wraps `invoke` so `onChange` learns how many calls are pending (for the activity bar).
 */
export function trackBusy(invoke: Invoke, onChange: (pending: number) => void): Invoke {
  let pending = 0;
  return async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
    pending += 1;
    onChange(pending);
    try {
      return await invoke<T>(command, args);
    } finally {
      pending -= 1;
      onChange(pending);
    }
  };
}

export function makeApi(invoke: Invoke) {
  return {
    openPath: (path: string) => invoke<string>("open_path", { path }),
    listDir: (path: string) => invoke<Item[]>("list_dir", { path }),
    saveAs: (args: {
      path: string;
      format: "raw" | "png" | "wav" | "mp3" | "obj" | "glb" | "txt" | "html" | "xml";
      dest: string;
      textureIndex?: number;
      index?: number;
    }) =>
      invoke<void>("save_as", {
        path: args.path,
        format: args.format,
        dest: args.dest,
        textureIndex: args.textureIndex ?? null,
        index: args.index ?? null,
      }),
    fileKinds: (paths: string[]) => invoke<[string, Kind][]>("file_kinds", { paths }),
    gameVersion: () => invoke<GameVersion | null>("game_version"),
    census: () => invoke<Census>("census"),
    completePath: (prefix: string) => invoke<string[]>("complete_path", { prefix }),
    findKind: (kind: string) => invoke<string[]>("find_kind", { kind }),
    search: (query: string, scope?: string) => invoke<string[]>("search", { query, scope: scope ?? null }),
    locPage: (path: string, query: string, offset: number, limit: number) =>
      invoke<LocPage>("loc_page", { path, query, offset, limit }),
    searchText: (query: string, scope?: string) => invoke<TextHit[]>("search_text", { query, scope: scope ?? null }),
    preview: (path: string, textureIndex?: number) =>
      invoke<Preview>("preview", { path, textureIndex: textureIndex ?? null }),
    image: (path: string, textureIndex?: number) =>
      invoke<ArrayBuffer>("get_image", { path, textureIndex: textureIndex ?? null }),
    mesh: (path: string) => invoke<ArrayBuffer>("get_mesh", { path }),
    clips: (path: string) => invoke<string[]>("get_clips", { path }),
    animatedGlb: (path: string, clip: string) => invoke<ArrayBuffer>("get_animated_glb", { path, clip }),
    texturedGlb: (path: string, clip: string | null) => invoke<ArrayBuffer>("get_textured_glb", { path, clip }),
    audio: (path: string, index: number) => invoke<ArrayBuffer>("get_audio", { path, index }),
    videoOpen: (path: string) => invoke<VideoInfo>("video_open", { path }),
    videoAudio: (path: string) => invoke<ArrayBuffer>("video_audio", { path }),
    videoFrames: (path: string, start: number, count: number) =>
      invoke<ArrayBuffer>("video_frames", { path, start, count }),
  };
}

export type Api = ReturnType<typeof makeApi>;
