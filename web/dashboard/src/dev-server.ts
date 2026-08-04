/**
 * Dev server with hot reload.
 *
 * Proxies `/api/*` to the Rust backend so the browser sees one origin — which
 * is what lets the session cookie work locally exactly as it does in production
 * (docs/DASHBOARD.md §9).
 *
 *   Terminal 1:  cargo run -p dashboard_api
 *   Terminal 2:  bun run dev
 */
import index from "../index.html";

const API_TARGET = process.env.DASHBOARD_API ?? "http://127.0.0.1:8080";
const PORT = Number(process.env.PORT ?? 3000);

const server = Bun.serve({
  port: PORT,
  development: true,

  routes: {
    "/api/*": async (request) => {
      const url = new URL(request.url);
      const target = new URL(url.pathname + url.search, API_TARGET);
      try {
        const upstream = await fetch(target, {
          method: request.method,
          headers: request.headers,
          body: request.body,
          // Bun requires this when forwarding a streaming body.
          // @ts-expect-error -- duplex is valid at runtime, missing from types
          duplex: "half",
          redirect: "manual",
        });
        // fetch() already decompressed the body but keeps the upstream
        // Content-Encoding header. Forwarding both makes the browser gunzip
        // plaintext → ERR_CONTENT_DECODING_FAILED. Drop the encoding headers
        // (and the stale length) so the response describes what is actually
        // being sent.
        const headers = new Headers(upstream.headers);
        headers.delete("content-encoding");
        headers.delete("content-length");
        return new Response(upstream.body, {
          status: upstream.status,
          statusText: upstream.statusText,
          headers,
        });
      } catch {
        return Response.json(
          {
            error: {
              code: "internal",
              message: `dashboard_api unreachable at ${API_TARGET} — is \`cargo run -p dashboard_api\` running?`,
            },
          },
          { status: 502 },
        );
      }
    },

    // Everything else is the SPA; Bun handles HMR for it.
    "/*": index,
  },
});

console.log(`dashboard dev server  →  ${server.url}`);
console.log(`proxying /api/*       →  ${API_TARGET}`);
