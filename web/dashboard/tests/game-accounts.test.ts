/**
 * Covers the one piece of real logic on the account page: characters are
 * fetched flat, with an `accountName` each, and joined client-side onto the
 * game-account list.
 *
 * That join is invisible to `tsc` — a wrong key produces a page that renders
 * perfectly and shows every character under the wrong login, or none at all.
 * The empty game account is the case worth pinning down: it appears in neither
 * character list, so anything driven off `/characters` alone would drop it, and
 * it is exactly the account a user has just created and wants to see.
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

const ALICE = { email: "alice@example.com", isVerified: true };

function character(accountName: string, name: string, level: number) {
  return {
    accountName,
    name,
    level,
    classId: 0,
    race: 0,
    sex: 0,
    onlineTime: 7200,
    lastAccess: 0,
    online: false,
  };
}

/**
 * Opens /account against a stubbed API.
 *
 * The production bundle points at an absolute API origin, so every call is
 * intercepted by pattern rather than served — nothing here touches a network.
 */
async function openAccountPage(stubs: {
  me?: unknown;
  gameAccounts?: unknown;
  characters?: unknown;
}): Promise<Page> {
  const page = await browser!.newPage({ viewport: { width: 1024, height: 900 } });

  const json = (body: unknown) => ({
    status: 200,
    contentType: "application/json",
    body: JSON.stringify(body),
  });

  await page.route("**/api/v1/**", (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith("/auth/me")) return route.fulfill(json(stubs.me ?? ALICE));
    if (path.endsWith("/account/game-accounts"))
      return route.fulfill(json(stubs.gameAccounts ?? []));
    if (path.endsWith("/account/characters")) return route.fulfill(json(stubs.characters ?? []));
    return route.fulfill({
      status: 404,
      contentType: "application/json",
      body: JSON.stringify({ error: { code: "internal", message: "unstubbed" } }),
    });
  });

  await page.goto(`http://localhost:${server!.port}/account`, { waitUntil: "load" });
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

describe("game accounts", () => {
  test("each character is listed under the game account it belongs to", async () => {
    if (skip()) return;

    const page = await openAccountPage({
      gameAccounts: ["alice1", "alice2", "alice3"],
      characters: [
        character("alice1", "Warrior", 40),
        character("alice1", "Healer", 22),
        character("alice2", "Mage", 65),
      ],
    });

    // Read back the rendered grouping: heading -> the names beneath it.
    // Keyed off each account's region rather than by walking siblings, so
    // wrapping the list for animation does not break the walk.
    const grouped = await page.evaluate(() => {
      const out: Record<string, string[]> = {};
      for (const region of document.querySelectorAll('[id^="game-account-"]')) {
        // The heading is what the user actually reads the login from, so take
        // it from there rather than from the id we generated.
        const login = region.previousElementSibling?.querySelector("p")?.textContent?.trim();
        if (!login) continue;
        out[login] = [...region.querySelectorAll("li p:first-child")].map((n) =>
          (n.textContent ?? "").trim(),
        );
      }
      return out;
    });
    const bodyText = await page.textContent("body");
    await page.close();

    expect(grouped).toEqual({
      alice1: ["Warrior", "Healer"],
      alice2: ["Mage"],
    });

    // alice3 has no characters, so it gets no collapsible region at all — but
    // the account itself must still be on the page, with its own empty state.
    expect(bodyText).toContain("alice3");
    expect(bodyText).toContain("No characters");
  });

  test("an unverified account is told to confirm instead of offered the form", async () => {
    if (skip()) return;

    const page = await openAccountPage({
      me: { email: "alice@example.com", isVerified: false },
      gameAccounts: [],
    });
    const bodyText = (await page.textContent("body")) ?? "";
    const hasButton = await page.evaluate(() =>
      [...document.querySelectorAll("button")].some((b) =>
        (b.textContent ?? "").includes("New game account"),
      ),
    );
    await page.close();

    // The server refuses this anyway (403 email_not_verified); the point is not
    // to hand the user a form that cannot succeed.
    expect(hasButton).toBe(false);
    expect(bodyText).toContain("Confirm your email address first");
  });

  test("collapsing one account hides its characters and leaves the others alone", async () => {
    if (skip()) return;

    const page = await openAccountPage({
      gameAccounts: ["alice1", "alice2"],
      characters: [character("alice1", "Warrior", 40), character("alice2", "Mage", 65)],
    });

    const toggle = page.locator('button[aria-controls="game-account-alice1"]');
    expect(await toggle.getAttribute("aria-expanded")).toBe("true");

    await toggle.click();
    // Past the 300ms transition, so this measures the resting state.
    await page.waitForTimeout(600);

    // The list stays in the DOM to be animated, so "hidden" is a measurement,
    // not an absence. `inert` is what carries the meaning unmounting used to:
    // no focus, and out of the accessibility tree.
    const after = await page.evaluate(() => {
      const region = (login: string) => document.querySelector(`#game-account-${login}`);
      return {
        alice1Height: region("alice1")?.getBoundingClientRect().height ?? -1,
        alice2Height: region("alice2")?.getBoundingClientRect().height ?? -1,
        alice1Inert: (region("alice1") as HTMLElement | null)?.hasAttribute("inert"),
        alice2Inert: (region("alice2") as HTMLElement | null)?.hasAttribute("inert"),
        expanded: document
          .querySelector('button[aria-controls="game-account-alice1"]')
          ?.getAttribute("aria-expanded"),
        body: document.body.textContent ?? "",
      };
    });
    await page.close();

    expect(after.alice1Height).toBe(0);
    expect(after.alice1Inert).toBe(true);
    expect(after.expanded).toBe("false");

    // Collapsing one panel must not touch its neighbour.
    expect(after.alice2Height).toBeGreaterThan(0);
    expect(after.alice2Inert).toBe(false);
    expect(after.body).toContain("Mage");

    // The count is what is left to judge a collapsed account by, so it stays.
    expect(after.body).toContain("1 character");
    expect(after.body).toContain("alice1");
  });

  /**
   * The collapse has to be driven by a transition rather than a class that
   * simply swaps the end states — that would pass every height assertion above
   * while snapping shut.
   */
  test("the collapse is animated rather than snapping", async () => {
    if (skip()) return;

    const page = await openAccountPage({
      gameAccounts: ["alice1"],
      characters: [
        character("alice1", "Warrior", 40),
        character("alice1", "Healer", 22),
        character("alice1", "Rogue", 31),
      ],
    });

    const style = await page.evaluate(() => {
      const region = document.querySelector("#game-account-alice1")!;
      const computed = getComputedStyle(region);
      return {
        property: computed.transitionProperty,
        duration: computed.transitionDuration,
        fullHeight: region.getBoundingClientRect().height,
      };
    });

    // Sampled through the browser's own animation registry rather than by
    // measuring the height at a chosen instant. An earlier version waited 120ms
    // and asserted a height strictly between the two end states; that flaked
    // when the machine was loaded, because the sample landed after the
    // transition had already finished. A running transition is observable for
    // its whole duration, and only exists at all if one was really started.
    await page.locator('button[aria-controls="game-account-alice1"]').click();
    const running = await page.evaluate(() =>
      document
        .querySelector("#game-account-alice1")!
        .getAnimations()
        .map((animation) => (animation as CSSTransition).transitionProperty),
    );

    await page.waitForTimeout(600);
    const restingHeight = await page.evaluate(
      () => document.querySelector("#game-account-alice1")!.getBoundingClientRect().height,
    );
    await page.close();

    expect(style.property).toContain("grid-template-rows");
    expect(style.duration).not.toBe("0s");
    expect(style.fullHeight).toBeGreaterThan(0);
    expect(running).toContain("grid-template-rows");
    // And it still arrives at the collapsed state rather than easing forever.
    expect(restingHeight).toBe(0);
  });

  test("a collapse survives a reload", async () => {
    if (skip()) return;

    const page = await openAccountPage({
      gameAccounts: ["alice1"],
      characters: [character("alice1", "Warrior", 40)],
    });

    await page.locator('button[aria-controls="game-account-alice1"]').click();
    await page.waitForTimeout(600);
    await page.reload({ waitUntil: "load" });
    await page.waitForTimeout(600);

    // A panel that springs back open on every visit is not really collapsible.
    const height = await page.evaluate(
      () => document.querySelector("#game-account-alice1")?.getBoundingClientRect().height ?? -1,
    );
    await page.close();

    expect(height).toBe(0);
  });

  test("an account with no characters has no collapse toggle", async () => {
    if (skip()) return;

    const page = await openAccountPage({ gameAccounts: ["alice1"], characters: [] });
    const toggles = await page.evaluate(
      () => document.querySelectorAll("button[aria-controls]").length,
    );
    const bodyText = (await page.textContent("body")) ?? "";
    await page.close();

    // Hiding one line of "log in to create a character" is a control that costs
    // more than it saves.
    expect(toggles).toBe(0);
    expect(bodyText).toContain("No characters");
  });

  test("the account page offers no way to change the email address", async () => {
    if (skip()) return;

    const page = await openAccountPage({ gameAccounts: ["alice1"] });
    const bodyText = (await page.textContent("body")) ?? "";
    const emailInputs = await page.evaluate(
      () => document.querySelectorAll('input[type="email"]').length,
    );
    await page.close();

    // The address is the account's identity and the only record of which game
    // accounts belong to it, so the API has no endpoint to move it. A form here
    // would be a dead control.
    expect(emailInputs).toBe(0);
    expect(bodyText).not.toContain("This is how you sign in");
    expect(bodyText).not.toContain("Send verification");
  });

  test("a verified account with no game accounts is offered the form", async () => {
    if (skip()) return;

    const page = await openAccountPage({ gameAccounts: [] });
    const hasButton = await page.evaluate(() =>
      [...document.querySelectorAll("button")].some((b) =>
        (b.textContent ?? "").includes("New game account"),
      ),
    );
    const bodyText = (await page.textContent("body")) ?? "";
    await page.close();

    expect(hasButton).toBe(true);
    expect(bodyText).toContain("No game accounts yet");
  });
});
