const UNITS: Record<"en" | "ru", readonly string[]> = {
  en: ["B", "KB", "MB", "GB", "TB"],
  ru: ["Б", "КБ", "МБ", "ГБ", "ТБ"],
};

/**
 * Human readable file size, e.g. `1536` -> `1.5 KB`.
 * Uses 1024 as the base, like most file managers do.
 */
export function formatBytes(bytes: number, lang: "en" | "ru" = "en"): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    throw new RangeError(`invalid byte count: ${bytes}`);
  }

  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS[lang].length - 1) {
    value /= 1024;
    unit++;
  }

  const digits = unit === 0 || value >= 100 ? 0 : 1;
  return `${value.toFixed(digits)} ${UNITS[lang][unit]}`;
}
