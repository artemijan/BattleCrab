/**
 * The admin section's own tabs. The header keeps a single "Admin" entry; the
 * sections under it live here, so the top bar does not grow with every admin
 * feature.
 */
import { Link, useLocation } from "react-router-dom";

import { cx } from "./ui";

const SECTIONS = [
  // Each tab decides "current" itself rather than through NavLink: Accounts
  // owns both the list and the per-account pages, which NavLink's matching
  // cannot express (and NavLink would override `aria-current` with its own).
  {
    to: "/admin",
    label: "Accounts",
    active: (path: string) => path === "/admin" || path.startsWith("/admin/accounts/"),
  },
  {
    to: "/admin/monitor",
    label: "Monitoring",
    active: (path: string) => path.startsWith("/admin/monitor"),
  },
  { to: "/admin/logs", label: "Logs", active: (path: string) => path.startsWith("/admin/logs") },
] as const;

export function AdminNav() {
  const { pathname } = useLocation();
  return (
    <nav aria-label="Admin sections" className="flex gap-1 overflow-x-auto">
      {SECTIONS.map((s) => (
        <Link
          key={s.to}
          to={s.to}
          aria-current={s.active(pathname) ? "page" : undefined}
          className={cx(
            "rounded-xl px-3.5 py-1.5 text-sm font-medium whitespace-nowrap transition-colors",
            s.active(pathname)
              ? "bg-(--surface-strong) text-brand-600 dark:text-brand-200"
              : "text-(--text-muted) hover:bg-(--surface-strong) hover:text-(--text)",
          )}
        >
          {s.label}
        </Link>
      ))}
    </nav>
  );
}
