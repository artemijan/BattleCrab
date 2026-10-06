/**
 * The admin Audit page against a stubbed API.
 *
 * Pins down what `tsc` cannot see: both servers' clients land in one table, a
 * shared address is flagged and filters to its siblings, the details follow a
 * client and say so when it disconnects, and a server that could not answer
 * is named rather than silently missing.
 *
 * Requires a built frontend (`bun run build`) and Google Chrome. Skips with a
 * clear message rather than failing when either is missing.
 */
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import type { Browser } from "playwright";

import type { ConnectedClient, ConnectedClients } from "../src/lib/api";

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

const ADMIN = { email: "admin@example.com", isVerified: true, isAdmin: true };
const NOW = 1_759_000_000_000;

/** Opens the Audit page with `/admin/monitor/clients` answered by `clients`
 *  (called once per request, so a test can change the answer over time). */
async function open(clients: () => { status: number; body: unknown }) {
  const page = await browser!.newPage({ viewport: { width: 1280, height: 1000 } });
  await page.route("**/api/v1/**", (route) => {
    const url = new URL(route.request().url());
    const json = (status: number, body: unknown) =>
      route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) });
    if (url.pathname.endsWith("/auth/me")) return json(200, ADMIN);
    if (url.pathname.endsWith("/admin/monitor/clients")) {
      const r = clients();
      return json(r.status, r.body);
    }
    return json(404, { error: { code: "not_found", message: "unstubbed" } });
  });
  await page.goto(`http://localhost:${server!.port}/admin/audit`, { waitUntil: "load" });
  await page.waitForTimeout(500);
  return page;
}

function client(over: Partial<ConnectedClient> & { id: number }): ConnectedClient {
  return {
    service: "game_server",
    ip: "10.0.0.1",
    port: 50000,
    connectedMs: NOW - 600_000,
    stage: "in_game",
    account: null,
    character: null,
    hwid: null,
    traffic: {
      packetsIn: 120,
      bytesIn: 4096,
      packetsOut: 900,
      bytesOut: 65536,
      lastPacketMs: NOW - 3_000,
    },
    details: {},
    ...over,
  };
}

const HERO = client({
  id: 7,
  account: "alice",
  character: "Hero",
  hwid: "AA:BB:CC:DD:EE:FF",
  details: {
    protocolVersion: 746,
    character: {
      objectId: 268_480_001,
      name: "Hero",
      title: "the Brave",
      level: 76,
      classId: 88,
      baseClassId: 88,
      race: 0,
      clan: "Crabs",
      accessLevel: 0,
      hero: false,
      noble: true,
      reputation: 0,
      pvpKills: 12,
      pkKills: 0,
      position: { x: 83_400, y: 147_900, z: -3_400 },
      hp: { cur: 3000, max: 4000 },
      mp: { cur: 900, max: 1200 },
      cp: { cur: 2000, max: 2000 },
      dead: false,
    },
  },
});
const ALT = client({
  id: 8,
  account: "alice2",
  stage: "lobby",
  details: { lobbyCharacters: [{ name: "Alt", level: 20, classId: 1 }] },
});
const NEWCOMER = client({
  id: 3,
  service: "login_server",
  ip: "192.168.1.5",
  stage: "handshaking",
  connectedMs: NOW - 4_000,
  traffic: { packetsIn: 0, bytesIn: 0, packetsOut: 1, bytesOut: 186, lastPacketMs: null },
});

function payload(clients: ConnectedClient[], loginUp = true): ConnectedClients {
  return {
    nowMs: NOW,
    clients,
    sources: [
      { service: "game_server", up: true, error: null },
      {
        service: "login_server",
        up: loginUp,
        error: loginUp ? null : "Connection refused (os error 61)",
      },
    ],
  };
}

describe("audit page", () => {
  test("lists both servers' clients and flags a shared address", async () => {
    if (skip()) return;
    const page = await open(() => ({ status: 200, body: payload([HERO, ALT, NEWCOMER]) }));
    const rows = page.locator("tbody tr");
    expect(await rows.count()).toBe(3);
    const text = await page.locator("tbody").innerText();
    expect(text).toContain("In game");
    expect(text).toContain("Lobby");
    expect(text).toContain("Logging in");
    expect(text).toContain("not logged in");
    expect(text).toContain("AA:BB:CC:DD:EE:FF");

    // Hero and Alt share 10.0.0.1 on the game server: flagged, and the marker
    // narrows the list to them.
    const marker = page.getByRole("button", { name: "×2" }).first();
    await marker.click();
    await page.waitForTimeout(200);
    expect(await rows.count()).toBe(2);
    expect(await page.getByLabel("Search").inputValue()).toBe("10.0.0.1");
    await page.close();
  });

  test("status chips and the server switch filter the table", async () => {
    if (skip()) return;
    const page = await open(() => ({ status: 200, body: payload([HERO, ALT, NEWCOMER]) }));
    await page.getByRole("button", { name: /^Lobby/ }).click();
    await page.waitForTimeout(200);
    expect(await page.locator("tbody tr").count()).toBe(1);
    expect(await page.locator("tbody").innerText()).toContain("alice2");

    await page.getByRole("button", { name: "Login", exact: true }).click();
    await page.waitForTimeout(200);
    // Switching server clears the status chip.
    expect(await page.locator("tbody tr").count()).toBe(1);
    expect(await page.locator("tbody").innerText()).toContain("192.168.1.5");
    await page.close();
  });

  test("details show the character and follow it until it disconnects", async () => {
    if (skip()) return;
    let calls = 0;
    const page = await open(() => {
      calls += 1;
      return { status: 200, body: payload(calls === 1 ? [HERO, ALT] : [ALT]) };
    });
    // The account cell's button: the row's keyboard path, and nowhere near
    // the ×N markers, which filter rather than open.
    await page.getByRole("button", { name: "alice", exact: true }).click();
    const dialog = page.getByRole("dialog");
    await dialog.waitFor();
    const text = await dialog.innerText();
    expect(text).toContain("Lv 76 Duelist");
    expect(text).toContain("Crabs");
    expect(text).toContain("83400, 147900, -3400");
    expect(text).toContain("10.0.0.1:50000");
    expect(text).toContain("HWID (MAC)");
    // Section titles are uppercased by CSS, which innerText reflects.
    expect(text.toLowerCase()).toContain("same ip (1 other)");
    expect(text).toContain("3,000 / 4,000");
    expect(text).not.toContain("Disconnected");

    // The next refresh no longer has Hero.
    await page.waitForTimeout(5_500);
    expect(await dialog.innerText()).toContain("This client has disconnected");
    expect(await dialog.innerText()).toContain("Lv 76 Duelist");

    // A sibling opens in place.
    await dialog.getByRole("button", { name: /alice2/ }).click();
    await page.waitForTimeout(200);
    expect((await dialog.innerText()).toLowerCase()).toContain("characters on the account (1)");
    await page.close();
  }, 15_000);

  test("a server that did not answer is named", async () => {
    if (skip()) return;
    const page = await open(() => ({ status: 200, body: payload([HERO], false) }));
    const note = await page.getByRole("status").innerText();
    expect(note).toContain("Login server did not answer");
    expect(note).toContain("Connection refused");
    await page.close();
  });

  test("monitoring switched off reads as disabled", async () => {
    if (skip()) return;
    const page = await open(() => ({
      status: 503,
      body: { error: { code: "unavailable", message: "server monitoring is disabled" } },
    }));
    expect(await page.getByRole("alert").innerText()).toContain("MonitorTargets is empty");
    await page.close();
  });
});
