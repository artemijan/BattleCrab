/**
 * Regenerates `assets/og-image.jpg`, the link-preview card crawlers show when
 * the site is shared.
 *
 *     bun run og-image
 *
 * The image is committed, so neither the build nor CI needs a browser — this
 * only runs when the artwork changes. It exists so the image is reproducible and
 * reviewable as code: the alternative is a binary nobody can regenerate or
 * confidently edit.
 *
 * 1200x630 is the size Facebook, LinkedIn and Slack all scale from, and is the
 * 1.91:1 ratio Twitter's `summary_large_image` expects. Below 600x315 several
 * of them fall back to a small square card instead.
 */
import { chromium } from "playwright";

// JPEG, not PNG: the artwork is a photographic gradient, which PNG stores at
// roughly four times the size for no visible gain at this scale. The card has
// no transparency to preserve, and every crawler that reads og:image handles
// JPEG. Smaller matters — this is fetched on every share and unfurl.
const OUT = new URL("../assets/og-image.jpg", import.meta.url).pathname;
const LOGO = new URL("../assets/logo.webp", import.meta.url).pathname;

const WIDTH = 1200;
const HEIGHT = 630;

// Inlined as a data URI rather than referenced by path: the page is loaded via
// setContent, which has no base URL for a relative src to resolve against.
const logo = await Bun.file(LOGO).arrayBuffer();
const logoDataUri = `data:image/webp;base64,${Buffer.from(logo).toString("base64")}`;

const html = `<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <style>
      * { margin: 0; padding: 0; box-sizing: border-box; }
      html, body { width: ${WIDTH}px; height: ${HEIGHT}px; }
      body {
        display: flex;
        align-items: center;
        gap: 64px;
        padding: 0 80px;
        background:
          radial-gradient(900px 600px at 12% 18%, #0b4ea2 0%, transparent 60%),
          radial-gradient(700px 500px at 88% 88%, #06356f 0%, transparent 55%),
          linear-gradient(135deg, #041c3a 0%, #072a55 100%);
        color: #fff;
        font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
        overflow: hidden;
      }
      .logo {
        width: 300px;
        height: auto;
        flex-shrink: 0;
        filter: drop-shadow(0 24px 60px rgba(0, 0, 0, 0.55));
      }
      .badge {
        display: inline-flex;
        align-items: center;
        gap: 10px;
        padding: 8px 18px;
        border: 1px solid rgba(255, 255, 255, 0.22);
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.08);
        font-size: 20px;
        font-weight: 500;
        color: rgba(255, 255, 255, 0.82);
      }
      .dot { width: 9px; height: 9px; border-radius: 999px; background: #ffd700; }
      h1 {
        margin-top: 26px;
        font-size: 56px;
        font-weight: 900;
        line-height: 1.06;
        letter-spacing: -0.025em;
        /* Without this the last line orphans a single word, which reads as a
           rendering bug on a card this small in a feed. */
        text-wrap: balance;
      }
      /* The one accent: the same gold the primary button uses. */
      .accent { color: #ffd700; }
      p {
        margin-top: 22px;
        font-size: 27px;
        line-height: 1.4;
        color: rgba(255, 255, 255, 0.74);
      }
    </style>
  </head>
  <body>
    <img class="logo" src="${logoDataUri}" alt="" />
    <div>
      <span class="badge"><span class="dot"></span>Early alpha &middot; Interlude Classic</span>
      <!-- nbsp: the product name must never break across lines. -->
      <h1>The world of <span class="accent">Lineage&nbsp;II</span>, made our own.</h1>
      <p>A custom server written from scratch in Rust.<br />Open to everyone &middot; battlecrab.com</p>
    </div>
  </body>
</html>`;

// The system Chrome, matching tests/ — CI needn't download a browser build.
const browser = await chromium.launch({ channel: "chrome" });
const page = await browser.newPage({
  viewport: { width: WIDTH, height: HEIGHT },
  // Crawler thumbnails are served at 1x; rendering at 2x would only quadruple
  // the bytes every share has to download.
  deviceScaleFactor: 1,
});

await page.setContent(html, { waitUntil: "load" });
// The logo is a data URI, so it decodes rather than downloads — but `load` can
// still fire before the first paint composites it.
await page.waitForTimeout(300);
await page.screenshot({ path: OUT, type: "jpeg", quality: 90 });

await browser.close();

const bytes = (await Bun.file(OUT).arrayBuffer()).byteLength;
console.log(`wrote ${OUT} (${WIDTH}x${HEIGHT}, ${(bytes / 1024).toFixed(1)} KiB)`);
