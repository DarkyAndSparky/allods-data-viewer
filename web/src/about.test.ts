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
