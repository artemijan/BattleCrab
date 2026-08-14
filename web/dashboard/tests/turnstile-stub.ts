import type { Page } from "playwright";

/**
 * Replaces Cloudflare's Turnstile loader with a local stand-in.
 *
 * Every test that opens /register or /forgot-password needs this: those pages
 * always render the widget, and without the stub headless Chrome would fetch
 * `challenges.cloudflare.com/turnstile/v0/api.js` from the real network — a
 * flake in CI, and a submit button that stays disabled whenever the fetch
 * doesn't come back.
 *
 * The stand-in mirrors the contract `@marsidev/react-turnstile` relies on: the
 * loader script defines `window.turnstile`, then calls the `?onload=` callback
 * the library registered on `window`. `render` reports success on the next
 * tick, so a submit gated on the token becomes clickable just like it does
 * against the real widget with a test key.
 */
export const STUB_TOKEN = "e2e-turnstile-token";

export async function stubTurnstile(page: Page): Promise<void> {
  await page.route("**/challenges.cloudflare.com/**", (route) =>
    route.fulfill({
      contentType: "text/javascript",
      body: `
        window.turnstile = {
          render(el, params) {
            setTimeout(() => params.callback(${JSON.stringify(STUB_TOKEN)}), 0);
            return "stub-widget";
          },
          reset() {},
          remove() {},
          execute() {},
          getResponse() { return ${JSON.stringify(STUB_TOKEN)}; },
          isExpired() { return false; },
        };
        const onload = new URL(document.currentScript.src).searchParams.get("onload");
        if (onload && typeof window[onload] === "function") window[onload]();
      `,
    }),
  );
}
