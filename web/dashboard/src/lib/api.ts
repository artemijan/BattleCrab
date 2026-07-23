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
  | "invalid_token"
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

export type AdminAccountList = {
  total: number;
  accounts: AdminMasterSummary[];
};

export type AdminAccountDetail = {
  master: AdminMasterSummary;
  gameAccounts: AdminGameAccount[];
  characters: Character[];
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

const post = <T,>(path: string, body: unknown) =>
  request<T>(path, { method: "POST", body: JSON.stringify(body) });

export const api = {
  register: (email: string, password: string) =>
    post<Account>("/auth/register", { email, password }),

  login: (email: string, password: string) => post<Account>("/auth/login", { email, password }),

  resendVerification: () => post<void>("/auth/resend-verification", {}),

  logout: () => post<void>("/auth/logout", {}),

  me: () => request<Account>("/auth/me"),

  forgotPassword: (email: string) => post<void>("/auth/forgot-password", { email }),

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

  status: () => request<ServerStatus>("/server/status"),

  /**
   * The `/admin` surface. Every call 403s for a non-admin session — the
   * `isAdmin` flag on `Account` only decides whether the UI offers these.
   *
   * Access levels only ever go to 0 (restore) or negative (ban) from here; the
   * server refuses anything positive, so there is deliberately no "promote".
   */
  admin: {
    accounts: (q: string, offset: number, limit: number) =>
      request<AdminAccountList>(
        `/admin/accounts?q=${encodeURIComponent(q)}&offset=${offset}&limit=${limit}`,
      ),

    account: (email: string) =>
      request<AdminAccountDetail>(`/admin/accounts/${encodeURIComponent(email)}`),

    verifyMaster: (email: string) =>
      post<void>(`/admin/accounts/${encodeURIComponent(email)}/verify`, {}),

    setMasterAccessLevel: (email: string, level: number) =>
      post<void>(`/admin/accounts/${encodeURIComponent(email)}/access-level`, { level }),

    searchGameAccounts: (q: string, limit: number) =>
      request<AdminGameAccount[]>(
        `/admin/game-accounts?q=${encodeURIComponent(q)}&limit=${limit}`,
      ),

    setGameAccountAccessLevel: (login: string, level: number) =>
      post<void>(`/admin/game-accounts/${encodeURIComponent(login)}/access-level`, { level }),

    setGameAccountPassword: (login: string, password: string) =>
      post<void>(`/admin/game-accounts/${encodeURIComponent(login)}/password`, { password }),
  },
};
