/**
 * Production build. Output lands in `dist/`, which `rust-embed` bakes into the
 * dashboard_api binary at compile time — so this must run *before* `cargo build`
 * (see docs/DASHBOARD.md §9).
 */
import { rm } from "node:fs/promises";
import tailwind from "bun-plugin-tailwind";

const outdir = "./dist";

// Sourcemaps are opt-in: `dist/` is embedded into the Rust binary, and the map
// is several times the size of the bundle itself.
const sourcemap = process.env.SOURCEMAP === "1";

// The API is on its own subdomain, so it must be an absolute URL — a relative
// path would resolve against the site's origin and 404. Override for staging or
// a local backend:
//
//   API_BASE_URL=http://localhost:8080/api/v1 bun run build
//
// Whatever is set here must also appear in the API's `AllowedOrigins`, from the
// *site's* side: the browser sends this site's origin, and the API only grants
// credentials to origins it lists.
const apiBaseUrl = process.env.API_BASE_URL ?? "https://api.battlecrab.com/api/v1";

await rm(outdir, { recursive: true, force: true });

const result = await Bun.build({
  entrypoints: ["./index.html"],
  outdir,
  target: "browser",
  minify: true,
  sourcemap: sourcemap ? "linked" : "none",
  // Without this React bundles its development build — bigger, and with dev-only
  // warnings and slower reconciliation.
  define: {
    "process.env.NODE_ENV": JSON.stringify("production"),
    // A bare identifier, not process.env.* — see the comment in src/lib/api.ts.
    __API_BASE__: JSON.stringify(apiBaseUrl),
  },
  // Content-hashed asset names, so everything except index.html can be cached
  // immutably (the cache headers in crates/dashboard_api/src/web.rs rely on it).
  naming: {
    entry: "[dir]/[name].[ext]",
    chunk: "[name]-[hash].[ext]",
    asset: "[name]-[hash].[ext]",
  },
  plugins: [tailwind],
});

if (!result.success) {
  console.error("build failed");
  for (const log of result.logs) console.error(log);
  process.exit(1);
}

// The link-preview card, copied rather than bundled.
//
// It must keep this exact filename: index.html names it in an absolute og:image
// URL, and every platform that has already scraped the site has that URL
// cached. A content-hashed name — which importing it would produce — would
// break all of them on the next rebuild. Regenerate with `bun run og-image`.
const OG_IMAGE = "og-image.jpg";
const ogImage = Bun.file(`./assets/${OG_IMAGE}`);

if (!(await ogImage.exists())) {
  // Failing loudly beats shipping a site whose every shared link renders as a
  // blank card — nobody notices that until someone posts it somewhere public.
  console.error(`build failed: assets/${OG_IMAGE} is missing — run \`bun run og-image\``);
  process.exit(1);
}
await Bun.write(`${outdir}/${OG_IMAGE}`, ogImage);

const total = result.outputs.reduce((sum, output) => sum + output.size, 0);
console.log(`built ${result.outputs.length} files (${(total / 1024).toFixed(1)} KiB) -> ${outdir}`);
console.log(`copied ${OG_IMAGE} (${(ogImage.size / 1024).toFixed(1)} KiB)`);
