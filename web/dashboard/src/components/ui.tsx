import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode } from "react";
import { forwardRef, useId } from "react";

export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

/* -------------------------------------------------------------------------- */
/* Panel                                                                      */
/* -------------------------------------------------------------------------- */

export function Panel({
  children,
  className,
  strong = false,
}: {
  children: ReactNode;
  className?: string;
  strong?: boolean;
}) {
  return (
    <div className={cx(strong ? "glass-strong" : "glass", "glass-sheen rounded-2xl", className)}>
      {children}
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* Button                                                                     */
/* -------------------------------------------------------------------------- */

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost";
  loading?: boolean;
};

export function Button({
  variant = "primary",
  loading = false,
  className,
  children,
  disabled,
  ...rest
}: ButtonProps) {
  const base =
    "relative inline-flex items-center justify-center gap-2 overflow-hidden rounded-xl px-5 py-2.5 " +
    // v4 compiles -translate-y/scale to the `translate`/`scale` properties,
    // so both must be named here or the hover lift will not animate.
    "text-sm font-semibold duration-200 " +
    "transition-[translate,scale,box-shadow,background-color,opacity] " +
    "disabled:cursor-not-allowed disabled:opacity-55 " +
    "not-disabled:hover:-translate-y-0.5 not-disabled:active:translate-y-0 not-disabled:active:scale-[0.99]";

  // Yellow is the accent, so it marks the single primary action on a view and
  // nothing else. Dark navy text on yellow clears contrast comfortably; white
  // would not.
  const variants = {
    primary:
      "bg-accent-400 text-brand-950 shadow-[0_10px_30px_-10px_rgba(255,215,0,0.7)] " +
      "not-disabled:hover:bg-accent-300 not-disabled:hover:shadow-[0_14px_36px_-10px_rgba(255,215,0,0.85)]",
    secondary:
      "bg-brand-500 text-white shadow-[0_10px_30px_-12px_rgba(0,87,183,0.9)] " +
      "not-disabled:hover:bg-brand-400",
    ghost:
      "border border-[var(--surface-border)] bg-[var(--surface)] text-[var(--text)] " +
      "backdrop-blur-md not-disabled:hover:bg-[var(--surface-strong)]",
  } as const;

  return (
    <button
      className={cx(base, variants[variant], className)}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading && <Spinner />}
      <span className={cx(loading && "opacity-90")}>{children}</span>

      {/* Light sweep on hover. Purely decorative, so it's hidden from AT and
          disabled wholesale by the reduced-motion rule in globals.css. */}
      {variant !== "ghost" && (
        <span
          aria-hidden
          className="pointer-events-none absolute inset-0 opacity-0 transition-opacity duration-300 hover:opacity-100"
        >
          <span className="absolute inset-y-0 -left-1/3 w-1/3 skew-x-12 bg-white/30 blur-md animate-[sweep_1.1s_var(--ease-out-soft)]" />
        </span>
      )}
    </button>
  );
}

export function Spinner({ className }: { className?: string }) {
  return (
    <span
      aria-hidden
      className={cx(
        "inline-block size-4 shrink-0 rounded-full border-2 border-current border-t-transparent",
        "animate-[spin_0.7s_linear_infinite]",
        className,
      )}
    />
  );
}

/* -------------------------------------------------------------------------- */
/* Field                                                                      */
/* -------------------------------------------------------------------------- */

type FieldProps = InputHTMLAttributes<HTMLInputElement> & {
  label: string;
  hint?: string;
  error?: string;
};

export const Field = forwardRef<HTMLInputElement, FieldProps>(function Field(
  { label, hint, error, className, id, ...rest },
  ref,
) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const describedBy = error ? `${inputId}-error` : hint ? `${inputId}-hint` : undefined;

  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={inputId} className="text-sm font-medium text-(--text-muted)">
        {label}
      </label>
      <input
        ref={ref}
        id={inputId}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
        className={cx(
          // 16px on phones, and not for looks: iOS Safari zooms the viewport
          // whenever a focused control computes below 16px, and it does not
          // zoom back out on blur. At 14px, tapping this field zoomed the page
          // in, left it scrolling sideways, and kept it that way after signing
          // in. `sm:text-sm` restores the smaller size from 640px up, where no
          // such behaviour exists.
          //
          // The usual shortcut — user-scalable=no or maximum-scale=1 in the
          // viewport meta — also stops the zoom, by removing pinch-to-zoom from
          // everyone, including people who need it to read at all. Sizing the
          // control correctly costs nothing.
          "w-full rounded-xl border px-4 py-2.5 text-base outline-none sm:text-sm",
          "bg-(--field-bg) text-(--text) backdrop-blur-md",
          "placeholder:text-(--text-faint)",
          "transition-[border-color,box-shadow] duration-200",
          error
            ? "border-red-400/70 focus:shadow-[0_0_0_3px_rgba(248,113,113,0.22)]"
            : "border-[var(--field-border)] focus:border-brand-400 focus:shadow-[0_0_0_3px_var(--surface-ring)]",
          className,
        )}
        {...rest}
      />
      {error ? (
        <p id={`${inputId}-error`} role="alert" className="text-xs text-red-500 dark:text-red-400">
          {error}
        </p>
      ) : hint ? (
        <p id={`${inputId}-hint`} className="text-xs text-(--text-faint)">
          {hint}
        </p>
      ) : null}
    </div>
  );
});

/* -------------------------------------------------------------------------- */
/* Alert                                                                      */
/* -------------------------------------------------------------------------- */

export function Alert({ kind, children }: { kind: "error" | "success"; children: ReactNode }) {
  const styles =
    kind === "error"
      ? "border-red-400/40 bg-red-500/10 text-red-600 dark:text-red-300"
      : "border-emerald-400/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300";

  return (
    <div
      role={kind === "error" ? "alert" : "status"}
      className={cx("animate-rise rounded-xl border px-4 py-3 text-sm backdrop-blur-md", styles)}
    >
      {children}
    </div>
  );
}
