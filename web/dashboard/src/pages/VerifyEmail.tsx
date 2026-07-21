import { useQuery } from "@tanstack/react-query";
import { Link, useSearchParams } from "react-router-dom";

import { ApiError, api } from "../lib/api";
import { Alert, Spinner } from "../components/ui";
import { AuthShell } from "./Auth";

/**
 * Consume an email-verification link: `/verify-email?token=...`.
 *
 * Clicking the link is what actually writes `accounts.email` — the address is
 * never stored before it is proven, which is how "an address on the account"
 * can mean "a verified address" without a separate column.
 *
 * Deliberately works logged out: the link is usually opened from a mail client,
 * often in a different browser from the one that requested the change.
 */
export function VerifyEmail() {
  const [params] = useSearchParams();
  const token = params.get("token") ?? "";

  const verify = useQuery({
    queryKey: ["verify-email", token],
    queryFn: async () => {
      await api.verifyEmail(token);
      // Must return something. The endpoint answers 204, so the client resolves
      // to `undefined` — which TanStack Query treats as a failed query, and the
      // page would report an error for a verification that actually succeeded.
      return true as const;
    },
    enabled: token.length > 0,
    // Fire exactly once. A bad token never becomes good, and a refetch would
    // spend a token that already succeeded — the second call always fails,
    // turning a success into an error on screen.
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    refetchOnMount: false,
    staleTime: Infinity,
    gcTime: 0,
  });

  const footer = (
    <>
      Go to{" "}
      <Link to="/account" className="font-semibold text-brand-600 dark:text-brand-300">
        your account
      </Link>
    </>
  );

  if (!token) {
    return (
      <AuthShell title="Confirm your email" subtitle="This link is incomplete." footer={footer}>
        <Alert kind="error">
          The link is missing its token. Copy the whole link from the email, or request a new one
          from your account page.
        </Alert>
      </AuthShell>
    );
  }

  return (
    <AuthShell
      title="Confirm your email"
      subtitle={verify.isPending ? "Checking your link…" : "Email verification"}
      footer={footer}
    >
      {verify.isPending && (
        <p className="flex items-center gap-3 text-sm text-[var(--text-muted)]">
          <Spinner /> Confirming…
        </p>
      )}

      {verify.isSuccess && (
        <Alert kind="success">
          Your email address is confirmed. You can use it to reset your password if you ever lose
          it.
        </Alert>
      )}

      {verify.isError && (
        <Alert kind="error">
          {verify.error instanceof ApiError && verify.error.code === "invalid_token"
            ? "This link has expired or has already been used. Request a new one from your account page."
            : "Something went wrong confirming your address. Try the link again in a moment."}
        </Alert>
      )}
    </AuthShell>
  );
}
