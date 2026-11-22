import pkg from "../package.json";
import { renderAbout } from "./about";
import thirdParty from "./third-party.json";
import { createPlayer, type Player } from "./player";
import { insideApp, makeApi, trackBusy, type Census, type GameVersion, toAppError, type Api, type AppError, type Invoke, type Item, type Kind, type LocPage, type Preview } from "./api";
import { tauriDialogs, withExtension, type Dialogs } from "./dialogs";
import { h } from "./dom";
import { formatBytes } from "./format";
import { detectLang, t, tf, type Key, type Lang } from "./i18n";
import { parseMesh } from "./mesh";
import { ModelViewer } from "./viewer";
import { filterItems, sortItems, withKinds, type Show, type SortBy } from "./view";
import { baseName, commonPrefix, isJunkName, isJunkPath, errorKey, formatDuration, joinPath } from "./paths";

const SEARCH_LIMIT = 300;
const TEXT_HIT_LIMIT = 300;
const KIND_LIMIT = 1000;
const REPO_URL = "https://github.com/DarkyAndSparky/allods-data-viewer";
const KIND_KEYS: Record<string, Key | undefined> = {
  text: "kindText",
  texture: "kindTexture",
  model: "kindModel",
  animation: "kindAnimation",
  audio: "kindAudio",
  video: "kindVideo",
  database: "kindDatabase",
};

function saved(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function save(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* storage can be unavailable; the app works without it */
  }
}

/** The whole window. `invoke` is injected so the UI can be tested without the desktop app. */
export class App {
  private lang: Lang;
  private api: Api | null;
  private rootLabel = "";
  private selected = "";
  /** Folder shown in the main view (Explorer-like), or "" when a file or nothing is shown. */
  private folderView: string | null = null;
  /** Folders opened in the folder view, with the position of the one shown (back/forward). */
  private folderHistory: string[] = [];
  private folderPos = -1;
  private urls: string[] = [];
  private player: Player | null = null;
  private busyTimer: number | null = null;
  private viewer: ModelViewer | null = null;
  private tree = h("div", { class: "tree", attrs: { role: "tree" } });
  private results = h("div", { class: "results", attrs: { hidden: "" } });
  private searchTimer = 0;
  private show: Show = "all";
  private sort: SortBy = "name";
  private query = "";
  private inTexts = false;
  /** Virtual path of the folder last clicked in the tree; search can be limited to it. */
  private scope = "";
  private scopeOnly = false;
  private hideJunk = saved("hideJunk") === "1";
  private scopeBox: HTMLInputElement | null = null;
  private scopeText: HTMLElement | null = null;
  private kinds = new Map<string, Kind>();
  private census: Census | null = null;
  private gameVer: GameVersion | null = null;
  private kindView: string | null = null;
  private stats = h("footer", { class: "statusbar" });
  /** The sound player pinned to the bottom of the window, shared by every sound bank. */
  private dockAudio = h("audio", { attrs: { controls: "" } }) as HTMLAudioElement;
  private dockTitle = h("span", { class: "dock-title" });
  private dockSpinner = h("span", { class: "spinner small", attrs: { role: "status", hidden: "" } });
  private dock = h("div", { class: "dock", attrs: { hidden: "" } });
  private dockLoopBox = h("input", { attrs: { type: "checkbox" } }) as HTMLInputElement;
  private dockLoopText = h("span");
  private dockLoop = h("label", { class: "loop" }, this.dockLoopBox, this.dockLoopText);
  private view = h("section", { class: "view" });
  private error = h("div", { class: "error", attrs: { role: "alert", hidden: "" } });
  private notice = h("div", { class: "notice", attrs: { role: "status", hidden: "" } });

  constructor(
    private host: HTMLElement,
    invoke: Invoke | null,
    languages: readonly string[],
    private base = "/",
    private dialogs: Dialogs | null = null,
  ) {
    const stored = saved("lang");
    this.lang = stored === "ru" || stored === "en" ? stored : detectLang(languages);
    this.api = invoke ? makeApi(trackBusy(invoke, (n) => this.setBusy(n))) : null;
  }

  /** Shows the activity bar when calls stay pending for more than a short moment. */
  private setBusy(pending: number): void {
    if (pending > 0) {
      if (this.busyTimer === null) {
        this.busyTimer = window.setTimeout(() => document.body.classList.add("busy"), 300);
      }
      return;
    }
    if (this.busyTimer !== null) window.clearTimeout(this.busyTimer);
    this.busyTimer = null;
    document.body.classList.remove("busy");
  }

  /** Builds the window. */
  mount(): void {
    this.render();
  }

  /** Forgets the opened folder or file and returns to the empty screen. */
  private clearOpened(): void {
    window.clearTimeout(this.searchTimer);
    this.stopAudio();
    this.closeViewer();
    this.rootLabel = "";
    this.selected = "";
    this.scope = "";
    this.updateScope();
    this.query = "";
    this.census = null;
    this.gameVer = null;
    this.kindView = null;
    this.kinds.clear();
    save("path", "");
    this.clearError();
    this.render();
  }

  /** Remembers the folder the search can be limited to and refreshes the checkbox. */
  private setScope(path: string): void {
    this.scope = path;
    this.updateScope();
    if (this.scopeOnly && this.query !== "") void this.runSearch(this.query);
  }

  private updateScope(): void {
    if (!this.scopeBox || !this.scopeText) return;
    this.scopeBox.disabled = !this.api || this.scope === "";
    this.scopeText.textContent =
      this.scope === "" ? this.tr("searchScope") : tf(this.lang, "searchScopeNamed", { name: this.scope });
    this.scopeText.title = this.scope;
  }

  /** The folder to search in, or nothing for the whole data. */
  private searchScope(): string | undefined {
    return this.scopeOnly && this.scope !== "" ? this.scope : undefined;
  }

  /** Clears the selected file: no highlight, no preview, back to the hint. */
  private deselect(): void {
    this.stopAudio();
    this.closeViewer();
    this.selected = "";
    for (const list of [this.tree, this.results]) list.querySelector(".node.selected")?.classList.remove("selected");
    this.showHint("selectFile");
  }

  /** Opens a path (used by the Open button and by drag and drop). */
  async open(path: string): Promise<void> {
    if (!this.api || path.trim() === "") return;
    this.clearError();
    try {
      this.rootLabel = await this.api.openPath(path.trim());
      save("path", path.trim());
      this.selected = "";
      this.scope = "";
      this.updateScope();
      this.showHint("selectFile");
      await this.loadRoot();
      void this.loadCensus();
    } catch (e) {
      this.showError(toAppError(e));
    }
  }

  private tr(key: Key): string {
    return t(this.lang, key);
  }

  private render(): void {
    const { lang } = this;
    document.documentElement.lang = lang;
    document.title = "Allods Data Viewer";
    const input = h("input", {
      class: "path",
      attrs: {
        type: "text",
        id: "path",
        placeholder: this.tr("pathPlaceholder"),
        "aria-label": this.tr("pathLabel"),
        spellcheck: "false",
      },
    });
    input.value = saved("path") ?? "";
    const openBtn = h("button", { class: "btn", text: this.tr("open"), attrs: { type: "button" } });
    const go = () => void this.open(input.value);
    openBtn.addEventListener("click", go);
    const suggest = h("div", { class: "suggest", attrs: { role: "listbox", hidden: "" } });
    const closeSuggest = (): void => {
      suggest.hidden = true;
      suggest.replaceChildren();
    };
    const complete = async (): Promise<void> => {
      if (!this.api) return;
      let found: string[] = [];
      try {
        found = await this.api.completePath(input.value);
      } catch {
        /* completion is a convenience; ignore failures */
      }
      if (found.length === 0) return closeSuggest();
      const common = commonPrefix(found);
      if (common.length > input.value.length) input.value = common;
      if (found.length === 1) return closeSuggest();
      suggest.replaceChildren(
        ...found.map((f) => {
          const item = h("button", { class: "suggest-item", text: f, attrs: { type: "button", role: "option" } });
          item.addEventListener("click", () => {
            input.value = f;
            closeSuggest();
            input.focus();
          });
          return item;
        }),
      );
      suggest.hidden = false;
    };
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        closeSuggest();
        go();
      } else if (e.key === "Tab" && !e.shiftKey && input.value !== "") {
        e.preventDefault();
        void complete();
      } else if (e.key === "Escape") closeSuggest();
    });
    input.addEventListener("blur", () => setTimeout(closeSuggest, 150));
    const folderBtn = h("button", { class: "btn", text: this.tr("openFolder"), attrs: { type: "button" } });
    folderBtn.disabled = !this.api || !this.dialogs;
    folderBtn.addEventListener("click", async () => {
      const dir = await this.dialogs?.pickFolder();
      if (dir) {
        input.value = dir;
        void this.open(dir);
      }
    });
    const fileBtn = h("button", { class: "btn", text: this.tr("openFile"), attrs: { type: "button" } });
    fileBtn.disabled = !this.api || !this.dialogs;
    fileBtn.addEventListener("click", async () => {
      const file = await this.dialogs?.pickFile([
        { name: this.tr("filterPak"), extensions: ["pak", "zip"] },
        { name: this.tr("filterAll"), extensions: ["*"] },
      ]);
      if (file) {
        input.value = file;
        void this.open(file);
      }
    });
    const clearBtn = h("button", {
      class: "btn",
      text: this.tr("clearPath"),
      attrs: { type: "button", title: this.tr("clearPathTip") },
    });
    clearBtn.disabled = !this.api;
    clearBtn.addEventListener("click", () => {
      input.value = "";
      this.clearOpened();
    });
    const langSel = h("select", { class: "lang", attrs: { "aria-label": this.tr("language") } });
    for (const [code, label] of [["en", "English"], ["ru", "Русский"]] as const) {
      const o = h("option", { text: label, attrs: { value: code } });
      o.selected = code === lang;
      langSel.append(o);
    }
    langSel.addEventListener("change", () => {
      this.lang = langSel.value === "ru" ? "ru" : "en";
      save("lang", this.lang);
      this.stopAudio();
      this.closeViewer();
      this.render();
      if (this.api && this.rootLabel) void this.loadRoot();
    });
    const aboutBtn = h("button", { class: "btn", text: this.tr("about"), attrs: { type: "button" } });
    const dialog = h("dialog", { attrs: { "aria-labelledby": "about-title" } });
    dialog.innerHTML = renderAbout({
      version: pkg.version,
      lang,
      repoUrl: REPO_URL,
      logoSrc: `${this.base}logo.png`,
      components: thirdParty.components,
    });
    dialog.addEventListener("click", (e) => {
      if (e.target === dialog) dialog.close();
    });
    aboutBtn.addEventListener("click", () => dialog.showModal());

    const top = h(
      "header",
      { class: "topbar" },
      h("img", { attrs: { src: `${this.base}icon.png`, alt: "" } }),
      h("span", { class: "title", text: "Allods Data Viewer" }),
      h("div", { class: "pathbox" }, input, suggest),
      openBtn,
      folderBtn,
      fileBtn,
      clearBtn,
      langSel,
      aboutBtn,
    );
    this.tree = h("div", { class: "tree", attrs: { role: "tree" } });
    this.tree.addEventListener("keydown", (e) => this.onTreeKey(e));
    this.view = h("section", { class: "view" });
    this.error = h("div", { class: "error", attrs: { role: "alert", hidden: "" } });
    this.notice = h("div", { class: "notice", attrs: { role: "status", hidden: "" } });
    this.results = h("div", { class: "results", attrs: { hidden: "" } });
    this.results.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && e.target instanceof HTMLElement && e.target.classList.contains("node")) this.deselect();
    });
    const search = h("input", {
      class: "search",
      attrs: {
        type: "search",
        placeholder: this.tr("searchPlaceholder"),
        "aria-label": this.tr("searchLabel"),
        spellcheck: "false",
      },
    });
    search.disabled = !this.api;
    search.addEventListener("input", () => {
      window.clearTimeout(this.searchTimer);
      this.searchTimer = window.setTimeout(() => void this.runSearch(search.value), 300);
    });
    const option = (value: string, label: Key, current: string) => {
      const o = h("option", { text: this.tr(label), attrs: { value } });
      o.selected = value === current;
      return o;
    };
    const showSel = h("select", { attrs: { "aria-label": this.tr("showLabel") } });
    showSel.append(
      option("all", "showAll", this.show),
      option("text", "showText", this.show),
      option("texture", "showTexture", this.show),
      option("model", "showModel", this.show),
      option("animation", "showAnimation", this.show),
      option("audio", "showAudio", this.show),
      option("video", "showVideo", this.show),
      option("database", "showDatabase", this.show),
    );
    const sortSel = h("select", { attrs: { "aria-label": this.tr("sortLabel") } });
    sortSel.append(
      option("name", "sortName", this.sort),
      option("size", "sortSize", this.sort),
      option("kind", "sortKind", this.sort),
    );
    showSel.disabled = sortSel.disabled = !this.api;
    const reload = () => {
      this.show = showSel.value as Show;
      this.sort = sortSel.value as SortBy;
      if (this.query !== "") void this.runSearch(this.query);
      else if (this.api && this.rootLabel) void this.loadRoot();
    };
    showSel.addEventListener("change", reload);
    sortSel.addEventListener("change", reload);
    const textsBox = h("input", { attrs: { type: "checkbox" } });
    textsBox.disabled = !this.api;
    textsBox.checked = this.inTexts;
    textsBox.addEventListener("change", () => {
      this.inTexts = textsBox.checked;
      if (this.query !== "") void this.runSearch(this.query);
    });
    const textsLabel = h(
      "label",
      { class: "check", attrs: { title: this.tr("searchInTextsTip") } },
      textsBox,
      h("span", { text: this.tr("searchInTexts") }),
    );
    const junkBox = h("input", { attrs: { type: "checkbox" } });
    junkBox.checked = this.hideJunk;
    junkBox.disabled = !this.api;
    junkBox.addEventListener("change", () => {
      this.hideJunk = junkBox.checked;
      save("hideJunk", this.hideJunk ? "1" : "0");
      if (this.query !== "") void this.runSearch(this.query);
      else if (this.api && this.rootLabel) void this.loadRoot();
    });
    const junkLabel = h("label", { class: "check" }, junkBox, h("span", { text: this.tr("hideJunk") }));
    const scopeBox = h("input", { attrs: { type: "checkbox" } });
    const scopeText = h("span", { text: "" });
    scopeBox.checked = this.scopeOnly;
    scopeBox.addEventListener("change", () => {
      this.scopeOnly = scopeBox.checked;
      if (this.query !== "") void this.runSearch(this.query);
    });
    this.scopeBox = scopeBox;
    this.scopeText = scopeText;
    this.updateScope();
    const scopeLabel = h("label", { class: "check" }, scopeBox, scopeText);
    const labelled = (key: Key, sel: HTMLElement) =>
      h("label", { class: "field" }, h("span", { class: "lbl", text: this.tr(key) }), sel);
    const filters = h("div", { class: "filters" }, labelled("showLabel", showSel), labelled("sortLabel", sortSel));
    this.host.replaceChildren(
      top,
      this.error,
      this.notice,
      h(
        "main",
        { class: "split" },
        h(
          "nav",
          { class: "side" },
          h("div", { class: "sidehead" }, search, textsLabel, scopeLabel, junkLabel, filters),
          this.tree,
          this.results,
        ),
        this.makeSplitter(),
        this.view,
      ),
      this.makeDock(),
      this.stats,
      dialog,
    );
    this.renderStats();
    if (!this.api) {
      this.showHintText(this.tr("emptyTitle"), this.tr("browserOnly"));
    } else if (this.rootLabel) {
      this.view.replaceChildren(h("p", { class: "muted", text: this.tr("selectFile") }));
    } else {
      this.showHintText(this.tr("emptyTitle"), this.tr("emptyHint"));
      if (this.dialogs) {
        const big = h("button", { class: "btn big", text: this.tr("openFolder"), attrs: { type: "button" } });
        big.addEventListener("click", () => folderBtn.click());
        this.view.append(big);
      }
    }
  }

  /** Draggable divider between the file list and the viewer; a click folds the list away. */
  private makeSplitter(): HTMLElement {
    const MIN = 224;
    const MAX = 900;
    const read = (k: string): string | null => {
      try {
        return localStorage.getItem(k);
      } catch {
        return null;
      }
    };
    const write = (k: string, v: string): void => {
      try {
        localStorage.setItem(k, v);
      } catch {
        /* storage may be blocked; the layout still works */
      }
    };
    const saved = Number(read("adv.sideWidth"));
    let width = Number.isFinite(saved) && saved >= MIN && saved <= MAX ? saved : 360;
    let collapsed = read("adv.sideCollapsed") === "1";
    const main = (): HTMLElement | null => this.host.querySelector("main.split");
    const apply = (): void => {
      const m = main();
      if (!m) return;
      m.style.setProperty("--side-w", `${width}px`);
      m.classList.toggle("collapsed", collapsed);
      toggle.textContent = collapsed ? "\u203A" : "\u2039";
      const label = this.tr(collapsed ? "sideExpand" : "sideCollapse");
      toggle.title = label;
      toggle.setAttribute("aria-label", label);
      bar.setAttribute("aria-expanded", String(!collapsed));
    };
    const toggle = h("button", { class: "split-toggle", attrs: { type: "button" } });
    const bar = h("div", {
      class: "splitter",
      attrs: { role: "separator", "aria-orientation": "vertical", tabindex: "0", title: this.tr("sideResize") },
    }, toggle);
    const setWidth = (w: number): void => {
      width = Math.min(MAX, Math.max(MIN, Math.round(w)));
      write("adv.sideWidth", String(width));
      apply();
    };
    const fold = (): void => {
      collapsed = !collapsed;
      write("adv.sideCollapsed", collapsed ? "1" : "0");
      apply();
    };
    toggle.addEventListener("click", (e) => {
      e.stopPropagation();
      fold();
    });
    toggle.addEventListener("pointerdown", (e) => e.stopPropagation());
    bar.addEventListener("dblclick", fold);
    bar.addEventListener("pointerdown", (e) => {
      if (collapsed) return;
      e.preventDefault();
      bar.setPointerCapture(e.pointerId);
      const left = main()?.getBoundingClientRect().left ?? 0;
      const move = (ev: PointerEvent): void => setWidth(ev.clientX - left);
      const up = (): void => {
        bar.removeEventListener("pointermove", move);
        bar.removeEventListener("pointerup", up);
        bar.removeEventListener("pointercancel", up);
      };
      bar.addEventListener("pointermove", move);
      bar.addEventListener("pointerup", up);
      bar.addEventListener("pointercancel", up);
    });
    bar.addEventListener("keydown", (e) => {
      if (e.key === "ArrowLeft") setWidth(width - 24);
      else if (e.key === "ArrowRight") setWidth(width + 24);
      else if (e.key === "Enter" || e.key === " ") fold();
      else return;
      e.preventDefault();
    });
    queueMicrotask(apply);
    return bar;
  }

  private showHint(key: Key): void {
    this.view.replaceChildren(h("p", { class: "muted", text: this.tr(key) }));
  }

  private showHintText(title: string, hint: string): void {
    this.view.replaceChildren(h("h2", { text: title }), h("p", { class: "muted", text: hint }));
  }

  private clearError(): void {
    this.error.hidden = true;
    this.error.replaceChildren();
    this.notice.hidden = true;
  }

  /** A "Save as…" button; the dialog and the export both run through the app commands. */
  private saveButton(
    label: Key,
    filter: Key,
    ext: string,
    format: "raw" | "png" | "wav" | "mp3" | "obj" | "glb" | "txt" | "html" | "xml",
    path: string,
    extra: { textureIndex?: number; index?: number; name?: string } = {},
  ): HTMLElement {
    const btn = h("button", { class: "btn", text: this.tr(label), attrs: { type: "button", title: this.tr(label) } });
    btn.disabled = !this.dialogs;
    btn.addEventListener("click", async () => {
      if (!this.api || !this.dialogs) return;
      const name = format === "raw" ? baseName(path) : withExtension(extra.name ?? baseName(path), ext);
      const filters = [{ name: this.tr(filter), extensions: [ext] }];
      const dest = await this.dialogs.saveTo(name, filters);
      if (!dest) return;
      this.clearError();
      try {
        await this.api.saveAs({ path, format, dest, ...extra });
        this.notice.textContent = tf(this.lang, "saved", { path: dest });
        this.notice.hidden = false;
      } catch (e) {
        this.showError(toAppError(e));
      }
    });
    return btn;
  }

  private showError(e: AppError): void {
    const details = h(
      "details",
      {},
      h("summary", { text: this.tr("errDetails") }),
      h("code", { text: e.detail }),
    );
    this.error.replaceChildren(h("strong", { text: this.tr(errorKey(e.code)) }), details);
    this.error.hidden = false;
  }

  /** Lists file paths as buttons in the results pane. */
  private showHits(hits: string[], limit: number, limitKey: Key): void {
    this.results.replaceChildren();
    if (hits.length === 0) this.results.append(h("p", { class: "muted", text: this.tr("noResults") }));
    if (hits.length > 0) {
      this.results.append(h("p", { class: "muted", text: tf(this.lang, "foundCount", { count: hits.length }) }));
    }
    if (hits.length >= limit) {
      this.results.append(h("p", { class: "muted", text: tf(this.lang, limitKey, { count: limit }) }));
    }
    for (const path of hits) {
      const btn = h(
        "button",
        { class: "node file", attrs: { type: "button", title: path } },
        h("span", { class: "name", text: baseName(path) }),
        h("span", { class: "meta", text: path.slice(0, Math.max(0, path.length - baseName(path).length - 1)) }),
      );
      btn.addEventListener("click", () => {
        this.results.querySelector(".node.selected")?.classList.remove("selected");
        btn.classList.add("selected");
        void this.select(path);
      });
      this.results.append(btn);
    }
  }

  /** Appends the matches found inside texts below the name matches. */
  private async showTextHits(q: string): Promise<void> {
    if (!this.api) return;
    const note = h("p", { class: "muted", text: this.tr("loading") });
    this.results.append(note);
    let hits = await this.api.searchText(q, this.searchScope());
    if (this.hideJunk) hits = hits.filter((hit) => !isJunkPath(hit.path));
    if (this.results.hidden || this.query !== q) return;
    note.remove();
    this.results.append(h("h3", { class: "group", text: this.tr("textHitsTitle") }));
    if (hits.length === 0) this.results.append(h("p", { class: "muted", text: this.tr("noResults") }));
    if (hits.length >= TEXT_HIT_LIMIT) {
      this.results.append(h("p", { class: "muted", text: tf(this.lang, "textHitsLimit", { count: TEXT_HIT_LIMIT }) }));
    }
    for (const hit of hits) {
      const btn = h(
        "button",
        { class: "node file", attrs: { type: "button", title: hit.path } },
        h("span", { class: "name", text: baseName(hit.path) }),
        h("span", { class: "meta", text: hit.path }),
        h("span", { class: "snippet", text: hit.snippet }),
      );
      btn.addEventListener("click", () => {
        this.results.querySelector(".node.selected")?.classList.remove("selected");
        btn.classList.add("selected");
        void this.select(hit.path, undefined, hit.id ?? undefined);
      });
      this.results.append(btn);
    }
  }

  /** Shows every file of one kind from the whole data; the same kind again goes back to the tree. */
  private async toggleKind(kind: string): Promise<void> {
    if (!this.api) return;
    if (this.kindView === kind) {
      this.kindView = null;
      this.renderStats();
      this.results.hidden = true;
      this.tree.hidden = this.query !== "" ? true : false;
      if (this.query !== "") void this.runSearch(this.query);
      return;
    }
    this.kindView = kind;
    this.query = "";
    for (const el of this.host.querySelectorAll<HTMLInputElement>(".search")) el.value = "";
    this.renderStats();
    this.clearError();
    this.tree.hidden = true;
    this.results.hidden = false;
    this.results.replaceChildren(h("p", { class: "muted", text: this.tr("loading") }));
    try {
      let hits = await this.api.findKind(kind);
      if (this.hideJunk) hits = hits.filter((p) => !isJunkPath(p));
      if (this.kindView !== kind) return;
      this.showHits(hits, KIND_LIMIT, "kindListLimit");
    } catch (e) {
      this.results.replaceChildren();
      this.showError(toAppError(e));
    }
  }

  private async loadCensus(): Promise<void> {
    if (!this.api) return;
    this.census = null;
    this.gameVer = null;
    this.kindView = null;
    this.renderStats();
    try {
      this.gameVer = await this.api.gameVersion();
    } catch {
      this.gameVer = null;
    }
    this.renderStats();
    try {
      const c = await this.api.census();
      this.census = c;
    } catch {
      this.census = null;
    }
    this.renderStats();
  }

  /** The bottom bar: build, totals and per-type counters that open the list of such files. */
  private renderStats(): void {
    const nf = new Intl.NumberFormat(this.lang);
    const bar = this.stats;
    const folded = saved("statsFolded") === "1";
    const fold = h("button", {
      class: "chip fold",
      text: folded ? "▴" : "▾",
      attrs: { type: "button", title: this.tr(folded ? "statusUnfold" : "statusFold"), "aria-expanded": String(!folded), "aria-label": this.tr(folded ? "statusUnfold" : "statusFold") },
    });
    fold.addEventListener("click", () => {
      save("statsFolded", folded ? "0" : "1");
      this.renderStats();
    });
    bar.replaceChildren(fold, h("span", { class: "stat", text: tf(this.lang, "statusVersion", { version: pkg.version }) }));
    if (folded || !this.api || !this.rootLabel) return;
    bar.append(
      h("span", {
        class: "stat",
        text: this.gameVer
          ? tf(this.lang, this.gameVer.revision ? "statusGameRev" : "statusGame", {
              version: this.gameVer.version,
              revision: this.gameVer.revision,
              date: this.gameVer.date,
            })
          : this.tr("statusGameUnknown"),
        attrs: this.gameVer ? { title: this.gameVer.head } : {},
      }),
    );
    const c = this.census;
    if (!c) {
      bar.append(h("span", { class: "stat muted", text: this.tr("statusCounting") }));
      return;
    }
    bar.append(
      h("span", {
        class: "stat",
        text: tf(this.lang, "statusTotals", {
          files: nf.format(c.files),
          paks: nf.format(c.paks),
          size: formatBytes(c.bytes, this.lang),
        }),
      }),
    );
    const chips = h("span", { class: "chips", attrs: { role: "group", "aria-label": this.tr("statusKindHint") } });
    for (const k of c.kinds) {
      const key = k.kind === "other" ? "kindOther" : KIND_KEYS[k.kind];
      const chip = h("button", {
        class: `chip${this.kindView === k.kind ? " active" : ""}`,
        text: `${key ? this.tr(key) : k.kind}: ${nf.format(k.count)}`,
        attrs: {
          type: "button",
          "aria-pressed": String(this.kindView === k.kind),
          title: `${formatBytes(k.bytes, this.lang)}${k.kind === "other" ? `. ${this.tr("kindOtherTip")}` : ""}`,
        },
      });
      chip.addEventListener("click", () => void this.toggleKind(k.kind));
      chips.append(chip);
    }
    bar.append(chips);
  }

  private async runSearch(query: string): Promise<void> {
    if (!this.api) return;
    const q = query.trim();
    this.query = q;
    if (q !== "" && this.kindView !== null) {
      this.kindView = null;
      this.renderStats();
    }
    if (q === "" && this.kindView !== null) return;
    this.tree.hidden = q !== "";
    this.results.hidden = q === "";
    if (q === "") return;
    this.clearError();
    this.results.replaceChildren(h("p", { class: "muted", text: this.tr("loading") }));
    try {
      let hits = await this.api.search(q, this.searchScope());
      if (this.hideJunk) hits = hits.filter((p) => !isJunkPath(p));
      if (this.results.hidden) return;
      if (this.show !== "all" || this.sort === "kind") {
        await this.learnKinds(hits.filter((p) => !this.kinds.has(p)));
        if (this.results.hidden) return;
      }
      if (this.show !== "all") hits = hits.filter((p) => this.kinds.get(p) === this.show);
      if (this.sort === "kind") {
        hits = [...hits].sort(
          (a, b) => (this.kinds.get(a) ?? "").localeCompare(this.kinds.get(b) ?? "") || a.localeCompare(b),
        );
      }
      this.showHits(hits, SEARCH_LIMIT, "resultsLimit");
      if (this.inTexts) await this.showTextHits(q);
    } catch (e) {
      this.results.replaceChildren();
      this.showError(toAppError(e));
    }
  }

  private async loadRoot(): Promise<void> {
    this.tree.replaceChildren(h("div", { class: "root", text: `${this.tr("opened")}: ${this.rootLabel}` }));
    const list = h("ul", { attrs: { role: "group" } });
    this.tree.append(list);
    await this.fill(list, "");
  }

  /** Looks into files whose real kind the name cannot tell (a `.bin` is a texture or a model). */
  private async learnKinds(paths: string[]): Promise<void> {
    if (!this.api) return;
    const todo = paths.filter((p) => p.toLowerCase().endsWith(".bin") || !p.includes("."));
    for (let i = 0; i < todo.length; i += 400) {
      for (const [p, k] of await this.api.fileKinds(todo.slice(i, i + 400))) this.kinds.set(p, k);
    }
    for (const p of paths) if (!this.kinds.has(p)) this.kinds.set(p, "other");
  }

  private async fill(list: HTMLElement, path: string): Promise<void> {
    if (!this.api) return;
    list.replaceChildren(h("li", { class: "muted", text: this.tr("loading") }));
    try {
      let items = await this.api.listDir(path);
      const unknown = items
        .filter((i) => !i.is_dir && i.kind === "other")
        .map((i) => joinPath(path, i.name))
        .filter((p) => !this.kinds.has(p));
      if (this.show !== "all" || this.sort === "kind") await this.learnKinds(unknown);
      if (this.hideJunk) items = items.filter((i) => !(i.is_dir && isJunkName(i.name)));
      items = sortItems(filterItems(withKinds(items, this.kinds, path), this.show), this.sort);
      list.replaceChildren();
      if (items.length === 0) list.append(h("li", { class: "muted", text: this.tr("emptyFolder") }));
      for (const item of items) list.append(this.row(item, joinPath(path, item.name)));
    } catch (e) {
      list.replaceChildren();
      this.showError(toAppError(e));
    }
  }

  /** Arrow keys in the tree: Up/Down move, Right opens or enters a folder, Left closes it or goes to its parent. */
  private onTreeKey(e: KeyboardEvent): void {
    const cur = e.target;
    if (!(cur instanceof HTMLElement) || !cur.classList.contains("node")) return;
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const nodes = [...this.tree.querySelectorAll<HTMLElement>(".node")].filter((n) => n.closest("ul[hidden]") === null);
    const i = nodes.indexOf(cur);
    const isDir = cur.classList.contains("dir");
    const expanded = cur.getAttribute("aria-expanded") === "true";
    let next: HTMLElement | undefined;
    switch (e.key) {
      case "ArrowDown": next = nodes[i + 1]; break;
      case "ArrowUp": next = nodes[i - 1]; break;
      case "Escape":
        e.preventDefault();
        this.deselect();
        return;
      case "Home": next = nodes[0]; break;
      case "End": next = nodes[nodes.length - 1]; break;
      case "ArrowRight":
        if (isDir && !expanded) cur.click();
        else if (isDir) next = nodes[i + 1];
        break;
      case "ArrowLeft":
        if (isDir && expanded) cur.click();
        else next = cur.closest("ul")?.parentElement?.querySelector<HTMLElement>(":scope > .node") ?? undefined;
        break;
      default: return;
    }
    e.preventDefault();
    next?.focus();
  }

  private row(item: Item, path: string): HTMLElement {
    const li = h("li", { attrs: { role: "treeitem" } });
    const kindKey = KIND_KEYS[item.kind];
    const meta =
      item.is_pak ? this.tr("archive")
      : item.is_dir ? (item.files !== null ? tf(this.lang, "files", { count: item.files }) : "")
      : [kindKey ? this.tr(kindKey) : "", item.size !== null ? formatBytes(item.size, this.lang) : ""]
          .filter((s) => s !== "")
          .join(", ");
    const label = h(
      "button",
      { class: `node ${item.is_dir ? "dir" : "file"}`, attrs: { type: "button", title: item.name } },
      h("span", { class: "twisty", text: item.is_dir ? "▸" : "·" }),
      h("span", { class: "name", text: item.name }),
      h("span", { class: "meta", text: meta }),
    );
    li.append(label);
    if (item.is_dir) {
      const sub = h("ul", { attrs: { role: "group", hidden: "" } });
      li.append(sub);
      let loaded = false;
      label.setAttribute("aria-expanded", "false");
      label.addEventListener("click", () => {
        const open = sub.hidden;
        sub.hidden = !open;
        label.setAttribute("aria-expanded", String(open));
        this.setScope(path);
        const twisty = label.querySelector(".twisty");
        if (twisty) twisty.textContent = open ? "▾" : "▸";
        if (open && !loaded) {
          loaded = true;
          void this.fill(sub, path);
        }
      });
      label.addEventListener("dblclick", () => void this.showFolder(path));
    } else {
      label.addEventListener("click", () => {
        this.tree.querySelector(".node.selected")?.classList.remove("selected");
        label.classList.add("selected");
        void this.select(path);
      });
    }
    return li;
  }

  private revoke(): void {
    for (const u of this.urls) URL.revokeObjectURL(u);
    this.urls = [];
  }

  private blobUrl(data: ArrayBuffer, type: string): string {
    const u = URL.createObjectURL(new Blob([data], { type }));
    this.urls.push(u);
    return u;
  }

  /**
   * An SVG file. The core has already sanitised it (allow-list of elements and attributes); the page
   * draws that clean copy as an image, where scripts and outside links do not run either.
   * The original text is one click away.
   */
  private svgView(path: string, svg: string, source: string, removed: number, names: string[]): HTMLElement {
    const pictureBtn = h("button", { class: "btn", text: this.tr("svgPicture"), attrs: { type: "button" } });
    const sourceBtn = h("button", { class: "btn", text: this.tr("svgSource"), attrs: { type: "button" } });
    const note = h("p", { class: "muted" });
    const img = h("img", {
      class: "picture",
      attrs: { src: this.blobUrl(new TextEncoder().encode(svg).buffer, "image/svg+xml"), alt: baseName(path) },
    });
    const original = h("pre", { class: "text", text: source });
    const show = (picture: boolean): void => {
      img.hidden = !picture;
      original.hidden = picture;
    };
    img.addEventListener("error", () => {
      note.textContent = this.tr("svgBroken");
      show(false);
    });
    pictureBtn.addEventListener("click", () => {
      note.textContent = "";
      show(true);
    });
    sourceBtn.addEventListener("click", () => show(false));
    show(true);
    const cleaned =
      removed > 0
        ? h("p", { class: "muted", text: tf(this.lang, "svgCleaned", { n: removed, names: names.join(", ") }) })
        : h("p", { class: "muted", text: this.tr("svgClean") });
    return h("div", {}, h("div", { class: "toolbar" }, pictureBtn, sourceBtn), cleaned, note, img, original);
  }

  private closeViewer(): void {
    this.viewer?.dispose();
    this.viewer = null;
  }

  /** The in-window player for a video; the app decodes, the page only converts and shows. */
  private videoPlayer(path: string): HTMLElement {
    const api = this.api;
    if (!api) return h("p", { class: "muted", text: this.tr("videoNote") });
    this.player?.dispose();
    this.player = createPlayer({
      open: () => api.videoOpen(path),
      frames: (start, count) => api.videoFrames(path, start, count),
      audio: () => api.videoAudio(path),
      onPlay: () => {
        this.dockAudio.pause();
        this.clearError();
      },
      label: (key) => this.tr(key),
    });
    return this.player.el;
  }

  private makeDock(): HTMLElement {
    const close = h("button", { class: "btn", text: "×", attrs: { type: "button" } });
    close.addEventListener("click", () => {
      this.dockAudio.pause();
      this.dock.setAttribute("hidden", "");
    });
    this.dockAudio.loop = false;
    this.dockLoopBox.addEventListener("change", () => {
      this.dockAudio.loop = this.dockLoopBox.checked;
    });
    this.dock.replaceChildren(this.dockTitle, this.dockSpinner, this.dockAudio, this.dockLoop, close);
    this.refreshDockLabels();
    return this.dock;
  }

  private refreshDockLabels(): void {
    this.dockLoopText.textContent = this.tr("loop");
    const close = this.dock.lastElementChild;
    if (close) {
      close.setAttribute("title", this.tr("closePlayer"));
      close.setAttribute("aria-label", this.tr("closePlayer"));
    }
    this.dockSpinner.setAttribute("aria-label", this.tr("audioLoading"));
  }

  private stopAudio(): void {
    this.player?.dispose();
    this.player = null;
  }

  /** Clickable path: the data root and every folder of `path` (`withLast` includes the last part). */
  private crumbs(path: string, withLast: boolean): HTMLElement {
    const nav = h("nav", { class: "crumbs", attrs: { "aria-label": this.tr("crumbsLabel") } });
    const parts = path === "" ? [] : path.split("/");
    const shown = withLast ? parts.length : Math.max(parts.length - 1, 0);
    const add = (text: string, target: string, current: boolean): void => {
      const b = h("button", { class: "crumb", text, attrs: { type: "button", title: target || text } });
      if (current) b.setAttribute("aria-current", "page");
      b.addEventListener("click", () => void this.showFolder(target));
      nav.append(b);
    };
    add(this.tr("crumbRoot"), "", shown === 0 && path === "");
    for (let i = 0; i < shown; i++) {
      nav.append(h("span", { class: "sep", text: "/" }));
      add(parts[i] ?? "", parts.slice(0, i + 1).join("/"), withLast && i === shown - 1);
    }
    return nav;
  }

  /** Explorer-like view of one folder in the main area: crumbs, "up" and the folder's contents. */
  private async showFolder(path: string, record = true): Promise<void> {
    if (!this.api) return;
    if (record && this.folderHistory[this.folderPos] !== path) {
      this.folderHistory = this.folderHistory.slice(0, this.folderPos + 1);
      this.folderHistory.push(path);
      if (this.folderHistory.length > 100) this.folderHistory.shift();
      this.folderPos = this.folderHistory.length - 1;
    }
    this.folderView = path;
    this.selected = "";
    this.clearError();
    this.stopAudio();
    this.closeViewer();
    this.revoke();
    this.setScope(path);
    this.view.replaceChildren(h("p", { class: "muted", text: this.tr("loading") }));
    try {
      let items = await this.api.listDir(path);
      if (this.folderView !== path) return;
      if (this.hideJunk) items = items.filter((i) => !(i.is_dir && isJunkName(i.name)));
      items = sortItems(filterItems(withKinds(items, this.kinds, path), this.show), this.sort);
      const head = h("div", { class: "toolbar" });
      const step = (delta: number, label: string, text: string): void => {
        const target = this.folderHistory[this.folderPos + delta];
        const b = h("button", { class: "btn", text, attrs: { type: "button", title: label, "aria-label": label } });
        if (target === undefined) b.setAttribute("disabled", "");
        b.addEventListener("click", () => {
          this.folderPos += delta;
          void this.showFolder(target ?? "", false);
        });
        head.append(b);
      };
      step(-1, this.tr("folderBack"), "←");
      step(1, this.tr("folderForward"), "→");
      if (path !== "") {
        const up = h("button", { class: "btn", text: "↑ " + this.tr("folderUp"), attrs: { type: "button" } });
        up.addEventListener("click", () => void this.showFolder(path.split("/").slice(0, -1).join("/")));
        head.append(up);
      }
      const list = h("ul", { class: "folder-list" });
      if (items.length === 0) list.append(h("li", { class: "muted", text: this.tr("emptyFolder") }));
      for (const item of items) {
        const full = joinPath(path, item.name);
        const kindKey = KIND_KEYS[item.kind];
        const meta =
          item.is_pak ? this.tr("archive")
          : item.is_dir ? (item.files !== null ? tf(this.lang, "files", { count: item.files }) : "")
          : [kindKey ? this.tr(kindKey) : "", item.size !== null ? formatBytes(item.size, this.lang) : ""]
              .filter((x) => x !== "")
              .join(", ");
        const b = h(
          "button",
          { class: `node ${item.is_dir ? "dir" : "file"}`, attrs: { type: "button", title: item.name } },
          h("span", { class: "twisty", text: item.is_dir ? "▸" : "·" }),
          h("span", { class: "name", text: item.name }),
          h("span", { class: "meta", text: meta }),
        );
        b.addEventListener("click", () => void (item.is_dir ? this.showFolder(full) : this.select(full)));
        list.append(h("li", {}, b));
      }
      this.view.replaceChildren(this.crumbs(path, true), head, list);
    } catch (e) {
      if (this.folderView === path) {
        this.view.replaceChildren();
        this.showError(toAppError(e));
      }
    }
  }

  private async select(path: string, textureIndex?: number, focusId?: number): Promise<void> {
    if (!this.api) return;
    this.folderView = null;
    this.selected = path;
    this.clearError();
    this.stopAudio();
    this.closeViewer();
    this.revoke();
    this.view.replaceChildren(h("p", { class: "muted", text: this.tr("loading") }));
    try {
      const p = await this.api.preview(path, textureIndex);
      if (this.selected !== path) return;
      const actions = h("div", { class: "toolbar" }, this.saveButton("saveRaw", "filterAll", "*", "raw", path));
      if (p.kind === "image") {
        actions.append(this.saveButton("savePng", "filterPng", "png", "png", path, { textureIndex: p.index }));
      }
      if (p.kind === "text" || p.kind === "loc") {
        actions.append(
          this.saveButton("saveTxt", "filterTxt", "txt", "txt", path),
          this.saveButton("saveHtml", "filterHtml", "html", "html", path),
          this.saveButton("saveXml", "filterXml", "xml", "xml", path),
        );
      }
      if (p.kind === "model") {
        actions.append(
          this.saveButton("saveGlb", "filterGlb", "glb", "glb", path),
          this.saveButton("saveObj", "filterObj", "obj", "obj", path),
        );
      }
      actions.classList.add("actions");
      this.view.replaceChildren(
        this.crumbs(path, false),
        h("h2", { class: "file-title", text: baseName(path) }),
        await this.body(path, p, focusId),
        actions,
      );
    } catch (e) {
      if (this.selected === path) {
        this.view.replaceChildren();
        this.showError(toAppError(e));
      }
    }
  }

  private locViewer(path: string, count: number, keys: boolean, focusId?: number): HTMLElement {
    const PAGE = 100;
    const box = h("div", { class: "loc" });
    const filter = h("input", { class: "loc-filter", attrs: { type: "search", placeholder: this.tr("locFilter") } });
    const info = h("span", { class: "muted" });
    const prev = h("button", { attrs: { type: "button" }, text: this.tr("locPrev") });
    const next = h("button", { attrs: { type: "button" }, text: this.tr("locNext") });
    const body = h("tbody");
    const head = h("tr", {}, h("th", { text: this.tr("locId") }));
    if (keys) head.append(h("th", { text: this.tr("locKey") }));
    head.append(h("th", { text: this.tr("locText") }));
    const table = h("table", { class: "loc-table" }, h("thead", {}, head), body);
    let offset = focusId === undefined ? 0 : Math.max(0, focusId - 5);
    let token = 0;
    const load = async (): Promise<void> => {
      const mine = ++token;
      let page: LocPage;
      try {
        page = await this.api!.locPage(path, filter.value.trim(), offset, PAGE);
      } catch (e) {
        this.showError(toAppError(e));
        return;
      }
      if (mine !== token) return;
      offset = page.offset;
      body.replaceChildren();
      let focus: HTMLElement | null = null;
      for (const it of page.items) {
        const tr = h("tr", {}, h("td", { text: String(it.id) }));
        if (keys) tr.append(h("td", { text: it.key ?? "" }));
        tr.append(h("td", { class: "loc-text", text: it.text }));
        if (it.id === focusId && !filter.value.trim()) {
          tr.classList.add("focus");
          focus = tr;
        }
        body.append(tr);
      }
      const end = page.offset + page.items.length;
      info.textContent = tf(this.lang, "locRange", {
        from: page.items.length ? page.offset + 1 : 0,
        to: end,
        matched: page.matched,
        total: page.total,
      });
      prev.disabled = page.offset === 0;
      next.disabled = end >= page.matched;
      (focus as HTMLElement | null)?.scrollIntoView({ block: "center" });
    };
    filter.addEventListener("input", () => {
      offset = 0;
      void load();
    });
    prev.addEventListener("click", () => {
      offset = Math.max(0, offset - PAGE);
      void load();
    });
    next.addEventListener("click", () => {
      offset += PAGE;
      void load();
    });
    box.append(h("div", { class: "toolbar" }, filter, prev, next, info), table);
    info.textContent = tf(this.lang, "locRange", { from: 0, to: 0, matched: count, total: count });
    void load();
    return box;
  }

  private async body(path: string, p: Preview, focusId?: number): Promise<HTMLElement> {
    const api = this.api;
    switch (p.kind) {
      case "loc":
        return this.locViewer(path, p.count, p.keys, focusId);
      case "bev": {
        const hex = `0x${p.version.toString(16).toUpperCase().padStart(8, "0")}`;
        const list = (title: string, items: string[]): HTMLElement[] => [
          h("h3", { text: `${title} (${items.length})` }),
          h("pre", { class: "text", text: items.join("\n") }),
        ];
        return h(
          "div",
          {},
          h("h3", { text: this.tr("bevTitle") }),
          h("p", { text: `${this.tr("bevProject")}: ${p.project}` }),
          h("p", { text: `${this.tr("bevVersion")}: ${hex}` }),
          h("p", { text: `${this.tr("bevLanguages")}: ${p.languages.join(", ")}` }),
          ...list(this.tr("bevChunks"), p.chunks.map(([tag, size]) => `${tag}  ${size}`)),
          ...list(this.tr("bevNames"), p.names),
          ...list(this.tr("bevPaths"), p.paths),
        );
      }
      case "svg":
        return this.svgView(path, p.svg, p.source, p.removed, p.removed_names);
      case "text": {
        const box = h("div", {}, h("pre", { class: "text", text: p.text }));
        if (p.truncated) box.prepend(h("p", { class: "muted", text: this.tr("textTruncated") }));
        return box;
      }
      case "binary":
        return h(
          "div",
          {},
          h("p", { class: "muted", text: tf(this.lang, "binaryInfo", { size: formatBytes(p.len, this.lang) }) }),
          h("pre", { class: "text", text: p.hex }),
        );
      case "image": {
        const box = h("div", {});
        box.append(
          h("p", { text: tf(this.lang, "imageInfo", { w: p.width, h: p.height, format: p.format }) }),
        );
        if (p.guessed && p.candidates.length > 1) {
          const sel = h("select", { attrs: { "aria-label": this.tr("sizeLabel") } });
          p.candidates.forEach(([w, hh], i) => {
            const label = p.labels[i] ? ` ${p.labels[i]}` : "";
            const o = h("option", { text: `${w}×${hh}${label}`, attrs: { value: String(i) } });
            o.selected = i === p.index;
            sel.append(o);
          });
          sel.addEventListener("change", () => void this.select(path, Number(sel.value)));
          const auto = h("button", { text: this.tr("sizeAuto"), attrs: { type: "button" } });
          auto.addEventListener("click", () => void this.select(path));
          // Ctrl+click on the size line steps through the candidates (8x512 -> 16x256 -> ...).
          const info = box.firstElementChild;
          if (info instanceof HTMLElement) {
            info.title = this.tr("sizeCycleHint");
            info.style.cursor = "pointer";
            info.addEventListener("click", (e: MouseEvent) => {
              if (!e.ctrlKey) return;
              void this.select(path, (p.index + 1) % p.candidates.length);
            });
          }
          box.append(h("p", { class: "muted", text: this.tr("sizeGuessed") }), sel, " ", auto);
        }
        if (api) {
          const png = await api.image(path, p.index);
          const img = h("img", { class: "picture", attrs: { src: this.blobUrl(png, "image/png"), alt: baseName(path) } });
          // Zoom is a percentage of the natural size; 0 means "fit to the window".
          let zoom = 0;
          const label = h("span", { class: "muted", text: "" });
          const apply = (): void => {
            img.style.maxWidth = zoom === 0 ? "100%" : "none";
            img.style.width = zoom === 0 ? "" : `${(p.width * zoom) / 100}px`;
            label.textContent = zoom === 0 ? this.tr("zoomFit") : `${zoom}%`;
          };
          const range = h("input", { attrs: { type: "range", min: "0", max: "800", step: "25", value: "0", "aria-label": this.tr("zoomLabel") } });
          range.addEventListener("input", () => { zoom = Number(range.value); apply(); });
          img.addEventListener("wheel", (e: WheelEvent) => {
            if (!e.ctrlKey) return;
            e.preventDefault();
            zoom = Math.min(800, Math.max(25, (zoom === 0 ? 100 : zoom) + (e.deltaY < 0 ? 25 : -25)));
            range.value = String(zoom);
            apply();
          }, { passive: false });
          apply();
          // Back to the default view: fit to the window, scrolled to the top left.
          const reset = h("button", { class: "btn", text: this.tr("resetView"), attrs: { type: "button" } });
          reset.addEventListener("click", () => {
            zoom = 0;
            range.value = "0";
            apply();
            this.view.scrollTo(0, 0);
          });
          box.append(h("label", { class: "muted" }, this.tr("zoomLabel") + " ", range, " ", label, " "), reset, img);
        }
        return box;
      }
      case "model": {
        const box = h("div", { class: "model" });
        box.append(
          h("p", { text: tf(this.lang, "modelInfo", {
              vertices: tf(this.lang, "vertexCount", { count: p.vertices }),
              triangles: tf(this.lang, "triangleCount", { count: p.triangles }),
            }) }),
          h("details", { class: "note" }, h("summary", { text: this.tr("modelNoteTitle") }), h("p", { class: "muted", text: this.tr("modelNote") })),
        );
        if (!api) return box;
        const stage = h("div", { class: "stage" });
        const bar = h("div", { class: "toolbar" });
        const wire = h("input", { attrs: { type: "checkbox", id: "wire" } });
        const zup = h("input", { attrs: { type: "checkbox", id: "zup" } });
        const reset = h("button", { class: "btn", text: this.tr("resetView"), attrs: { type: "button" } });
        bar.append(
          h("label", {}, wire, this.tr("wireframe")),
          h("label", {}, zup, this.tr("zUp")),
          reset,
        );
        box.append(bar, stage);
        try {
          const viewer = new ModelViewer(stage, parseMesh(await api.mesh(path)));
          this.viewer = viewer;
          wire.addEventListener("change", () => viewer.setWireframe(wire.checked));
          zup.addEventListener("change", () => viewer.setZUp(zup.checked));
          reset.addEventListener("click", () => viewer.resetView());
        } catch (e) {
          stage.replaceChildren(h("p", { class: "muted", text: this.tr("webglMissing") }));
          console.error(e);
        }
        return box;
      }
      case "animation":
        return h(
          "div",
          {},
          h("p", {
            text: tf(this.lang, "animInfo", {
              frames: tf(this.lang, "frameCount", { count: p.frames }),
              fps: p.fps,
              seconds: p.seconds.toFixed(2),
              bones: tf(this.lang, "boneCount", { count: p.bones }),
            }),
          }),
          h("p", { class: "muted", text: this.tr("animNote") }),
        );
      case "video": {
        const nf = new Intl.NumberFormat(this.lang);
        const parts = [
          tf(this.lang, "videoSize", { width: p.width, height: p.height }),
          tf(this.lang, "videoFps", { fps: nf.format(Math.round(p.fps * 100) / 100) }),
        ];
        if (p.seconds !== null) parts.push(tf(this.lang, "videoLength", { seconds: p.seconds.toFixed(1) }));
        if (p.frames !== null) parts.push(tf(this.lang, "frameCount", { count: p.frames }));
        return h(
          "div",
          {},
          this.videoPlayer(path),
          h("p", { class: "muted", text: parts.join(", ") }),
        );
      }
      case "audio": {
        const box = h("div", {}, h("p", { text: tf(this.lang, "audioInfo", { count: p.streams.length }) }));
        p.streams.forEach((s, i) => {
          const name = s.name || tf(this.lang, "unnamedStream", { n: i + 1 });
          const info = [
            s.seconds !== null ? formatDuration(s.seconds) : "",
            s.frequency !== null ? `${s.frequency} ${this.tr("hertz")}` : "",
            `${s.channels} ${this.tr("channels")}`,
          ]
            .filter((x) => x !== "")
            .join(", ");
          const btn = h("button", { class: "btn", text: this.tr("play"), attrs: { type: "button" } });
          btn.addEventListener("click", () => void this.play(path, i, name));
          const saveBtn = this.saveButton("saveWav", "filterWav", "wav", "wav", path, { index: i, name });
          const mp3Btn = this.saveButton("saveMp3", "filterMp3", "mp3", "mp3", path, { index: i, name });
          box.append(
            h(
              "div",
              { class: "stream" },
              btn,
              saveBtn,
              mp3Btn,
              h("span", { class: "name", text: name, attrs: { title: name } }),
              h("span", { class: "meta", text: info }),
            ),
          );
        });
        return box;
      }
    }
  }

  private async play(path: string, index: number, name: string): Promise<void> {
    if (!this.api) return;
    this.clearError();
    this.dockTitle.textContent = name;
    this.dockTitle.setAttribute("title", name);
    this.dock.removeAttribute("hidden");
    this.dockSpinner.removeAttribute("hidden");
    this.refreshDockLabels();
    try {
      const wav = await this.api.audio(path, index);
      this.dockAudio.src = this.blobUrl(wav, "audio/wav");
      await this.dockAudio.play();
    } catch (e) {
      this.showError(toAppError(e));
    } finally {
      this.dockSpinner.setAttribute("hidden", "");
    }
  }
}

/** Starts the app in `host`, using the desktop bridge when the page runs inside the app. */
export async function start(host: HTMLElement, base: string): Promise<App> {
  let invoke: Invoke | null = null;
  if (insideApp()) {
    const core = await import("@tauri-apps/api/core");
    invoke = core.invoke as Invoke;
  }
  let dialogs: Dialogs | null = null;
  if (invoke) {
    try {
      dialogs = await tauriDialogs();
    } catch {
      /* without the plugin the path box still works */
    }
  }
  const app = new App(host, invoke, navigator.languages, base, dialogs);
  app.mount();
  if (invoke) {
    try {
      const { getCurrentWebview } = await import("@tauri-apps/api/webview");
      await getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          const first = event.payload.paths[0];
          if (first) void app.open(first);
        }
      });
    } catch {
      /* drag and drop is a convenience; the path box still works */
    }
  }
  return app;
}
