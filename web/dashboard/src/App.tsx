import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { BrowserRouter, Navigate, Route, Routes, useLocation } from "react-router-dom";

import { Header, Page } from "./components/Layout";
import { Spinner } from "./components/ui";
import { ApiError, api } from "./lib/api";
import { ThemeProvider } from "./lib/theme";
import { AccountPage } from "./pages/AccountPage";
import { Login, Register } from "./pages/Auth";
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

/** Resolves the current session. `null` (not an error) when logged out. */
function useAccount() {
  return useQuery({
    queryKey: ["me"],
    queryFn: async () => {
      try {
        return await api.me();
      } catch (error) {
        if (error instanceof ApiError && error.status === 401) return null;
        throw error;
      }
    },
  });
}

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
    <>
      <Header account={account.data} />
      <Page>
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
    </>
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
