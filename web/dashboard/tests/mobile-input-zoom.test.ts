/**
 * iOS Safari zooms the viewport whenever a focused form control has a computed
 * font-size below 16px, and it does not zoom back out on blur. That single rule
 * produces the whole reported symptom: tapping the login field zooms in, the
 * page becomes horizontally scrollable because the viewport is now wider than
 * the screen, and it stays that way after signing in.
 *
 * The zoom itself cannot be reproduced in headless Chrome — it is a Safari
 * behaviour — but its *cause* is a plain computed style, which is exactly what
 * is asserted here. The threshold is not a style preference; it is the number
 * iOS actually compares against.
 *
 * Requires a built frontend (`bun run build`) and Google Chrome. Skips with a
 * clear message rather than failing when either is missing.
 */
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import type { Browser, Page } from "playwright";

import { stubTurnstile } from "./turnstile-stub";

const DIST = new URL("../dist", import.meta.url).pathname;

const distBuilt = await Bun.file(`${DIST}/index.html`).exists();

/** The size iOS Safari compares against before deciding to zoom. */
const IOS_ZOOM_THRESHOLD_PX = 16;

/** Real phone widths, plus the narrowest the site supports. */
const PHONE_WIDTHS = [320, 360, 390, 412];

let browser: Browser | null = null;
let server: ReturnType<typeof Bun.serve> | null = null;

beforeAll(async () => {
  if (!distBuilt) return;
  const { chromium } = await import("playwright");
  try {
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
      return new Response((await file.exists()) ? file : Bun.file(`${DIST}/index.html`));
    },
  });
});

afterAll(async () => {
  await browser?.close();
  server?.stop();
});

function skip(): boolean {
  if (!distBuilt) {
    console.warn("skipped: run `bun run build` first");
    return true;
  }
  if (!browser || !server) {
    console.warn("skipped: Google Chrome not available");
    return true;
  }
  return false;
}

async function open(path: string, width: number, signedIn = false): Promise<Page> {
  const page = await browser!.newPage({
    viewport: { width, height: 780 },
    isMobile: true,
    hasTouch: true,
    deviceScaleFactor: 3,
  });

  const json = (body: unknown, status = 200) => ({
    status,
    contentType: "application/json",
    body: JSON.stringify(body),
  });

  await stubTurnstile(page);
  await page.route("**/api/v1/**", (route) => {
    const p = new URL(route.request().url()).pathname;
    if (p.endsWith("/auth/me")) {
      return signedIn
        ? route.fulfill(json({ email: "alice@example.com", isVerified: true }))
        : route.fulfill(json({ error: { code: "unauthorized", message: "no" } }, 401));
    }
    if (p.endsWith("/account/game-accounts")) return route.fulfill(json(["alice1"]));
    if (p.endsWith("/account/characters")) return route.fulfill(json([]));
    return route.fulfill(json({ online: true, playersOnline: 0 }));
  });

  await page.goto(`http://localhost:${server!.port}${path}`, { waitUntil: "load" });
  await page.waitForTimeout(350);
  return page;
}

/** Every form control's computed font-size, in px. */
async function controlFontSizes(page: Page): Promise<number[]> {
  return page.evaluate(() =>
    [...document.querySelectorAll("input, select, textarea")].map((el) =>
      parseFloat(getComputedStyle(el).fontSize),
    ),
  );
}

describe("mobile input zoom", () => {
  for (const path of ["/login", "/register", "/forgot-password"]) {
    test(`no control on ${path} is small enough to trigger iOS zoom`, async () => {
      if (skip()) return;

      const page = await open(path, 390);
      const sizes = await controlFontSizes(page);
      await page.close();

      // A page with no controls would pass every assertion below vacuously.
      expect(sizes.length).toBeGreaterThan(0);
      for (const size of sizes) {
        expect(size).toBeGreaterThanOrEqual(IOS_ZOOM_THRESHOLD_PX);
      }
    });
  }

  test("the signed-in account page is safe too", async () => {
    if (skip()) return;

    // The password form is the one people actually type into on a phone.
    const page = await open("/account", 390, true);
    const sizes = await controlFontSizes(page);
    await page.close();

    expect(sizes.length).toBeGreaterThan(0);
    for (const size of sizes) {
      expect(size).toBeGreaterThanOrEqual(IOS_ZOOM_THRESHOLD_PX);
    }
  });

  for (const width of PHONE_WIDTHS) {
    test(`the login page does not scroll sideways at ${width}px`, async () => {
      if (skip()) return;

      const page = await open("/login", width);
      const measured = await page.evaluate(() => ({
        scrollWidth: document.documentElement.scrollWidth,
        clientWidth: document.documentElement.clientWidth,
      }));
      await page.close();

      // Independent of the zoom: real overflow would scroll sideways even at
      // 100%. Rounding means a sub-pixel excess is not a defect.
      expect(measured.scrollWidth).toBeLessThanOrEqual(measured.clientWidth + 1);
    });
  }

  test("pinch zoom is still permitted", async () => {
    if (skip()) return;

    const html = await Bun.file(`${DIST}/index.html`).text();
    const viewport = html.match(/<meta\s+name="viewport"\s+content="([^"]+)"/)?.[1] ?? "";

    // The widely-copied "fix" for this bug is user-scalable=no or
    // maximum-scale=1. Both stop the zoom by taking pinch-to-zoom away from
    // everyone, including people who need it to read at all. Sizing the
    // controls correctly costs nothing and keeps zoom available.
    expect(viewport).not.toContain("user-scalable=no");
    expect(viewport).not.toContain("maximum-scale");
  });
});
