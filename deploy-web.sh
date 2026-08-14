#!/usr/bin/env bash
# Builds the dashboard SPA and deploys it to Cloudflare Workers with wrangler.
#
# Deploys as an "assets-only" Worker (no server code): wrangler uploads dist/
# and Cloudflare serves it. In the dashboard this is Workers & Pages ->
# battlecrab.
#
# This is the FRONTEND half of the deployment. The API is deployed separately by
# deploy-dashboard.sh, to the game server box. They are independent: the SPA is
# static and served by Cloudflare, and it calls the API cross-origin at
# $API_BASE_URL.
#
# This script is committed and holds no secrets: the Cloudflare API token and
# account ID come from deploy.env (gitignored — see deploy.env.example) or the
# environment.
#
# Requires Bun (https://bun.sh). Wrangler is run through `bun x`, so it needs no
# global install.
#
#   ./deploy-web.sh              build and deploy
#   DRY_RUN=1 ./deploy-web.sh    build and prepare, stop before uploading

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Shares deploy.env with deploy-dashboard.sh — that is where TURNSTILE_SITE_KEY
# lives, and the two halves MUST agree: this script bakes the site key into the
# SPA, deploy-dashboard.sh gives the API the matching secret. Building the site
# without it ships Cloudflare's always-passing TEST key, whose tokens the real
# secret then rejects — every register/reset 403s as captcha_failed.
ENV_FILE="${1:-$SCRIPT_DIR/deploy.env}"
if [[ -f "$ENV_FILE" ]]; then
    # shellcheck disable=SC1090
    source "$ENV_FILE"
fi

# --- Cloudflare credentials ---------------------------------------------------
# Create a token at https://dash.cloudflare.com/profile/api-tokens ->
# "Create Custom Token", then pick the **Edit Cloudflare Workers** template.
# That is what Cloudflare's own CI guide specifies for `wrangler deploy`.
#
# It grants (account-scoped unless noted):
#   Workers Scripts           Edit    upload the Worker + its static assets
#   Workers KV Storage        Edit    wrangler stages the asset manifest
#   Workers R2 Storage        Edit    included by the template; unused here
#   Workers Tail              Read    `wrangler tail` live logs
#   Account Settings          Read    resolve the account
#   User -> Memberships       Read    identify the calling user
#   Zone -> Workers Routes    Edit    needed only for a custom domain/route
#
# Scope it to this one account (and, for the zone part, just battlecrab.com).
# If you never attach a custom domain from the CLI, the zone permission can be
# dropped — the domain can be attached once in the dashboard instead.
#
# Set both in deploy.env (or export them in your shell before running).
CLOUDFLARE_API_TOKEN="${CLOUDFLARE_API_TOKEN:-}"

# Account ID is on the right-hand side of any Cloudflare dashboard page.
CLOUDFLARE_ACCOUNT_ID="${CLOUDFLARE_ACCOUNT_ID:-}"

# The Worker name — must match the service in the dashboard.
CF_WORKER_NAME="${CF_WORKER_NAME:-battlecrab}"

# Pins Workers runtime behaviour. Must be >= 2025-04-01 for navigation requests
# to be served from static assets without invoking a Worker script.
CF_COMPATIBILITY_DATE="${CF_COMPATIBILITY_DATE:-2025-04-01}"

# --- Build settings -----------------------------------------------------------
# Baked into the bundle at build time. Must be absolute: the API lives on a
# different origin from the site, so a relative path would resolve to the
# Cloudflare host and 404.
#
# Whatever host this points at must list this site's origin in the API's
# AllowedOrigins, or the browser will block every call.
API_BASE_URL="${API_BASE_URL:-https://api.battlecrab.com/api/v1}"

# Turnstile *site* key (public), baked into the bundle like API_BASE_URL. Must
# be the widget whose SECRET the API was deployed with (deploy-dashboard.sh).
# Left empty, the build falls back to Cloudflare's always-passing TEST key —
# fine only while the API has the captcha disabled too.
TURNSTILE_SITE_KEY="${TURNSTILE_SITE_KEY:-}"

export CLOUDFLARE_API_TOKEN CLOUDFLARE_ACCOUNT_ID

WRANGLER=(bun x wrangler)
DIST="web/dashboard/dist"

echo "==> Cloudflare Worker: $CF_WORKER_NAME"
echo "==> API base baked into the bundle: $API_BASE_URL"

# --- Preconditions ------------------------------------------------------------
if ! command -v bun >/dev/null 2>&1; then
    echo "error: bun not found. Install from https://bun.sh" >&2
    exit 1
fi

if [[ -z "$CLOUDFLARE_API_TOKEN" ]]; then
    echo "error: CLOUDFLARE_API_TOKEN is not set. Put it in $ENV_FILE (or export it)," >&2
    echo "       created from the 'Edit Cloudflare Workers' template." >&2
    exit 1
fi
if [[ -z "$CLOUDFLARE_ACCOUNT_ID" ]]; then
    echo "error: CLOUDFLARE_ACCOUNT_ID is not set. Put it in $ENV_FILE (find it on any" >&2
    echo "       Cloudflare dashboard page)." >&2
    exit 1
fi

# --- Build --------------------------------------------------------------------
echo "==> Building SPA"
(
    cd web/dashboard
    bun install --frozen-lockfile
    API_BASE_URL="$API_BASE_URL" TURNSTILE_SITE_KEY="$TURNSTILE_SITE_KEY" bun run build
)

if [[ ! -f "$DIST/index.html" ]]; then
    echo "error: $DIST/index.html missing after build" >&2
    exit 1
fi
# The substitution failing is silent and nasty: the bundle looks fine but every
# request goes to the Cloudflare origin instead of the API.
if ! grep -qr "$API_BASE_URL" "$DIST"/*.js; then
    echo "error: built bundle does not contain $API_BASE_URL — the API base was not" >&2
    echo "       substituted, and the SPA would call its own origin instead." >&2
    exit 1
fi
# Same failure mode for the captcha: a missed substitution ships the test key
# and the production API rejects every register/reset as captcha_failed.
if [[ -n "$TURNSTILE_SITE_KEY" ]] && ! grep -qr "$TURNSTILE_SITE_KEY" "$DIST"/*.js; then
    echo "error: built bundle does not contain the Turnstile site key — the substitution" >&2
    echo "       failed and the SPA would ship Cloudflare's always-passing TEST key." >&2
    exit 1
fi
if [[ -z "$TURNSTILE_SITE_KEY" ]]; then
    echo "    WARNING: TURNSTILE_SITE_KEY is unset — the bundle carries Cloudflare's TEST"
    echo "    site key. If the API has its captcha enabled, register/reset will fail."
fi

# --- Cloudflare-specific files ------------------------------------------------
# Written here rather than committed, because they are deployment-target
# specific — the Rust binary serves the same SPA with its own fallback and cache
# headers (crates/dashboard_api/src/web.rs) and needs neither file.

# NOTE: no _redirects file. SPA routing is handled by
# assets.not_found_handling=single-page-application in the wrangler config
# below, which is the documented mechanism for Workers Static Assets. A
# catch-all `/* /index.html 200` rewrite here would be redundant and risks
# shadowing real asset paths, since redirects are evaluated before assets.

# Bun content-hashes every asset except index.html, so those can be cached
# permanently; index.html must not be, or a deploy leaves clients on a stale
# shell referencing deleted bundles.
cat >"$DIST/_headers" <<'EOF'
/*
  X-Content-Type-Options: nosniff
  Referrer-Policy: strict-origin-when-cross-origin
  X-Frame-Options: DENY

/index.html
  Cache-Control: public, max-age=0, must-revalidate

/*.js
  Cache-Control: public, max-age=31536000, immutable

/*.css
  Cache-Control: public, max-age=31536000, immutable

/*.webp
  Cache-Control: public, max-age=31536000, immutable

/*.svg
  Cache-Control: public, max-age=31536000, immutable

/*.png
  Cache-Control: public, max-age=31536000, immutable
EOF

echo "==> Prepared $DIST ($(find "$DIST" -type f | wc -l | tr -d ' ') files)"

# --- Wrangler config ----------------------------------------------------------
# Generated into a temp dir rather than committed, so this gitignored script
# stays the single source of deployment truth and no stray wrangler.jsonc shows
# up in `git status`. `assets.directory` is absolute because the config no
# longer sits next to dist/.
#
# not_found_handling=single-page-application is what makes client-side routes
# (/account, /reset-password?token=...) serve index.html instead of 404ing.
# There is no CLI flag for it — it only exists in the config file.
WRANGLER_CONFIG_DIR="$(mktemp -d)"
trap 'rm -rf "$WRANGLER_CONFIG_DIR"' EXIT
WRANGLER_CONFIG="$WRANGLER_CONFIG_DIR/wrangler.json"

cat >"$WRANGLER_CONFIG" <<EOF
{
  "name": "$CF_WORKER_NAME",
  "compatibility_date": "$CF_COMPATIBILITY_DATE",
  "assets": {
    "directory": "$SCRIPT_DIR/$DIST",
    "not_found_handling": "single-page-application"
  }
}
EOF

if [[ -n "${DRY_RUN:-}" ]]; then
    echo "==> DRY_RUN — building and validating without uploading"
    "${WRANGLER[@]}" deploy --config "$WRANGLER_CONFIG" --dry-run
    echo "==> DRY_RUN complete; nothing was uploaded."
    exit 0
fi

# --- Deploy -------------------------------------------------------------------
echo "==> Deploying to Cloudflare Workers"
"${WRANGLER[@]}" deploy --config "$WRANGLER_CONFIG"

echo
echo "==> Done."
echo "    Service: https://dash.cloudflare.com/$CLOUDFLARE_ACCOUNT_ID/workers/services/view/$CF_WORKER_NAME/production"
echo
echo "    If the custom domain is not attached yet, add it once under"
echo "    Workers & Pages -> $CF_WORKER_NAME -> Settings -> Domains & Routes."
echo "    The API only accepts browser origins under battlecrab.com, so the"
echo "    *.workers.dev URL will be blocked by CORS until the real domain is live."
