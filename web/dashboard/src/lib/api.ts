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
  | "registration_disabled"
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

export type Account = {
  login: string;
  email: string | null;
};

export type Character = {
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

const BASE = "/api/v1";

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
  register: (login: string, password: string) =>
    post<Account>("/auth/register", { login, password }),

  login: (login: string, password: string) => post<Account>("/auth/login", { login, password }),

  logout: () => post<void>("/auth/logout", {}),

  me: () => request<Account>("/auth/me"),

  forgotPassword: (email: string) => post<void>("/auth/forgot-password", { email }),

  resetPassword: (token: string, password: string) =>
    post<void>("/auth/reset-password", { token, password }),

  changePassword: (currentPassword: string, newPassword: string) =>
    post<void>("/account/password", { currentPassword, newPassword }),

  changeEmail: (email: string) => post<void>("/account/email", { email }),

  characters: () => request<Character[]>("/account/characters"),

  status: () => request<ServerStatus>("/server/status"),
};
