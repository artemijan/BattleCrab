import { useTheme } from "../lib/theme";

/**
 * Sun/moon toggle. The two icons cross-fade and rotate rather than swapping,
 * which reads as one control changing state instead of two different buttons.
 */
export function ThemeToggle() {
  const { theme, toggle } = useTheme();
  const isDark = theme === "dark";

  return (
    <button
      type="button"
      onClick={toggle}
      aria-label={isDark ? "Switch to light theme" : "Switch to dark theme"}
      aria-pressed={isDark}
      title={isDark ? "Light theme" : "Dark theme"}
      className="glass glass-sheen relative grid size-9 shrink-0 place-items-center rounded-xl
                 transition-transform duration-200 hover:-translate-y-0.5 active:scale-95 sm:size-10"
    >
      <span className="relative block size-5">
        <svg
          aria-hidden
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          className="absolute inset-0 size-5 text-accent-500 transition-all duration-400
                     [transition-timing-function:var(--ease-spring)]"
          style={{
            opacity: isDark ? 0 : 1,
            transform: isDark ? "rotate(-90deg) scale(0.4)" : "rotate(0) scale(1)",
          }}
        >
          <circle cx="12" cy="12" r="4.2" />
          <path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.1 5.1l1.4 1.4M17.5 17.5l1.4 1.4M18.9 5.1l-1.4 1.4M6.5 17.5l-1.4 1.4" />
        </svg>

        <svg
          aria-hidden
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="absolute inset-0 size-5 text-accent-300 transition-all duration-400
                     [transition-timing-function:var(--ease-spring)]"
          style={{
            opacity: isDark ? 1 : 0,
            transform: isDark ? "rotate(0) scale(1)" : "rotate(90deg) scale(0.4)",
          }}
        >
          <path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5Z" />
        </svg>
      </span>
    </button>
  );
}
