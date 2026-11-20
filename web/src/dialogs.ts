/** Native dialogs. The app gets them from the Tauri dialog plugin; tests pass fakes. */
export interface FileFilter {
  name: string;
  extensions: string[];
}

export interface Dialogs {
  /** Choose a folder; `null` when cancelled. */
  pickFolder(): Promise<string | null>;
  /** Choose one file; `null` when cancelled. */
  pickFile(filters: FileFilter[]): Promise<string | null>;
  /** Choose where to save; `null` when cancelled. */
  saveTo(defaultName: string, filters: FileFilter[]): Promise<string | null>;
}

/** Dialogs backed by `@tauri-apps/plugin-dialog`. */
export async function tauriDialogs(): Promise<Dialogs> {
  const { open, save } = await import("@tauri-apps/plugin-dialog");
  return {
    pickFolder: async () => {
      const r = await open({ directory: true, multiple: false });
      return typeof r === "string" ? r : null;
    },
    pickFile: async (filters) => {
      const r = await open({ directory: false, multiple: false, filters });
      return typeof r === "string" ? r : null;
    },
    saveTo: async (defaultPath, filters) => save({ defaultPath, filters }),
  };
}

/** File name with its extension replaced (or added). */
export function withExtension(name: string, ext: string): string {
  const dot = name.lastIndexOf(".");
  const stem = dot > 0 ? name.slice(0, dot) : name;
  return `${stem}.${ext}`;
}
