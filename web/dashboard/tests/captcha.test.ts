/**
 * The captcha's two contracts with the API, driven through a real browser:
 *
 * - Register always carries a Turnstile token in the request body.
 * - Login starts widget-free; a `captcha_required` answer makes the widget
 *   appear, and the retry then carries the token.
 *
 * The widget itself is stubbed (`turnstile-stub.ts`) — these tests are about
 * the forms' behavior around it, not about Cloudflare.
 *
 * Requires a built frontend (`bun run build`) and Google Chrome. Skips with a
 * clear message rather than failing when either is missing.
 */
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import type { Browser, Page } from "playwright";

import { STUB_TOKEN, stubTurnstile } from "./turnstile-stub";

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

const json = (body: unknown, status = 200) => ({
  status,
  contentType: "application/json",
  body: JSON.stringify(body),
});

async function open(path: string): Promise<Page> {
  const page = await browser!.newPage({ viewport: { width: 1024, height: 900 } });
  await stubTurnstile(page);
  await page.goto(`http://localhost:${server!.port}${path}`, { waitUntil: "load" });
  await page.waitForTimeout(300);
  return page;
}

describe("captcha", () => {
  test("register sends the token, and the button waits for it", async () => {
    if (skip()) return;

    const page = await open("/register");
    let registerBody: unknown = null;

    await page.route("**/api/v1/**", (route) => {
      const p = new URL(route.request().url()).pathname;
      if (p.endsWith("/auth/register")) {
        registerBody = route.request().postDataJSON();
        return route.fulfill(json({ email: "alice@example.com", isVerified: false }, 201));
      }
      if (p.endsWith("/auth/me")) {
        return route.fulfill(json({ error: { code: "unauthorized", message: "no" } }, 401));
      }
      return route.fulfill(json({ online: true, playersOnline: 0 }));
    });

    await page.locator('input[type="email"]').fill("alice@example.com");
    await page.locator('input[type="password"]').first().fill("correct-horse");
    await page.locator('input[type="password"]').nth(1).fill("correct-horse");

    // The stub succeeds on the next tick after render, so by now the token is
    // in and the button enabled — the assertion still documents the gate.
    // Scoped to the form: the page header carries a "Create account" CTA too.
    const button = page.locator("form").getByRole("button", { name: "Create account" });
    await page.waitForTimeout(200);
    expect(await button.isEnabled()).toBe(true);

    await button.click();
    await page.waitForTimeout(400);
    await page.close();

    expect(registerBody).toMatchObject({
      email: "alice@example.com",
      password: "correct-horse",
      captchaToken: STUB_TOKEN,
    });
  });

  test("forgot-password sends the token too", async () => {
    if (skip()) return;

    const page = await open("/forgot-password");
    let forgotBody: unknown = null;

    await page.route("**/api/v1/**", (route) => {
      const p = new URL(route.request().url()).pathname;
      if (p.endsWith("/auth/forgot-password")) {
        forgotBody = route.request().postDataJSON();
        return route.fulfill({ status: 202, body: "" });
      }
      if (p.endsWith("/auth/me")) {
        return route.fulfill(json({ error: { code: "unauthorized", message: "no" } }, 401));
      }
      return route.fulfill(json({ online: true, playersOnline: 0 }));
    });

    await page.locator('input[type="email"]').fill("alice@example.com");
    await page.waitForTimeout(200);
    await page.getByRole("button", { name: "Send reset link" }).click();
    await page.waitForTimeout(400);

    const body = (await page.textContent("body")) ?? "";
    await page.close();

    expect(forgotBody).toMatchObject({
      email: "alice@example.com",
      captchaToken: STUB_TOKEN,
    });
    // The non-enumerating success state still renders.
    expect(body).toContain("a reset link is on its way");
  });

  test("login shows the widget only when the server demands it", async () => {
    if (skip()) return;

    const page = await open("/login");
    const loginBodies: Array<Record<string, unknown>> = [];
    let signedIn = false;

    await page.route("**/api/v1/**", (route) => {
      const p = new URL(route.request().url()).pathname;
      if (p.endsWith("/auth/login")) {
        const body = route.request().postDataJSON() as Record<string, unknown>;
        loginBodies.push(body);
        // First attempt: throttled, demand a captcha. Retry with a token: in.
        if (body.captchaToken) {
          signedIn = true;
          return route.fulfill(json({ email: "alice@example.com", isVerified: true }));
        }
        return route.fulfill(
          json({ error: { code: "captcha_required", message: "captcha" } }, 429),
        );
      }
      if (p.endsWith("/auth/me")) {
        // Stateful, like a real session: the account page re-checks it after
        // the login navigates there, and a hardcoded 401 would bounce us back.
        return signedIn
          ? route.fulfill(json({ email: "alice@example.com", isVerified: true }))
          : route.fulfill(json({ error: { code: "unauthorized", message: "no" } }, 401));
      }
      if (p.endsWith("/account/game-accounts")) return route.fulfill(json([]));
      if (p.endsWith("/account/characters")) return route.fulfill(json([]));
      return route.fulfill(json({ online: true, playersOnline: 0 }));
    });

    // No widget on the happy path.
    expect(await page.locator("#cf-turnstile").count()).toBe(0);

    await page.locator('input[type="email"]').fill("alice@example.com");
    await page.locator('input[type="password"]').fill("correct-horse");
    await page.getByRole("button", { name: "Log in", exact: true }).click();
    await page.waitForTimeout(400);

    // The demand makes the widget appear, plus a message saying why.
    expect(await page.locator("#cf-turnstile").count()).toBe(1);
    expect((await page.textContent("body")) ?? "").toContain("complete the check");

    // The stub has already produced a token; retry carries it.
    await page.waitForTimeout(200);
    await page.getByRole("button", { name: "Log in", exact: true }).click();
    await page.waitForTimeout(600);

    const url = page.url();
    await page.close();

    expect(loginBodies.length).toBe(2);
    expect(loginBodies[0]?.captchaToken ?? null).toBeNull();
    expect(loginBodies[1]?.captchaToken).toBe(STUB_TOKEN);
    // The assisted login went through: we're on the account page.
    expect(url).toContain("/account");
  });
});
