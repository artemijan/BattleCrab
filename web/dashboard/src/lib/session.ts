/**
 * The current session, shared by everything that needs to know whether someone
 * is signed in.
 *
 * It lives here rather than in App.tsx so pages can read it without importing
 * from App, which imports them back — a cycle that resolves to `undefined` at
 * module-init time rather than failing loudly.
 *
 * One query key (`["me"]`) across the whole app, so the routing guard, the
 * header and the landing page all read one cached answer instead of each firing
 * their own `/auth/me`.
 */
import { useQuery } from "@tanstack/react-query";

import { ApiError, api, type Account } from "./api";

/** Resolves the current session. `null` (not an error) when logged out. */
export function useAccount() {
  return useQuery<Account | null>({
    queryKey: ["me"],
    queryFn: async () => {
      try {
        return await api.me();
      } catch (error) {
        // Logged out is the expected answer here, not a failure — surfacing it
        // as an error would make every caller special-case a 401.
        if (error instanceof ApiError && error.status === 401) return null;
        throw error;
      }
    },
  });
}
