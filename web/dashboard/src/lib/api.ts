/**
 * Thin API client.
 *
 * Auth rides on an HttpOnly session cookie, so every request needs
 * `credentials: "include"` and there is no token for JS to hold — the cookie is
 * deliberately unreadable from here.
 */

export type ApiErrorCode =
  | "bad_request"
  | "invalid_credentials"
  | "unauthorized"
  | "login_taken"
  | "email_taken"
  | "email_not_verified"
  | "too_many_game_accounts"
  | "registration_disabled"
  | "forbidden"
  | "account_banned"
  | "not_found"
  | "rate_limited"
  | "captcha_required"
  | "captcha_failed"
  | "invalid_token"
  /** A feature switched off server-side (monitoring, log search): 503. */
  | "unavailable"
  /** A game or login server the request needed did not answer: 502. */
  | "upstream"
  | "internal";

export class ApiError extends Error {
  constructor(
    readonly code: ApiErrorCode,
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

/**
 * The signed-in *master* account. It has no game login of its own — the address
 * is the identity — and owns zero or more game accounts (`GameAccount`).
 */
export type Account = {
  email: string | null;
  isVerified: boolean;
  /** Grants the /admin routes. Enforced server-side; this only shapes the UI. */
  isAdmin: boolean;
};

/** A game account's login name, as typed into the game client. */
export type GameAccount = string;

export type Character = {
  /** Which of the master's game accounts this character sits on. */
  accountName: string;
  name: string;
  level: number;
  classId: number;
  race: number;
  sex: number;
  onlineTime: number;
  lastAccess: number;
  online: boolean;
};

export type ServerStatus = {
  online: boolean;
  playersOnline: number;
};

/** One row of a character's belongings — a stack, a worn piece, or a deposit. */
export type CharacterItem = {
  /** Unique per row; stacks of the same item are distinct rows. */
  objectId: number;
  itemId: number;
  name: string;
  /** `Weapon`, `Armor`, `EtcItem` — or `Unknown` when the item is off-catalog. */
  type: string;
  /** Atlas key for `ItemIcon`; null when the catalog has no icon for it. */
  icon: string | null;
  count: number;
  enchant: number;
  equipped: boolean;
  /** Paperdoll slot id for worn items (see `PaperdollSlot` server-side); null in the bag. */
  slot: number | null;
  /** Quest items get their own tab, as in the game client. */
  quest: boolean;
};

export type CharacterItems = {
  /** Carried and worn, worn gear first. */
  inventory: CharacterItem[];
  warehouse: CharacterItem[];
};

/* ----------------------------- Admin types ------------------------------ */

export type AdminMasterSummary = {
  email: string;
  isVerified: boolean;
  accessLevel: number;
  /** SQLite's CURRENT_TIMESTAMP text, e.g. "2026-07-23 10:15:00". */
  createdTime: string;
  /** Millis since epoch; 0 when the account never logged in. */
  lastActive: number;
  gameAccounts: number;
  characters: number;
};

export type AdminGameAccount = {
  login: string;
  /** Owning master's address; null for a row auto-created by the login server. */
  email: string | null;
  accessLevel: number;
  lastActive: number;
  lastIp: string | null;
  characters: number;
};

/** Server-side sort whitelist for the master list — must match `MasterSort::parse`. */
export type AdminSortKey =
  | "created"
  | "email"
  | "accessLevel"
  | "verified"
  | "lastActive"
  | "gameAccounts"
  | "characters";

export type AdminSortDir = "asc" | "desc";

export type AdminAccountList = {
  total: number;
  accounts: AdminMasterSummary[];
};

export type AdminAccountDetail = {
  master: AdminMasterSummary;
  gameAccounts: AdminGameAccount[];
  characters: Character[];
};

/* --------------------------- Monitoring types ---------------------------- */
/* Shapes of /admin/monitor/* — docs/MONITORING.md §5. */

export type MonitorService = "game_server" | "login_server";

export type MonitorServiceStatus = {
  service: MonitorService;
  address: string;
  /** The dashboard's last poll of this server succeeded. */
  up: boolean;
  lastPollMs: number | null;
  lastSampleTs: number | null;
  startedMs: number | null;
  lastError: string | null;
  /** Null while down. */
  uptimeSeconds: number | null;
};

export type MonitorServices = {
  services: MonitorServiceStatus[];
  pollSeconds: number;
  retentionDays: number;
};

/**
 * Bucketed series, columnar: `ts[i]` pairs with `series[name][i]`. `sum`
 * series are per-bucket totals (divide by `intervalMs` for a rate), `max`
 * series the bucket's peak.
 */
export type MonitorSeries = {
  from: number;
  to: number;
  bucketMs: number;
  aggregation: Record<string, "sum" | "max">;
  ts: number[];
  /** Raw samples per bucket — fewer than expected is a gap. */
  samples: number[];
  intervalMs: number[];
  series: Record<string, Array<number | null>>;
};

/* ----------------------------- Audit types ------------------------------ */
/* Shape of /admin/monitor/clients — docs/MONITORING.md §10. */

/** Login: handshaking → logged_in → joining_game. Game: authenticating →
 *  lobby → entering → in_game. Unknown keys render as-is. */
export type ClientStage =
  | "handshaking"
  | "logged_in"
  | "joining_game"
  | "authenticating"
  | "lobby"
  | "entering"
  | "in_game";

export type ClientTraffic = {
  packetsIn: number;
  bytesIn: number;
  packetsOut: number;
  bytesOut: number;
  /** Epoch ms of the client's last packet; null before its first. */
  lastPacketMs: number | null;
};

/** One open connection. `details` is server-specific; see `ClientDetails`. */
export type ConnectedClient = {
  service: MonitorService;
  /** Unique per server process; pair with `service` for a key. */
  id: number;
  ip: string;
  port: number;
  connectedMs: number;
  stage: ClientStage | string;
  account: string | null;
  character: string | null;
  /** The MAC address from RequestHardWareInfo — null when never reported. */
  hwid: string | null;
  traffic: ClientTraffic;
  details: ClientDetails;
};

type Gauge = { cur: number; max: number };

export type ClientDetails = {
  /** Game server. */
  protocolVersion?: number;
  hardware?: {
    cpu: string;
    cpuSpeedMhz: number;
    cpuCores: number;
    gpu: string;
    gpuDriver: string;
    windows: string;
  };
  lobbyCharacters?: Array<{ name: string; level: number; classId: number }>;
  character?: {
    objectId: number;
    name: string;
    title: string;
    level: number;
    classId: number;
    baseClassId: number;
    race: number;
    clan: string | null;
    accessLevel: number;
    hero: boolean;
    noble: boolean;
    reputation: number;
    pvpKills: number;
    pkKills: number;
    position: { x: number; y: number; z: number } | null;
    hp: Gauge | null;
    mp: Gauge | null;
    cp: Gauge | null;
    dead: boolean;
  };
  /** Login server. */
  accessLevel?: number;
  lastServer?: number;
  joiningServer?: number;
};

export type ClientSource = { service: MonitorService; up: boolean; error: string | null };

export type ConnectedClients = {
  /** The dashboard's clock when it answered; durations are measured from it. */
  nowMs: number;
  clients: ConnectedClient[];
  /** One per monitored server, in MonitorTargets order. */
  sources: ClientSource[];
};

/* ----------------------------- IP ban types ------------------------------ */
/* Shapes of /admin/ip-bans — docs/MONITORING.md §10. */

export type IpBan = {
  /** An address; trailing `.0` octets make it a range (10.1.2.0 = 10.1.2.*). */
  ip: string;
  /** Epoch ms; null for a permanent ban. */
  expiresAt: number | null;
  reason: string;
  /** The admin's master address, or the server that placed an automatic
   *  ban: `login_server` (wrong passwords, a game server's temp ban) or
   *  `game_server` (connections that kept failing to authenticate). */
  bannedBy: string;
  createdAt: number;
};

export type IpBans = {
  /** The dashboard's clock, to tell expired bans from active ones. */
  nowMs: number;
  bans: IpBan[];
};

export type IpBanInput = { ip: string; expiresAt: number | null; reason: string };

export type IpBanCreated = {
  ban: IpBan;
  /** Connections closed because the ban was placed with `disconnect`. */
  disconnected: number;
  /** Servers that could not be asked to disconnect, with why. */
  disconnectErrors: string[];
};

/* --------------------------- Log search types ---------------------------- */
/* Shapes of /admin/logs/* — docs/MONITORING.md §6. */

export type LogStreamInfo = {
  service: string;
  /** `diagnostic`, `error`, or `audit:<category>`. */
  stream: string;
  files: number;
  oldest: number | null;
  newestEnd: number | null;
};

export type LogHit = {
  file: string;
  offset: number;
  ts: number | null;
  /** A parsed JSON line (diagnostic, audit) or a raw text line (`_error.log`). */
  line: Record<string, unknown> | string;
};

export type LogSearchResult = {
  service: string;
  stream: string;
  from: number;
  to: number;
  hits: LogHit[];
  /** Present when there is more to read; pass back to continue. */
  cursor: string | null;
  stopped: "limit" | "maxBytes" | "deadline" | null;
  /** The scan hit a time or size budget before covering the range. */
  truncated: boolean;
  scannedBytes: number;
  filesScanned: string[];
  skippedOversized: number;
};

export type LogSearchParams = {
  service: string;
  stream: string;
  from: number;
  to: number;
  q: string;
  regex: boolean;
  /** Minimum level; omit for audit streams. */
  level?: string;
  limit?: number;
  cursor?: string;
};

/**
 * Where the API lives.
 *
 * `__API_BASE__` is substituted at build time from the `API_BASE_URL` env var
 * (see `build.ts`), defaulting to the production API on its own subdomain. When
 * it is absent — `bun run dev`, which applies no defines — this falls back to a
 * same-origin relative path, which the dev server proxies to a local backend
 * and which therefore needs no CORS at all.
 *
 * It must be a bare identifier rather than `process.env.X`. `process` does not
 * exist in the browser, so any `typeof process` guard around it short-circuits
 * to the fallback at runtime and the substituted value is never read — the
 * absolute URL sits in the bundle looking correct while every request quietly
 * goes to the site's own origin. `typeof` on an *undeclared* identifier is
 * safe, so this form works whether or not the define was applied.
 */
declare const __API_BASE__: string | undefined;

const BASE = (typeof __API_BASE__ !== "undefined" && __API_BASE__) || "/api/v1";

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${BASE}${path}`, {
    ...init,
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
      // The server requires this on mutations; with SameSite=Lax it is what
      // stops a cross-site form post from counting as an authenticated request.
      "X-Requested-With": "XMLHttpRequest",
      ...init?.headers,
    },
  });

  if (response.status === 204) return undefined as T;

  const text = await response.text();
  const body = text ? JSON.parse(text) : null;

  if (!response.ok) {
    const error = body?.error;
    throw new ApiError(
      error?.code ?? "internal",
      error?.message ?? "Something went wrong.",
      response.status,
    );
  }
  return body as T;
}

const post = <T>(path: string, body: unknown) =>
  request<T>(path, { method: "POST", body: JSON.stringify(body) });

const put = <T>(path: string, body: unknown) =>
  request<T>(path, { method: "PUT", body: JSON.stringify(body) });

const del = <T>(path: string) => request<T>(path, { method: "DELETE" });

export const api = {
  register: (email: string, password: string, captchaToken: string | null) =>
    post<Account>("/auth/register", { email, password, captchaToken }),

  // The token is optional: the server only demands one once the rate limiter
  // has started rejecting this client (error code `captcha_required`).
  login: (email: string, password: string, captchaToken?: string | null) =>
    post<Account>("/auth/login", { email, password, captchaToken }),

  resendVerification: () => post<void>("/auth/resend-verification", {}),

  logout: () => post<void>("/auth/logout", {}),

  me: () => request<Account>("/auth/me"),

  forgotPassword: (email: string, captchaToken: string | null) =>
    post<void>("/auth/forgot-password", { email, captchaToken }),

  resetPassword: (token: string, password: string) =>
    post<void>("/auth/reset-password", { token, password }),

  changePassword: (currentPassword: string, newPassword: string) =>
    post<void>("/account/password", { currentPassword, newPassword }),

  // There is deliberately no changeEmail: the address is the account's identity
  // and the only record of which game accounts belong to it, so moving it is an
  // account migration rather than a setting. The API has no endpoint for it.

  // GET with the token in the query string, and deliberately no session
  // required: the link is clicked from an inbox, often in a different browser
  // from the one that requested it.
  verifyEmail: (token: string) =>
    request<void>(`/account/email/verify?token=${encodeURIComponent(token)}`),

  gameAccounts: () => request<GameAccount[]>("/account/game-accounts"),

  /**
   * Creates a game account under the signed-in master account. The address is
   * taken from the session server-side — there is deliberately no way to name
   * a different owner from here.
   */
  createGameAccount: (login: string, password: string) =>
    post<{ login: string }>("/account/game-accounts", { login, password }),

  characters: () => request<Character[]>("/account/characters"),

  /** Inventory and warehouse of one of the session's own characters. */
  characterItems: (name: string) =>
    request<CharacterItems>(`/account/characters/${encodeURIComponent(name)}/items`),

  status: () => request<ServerStatus>("/server/status"),

  /**
   * The `/admin` surface. Every call 403s for a non-admin session — the
   * `isAdmin` flag on `Account` only decides whether the UI offers these.
   *
   * Access levels only ever go to 0 (restore) or negative (ban) from here; the
   * server refuses anything positive, so there is deliberately no "promote".
   */
  admin: {
    accounts: (q: string, offset: number, limit: number, sort: AdminSortKey, dir: AdminSortDir) =>
      request<AdminAccountList>(
        `/admin/accounts?q=${encodeURIComponent(q)}&offset=${offset}&limit=${limit}` +
          `&sort=${sort}&dir=${dir}`,
      ),

    account: (email: string) =>
      request<AdminAccountDetail>(`/admin/accounts/${encodeURIComponent(email)}`),

    verifyMaster: (email: string) =>
      post<void>(`/admin/accounts/${encodeURIComponent(email)}/verify`, {}),

    setMasterAccessLevel: (email: string, level: number) =>
      post<void>(`/admin/accounts/${encodeURIComponent(email)}/access-level`, { level }),

    searchGameAccounts: (q: string, limit: number) =>
      request<AdminGameAccount[]>(`/admin/game-accounts?q=${encodeURIComponent(q)}&limit=${limit}`),

    /** Creates a game account under the admin's own master, at the admin's
     *  access level — a GM game account. */
    createGameAccount: (login: string, password: string) =>
      post<void>(`/admin/game-accounts`, { login, password }),

    setGameAccountAccessLevel: (login: string, level: number) =>
      post<void>(`/admin/game-accounts/${encodeURIComponent(login)}/access-level`, { level }),

    setGameAccountPassword: (login: string, password: string) =>
      post<void>(`/admin/game-accounts/${encodeURIComponent(login)}/password`, { password }),

    monitor: {
      services: () => request<MonitorServices>("/admin/monitor/services"),

      series: (service: MonitorService, from: number, to: number, maxPoints: number) =>
        request<MonitorSeries>(
          `/admin/monitor/series?service=${service}&from=${from}&to=${to}&maxPoints=${maxPoints}`,
        ),

      clients: () => request<ConnectedClients>("/admin/monitor/clients"),

      /** Closes one connection; 404 when it already left. Recorded in gmaudit. */
      disconnect: (c: ConnectedClient) =>
        post<void>("/admin/monitor/clients/disconnect", {
          service: c.service,
          id: c.id,
          connectedMs: c.connectedMs,
          ip: c.ip,
          account: c.account,
        }),
    },

    /** The login server's IP ban list. Every change is recorded in gmaudit. */
    ipBans: {
      list: () => request<IpBans>("/admin/ip-bans"),

      /** Bans `ip` (replacing a ban already on it); with `disconnect`, also
       *  closes every connection it covers, on both servers. */
      create: (ban: IpBanInput, disconnect: boolean) =>
        post<IpBanCreated>("/admin/ip-bans", { ...ban, disconnect }),

      update: (ip: string, ban: IpBanInput) =>
        put<IpBan>(`/admin/ip-bans/${encodeURIComponent(ip)}`, ban),

      remove: (ip: string) => del<void>(`/admin/ip-bans/${encodeURIComponent(ip)}`),
    },

    logs: {
      streams: () => request<{ streams: LogStreamInfo[] }>("/admin/logs/streams"),

      /** Every call is recorded server-side in the gmaudit log. */
      search: (p: LogSearchParams) => {
        const qs = new URLSearchParams({
          service: p.service,
          stream: p.stream,
          from: String(p.from),
          to: String(p.to),
          q: p.q,
          regex: String(p.regex),
        });
        if (p.level) qs.set("level", p.level);
        if (p.limit) qs.set("limit", String(p.limit));
        if (p.cursor) qs.set("cursor", p.cursor);
        return request<LogSearchResult>(`/admin/logs/search?${qs}`);
      },
    },
  },
};
