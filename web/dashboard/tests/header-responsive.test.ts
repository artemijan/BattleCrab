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


/**
 * A page that never depends on a reachable API.
 *
 * The production bundle points at https://api.battlecrab.com, so `networkidle`
 * would wait on a host that does not answer in CI and the test would time out
 * having proved nothing about layout. API calls are stubbed instead, and we
 * wait for `load` rather than network silence.
 */
async function openPage(width: number, height: number, path: string) {
  const page = await browser!.newPage({ viewport: { width, height } });
  await page.route("**/api/v1/**", (route) =>
    route.fulfill({
      status: 401,
      contentType: "application/json",
      body: JSON.stringify({ error: { code: "unauthorized", message: "stubbed" } }),
    }),
  );
  await page.goto(`http://localhost:${server!.port}${path}`, { waitUntil: "load" });
  await page.waitForTimeout(250);
  return page;
}

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

      const page = await openPage(width, 740, "/");

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
      const page = await openPage(width, 740, "/");

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

/**
 * Regression test for a phantom vertical scrollbar.
 *
 * The sticky header was rendered as a *sibling* above the `min-h-dvh` column
 * rather than inside it. A sticky element still occupies flow space, so the
 * document came out `100dvh + header` tall and every route scrolled by exactly
 * the header's height (82px) — even the login page, which otherwise fits with
 * hundreds of pixels to spare.
 */
describe("page height", () => {
  const DESKTOP = [
    [1440, 900],
    [1280, 800],
    [1366, 768],
    [1024, 768],
  ] as const;

  for (const [width, height] of DESKTOP) {
    test(`/login does not scroll at ${width}x${height}`, async () => {
      if (!distBuilt || !browser || !server) {
        console.warn("skipped: needs a built dist and Google Chrome");
        return;
      }

      const page = await openPage(width, height, "/login");

      const measured = await page.evaluate(() => ({
        scrollHeight: document.documentElement.scrollHeight,
        clientHeight: document.documentElement.clientHeight,
      }));
      await page.close();

      // Login is the short form; it must fit every desktop viewport we claim to
      // support. (Register carries a third field and legitimately needs ~826px,
      // so it is deliberately not asserted here.)
      expect(measured.scrollHeight).toBeLessThanOrEqual(measured.clientHeight);
    });
  }
});
