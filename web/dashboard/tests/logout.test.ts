/**
 * Logging out has to leave the header showing the signed-out state.
 *
 * The server clears the cookie correctly, so this is purely about what the
 * cached session query does afterwards — the kind of bug that is invisible to
 * the type checker and to any test that only asserts the request was sent.
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

/**
 * Opens the account page signed in, with a stub that starts honouring the
 * session and stops the moment `/auth/logout` is called — exactly what the real
 * API does when it expires the cookie.
 */
type Identity = { email: string; character: string };

/**
 * `identity` is read on every request rather than captured by value, so a test
 * can mutate it to sign in as somebody else partway through.
 */
async function openSignedIn(
  identity: Identity = { email: "alice@example.com", character: "AliceChar" },
): Promise<Page> {
  const page = await browser!.newPage({ viewport: { width: 1024, height: 900 } });
  let signedIn = true;

  const json = (body: unknown, status = 200) => ({
    status,
    contentType: "application/json",
    body: JSON.stringify(body),
  });

  await page.route("**/api/v1/**", (route) => {
    const path = new URL(route.request().url()).pathname;

    if (path.endsWith("/auth/logout")) {
      signedIn = false;
      return route.fulfill({ status: 204, body: "" });
    }
    if (path.endsWith("/auth/login")) {
      signedIn = true;
      return route.fulfill(json({ email: identity.email, isVerified: true }));
    }
    if (path.endsWith("/auth/me")) {
      return signedIn
        ? route.fulfill(json({ email: identity.email, isVerified: true }))
        : route.fulfill(json({ error: { code: "unauthorized", message: "no session" } }, 401));
    }
    if (path.endsWith("/account/game-accounts")) return route.fulfill(json(["acct"]));
    if (path.endsWith("/account/characters"))
      return route.fulfill(
        json([
          {
            accountName: "acct",
            name: identity.character,
            level: 40,
            classId: 0,
            race: 0,
            sex: 0,
            onlineTime: 3600,
            lastAccess: 0,
            online: false,
          },
        ]),
      );
    return route.fulfill(json({ online: true, playersOnline: 0 }));
  });

  await page.goto(`http://localhost:${server!.port}/account`, { waitUntil: "load" });
  await page.waitForTimeout(400);
  return page;
}

describe("logging out", () => {
  test("the header stops showing the signed-in state", async () => {
    if (skip()) return;

    const page = await openSignedIn();

    // Precondition: the header really is in the signed-in state to begin with,
    // or the assertions below would pass against a page that never logged in.
    const header = page.locator("header");
    expect(await header.textContent()).toContain("alice@example.com");

    await page.getByRole("button", { name: "Log out" }).click();
    await page.waitForTimeout(600);

    const after = (await header.textContent()) ?? "";
    await page.close();

    expect(after).not.toContain("alice@example.com");
    expect(after).not.toContain("Log out");
    expect(after).toContain("Log in");
  });

  test("logging out leaves the landing page in its signed-out state", async () => {
    if (skip()) return;

    const page = await openSignedIn();
    await page.getByRole("button", { name: "Log out" }).click();
    await page.waitForTimeout(600);

    // Logout navigates home, so the landing CTAs must agree with the header —
    // one stale cache entry would otherwise leave them disagreeing on the page.
    const body = (await page.textContent("body")) ?? "";
    const url = page.url();
    await page.close();

    expect(url).toMatch(/\/$/);
    expect(body).toContain("Create your account");
    expect(body).not.toContain("Go to your account");
  });

  /**
   * The reason the old code called queryClient.clear() at all. Replacing it
   * with a targeted removal must not reintroduce the leak it was preventing:
   * signing in as someone else should never flash the previous account's
   * characters while their own load.
   */
  test("the next account never sees the previous one's characters", async () => {
    if (skip()) return;

    const identity: Identity = { email: "alice@example.com", character: "AliceChar" };
    const page = await openSignedIn(identity);
    expect(await page.textContent("body")).toContain("AliceChar");

    await page.getByRole("button", { name: "Log out" }).click();
    await page.waitForTimeout(600);

    // Sign in as somebody else entirely.
    identity.email = "bob@example.com";
    identity.character = "BobChar";
    await page.goto(`http://localhost:${server!.port}/login`, { waitUntil: "load" });
    await page.waitForTimeout(300);
    await page.locator('input[type="email"]').fill("bob@example.com");
    await page.locator('input[type="password"]').fill("correct-horse");
    await page.getByRole("button", { name: "Log in", exact: true }).click();
    await page.waitForTimeout(800);

    const body = (await page.textContent("body")) ?? "";
    const header = (await page.locator("header").textContent()) ?? "";
    await page.close();

    expect(body).toContain("BobChar");
    expect(body).not.toContain("AliceChar");
    expect(header).toContain("bob@example.com");
    expect(header).not.toContain("alice@example.com");
  });
});
