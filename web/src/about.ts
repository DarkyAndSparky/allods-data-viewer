import { t, type Lang } from "./i18n";

/** Data shown in the About dialog. */
export interface AboutInfo {
  version: string;
  lang: Lang;
  repoUrl: string;
  logoSrc: string;
}

/** Escape text for safe use inside HTML. */
export function escapeHtml(text: string): string {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

/** Inner HTML of the About dialog. */
export function renderAbout(info: AboutInfo): string {
  const { lang } = info;
  return `
    <img class="about-logo" src="${escapeHtml(info.logoSrc)}" alt="Allods Data Viewer" width="220" height="220" />
    <h2 id="about-title">${escapeHtml(t(lang, "aboutTitle"))}</h2>
    <p>${escapeHtml(t(lang, "tagline"))}</p>
    <dl class="about-facts">
      <dt>${escapeHtml(t(lang, "version"))}</dt><dd>${escapeHtml(info.version)}</dd>
      <dt>${escapeHtml(t(lang, "license"))}</dt><dd>${escapeHtml(t(lang, "licenseValue"))}</dd>
      <dt>${escapeHtml(t(lang, "source"))}</dt><dd><a href="${escapeHtml(info.repoUrl)}" target="_blank" rel="noopener noreferrer">${escapeHtml(info.repoUrl)}</a></dd>
    </dl>
    <p class="about-note">${escapeHtml(t(lang, "formats"))}</p>
    <p class="about-note">${escapeHtml(t(lang, "legal"))}</p>
    <form method="dialog"><button class="btn">${escapeHtml(t(lang, "close"))}</button></form>
  `;
}
