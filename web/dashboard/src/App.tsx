import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { BrowserRouter, Navigate, Route, Routes, useLocation } from "react-router-dom";

import { Page } from "./components/Layout";
import { Spinner } from "./components/ui";
import { ApiError } from "./lib/api";
import { useAccount } from "./lib/session";
import { ThemeProvider } from "./lib/theme";
import { AccountPage } from "./pages/AccountPage";
import { Login, Register } from "./pages/Auth";
import { ForgotPassword, ResetPassword } from "./pages/PasswordReset";
import { VerifyEmail } from "./pages/VerifyEmail";
import { Landing } from "./pages/Landing";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      // A 401 means the session is gone; retrying just delays the redirect.
      retry: (failureCount, error) =>
        !(error instanceof ApiError && error.status === 401) && failureCount < 2,
    },
  },
});

function RequireAuth({ children }: { children: ReactNode }) {
  const account = useAccount();
  const location = useLocation();

  if (account.isPending) {
    return (
      <div className="grid place-items-center py-24 text-[var(--text-muted)]">
        <Spinner className="size-6" />
      </div>
    );
  }
  if (!account.data) {
    return <Navigate to="/login" replace state={{ from: location.pathname }} />;
  }
  return <>{children}</>;
}

/** Keeps logged-in users off the login/register pages. */
function RedirectIfAuthed({ children }: { children: ReactNode }) {
  const account = useAccount();
  if (account.data) return <Navigate to="/account" replace />;
  return <>{children}</>;
}

function Shell() {
  const account = useAccount();

  return (
    <Page account={account.data}>
      <Routes>
        <Route path="/" element={<Landing />} />
        <Route
          path="/login"
          element={
            <RedirectIfAuthed>
              <Login />
            </RedirectIfAuthed>
          }
        />
        <Route
          path="/register"
          element={
            <RedirectIfAuthed>
              <Register />
            </RedirectIfAuthed>
          }
        />
        {/* Token-bearing links from email. Deliberately NOT wrapped in
            RedirectIfAuthed: they are opened from an inbox, sometimes while
            already signed in, and bouncing to /account would swallow the
            token. */}
        <Route path="/forgot-password" element={<ForgotPassword />} />
        <Route path="/reset-password" element={<ResetPassword />} />
        <Route path="/verify-email" element={<VerifyEmail />} />
        <Route
          path="/account"
          element={
            <RequireAuth>
              <AccountPage />
            </RequireAuth>
          }
        />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Page>
  );
}

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        <BrowserRouter>
          <Shell />
        </BrowserRouter>
      </ThemeProvider>
    </QueryClientProvider>
  );
}
