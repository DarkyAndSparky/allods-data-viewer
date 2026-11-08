const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/**
 * Human readable file size, e.g. `1536` -> `1.5 KB`.
 * Uses 1024 as the base, like most file managers do.
 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    throw new RangeError(`invalid byte count: ${bytes}`);
  }

  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit++;
  }

  const digits = unit === 0 || value >= 100 ? 0 : 1;
  return `${value.toFixed(digits)} ${UNITS[unit]}`;
}
