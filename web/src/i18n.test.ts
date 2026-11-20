import { describe, expect, it } from "vitest";
import { detectLang, keys, resources, t, type Lang } from "./i18n";

const SUFFIX = /_(zero|one|two|few|many|other)$/;
const hasCyrillic = (s: string) => /[а-яё]/i.test(s);

function baseOf(lang: Lang): Set<string> {
  return new Set(Object.keys(resources[lang]).map((k) => k.replace(SUFFIX, "")));
}

describe("translations", () => {
  it("define the same keys in both languages", () => {
    expect([...baseOf("ru")].sort()).toEqual([...baseOf("en")].sort());
    expect(keys.length).toBeGreaterThan(0);
  });

  it("have every plural form the language needs", () => {
    for (const lang of ["en", "ru"] as const) {
      const categories = new Intl.PluralRules(lang).resolvedOptions().pluralCategories;
      const plural = new Set(
        Object.keys(resources[lang]).filter((k) => SUFFIX.test(k)).map((k) => k.replace(SUFFIX, "")),
      );
      for (const base of plural) {
        for (const c of categories) {
          expect(resources[lang][`${base}_${c}`], `${lang} ${base}_${c}`).toBeDefined();
        }
      }
    }
    // A key that is plural in one language is plural in the other.
    const en = Object.keys(resources.en).filter((k) => SUFFIX.test(k)).map((k) => k.replace(SUFFIX, ""));
    const ru = Object.keys(resources.ru).filter((k) => SUFFIX.test(k)).map((k) => k.replace(SUFFIX, ""));
    expect([...new Set(ru)].sort()).toEqual([...new Set(en)].sort());
  });

  it("are non-empty and in the right script", () => {
    for (const [lang, dict] of Object.entries(resources)) {
      for (const [key, value] of Object.entries(dict)) {
        expect(value.trim(), `${lang} ${key}`).not.toBe("");
        if (lang === "en") expect(hasCyrillic(value), `en ${key}`).toBe(false);
        // Words that are the same in both languages (the license name) are allowed.
        else if (value !== resources.en[key]) expect(hasCyrillic(value), `ru ${key}`).toBe(true);
      }
    }
  });

  it("use the same placeholders in both languages", () => {
    const names = (s: string) => [...s.matchAll(/\{\{(\w+)\}\}/g)].map((m) => m[1]).sort();
    for (const key of keys) {
      const en = Object.entries(resources.en).find(([k]) => k.replace(SUFFIX, "") === key)?.[1] ?? "";
      const ru = Object.entries(resources.ru).find(([k]) => k.replace(SUFFIX, "") === key)?.[1] ?? "";
      const strip = (a: (string | undefined)[]) => a.filter((n) => n !== "count");
      expect(strip(names(ru)), key).toEqual(strip(names(en)));
    }
  });

  it("choose Russian plural forms", () => {
    expect(t("ru", "files", { count: 1 })).toBe("1 файл");
    expect(t("ru", "files", { count: 2 })).toBe("2 файла");
    expect(t("ru", "files", { count: 5 })).toBe("5 файлов");
    expect(t("ru", "files", { count: 21 })).toBe("21 файл");
    expect(t("ru", "files", { count: 11 })).toBe("11 файлов");
    expect(t("en", "files", { count: 1 })).toBe("1 file");
    expect(t("en", "files", { count: 2 })).toBe("2 files");
  });

  it("fill placeholders without escaping", () => {
    expect(t("en", "imageInfo", { w: 16, h: 8, format: "DXT1" })).toBe("Picture 16×8, format DXT1");
  });

  it("detect the language", () => {
    expect(detectLang(["ru-RU", "en"])).toBe("ru");
    expect(detectLang(["de", "en-US"])).toBe("en");
    expect(detectLang([])).toBe("en");
  });
});
