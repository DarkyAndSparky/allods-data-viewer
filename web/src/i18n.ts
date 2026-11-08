/** Languages the interface is translated to. */
export type Lang = "en" | "ru";

const en = {
  about: "About",
  aboutTitle: "About Allods Data Viewer",
  close: "Close",
  tagline: "Open source viewer and extractor for game client resources.",
  version: "Version",
  license: "License",
  licenseValue: "MIT",
  source: "Source code",
  legal:
    "Allods Data Viewer is an independent project and is not affiliated with the game's developers or publishers. It contains no game assets. Game and company names belong to their respective owners.",
  formats:
    "Based on notes about the file formats; no code from other viewers is used.",
};

/** Every translatable string key. */
export type Key = keyof typeof en;

const ru: Record<Key, string> = {
  about: "О программе",
  aboutTitle: "О программе Allods Data Viewer",
  close: "Закрыть",
  tagline: "Открытый просмотрщик и распаковщик ресурсов клиента игры.",
  version: "Версия",
  license: "Лицензия",
  licenseValue: "MIT",
  source: "Исходный код",
  legal:
    "Allods Data Viewer — независимый проект, не связанный с разработчиками и издателями игры. Ресурсов игры в нём нет. Названия игры и компаний принадлежат их владельцам.",
  formats:
    "Основано на описании форматов файлов; код других просмотрщиков не используется.",
};

const dictionaries: Record<Lang, Record<Key, string>> = { en, ru };

/** Pick a supported language from the browser's preference list, English by default. */
export function detectLang(languages: readonly string[]): Lang {
  for (const tag of languages) {
    const primary = tag.toLowerCase().split("-")[0];
    if (primary === "ru") return "ru";
    if (primary === "en") return "en";
  }
  return "en";
}

/** Look up a string in the given language. */
export function t(lang: Lang, key: Key): string {
  return dictionaries[lang][key];
}
