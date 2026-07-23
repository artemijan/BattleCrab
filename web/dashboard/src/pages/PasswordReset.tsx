import { useMutation } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { Link, useSearchParams } from "react-router-dom";

import { ApiError, api } from "../lib/api";
import { Alert, Button, Field } from "../components/ui";
import { AuthShell, messageFor } from "./Auth";

/**
 * Request a reset link.
 *
 * The API answers 202 whether or not the address exists, so this page must not
 * imply otherwise — saying "we sent it" only when the account is real would
 * turn the form into an account-existence oracle and undo the server's care.
 */
export function ForgotPassword() {
  const [email, setEmail] = useState("");

  const submit = useMutation({
    mutationFn: () => api.forgotPassword(email),
  });

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    submit.mutate();
  };

  return (
    <AuthShell
      title="Reset your password"
      subtitle="We'll email you a link to choose a new one."
      footer={
        <>
          Remembered it?{" "}
          <Link to="/login" className="font-semibold text-brand-600 dark:text-brand-300">
            Log in
          </Link>
        </>
      }
    >
      {submit.isSuccess ? (
        <Alert kind="success">
          If an account uses that address, a reset link is on its way. The link is valid for one
          hour.
        </Alert>
      ) : (
        <form onSubmit={onSubmit} className="flex flex-col gap-3.5">
          {submit.isError && <Alert kind="error">{messageFor(submit.error)}</Alert>}

          <Field
            label="Email address"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            hint="The address on your account. If you never set one, contact an admin."
            autoComplete="email"
            autoFocus
            required
          />
          <Button type="submit" loading={submit.isPending} className="mt-1 w-full py-3">
            Send reset link
          </Button>
        </form>
      )}
    </AuthShell>
  );
}

/**
 * Consume a reset link: `/reset-password?token=...`.
 *
 * The token is single-use server-side — it is signed over the account's current
 * password hash, so it stops verifying the moment the password changes. That
 * means a successful reset also invalidates every existing session, which is
 * why this page sends the user to the login form rather than assuming one.
 */
export function ResetPassword() {
  const [params] = useSearchParams();
  const token = params.get("token") ?? "";

  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const mismatch = confirm.length > 0 && confirm !== password;

  const submit = useMutation({
    mutationFn: () => api.resetPassword(token, password),
  });

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (mismatch) return;
    submit.mutate();
  };

  const footer = (
    <>
      Back to{" "}
      <Link to="/login" className="font-semibold text-brand-600 dark:text-brand-300">
        log in
      </Link>
    </>
  );

  // A link pasted without its query string is a common enough mistake to be
  // worth its own message rather than a generic failure after submitting.
  if (!token) {
    return (
      <AuthShell title="Reset your password" subtitle="This link is incomplete." footer={footer}>
        <Alert kind="error">
          The reset link is missing its token. Copy the whole link from the email, or{" "}
          <Link to="/forgot-password" className="font-semibold underline">
            request a new one
          </Link>
          .
        </Alert>
      </AuthShell>
    );
  }

  if (submit.isSuccess) {
    return (
      <AuthShell title="Password updated" subtitle="You can log in with it now." footer={footer}>
        <Alert kind="success">
          Your password has been changed — in the game client too. Any other devices already signed
          in have been signed out.
        </Alert>
      </AuthShell>
    );
  }

  return (
    <AuthShell
      title="Choose a new password"
      subtitle="This also changes the password you use in the game client."
      footer={footer}
    >
      <form onSubmit={onSubmit} className="flex flex-col gap-3.5">
        {submit.isError && (
          <Alert kind="error">
            {submit.error instanceof ApiError && submit.error.code === "invalid_token" ? (
              <>
                This link has expired or has already been used.{" "}
                <Link to="/forgot-password" className="font-semibold underline">
                  Request a new one
                </Link>
                .
              </>
            ) : (
              messageFor(submit.error)
            )}
          </Alert>
        )}

        <Field
          label="New password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          hint="At least 8 characters."
          autoComplete="new-password"
          autoFocus
          required
        />
        <Field
          label="Confirm password"
          type="password"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          error={mismatch ? "Passwords don't match." : undefined}
          autoComplete="new-password"
          required
        />
        <Button
          type="submit"
          loading={submit.isPending}
          disabled={mismatch}
          className="mt-1 w-full py-3"
        >
          Set new password
        </Button>
      </form>
    </AuthShell>
  );
}
