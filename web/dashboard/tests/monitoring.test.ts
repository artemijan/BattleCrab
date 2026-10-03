/**
 * The admin monitoring and log-search pages against a stubbed API.
 *
 * What these pin down is the part `tsc` cannot see: that a gap in the data is
 * drawn as a gap, that game-only charts stay off the login server's tab, that
 * "Load older" really sends the server's cursor back, and that a search cut
 * short by its budget is presented as unfinished rather than as the end.
 *
 * Requires a built frontend (`bun run build`) and Google Chrome. Skips with a
 * clear message rather than failing when either is missing.
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

type Handler = (url: URL) => unknown;

/** Opens `path` with `/auth/me` as an admin and each other call answered by
 *  the first handler whose key the path ends with. Every API request URL is
 *  recorded, for asserting what the page asked for. */
async function open(path: string, handlers: Record<string, Handler>) {
  const page = await browser!.newPage({ viewport: { width: 1200, height: 1400 } });
  const requests: URL[] = [];
  await page.route("**/api/v1/**", (route) => {
    const url = new URL(route.request().url());
    requests.push(url);
    const json = (status: number, body: unknown) =>
      route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) });
    if (url.pathname.endsWith("/auth/me")) return json(200, ADMIN);
    for (const [suffix, handler] of Object.entries(handlers)) {
      if (url.pathname.endsWith(suffix)) return json(200, handler(url));
    }
    return json(404, { error: { code: "not_found", message: "unstubbed" } });
  });
  await page.goto(`http://localhost:${server!.port}${path}`, { waitUntil: "load" });
  await page.waitForTimeout(500);
  return { page, requests };
}

const SERVICES = {
  services: [
    {
      service: "game_server",
      address: "127.0.0.1:7779",
      up: true,
      lastPollMs: 1,
      lastSampleTs: 1,
      startedMs: 1,
      lastError: null,
      uptimeSeconds: 2 * 3600 + 300,
    },
    {
      service: "login_server",
      address: "127.0.0.1:7780",
      up: false,
      lastPollMs: 1,
      lastSampleTs: null,
      startedMs: null,
      lastError: "Connection refused (os error 61)",
      uptimeSeconds: null,
    },
  ],
  pollSeconds: 5,
  retentionDays: 7,
};

/** Series over the requested range: 12 buckets with a two-bucket hole. */
function series(url: URL) {
  const from = Number(url.searchParams.get("from"));
  const to = Number(url.searchParams.get("to"));
  const bucketMs = 300_000;
  const ts: number[] = [];
  for (let t = Math.ceil(from / bucketMs) * bucketMs; t < to && ts.length < 12; t += bucketMs) {
    ts.push(t);
  }
  const hole = (i: number) => i === 5 || i === 6;
  const kept = ts.filter((_, i) => !hole(i));
  const col = (v: number) => kept.map(() => v);
  return {
    from,
    to,
    bucketMs,
    aggregation: {},
    ts: kept,
    samples: col(60),
    intervalMs: col(300_000),
    series: {
      packets_in: col(300_000),
      packets_out: col(600_000),
      bytes_in: col(3e7),
      bytes_out: col(6e7),
      connections_open: col(12),
      players_online: col(9),
      connections_accepted: col(30),
      cpu_micros: col(30_000_000),
      rss_bytes: col(512 * 1024 * 1024),
      heap_bytes: col(256 * 1024 * 1024),
      tick_busy_micros_total: col(6_000_000),
      ticks: col(3000),
      tick_overruns: col(0),
    },
  };
}

describe("monitoring page", () => {
  test("server cards, rates, and a hole in the data drawn as a hole", async () => {
    if (skip()) return;
    const { page, requests } = await open("/admin/monitor", {
      "/admin/monitor/services": () => SERVICES,
      "/admin/monitor/series": series,
    });
    const body = (await page.textContent("body")) ?? "";
    const packetsPath = await page.evaluate(() => {
      const panel = [...document.querySelectorAll("h3")].find((h) => h.textContent === "Packets");
      return panel?.closest("div.p-4")?.querySelector("path")?.getAttribute("d") ?? "";
    });
    const titles = await page.$$eval("h3", (hs) => hs.map((h) => h.textContent));
    await page.close();

    expect(body).toContain("Up 2h 5m");
    expect(body).toContain("Down");
    expect(body).toContain("Connection refused");
    // 300 000 packets over a 300 s bucket is 1 000/s; CPU 30 s over 300 s is 10%.
    expect(body).toContain("1k/s");
    expect(body).toContain("10%");
    // Mean tick: 6 000 000 µs over 3 000 ticks = 2 ms.
    expect(body).toContain("2 ms");
    // Two runs, not one line bridging the missing buckets.
    expect(packetsPath.match(/M/g)?.length).toBe(2);
    expect(titles).toContain("Game loop — mean tick (budget 100 ms)");
    expect(requests.some((u) => u.searchParams.get("service") === "game_server")).toBe(true);
  });

  test("the login server tab drops game-only charts and asks for the login server", async () => {
    if (skip()) return;
    const { page, requests } = await open("/admin/monitor", {
      "/admin/monitor/services": () => SERVICES,
      "/admin/monitor/series": series,
    });
    await page.getByRole("tab", { name: "Login server" }).click();
    await page.waitForTimeout(400);
    const titles = await page.$$eval("h3", (hs) => hs.map((h) => h.textContent ?? ""));
    const legend = (await page.textContent("main")) ?? "";
    await page.close();

    expect(requests.some((u) => u.searchParams.get("service") === "login_server")).toBe(true);
    expect(titles.some((t) => t.startsWith("Game loop"))).toBe(false);
    expect(legend).not.toContain("Players");
    expect(legend).not.toContain("Heap");
  });

  test("monitoring disabled server-side says so instead of showing empty charts", async () => {
    if (skip()) return;
    const page = await browser!.newPage();
    await page.route("**/api/v1/**", (route) => {
      const url = new URL(route.request().url());
      if (url.pathname.endsWith("/auth/me"))
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify(ADMIN),
        });
      return route.fulfill({
        status: 503,
        contentType: "application/json",
        body: JSON.stringify({
          error: { code: "unavailable", message: "server monitoring is disabled" },
        }),
      });
    });
    await page.goto(`http://localhost:${server!.port}/admin/monitor`, { waitUntil: "load" });
    await page.waitForTimeout(1500);
    const body = (await page.textContent("body")) ?? "";
    await page.close();
    expect(body).toContain("monitoring is disabled");
  });
});

const STREAMS = {
  streams: [
    { service: "game_server", stream: "diagnostic", files: 2, oldest: 0, newestEnd: 1 },
    { service: "game_server", stream: "audit:chat", files: 1, oldest: 0, newestEnd: 1 },
    { service: "login_server", stream: "error", files: 1, oldest: 0, newestEnd: 1 },
  ],
};

function hit(i: number) {
  return {
    file: "game_server.2026-08-14.json",
    offset: 1000 - i,
    ts: 1_786_700_000_000 - i * 1000,
    line: {
      timestamp: "2026-08-14T00:00:00Z",
      level: i === 1 ? "ERROR" : "INFO",
      message: `event ${i}`,
      target: "gameserver::net",
      span: { account: "bob" },
    },
  };
}

describe("logs page", () => {
  test("searches with the chosen filters, pages with the cursor, and flags truncation", async () => {
    if (skip()) return;
    const { page, requests } = await open("/admin/logs", {
      "/admin/logs/streams": () => STREAMS,
      "/admin/logs/search": (url) =>
        url.searchParams.get("cursor") === "page2"
          ? {
              hits: [hit(3)],
              cursor: "page3",
              stopped: "maxBytes",
              truncated: true,
              scannedBytes: 1024,
              filesScanned: ["game_server.2026-08-13.json"],
              skippedOversized: 0,
            }
          : {
              hits: [hit(1), hit(2)],
              cursor: "page2",
              stopped: "limit",
              truncated: false,
              scannedBytes: 2048,
              filesScanned: ["game_server.2026-08-14.json"],
              skippedOversized: 0,
            },
    });

    await page.getByLabel("Text (case-insensitive)").fill("kaboom");
    await page.getByLabel("Minimum level").selectOption("warn");
    await page.getByRole("button", { name: "Search" }).click();
    await page.waitForTimeout(400);
    const firstPage = (await page.textContent("main")) ?? "";
    await page.getByRole("button", { name: "Load older" }).click();
    await page.waitForTimeout(400);
    const afterOlder = (await page.textContent("main")) ?? "";
    const keepSearching = await page.getByRole("button", { name: "Keep searching" }).count();
    await page.close();

    const searches = requests.filter((u) => u.pathname.endsWith("/admin/logs/search"));
    expect(searches.length).toBe(2);
    const first = searches[0]!.searchParams;
    expect(first.get("service")).toBe("game_server");
    expect(first.get("stream")).toBe("diagnostic");
    expect(first.get("q")).toBe("kaboom");
    expect(first.get("level")).toBe("warn");
    expect(Number(first.get("to")) - Number(first.get("from"))).toBe(86_400_000);
    expect(searches[1]!.searchParams.get("cursor")).toBe("page2");

    expect(firstPage).toContain("every search is recorded");
    expect(firstPage).toContain("event 1");
    expect(firstPage).toContain("span=");
    expect(afterOlder.indexOf("event 2")).toBeLessThan(afterOlder.indexOf("event 3"));
    expect(afterOlder).toContain("older lines have not been searched yet");
    expect(keepSearching).toBe(1);
  });

  test("audit streams disable the level filter and never send one", async () => {
    if (skip()) return;
    const { page, requests } = await open("/admin/logs", {
      "/admin/logs/streams": () => STREAMS,
      "/admin/logs/search": () => ({
        hits: [],
        cursor: null,
        stopped: null,
        truncated: false,
        scannedBytes: 0,
        filesScanned: [],
        skippedOversized: 0,
      }),
    });
    await page.getByLabel("Minimum level").selectOption("error");
    await page.getByLabel("Log", { exact: true }).selectOption("audit:chat");
    const disabled = await page.getByLabel("Minimum level").isDisabled();
    await page.getByRole("button", { name: "Search" }).click();
    await page.waitForTimeout(400);
    const body = (await page.textContent("main")) ?? "";
    await page.close();

    expect(disabled).toBe(true);
    const search = requests.find((u) => u.pathname.endsWith("/admin/logs/search"))!;
    expect(search.searchParams.get("stream")).toBe("audit:chat");
    expect(search.searchParams.has("level")).toBe(false);
    expect(body).toContain("No matching lines");
  });

  test("the admin tabs mark the current section, including account detail pages", async () => {
    if (skip()) return;
    const current = async (path: string) => {
      const { page } = await open(path, {
        "/admin/logs/streams": () => STREAMS,
        "/admin/accounts/bob%40example.com": () => ({
          master: {
            email: "bob@example.com",
            isVerified: true,
            accessLevel: 0,
            createdTime: "2026-08-01 10:00:00",
            lastActive: 0,
            gameAccounts: 0,
            characters: 0,
          },
          gameAccounts: [],
          characters: [],
        }),
      });
      const tabs = await page.$$eval(
        'nav[aria-label="Admin sections"] [aria-current="page"]',
        (as) => as.map((a) => a.textContent),
      );
      await page.close();
      return tabs;
    };
    expect(await current("/admin/logs")).toEqual(["Logs"]);
    // Not a prefix of /admin/accounts/…, yet still the Accounts section.
    expect(await current("/admin/accounts/bob%40example.com")).toEqual(["Accounts"]);
  });
});
