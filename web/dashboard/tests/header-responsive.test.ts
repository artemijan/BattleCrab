/**
 * Regression test for the header overflowing on narrow phones.
 *
 * The theme toggle escaped the header panel at 360x740 (reported), because the
 * brand could not shrink and the nav could not hold its intrinsic width. Nothing
 * in the type checker or the Rust suite can catch a layout overflow, so this
 * drives a real browser and measures.
 *
 * Requires a built frontend (`bun run build`) and Google Chrome. Skips with a
 * clear message rather than failing when either is missing, so it does not break
 * a machine that has neither.
 */
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import type { Browser } from "playwright";

const DIST = new URL("../dist", import.meta.url).pathname;

const distBuilt = await Bun.file(`${DIST}/index.html`).exists();

let browser: Browser | null = null;
let server: ReturnType<typeof Bun.serve> | null = null;

beforeAll(async () => {
  if (!distBuilt) return;
  const { chromium } = await import("playwright");
  try {
    // The system Chrome, so CI needn't download a browser build.
    browser = await chromium.launch({ channel: "chrome" });
  } catch {
    browser = null;
    return;
  }
  server = Bun.serve({
    port: 0,
    async fetch(request) {
      const url = new URL(request.url);
      const path = url.pathname === "/" ? "/index.html" : url.pathname;
      const file = Bun.file(DIST + path);
      // Unknown paths fall back to index.html, mirroring the SPA fallback in
      // crates/dashboard_api/src/web.rs.
      return new Response((await file.exists()) ? file : Bun.file(`${DIST}/index.html`));
    },
  });
});

afterAll(async () => {
  await browser?.close();
  server?.stop();
});

/** Widths that actually ship on phones, plus the narrowest we support. */
const WIDTHS = [320, 360, 390, 412, 480];

describe("header layout", () => {
  for (const width of WIDTHS) {
    test(`no control escapes the header at ${width}px`, async () => {
      if (!distBuilt) {
        console.warn("skipped: run `bun run build` first");
        return;
      }
      if (!browser || !server) {
        console.warn("skipped: Google Chrome not available");
        return;
      }

      const page = await browser.newPage({ viewport: { width, height: 740 } });
      await page.goto(`http://localhost:${server.port}/`, { waitUntil: "networkidle" });

      const measured = await page.evaluate(() => {
        const panel = document.querySelector("header > div");
        const toggle = document.querySelector("header button[aria-pressed]");
        if (!panel || !toggle) return null;
        const p = panel.getBoundingClientRect();
        const t = toggle.getBoundingClientRect();
        return {
          panelLeft: p.left,
          panelRight: p.right,
          toggleLeft: t.left,
          toggleRight: t.right,
          scrollWidth: document.documentElement.scrollWidth,
          clientWidth: document.documentElement.clientWidth,
        };
      });
      await page.close();

      expect(measured).not.toBeNull();
      const m = measured!;

      // The toggle is the last control, so it is the first thing to escape.
      expect(m.toggleRight).toBeLessThanOrEqual(m.panelRight);
      expect(m.toggleLeft).toBeGreaterThanOrEqual(m.panelLeft);

      // An overflowing header also drags the whole page sideways.
      expect(m.scrollWidth).toBeLessThanOrEqual(m.clientWidth);
    });
  }

  test("the wordmark is never clipped — it is hidden or shown whole", async () => {
    if (!distBuilt || !browser || !server) {
      console.warn("skipped: needs a built dist and Google Chrome");
      return;
    }

    // Truncation ("Battl…") reads as breakage; below the cutoff the wordmark
    // must be absent entirely rather than ellipsised.
    for (const width of WIDTHS) {
      const page = await browser.newPage({ viewport: { width, height: 740 } });
      await page.goto(`http://localhost:${server.port}/`, { waitUntil: "networkidle" });

      const clipped = await page.evaluate(() => {
        const word = document.querySelector("header a > span:last-child");
        if (!word) return false;
        const style = getComputedStyle(word);
        if (style.display === "none") return false; // hidden is fine
        return word.scrollWidth > word.clientWidth + 1; // ellipsised is not
      });
      await page.close();

      expect(clipped).toBe(false);
    }
  });
});
