/**
 * The landing page's calls to action depend on whether anyone is signed in.
 *
 * Offering "Create your account" to someone who is already signed in is the
 * specific bug this pins: nothing in the type checker can see it, because both
 * branches are perfectly valid JSX.
 *
 * Requires a built frontend (`bun run build`) and Google Chrome. Skips with a
 * clear message rather than failing when either is missing.
 */
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import type { Browser, Page } from "playwright";

const DIST = new URL("../dist", import.meta.url).pathname;

const distBuilt = await Bun.file(`${DIST}/index.html`).exists();

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

/** Opens the landing page with `/auth/me` answering signed-in or signed-out. */
async function openLanding(signedIn: boolean): Promise<Page> {
  const page = await browser!.newPage({ viewport: { width: 1024, height: 900 } });

  await page.route("**/api/v1/**", (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith("/auth/me")) {
      return signedIn
        ? route.fulfill({
            status: 200,
            contentType: "application/json",
            body: JSON.stringify({ email: "alice@example.com", isVerified: true }),
          })
        : route.fulfill({
            status: 401,
            contentType: "application/json",
            body: JSON.stringify({ error: { code: "unauthorized", message: "no session" } }),
          });
    }
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ online: true, playersOnline: 0 }),
    });
  });

  await page.goto(`http://localhost:${server!.port}/`, { waitUntil: "load" });
  await page.waitForTimeout(400);
  return page;
}

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

/** Every link target on the page, so a CTA cannot hide behind matching text. */
async function hrefs(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    [...document.querySelectorAll("a")].map((a) => a.getAttribute("href") ?? ""),
  );
}

describe("landing page calls to action", () => {
  test("a signed-out visitor is invited to register", async () => {
    if (skip()) return;

    const page = await openLanding(false);
    const body = (await page.textContent("body")) ?? "";
    const links = await hrefs(page);
    await page.close();

    expect(body).toContain("Create your account");
    expect(links).toContain("/register");
  });

  test("a signed-in visitor is never offered account creation", async () => {
    if (skip()) return;

    const page = await openLanding(true);
    const body = (await page.textContent("body")) ?? "";
    const links = await hrefs(page);
    await page.close();

    // Both CTAs — the hero and the "Ready to play?" panel — have to switch.
    // Checking hrefs rather than only copy catches a button whose label was
    // changed while its destination was left pointing at the sign-up flow.
    expect(links).not.toContain("/register");
    expect(body).not.toContain("Create your account");
    expect(body).not.toContain("Create account");
    expect(body).not.toContain("I already have one");

    // And it points them somewhere useful instead.
    expect(links).toContain("/account");
    expect(body).toContain("Go to your account");
  });

  test("the launcher download is offered either way", async () => {
    if (skip()) return;

    // It is the actual next step for both audiences, so neither branch may
    // drop it while rearranging the buttons around it.
    for (const signedIn of [true, false]) {
      const page = await openLanding(signedIn);
      const body = (await page.textContent("body")) ?? "";
      await page.close();
      expect(body).toContain("Download launcher");
    }
  });
});
