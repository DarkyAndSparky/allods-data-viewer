import { describe, expect, it } from "vitest";
import { baseName, commonPrefix, isJunkName, isJunkPath, errorKey, formatDuration, joinPath } from "./paths";
import { toAppError, insideApp } from "./api";

describe("paths", () => {
  it("joins and splits", () => {
    expect(joinPath("", "a", "", "b.txt")).toBe("a/b.txt");
    expect(joinPath("")).toBe("");
    expect(baseName("a/b/c.bin")).toBe("c.bin");
    expect(baseName("c.bin")).toBe("c.bin");
  });

  it("maps every app error code to a key", () => {
    expect(errorKey("not_found")).toBe("errNotFound");
    expect(errorKey("bad_path")).toBe("errBadPath");
    expect(errorKey("wrong_kind")).toBe("errWrongKind");
    expect(errorKey("failed")).toBe("errFailed");
    expect(errorKey("no_session")).toBe("errNoSession");
    expect(errorKey("whatever")).toBe("errUnknown");
  });

  it("formats durations", () => {
    expect(formatDuration(0)).toBe("0:00");
    expect(formatDuration(65.4)).toBe("1:05");
    expect(formatDuration(-3)).toBe("0:00");
  });
});

describe("api helpers", () => {
  it("normalises errors", () => {
    expect(toAppError({ code: "failed", detail: "x" })).toEqual({ code: "failed", detail: "x" });
    expect(toAppError(new Error("boom"))).toEqual({ code: "unknown", detail: "boom" });
    expect(toAppError("text").code).toBe("unknown");
  });

  it("detects the desktop app", () => {
    expect(insideApp({})).toBe(false);
    expect(insideApp({ __TAURI_INTERNALS__: {} })).toBe(true);
  });
});

describe("makeApi", () => {
  it("passes command names and arguments to invoke", async () => {
    const { makeApi } = await import("./api");
    const calls: [string, unknown][] = [];
    const api = makeApi(async <T>(c: string, a?: Record<string, unknown>) => {
      calls.push([c, a]);
      return [] as unknown as T;
    });
    await api.listDir("a/b");
    await api.preview("x.bin");
    await api.preview("x.bin", 2);
    await api.audio("s.bsb", 1);
    await api.mesh("m.bin");
    await api.search("abc");
    await api.search("abc", "Packs/A.pak");
    await api.searchText("abc", "Packs");
    await api.fileKinds(["a"]);
    await api.saveAs({ path: "p", format: "obj", dest: "/x.obj" });
    expect(calls).toEqual([
      ["list_dir", { path: "a/b" }],
      ["preview", { path: "x.bin", textureIndex: null }],
      ["preview", { path: "x.bin", textureIndex: 2 }],
      ["get_audio", { path: "s.bsb", index: 1 }],
      ["get_mesh", { path: "m.bin" }],
      ["search", { query: "abc", scope: null }],
      ["search", { query: "abc", scope: "Packs/A.pak" }],
      ["search_text", { query: "abc", scope: "Packs" }],
      ["file_kinds", { paths: ["a"] }],
      ["save_as", { path: "p", format: "obj", dest: "/x.obj", textureIndex: null, index: null }],
    ]);
  });
});

describe("withExtension", () => {
  it("replaces or adds an extension", async () => {
    const { withExtension } = await import("./dialogs");
    expect(withExtension("a.bin", "png")).toBe("a.png");
    expect(withExtension("a.hi.bin", "obj")).toBe("a.hi.obj");
    expect(withExtension("noext", "wav")).toBe("noext.wav");
    expect(withExtension(".hidden", "txt")).toBe(".hidden.txt");
  });
});

describe("commonPrefix", () => {
  it("finds the shared start ignoring case and keeps the first spelling", () => {
    expect(commonPrefix(["C:\\Data\\Packs\\", "c:\\data\\Pack2\\"])).toBe("C:\\Data\\Pack");
    expect(commonPrefix(["abc"])).toBe("abc");
    expect(commonPrefix([])).toBe("");
    expect(commonPrefix(["a", "b"])).toBe("");
  });
});

describe("junk folders", () => {
  it("recognises mods and addons folders in any case", () => {
    expect(isJunkName("Mods")).toBe(true);
    expect(isJunkName("addons")).toBe(true);
    expect(isJunkName("Modsy")).toBe(false);
    expect(isJunkPath("data/Mods/Docs/a.txt")).toBe(true);
    expect(isJunkPath("Packs/Addons/x.bin")).toBe(true);
    expect(isJunkPath("Packs/World/a.bin")).toBe(false);
    expect(isJunkPath("Mods")).toBe(false);
  });
});
