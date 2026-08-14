import { Turnstile, type TurnstileInstance } from "@marsidev/react-turnstile";
import { useImperativeHandle, useRef, type Ref } from "react";

import { useTheme } from "../lib/theme";

/**
 * Substituted at build time from the `TURNSTILE_SITE_KEY` env var — same
 * bare-identifier mechanism as `__API_BASE__` (see the long comment in
 * `lib/api.ts` for why it must not be `process.env.X`).
 *
 * The fallback is Cloudflare's public always-passes test key, which pairs with
 * the backend's captcha-disabled dev mode: `bun run dev` applies no defines,
 * the widget renders and immediately succeeds, and the token it produces is
 * never actually checked.
 */
declare const __TURNSTILE_SITE_KEY__: string | undefined;

const SITE_KEY =
  (typeof __TURNSTILE_SITE_KEY__ !== "undefined" && __TURNSTILE_SITE_KEY__) ||
  "1x00000000000000000000AA";

export type CaptchaHandle = {
  /** Discards the current token and asks Turnstile for a fresh challenge. */
  reset: () => void;
};

/**
 * The Turnstile widget, reduced to one concern: keep the parent told about the
 * current token (`null` whenever there isn't a valid one).
 *
 * Tokens are single-use server-side, so any failed submit must `reset()` via
 * the handle before the user can sensibly retry — a kept token would only fail
 * again as `timeout-or-duplicate`.
 */
export function Captcha({
  ref,
  onToken,
  interactionOnly = false,
}: {
  ref?: Ref<CaptchaHandle>;
  onToken: (token: string | null) => void;
  /**
   * Keep the widget collapsed while it solves invisibly, expanding only when
   * Cloudflare actually demands a click. For pages with a height budget (the
   * register form has to fit an 800px viewport) — the trade-off is a layout
   * shift for the minority of visitors who do get challenged.
   */
  interactionOnly?: boolean;
}) {
  const widget = useRef<TurnstileInstance | null>(null);
  const { theme } = useTheme();

  useImperativeHandle(ref, () => ({
    reset: () => {
      widget.current?.reset();
      onToken(null);
    },
  }));

  return (
    <Turnstile
      ref={widget}
      siteKey={SITE_KEY}
      // `flexible` stretches the 65px-tall widget to the form's width instead
      // of leaving a fixed 300px box floating in the panel.
      options={{
        theme,
        size: "flexible",
        appearance: interactionOnly ? "interaction-only" : "always",
      }}
      onSuccess={(token) => onToken(token)}
      onExpire={() => onToken(null)}
      onError={() => onToken(null)}
    />
  );
}
