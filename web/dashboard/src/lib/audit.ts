/**
 * The Audit page's arithmetic: stage names, search, sorting and the
 * shared-address counts. Kept out of the component so it can be tested
 * without a browser.
 */
import type { ConnectedClient } from "./api";

export type StageTone = "ok" | "info" | "warn" | "neutral";

type StageInfo = { label: string; tone: StageTone; hint: string };

/** In login order, login server first: the order filters and sorts use. */
export const STAGES: Record<string, StageInfo> = {
  handshaking: {
    label: "Logging in",
    tone: "warn",
    hint: "Login server: connected, has not authenticated yet.",
  },
  logged_in: {
    label: "Server list",
    tone: "info",
    hint: "Login server: authenticated, choosing a game server.",
  },
  joining_game: {
    label: "Joining game",
    tone: "info",
    hint: "Login server: picked a game server and is about to move to it.",
  },
  authenticating: {
    label: "Authenticating",
    tone: "warn",
    hint: "Game server: connected, session key not confirmed by the login server yet.",
  },
  lobby: {
    label: "Lobby",
    tone: "neutral",
    hint: "Game server: at character selection.",
  },
  entering: {
    label: "Entering world",
    tone: "info",
    hint: "Game server: a character picked, loading into the world.",
  },
  in_game: { label: "In game", tone: "ok", hint: "Game server: a character in the world." },
};

const STAGE_ORDER = Object.keys(STAGES);

export function stageInfo(stage: string): StageInfo {
  return STAGES[stage] ?? { label: stage, tone: "neutral", hint: "" };
}

export const SERVICE_LABEL: Record<string, string> = {
  game_server: "Game",
  login_server: "Login",
};

/** Stable across polls: a server's ids are unique per process, and the
 *  connect time tells a restarted server's reused id apart. */
export function clientKey(c: ConnectedClient): string {
  return `${c.service}:${c.id}:${c.connectedMs}`;
}

/** Case-insensitive substring match over everything a GM might paste in. */
export function matches(c: ConnectedClient, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [c.ip, c.account, c.character, c.hwid, `${c.ip}:${c.port}`].some((v) =>
    v?.toLowerCase().includes(q),
  );
}

export type SortKey = "service" | "stage" | "account" | "character" | "ip" | "connected" | "idle";

/** What a click on a column sorts by first. Times sort newest first. */
export const FIRST_DIR: Record<SortKey, "asc" | "desc"> = {
  service: "asc",
  stage: "desc",
  account: "asc",
  character: "asc",
  ip: "asc",
  connected: "desc",
  idle: "desc",
};

/** IPv4 dotted quads compare numerically; anything else as text. */
function ipValue(ip: string): string {
  const parts = ip.split(".");
  if (parts.length === 4 && parts.every((p) => /^\d+$/.test(p))) {
    return parts.map((p) => p.padStart(3, "0")).join(".");
  }
  return ip;
}

function sortValue(c: ConnectedClient, key: SortKey): string | number {
  switch (key) {
    case "service":
      return c.service;
    case "stage": {
      const i = STAGE_ORDER.indexOf(c.stage);
      return i < 0 ? STAGE_ORDER.length : i;
    }
    case "account":
      return c.account?.toLowerCase() ?? "";
    case "character":
      return c.character?.toLowerCase() ?? "";
    case "ip":
      return ipValue(c.ip);
    case "connected":
      return c.connectedMs;
    case "idle":
      // Most recent packet = least idle; never-spoke counts as idlest.
      return -(c.traffic.lastPacketMs ?? c.connectedMs);
  }
}

/** Sorted copy. Ties fall back to newest connection first, so the order
 *  does not shuffle between polls. Empty text values go last either way. */
export function sortClients(
  clients: ConnectedClient[],
  key: SortKey,
  dir: "asc" | "desc",
): ConnectedClient[] {
  const sign = dir === "asc" ? 1 : -1;
  return [...clients].sort((a, b) => {
    const va = sortValue(a, key);
    const vb = sortValue(b, key);
    if (va !== vb) {
      if (va === "") return 1;
      if (vb === "") return -1;
      return (va < vb ? -1 : 1) * sign;
    }
    return b.connectedMs - a.connectedMs || a.id - b.id;
  });
}

/** How many game-server connections share each IP / HWID. Login-server rows
 *  are left out: a player passes through it on the way in, so counting both
 *  would flag every in-game player as a double. */
export function sharedCounts(clients: ConnectedClient[]): {
  ip: Map<string, number>;
  hwid: Map<string, number>;
} {
  const ip = new Map<string, number>();
  const hwid = new Map<string, number>();
  for (const c of clients) {
    if (c.service !== "game_server") continue;
    ip.set(c.ip, (ip.get(c.ip) ?? 0) + 1);
    if (c.hwid) hwid.set(c.hwid, (hwid.get(c.hwid) ?? 0) + 1);
  }
  return { ip, hwid };
}

/** "12s", "4m 05s", "3h 12m", "2d 4h". */
export function formatSince(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${String(sec).padStart(2, "0")}s`;
  return `${sec}s`;
}

/* --------------------------------- IP bans -------------------------------- */

/** Every ban entry that would cover `ip` — the server's rule
 *  (`models::repo::ip_bans::covering`, after Java's `isBannedAddress`): the address
 *  itself, then its `.0`, `.0.0` and `.0.0.0` ranges. */
export function banEntriesCovering(ip: string): string[] {
  const parts = ip.split(".");
  if (parts.length !== 4) return [ip];
  const [a, b, c] = parts;
  return [ip, `${a}.${b}.${c}.0`, `${a}.${b}.0.0`, `${a}.0.0.0`];
}

/** Whether a ban on `ban` refuses (and, placed with "disconnect", kicks) `ip`. */
export function banCovers(ban: string, ip: string): boolean {
  return banEntriesCovering(ip).includes(ban);
}

/** What a ban on `ban` covers, in words: "10.1.2.*" for `10.1.2.0`. */
export function banScope(ban: string): string {
  const parts = ban.split(".");
  if (parts.length !== 4 || parts[3] !== "0") return ban;
  let keep = 3;
  while (keep > 1 && parts[keep - 1] === "0") keep--;
  return [...parts.slice(0, keep), ...Array(4 - keep).fill("*")].join(".");
}

/** Ban lengths offered by the forms; `null` is permanent. */
export const BAN_DURATIONS: Array<{ label: string; ms: number | null }> = [
  { label: "1 hour", ms: 3_600_000 },
  { label: "1 day", ms: 86_400_000 },
  { label: "7 days", ms: 7 * 86_400_000 },
  { label: "30 days", ms: 30 * 86_400_000 },
  { label: "Permanent", ms: null },
];

/** "Permanent", "in 3h 12m", or "expired 2d 4h ago". */
export function formatExpiry(expiresAt: number | null, now: number): string {
  if (expiresAt === null) return "Permanent";
  return expiresAt > now
    ? `in ${formatSince(expiresAt - now)}`
    : `expired ${formatSince(now - expiresAt)} ago`;
}
