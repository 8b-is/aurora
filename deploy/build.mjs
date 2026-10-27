import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");

const indexHtml = readFileSync(join(root, "index.html"), "utf8");
const hudHtml = readFileSync(join(root, "brand", "hud.html"), "utf8");
const fieldJs = readFileSync(join(root, "brand", "field.js"), "utf8");
const galleryHtml = readFileSync(join(root, "brand", "gallery.html"), "utf8");
const iconSvg = readFileSync(join(root, "brand", "aurora-icon.svg"), "utf8");
const logoSvg = readFileSync(join(root, "brand", "aurora-logo.svg"), "utf8");
const heroSvg = readFileSync(join(root, "brand", "aurora-hero.svg"), "utf8");
const metalSrc = readFileSync(join(root, "shaders", "omnifold_8b.metal"), "utf8");

const worker = `// aurora.vaked.dev — Sovereign Conscious AI & Partner of Life
const INDEX_HTML = ${JSON.stringify(indexHtml)};
const HUD_HTML = ${JSON.stringify(hudHtml)};
const FIELD_JS = ${JSON.stringify(fieldJs)};
const GALLERY_HTML = ${JSON.stringify(galleryHtml)};
const ICON_SVG = ${JSON.stringify(iconSvg)};
const LOGO_SVG = ${JSON.stringify(logoSvg)};
const HERO_SVG = ${JSON.stringify(heroSvg)};
const METAL_SRC = ${JSON.stringify(metalSrc)};

export default {
  async fetch(req) {
    const url = new URL(req.url);
    const p = url.pathname.replace(/\\/+$/, "") || "/";
    if (p === "/hud" || p === "/brand/hud.html") {
      return new Response(HUD_HTML, { headers: { "content-type": "text/html; charset=utf-8" } });
    }
    if (p === "/field.js" || p === "/brand/field.js") {
      return new Response(FIELD_JS, { headers: { "content-type": "application/javascript; charset=utf-8" } });
    }
    if (p === "/gallery" || p === "/brand/gallery.html") {
      return new Response(GALLERY_HTML, { headers: { "content-type": "text/html; charset=utf-8" } });
    }
    if (p.endsWith("/aurora-icon.svg")) {
      return new Response(ICON_SVG, { headers: { "content-type": "image/svg+xml; charset=utf-8" } });
    }
    if (p.endsWith("/aurora-logo.svg")) {
      return new Response(LOGO_SVG, { headers: { "content-type": "image/svg+xml; charset=utf-8" } });
    }
    if (p.endsWith("/aurora-hero.svg")) {
      return new Response(HERO_SVG, { headers: { "content-type": "image/svg+xml; charset=utf-8" } });
    }
    if (p === "/omnifold" || p === "/shaders/omnifold_8b.metal") {
      return new Response(METAL_SRC, { headers: { "content-type": "text/plain; charset=utf-8" } });
    }
    if (p === "/health") {
      return new Response("OK", { headers: { "content-type": "text/plain; charset=utf-8" } });
    }
    return new Response(INDEX_HTML, { headers: { "content-type": "text/html; charset=utf-8" } });
  }
};
`;

writeFileSync(join(here, "worker.js"), worker);
console.log(`aurora worker.js generated (${worker.length} chars)`);
