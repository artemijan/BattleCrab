/** The Audit page's list arithmetic (`src/lib/audit.ts`). */
import { describe, expect, test } from "bun:test";

import type { ConnectedClient } from "../src/lib/api";
import { formatSince, matches, sharedCounts, sortClients } from "../src/lib/audit";

function client(over: Partial<ConnectedClient> & { id: number }): ConnectedClient {
  return {
    service: "game_server",
    ip: "10.0.0.1",
    port: 50000,
    connectedMs: 1_000_000,
    stage: "in_game",
    account: null,
    character: null,
    hwid: null,
    traffic: { packetsIn: 0, bytesIn: 0, packetsOut: 0, bytesOut: 0, lastPacketMs: null },
    details: {},
    ...over,
  };
}

describe("search", () => {
  test("matches ip, account, character and hwid, ignoring case", () => {
    const c = client({ id: 1, account: "Alice", character: "Hero", hwid: "AA:BB" });
    for (const q of ["10.0.0", "alice", "HERO", "aa:bb", "  ", "10.0.0.1:50000"]) {
      expect(matches(c, q)).toBe(true);
    }
    expect(matches(c, "bob")).toBe(false);
  });
});

describe("sorting", () => {
  test("IPs sort numerically, not as text", () => {
    const rows = [
      client({ id: 1, ip: "10.0.0.10" }),
      client({ id: 2, ip: "10.0.0.9" }),
      client({ id: 3, ip: "9.1.1.1" }),
    ];
    expect(sortClients(rows, "ip", "asc").map((c) => c.id)).toEqual([3, 2, 1]);
  });

  test("stage sorts in login order and blank accounts go last either way", () => {
    const rows = [
      client({ id: 1, stage: "lobby", account: "b" }),
      client({ id: 2, stage: "handshaking", account: null }),
      client({ id: 3, stage: "in_game", account: "a" }),
    ];
    expect(sortClients(rows, "stage", "desc").map((c) => c.id)).toEqual([3, 1, 2]);
    expect(sortClients(rows, "account", "asc").map((c) => c.id)).toEqual([3, 1, 2]);
    expect(sortClients(rows, "account", "desc").map((c) => c.id)).toEqual([1, 3, 2]);
  });

  test("a client that never spoke is the idlest", () => {
    const rows = [
      client({ id: 1, traffic: { ...client({ id: 0 }).traffic, lastPacketMs: 5_000_000 } }),
      client({ id: 2 }),
    ];
    expect(sortClients(rows, "idle", "desc").map((c) => c.id)).toEqual([2, 1]);
  });
});

describe("shared addresses", () => {
  test("only game-server connections count", () => {
    const counts = sharedCounts([
      client({ id: 1, hwid: "X" }),
      client({ id: 2, hwid: "X" }),
      client({ id: 3, service: "login_server" }),
      client({ id: 4, ip: "10.0.0.2" }),
    ]);
    expect(counts.ip.get("10.0.0.1")).toBe(2);
    expect(counts.ip.get("10.0.0.2")).toBe(1);
    expect(counts.hwid.get("X")).toBe(2);
  });
});

test("formatSince", () => {
  expect(formatSince(-5)).toBe("0s");
  expect(formatSince(12_000)).toBe("12s");
  expect(formatSince(245_000)).toBe("4m 05s");
  expect(formatSince(3 * 3_600_000 + 12 * 60_000)).toBe("3h 12m");
  expect(formatSince(2 * 86_400_000 + 4 * 3_600_000)).toBe("2d 4h");
});
