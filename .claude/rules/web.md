---
paths:
  - "web/**"
---
- Text from game files is untrusted: render it with `textContent` or `escapeHtml`, never raw `innerHTML`.
- No inline scripts, no `eval`, no remote scripts or fonts; the page must work under a strict CSP and without network.
- UI strings go through `i18n.ts` (ru and en). The game font is optional and never committed.
- Do not add a dependency without approval; its release must be from the current calendar year.
- Run `npm run typecheck`, `npm test` and `npm run build` before reporting done.
