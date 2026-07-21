import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";

import { ApiError, api, type Account } from "../lib/api";
import { Alert, Button, Field, Panel } from "../components/ui";

export function AuthShell({
  title,
  subtitle,
  children,
  footer,
}: {
  title: string;
  subtitle: string;
  children: React.ReactNode;
  footer: React.ReactNode;
}) {
  return (
    // Vertical rhythm stays tight so the taller form (register, three fields)
    // still fits an 800px-high laptop viewport without scrolling.
    <div className="mx-auto max-w-md py-4 sm:py-6">
      <Panel strong className="animate-rise p-6 sm:p-7">
        <h1 className="text-2xl font-bold tracking-tight">{title}</h1>
        <p className="mt-1.5 text-sm text-[var(--text-muted)]">{subtitle}</p>
        <div className="mt-5">{children}</div>
      </Panel>
      <p className="mt-4 text-center text-sm text-[var(--text-muted)]">{footer}</p>
    </div>
  );
}

/** Turns an ApiError into something worth showing a player. */
export function messageFor(error: unknown): string {
  if (error instanceof ApiError) {
    switch (error.code) {
      case "invalid_credentials":
        return "That email and password don't match.";
      case "login_taken":
        return "That username is already taken.";
      case "email_taken":
        return "An account already exists for that email address.";
      case "rate_limited":
        return "Too many attempts. Wait a few minutes and try again.";
      case "registration_disabled":
        return "Registration is currently closed.";
      default:
        return error.message;
    }
  }
  return "Something went wrong. Try again.";
}

export function Login() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");

  const submit = useMutation({
    mutationFn: () => api.login(email, password),
    onSuccess: (account: Account) => {
      queryClient.setQueryData(["me"], account);
      navigate("/account");
    },
  });

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    submit.mutate();
  };

  return (
    <AuthShell
      title="Welcome back"
      subtitle="Log in with your email to manage your account and characters."
      footer={
        <>
          No account yet?{" "}
          <Link to="/register" className="font-semibold text-brand-600 dark:text-brand-300">
            Create one
          </Link>
        </>
      }
    >
      <form onSubmit={onSubmit} className="flex flex-col gap-3.5">
        {submit.isError && <Alert kind="error">{messageFor(submit.error)}</Alert>}

        {/* type="email" gets the right mobile keyboard and lets the browser
            catch a malformed address before the request is made. */}
        <Field
          label="Email"
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          autoComplete="email"
          autoFocus
          required
        />
        <Field
          label="Password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="current-password"
          required
        />
        {/* Without this the reset flow is unreachable: nothing else in the UI
            can trigger a reset email. */}
        <Link
          to="/forgot-password"
          className="-mt-1 self-end text-xs font-medium text-[var(--text-muted)]
                     transition-colors hover:text-brand-600 dark:hover:text-brand-300"
        >
          Forgot your password?
        </Link>
        <Button type="submit" loading={submit.isPending} className="mt-1 w-full py-3">
          Log in
        </Button>
      </form>
    </AuthShell>
  );
}

export function Register() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");

  // Checked here purely for a fast, friendly message — the server enforces the
  // real rules, and these must not drift from routes::validate_* .
  const mismatch = confirm.length > 0 && confirm !== password;

  const submit = useMutation({
    mutationFn: () => api.register(email, password),
    onSuccess: (account: Account) => {
      queryClient.setQueryData(["me"], account);
      navigate("/account");
    },
  });

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (mismatch) return;
    submit.mutate();
  };

  return (
    <AuthShell
      title="Create your account"
      subtitle="Sign up with your email. You'll create game logins afterwards."
      footer={
        <>
          Already registered?{" "}
          <Link to="/login" className="font-semibold text-brand-600 dark:text-brand-300">
            Log in
          </Link>
        </>
      }
    >
      <form onSubmit={onSubmit} className="flex flex-col gap-3.5">
        {submit.isError && <Alert kind="error">{messageFor(submit.error)}</Alert>}

        <Field
          label="Email"
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          // Kept to one line at the panel's width — a wrapping hint costs 16px
          // of height on the page that least affords it.
          hint="We'll send a confirmation link here."
          autoComplete="email"
          autoFocus
          required
        />
        <Field
          label="Password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          hint="At least 8 characters."
          autoComplete="new-password"
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
          Create account
        </Button>
      </form>
    </AuthShell>
  );
}
