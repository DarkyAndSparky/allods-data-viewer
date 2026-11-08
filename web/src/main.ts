import "./style.css";
import pkg from "../package.json";
import { renderAbout } from "./about";
import { formatBytes } from "./format";
import { detectLang, t } from "./i18n";

const REPO_URL = "https://github.com/DarkyAndSparky/allods-data-viewer";
const lang = detectLang(navigator.languages);
const base = import.meta.env.BASE_URL;

const app = document.querySelector<HTMLDivElement>("#app");

if (app) {
  app.innerHTML = `
    <header class="topbar">
      <img src="${base}icon.png" alt="" />
      <span class="title">Allods Data Viewer</span>
      <button class="btn" id="about-btn" type="button">${t(lang, "about")}</button>
    </header>
    <main>
      <h1>Allods Data Viewer</h1>
      <p>${t(lang, "tagline")}</p>
      <p>Work in progress (milestone M0). Shared helper check: 1536 bytes is ${formatBytes(1536)}.</p>
    </main>
    <dialog id="about" aria-labelledby="about-title"></dialog>
  `;

  const dialog = app.querySelector<HTMLDialogElement>("#about");
  if (dialog) {
    dialog.innerHTML = renderAbout({
      version: pkg.version,
      lang,
      repoUrl: REPO_URL,
      logoSrc: `${base}logo.png`,
    });
    dialog.addEventListener("click", (event) => {
      if (event.target === dialog) dialog.close();
    });
    app.querySelector("#about-btn")?.addEventListener("click", () => dialog.showModal());
  }
}
