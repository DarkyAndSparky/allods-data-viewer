import { describe, expect, it } from "vitest";
import { escapeHtml, renderAbout } from "./about";
import { detectLang, t } from "./i18n";

const info = {
  version: "1.2.3",
  lang: "en" as const,
  repoUrl: "https://github.com/DarkyAndSparky/allods-data-viewer",
  logoSrc: "/logo.png",
};

describe("escapeHtml", () => {
  it("escapes markup characters", () => {
    expect(escapeHtml(`<a href="x">&</a>`)).toBe("&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;");
  });
});

describe("renderAbout", () => {
  it("shows version, license and link", () => {
    const html = renderAbout(info);
    expect(html).toContain("1.2.3");
    expect(html).toContain("MIT");
    expect(html).toContain(info.repoUrl);
  });

  it("does not let the version inject markup", () => {
    expect(renderAbout({ ...info, version: "<script>" })).not.toContain("<script>");
  });

  it("is translated", () => {
    expect(renderAbout({ ...info, lang: "ru" })).toContain("О программе");
  });
});

describe("components list", () => {
  const items = [
    { name: "x<y", version: "1", license: "MIT", url: "https://e.org", kind: "npm", ru: "русский", en: "english" },
    { name: "bad", version: "2", license: "MIT", url: "javascript:alert(1)", kind: "npm", ru: "р", en: "e" },
  ];

  it("lists name, version, licence and purpose in the chosen language", () => {
    const en = renderAbout({ ...info, components: items });
    expect(en).toContain("Components and licenses");
    expect(en).toContain("english");
    expect(en).not.toContain("русский");
    const ru = renderAbout({ ...info, lang: "ru", components: items });
    expect(ru).toContain("Компоненты и лицензии");
    expect(ru).toContain("русский");
  });

  it("escapes names and links only to https", () => {
    const html = renderAbout({ ...info, components: items });
    expect(html).toContain("x&lt;y");
    expect(html).not.toContain("javascript:");
    expect(html).toContain('href="https://e.org"');
  });

  it("the generated list has both languages for every component", async () => {
    const data = (await import("./third-party.json")).default;
    expect(data.components.length).toBeGreaterThan(5);
    for (const c of data.components) {
      expect(c.ru, c.name).not.toBe("");
      expect(c.en, c.name).not.toBe("");
      expect(c.license, c.name).not.toBe("UNKNOWN");
    }
  });
});

describe("i18n", () => {
  it("detects language", () => {
    expect(detectLang(["de-DE", "ru-RU"])).toBe("ru");
    expect(detectLang(["en-GB"])).toBe("en");
    expect(detectLang(["fr"])).toBe("en");
    expect(detectLang([])).toBe("en");
  });

  it("has a translation for every key in both languages", () => {
    expect(t("ru", "close")).not.toBe(t("en", "close"));
  });
});
