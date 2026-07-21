/**
 * Production build. Output lands in `dist/`, which `rust-embed` bakes into the
 * dashboard_api binary at compile time — so this must run *before* `cargo build`
 * (see docs/PLAN_DASHBOARD.md §9).
 */
import { rm } from "node:fs/promises";
import tailwind from "bun-plugin-tailwind";

const outdir = "./dist";

// Sourcemaps are opt-in: `dist/` is embedded into the Rust binary, and the map
// is several times the size of the bundle itself.
const sourcemap = process.env.SOURCEMAP === "1";

await rm(outdir, { recursive: true, force: true });

const result = await Bun.build({
  entrypoints: ["./index.html"],
  outdir,
  target: "browser",
  minify: true,
  sourcemap: sourcemap ? "linked" : "none",
  // Without this React bundles its development build — bigger, and with dev-only
  // warnings and slower reconciliation.
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
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

const total = result.outputs.reduce((sum, output) => sum + output.size, 0);
console.log(
  `built ${result.outputs.length} files (${(total / 1024).toFixed(1)} KiB) -> ${outdir}`,
);
