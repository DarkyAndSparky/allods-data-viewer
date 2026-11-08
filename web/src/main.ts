import { formatBytes } from "./format";

const app = document.querySelector<HTMLDivElement>("#app");

if (app) {
  app.innerHTML = `
    <main style="font-family: system-ui, sans-serif; max-width: 40rem; margin: 4rem auto; padding: 0 1rem;">
      <h1>PakScope</h1>
      <p>Game resource viewer. Work in progress (milestone M0).</p>
      <p>Example of a shared helper: 1536 bytes is ${formatBytes(1536)}.</p>
    </main>
  `;
}
