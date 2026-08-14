#!/usr/bin/env bash
# Builds and deploys dashboard_api to the same host as deploy.sh, as a systemd
# service (start-on-boot, graceful SIGTERM with a 30s grace period, logs to a
# per-service file).
#
# This is the BACKEND half only. The site itself is deployed separately by
# deploy-web.sh, to Cloudflare Pages — no frontend files are copied to this
# server. The SPA is still *built* here because a release build embeds it into
# the binary (rust-embed), so api.battlecrab.com keeps serving a working copy as
# a fallback; the canonical site is the Cloudflare one.
#
# Shares deploy.env with deploy.sh — same REMOTE_HOST / REMOTE_USER /
# REMOTE_PATH / SSH_* / TARGET_TRIPLE. This script is committed and holds no
# secrets; every credential comes from deploy.env (gitignored — see
# deploy.env.example). See the DASHBOARD_* block below for the extra variables.
#
# Cross-compilation uses cargo-zigbuild (https://github.com/rust-cross/cargo-zigbuild):
#   cargo install cargo-zigbuild && brew install zig
#   rustup target add <target-triple>
# The frontend build needs Bun: https://bun.sh
#
# The remote user needs passwordless sudo for `mv` into /etc/systemd/system and
# `systemctl` — the same grant deploy.sh relies on, widened to
# l2-dashboard.service:
#   debian ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/systemd/system/l2-*.service, /usr/bin/systemctl
#
# Configuring the Cloudflare Tunnel additionally needs:
#   debian ALL=(ALL) NOPASSWD: /usr/bin/dpkg, /usr/bin/cloudflared, /usr/bin/journalctl
#
# HOW IT IS EXPOSED: the API listens on plain HTTP, bound to loopback by
# default. Cloudflare Tunnel (cloudflared, configured below when
# CLOUDFLARE_TUNNEL_TOKEN is set) forwards https://api.battlecrab.com to it, so
# there is no inbound port to open and no TLS certificate on the box. Browsers
# still reach the API over HTTPS, which matters: the session cookie is only
# marked Secure when PublicBaseUrl is an https:// URL.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

ENV_FILE="${1:-$SCRIPT_DIR/deploy.env}"
if [[ ! -f "$ENV_FILE" ]]; then
    echo "error: env file not found: $ENV_FILE" >&2
    echo "create it with REMOTE_HOST, REMOTE_USER, REMOTE_PATH (see deploy.sh header) before deploying." >&2
    exit 1
fi
# shellcheck disable=SC1090
source "$ENV_FILE"

: "${REMOTE_HOST:?REMOTE_HOST must be set in $ENV_FILE}"
: "${REMOTE_USER:?REMOTE_USER must be set in $ENV_FILE}"
: "${REMOTE_PATH:?REMOTE_PATH must be set in $ENV_FILE}"
SSH_PORT="${SSH_PORT:-22}"

# --- Dashboard settings (all overridable from deploy.env) ---------------------
# These are written into the remote Dashboard.ini on every deploy, so the
# deployed config always matches what the SPA was built against.
DASHBOARD_PORT="${DASHBOARD_PORT:-8080}"
DASHBOARD_PUBLIC_URL="${DASHBOARD_PUBLIC_URL:-https://api.battlecrab.com}"
DASHBOARD_SITE_URL="${DASHBOARD_SITE_URL:-https://battlecrab.com}"
DASHBOARD_ALLOWED_ORIGINS="${DASHBOARD_ALLOWED_ORIGINS:-battlecrab.com}"
# Baked into the SPA bundle at build time; must be an absolute URL because the
# API is on a different origin from the site.
DASHBOARD_API_BASE_URL="${DASHBOARD_API_BASE_URL:-${DASHBOARD_PUBLIC_URL}/api/v1}"

# ---session signing key ------------------------------------------------------
# dashboard_api reads this from the environment ONLY. It is deliberately not a
# key in Dashboard.ini, because that file is committed to the repository — a
# secret there would end up in git history and in every clone. For the same
# reason it is NOT defaulted here: this script is committed too, so the value
# must come from deploy.env.
#
# Every deploy must install the SAME value: changing it invalidates every
# session and every outstanding password-reset link. Rotating it is therefore
# the intended "log everyone out" switch: replace the value in deploy.env and
# redeploy. Generate with:  openssl rand -hex 32
DASHBOARD_SESSION_SECRET="${DASHBOARD_SESSION_SECRET:-}"

# Where the secret lands on the server, read by the systemd unit. Kept outside
# dist/ so no rsync in either deploy script can reach it.
DASHBOARD_ENV_FILE="${DASHBOARD_ENV_FILE:-$REMOTE_PATH/dashboard.env}"

# --- Email (Amazon SES) -------------------------------------------------------
# Powers password-reset and email-verification links. Leave DASHBOARD_SMTP_HOST
# empty to deploy with email disabled; the API then logs the links instead of
# sending them, and warns at boot.
#
# The username/password are SES *SMTP credentials* (SES console -> SMTP
# settings -> Create SMTP credentials), NOT AWS access keys — SES derives the
# SMTP password from a secret access key, and pasting the access key itself is
# the usual reason authentication fails.
#
# They are secrets, so they come from deploy.env and go to the chmod-600 env
# file on the server, never into Dashboard.ini (which is committed).
#
# Endpoint is `email-smtp.`, not `smtp.`. Port 587 uses STARTTLS.
DASHBOARD_SMTP_HOST="${DASHBOARD_SMTP_HOST:-}"
DASHBOARD_SMTP_PORT="${DASHBOARD_SMTP_PORT:-587}"
DASHBOARD_SMTP_FROM="${DASHBOARD_SMTP_FROM:-BattleCrab <no-reply@battlecrab.com>}"
DASHBOARD_SMTP_USERNAME="${DASHBOARD_SMTP_USERNAME:-}"
DASHBOARD_SMTP_PASSWORD="${DASHBOARD_SMTP_PASSWORD:-}"

# --- Turnstile captcha --------------------------------------------------------
# Protects register / forgot-password / throttled logins. Create the widget at
# Cloudflare dashboard -> Turnstile -> Add widget (hostname battlecrab.com,
# mode Managed), then set both values in deploy.env (preferred) or here.
#
# The SECRET goes to the chmod-600 env file, environment-only like the session
# secret. Leave empty to deploy with the captcha disabled (the API warns at
# boot). The SITE key is public and is baked into the SPA bundle at build time.
DASHBOARD_TURNSTILE_SECRET="${DASHBOARD_TURNSTILE_SECRET:-}"
TURNSTILE_SITE_KEY="${TURNSTILE_SITE_KEY:-}"

# --- Cloudflare Tunnel --------------------------------------------------------
# Cloudflare's proxy connects to an origin on 443 (or 80 in Flexible mode) and
# NEVER on 8080 — which is why a plain listener on 8080 shows up as a 521
# "web server is down" no matter how healthy the process is.
#
# A tunnel closes that gap without opening any inbound port: cloudflared dials
# OUT to Cloudflare and forwards api.battlecrab.com to 127.0.0.1:$DASHBOARD_PORT.
# No certificate to install or renew, and nothing on the box is reachable from
# the internet directly.
#
# Create it once in the dashboard:
#   Zero Trust -> Networks -> Tunnels -> Create a tunnel -> Cloudflared
#   Copy the token, then add a Public Hostname:
#       api.battlecrab.com  ->  HTTP  ->  localhost:8080
#
# Adding the public hostname creates the DNS record for you. If a proxied A
# record for api.battlecrab.com already exists, delete it first — the tunnel
# needs its own CNAME and the dashboard will refuse to overwrite a conflict.
#
# Put the token in deploy.env. Leave empty to skip tunnel setup entirely.
CLOUDFLARE_TUNNEL_TOKEN="${CLOUDFLARE_TUNNEL_TOKEN:-}"

# With a tunnel in front, the API should listen on loopback only: cloudflared
# reaches it locally, and nothing else can. Set to 0.0.0.0 only if you
# deliberately want the port exposed on the host's public interface.
DASHBOARD_BIND_ADDRESS="${DASHBOARD_BIND_ADDRESS:-127.0.0.1}"

SSH_OPTS=(-p "$SSH_PORT")
RSYNC_SSH="ssh -p $SSH_PORT"
if [[ -n "${SSH_KEY:-}" ]]; then
    SSH_OPTS+=(-i "${SSH_KEY/#\~/$HOME}")
    RSYNC_SSH="$RSYNC_SSH -i ${SSH_KEY/#\~/$HOME}"
fi

remote() {
    # ssh flattens argv into a single string with plain spaces before handing it
    # to the remote shell, so any arg containing a space (the sed scripts below)
    # gets word-split remotely unless we re-quote it here.
    local cmd
    printf -v cmd '%q ' "$@"
    ssh "${SSH_OPTS[@]}" "$REMOTE_USER@$REMOTE_HOST" "$cmd"
}

echo "==> Target: $REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH"
echo "==> API $DASHBOARD_PUBLIC_URL (port $DASHBOARD_PORT)  site $DASHBOARD_SITE_URL"

# --- Guard: deploy.sh must not clobber Dashboard.ini --------------------------
# deploy.sh rsyncs all of dist/game/. Dashboard.ini lives there, and while it no
# longer holds any secret, overwriting it would still reset the deployed
# PublicBaseUrl / SiteBaseUrl / AllowedOrigins to the repo defaults.
if [[ -f "$SCRIPT_DIR/deploy.sh" ]] && ! grep -q "config/Dashboard.ini" "$SCRIPT_DIR/deploy.sh"; then
    echo
    echo "    WARNING: deploy.sh syncs dist/game/ without excluding config/Dashboard.ini."
    echo "    Running it will overwrite the remote dashboard config. Add this to"
    echo "    deploy.sh's dist/game rsync, next to the other --exclude lines:"
    echo
    echo "        --exclude='config/Dashboard.ini' \\"
    echo
fi

# --- Local prerequisites ------------------------------------------------------
if ! command -v bun >/dev/null 2>&1; then
    echo "error: bun not found. Install from https://bun.sh — the SPA is embedded in the binary." >&2
    exit 1
fi
if ! command -v cargo-zigbuild >/dev/null 2>&1; then
    echo "error: cargo-zigbuild not found. Install with: cargo install cargo-zigbuild && brew install zig" >&2
    exit 1
fi
# Fail before doing any work, rather than after a five-minute build.
if [[ ${#DASHBOARD_SESSION_SECRET} -lt 32 ]]; then
    echo "error: DASHBOARD_SESSION_SECRET is ${#DASHBOARD_SESSION_SECRET} chars; dashboard_api requires at least 32 and will refuse to boot." >&2
    echo "       Generate one with: openssl rand -hex 32" >&2
    exit 1
fi

# --- Remote prerequisites -----------------------------------------------------
if ! remote command -v rsync >/dev/null 2>&1; then
    echo "==> rsync not found on remote, installing"
    remote sudo apt-get update -qq
    remote sudo apt-get install -y rsync
fi

# --- Resolve target triple ----------------------------------------------------
if [[ -z "${TARGET_TRIPLE:-}" ]]; then
    remote_arch="$(remote uname -m)"
    case "$remote_arch" in
        x86_64) TARGET_TRIPLE=x86_64-unknown-linux-gnu ;;
        aarch64 | arm64) TARGET_TRIPLE=aarch64-unknown-linux-gnu ;;
        *)
            echo "error: cannot map remote arch '$remote_arch' to a Rust target triple; set TARGET_TRIPLE in $ENV_FILE" >&2
            exit 1
            ;;
    esac
    echo "==> Detected remote arch $remote_arch -> $TARGET_TRIPLE"
fi

if ! rustup target list --installed | grep -qx "$TARGET_TRIPLE"; then
    echo "error: rustup target $TARGET_TRIPLE not installed. Run: rustup target add $TARGET_TRIPLE" >&2
    exit 1
fi

# --- Build the SPA first ------------------------------------------------------
# Not for deployment — deploy-web.sh ships the site to Cloudflare. This exists
# because a release build embeds web/dashboard/dist into the binary via
# rust-embed, and dist/ is gitignored: without building, the binary would embed
# whatever stale bundle happened to be lying around from a previous run (or an
# empty directory, serving "frontend not built"). Building here keeps the
# embedded fallback consistent with the API being deployed.
echo "==> Building SPA to embed in the binary (API base: $DASHBOARD_API_BASE_URL)"
(
    cd web/dashboard
    bun install --frozen-lockfile
    API_BASE_URL="$DASHBOARD_API_BASE_URL" TURNSTILE_SITE_KEY="$TURNSTILE_SITE_KEY" bun run build
)

# Fail loudly rather than shipping a binary that serves "frontend not built".
if [[ ! -f web/dashboard/dist/index.html ]]; then
    echo "error: web/dashboard/dist/index.html missing after build" >&2
    exit 1
fi
if ! grep -qr "$DASHBOARD_API_BASE_URL" web/dashboard/dist/*.js; then
    echo "error: built bundle does not contain $DASHBOARD_API_BASE_URL — the API base was not" >&2
    echo "       substituted, and the SPA would call its own origin instead." >&2
    exit 1
fi
if [[ -n "$TURNSTILE_SITE_KEY" ]] && ! grep -qr "$TURNSTILE_SITE_KEY" web/dashboard/dist/*.js; then
    echo "error: built bundle does not contain the Turnstile site key — the substitution" >&2
    echo "       failed and the SPA would ship Cloudflare's always-passing TEST key." >&2
    exit 1
fi

# --- Build the API ------------------------------------------------------------
echo "==> Building dashboard_api for $TARGET_TRIPLE (release)"
cargo zigbuild --release --target "$TARGET_TRIPLE" -p dashboard_api

BIN_DIR="target/$TARGET_TRIPLE/release"

# --- Remote layout ------------------------------------------------------------
echo "==> Ensuring remote directories"
remote mkdir -p "$REMOTE_PATH/dist/game/config" "$REMOTE_PATH/dist/game/log"

# --- Sync binary --------------------------------------------------------------
# rsync's temp-file-then-rename means the running service keeps executing the
# old (now-unlinked) inode until it is restarted below — safe to sync live.
echo "==> Syncing dashboard_api"
rsync -avz -e "$RSYNC_SSH" "$BIN_DIR/dashboard_api" \
    "$REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH/"

# --- Session secret -----------------------------------------------------------
# Written to a chmod-600 env file loaded by the systemd unit — never into
# Dashboard.ini, which is committed and world-readable on the box.
echo "==> Writing session secret to $DASHBOARD_ENV_FILE"
# Create and lock down the file *before* writing, so the value is never even
# briefly readable by other users on the host.
remote touch "$DASHBOARD_ENV_FILE"
remote chmod 600 "$DASHBOARD_ENV_FILE"
# Single-quoted on the remote side so values cannot be word-split or
# glob-expanded by the remote shell. Written in one shot (> then >>) so the
# file is never left half-populated if the connection drops mid-way.
# The whole env file is built locally and copied in one shot. Two reasons:
#
#   * Values are single-quoted. systemd's EnvironmentFile parser treats an
#     unquoted '#' as starting a comment, so a secret containing one would be
#     silently TRUNCATED — the service then starts, reports email as enabled,
#     and fails SMTP auth with no clue why.
#   * Building it here avoids pushing secrets through a remote shell command
#     line, where they would be visible in `ps` for the life of the call.
#
# ALL SMTP settings live here now, not in Dashboard.ini. The ini keys were
# removed because `sed "s|^SmtpHost.*|...|"` against a remote file that lacked
# the key changed nothing and still exited 0, so the deploy claimed success
# while email stayed disabled.
echo "==> Writing $DASHBOARD_ENV_FILE (session secret + SMTP)"

TMP_ENV="$(mktemp)"
trap 'rm -f "$TMP_ENV"' EXIT

env_line() {
    # A single quote in a value would break the quoting; refuse rather than
    # write a file that silently means something else.
    case "$2" in
        *"'"*)
            echo "error: $1 contains a single quote, which this writer cannot escape." >&2
            echo "       Regenerate the credential without one." >&2
            exit 1
            ;;
    esac
    printf "%s='%s'\n" "$1" "$2" >>"$TMP_ENV"
}

env_line DASHBOARD_SESSION_SECRET "$DASHBOARD_SESSION_SECRET"

if [[ -n "$DASHBOARD_TURNSTILE_SECRET" ]]; then
    echo "    captcha ENABLED (Turnstile)"
    env_line DASHBOARD_TURNSTILE_SECRET "$DASHBOARD_TURNSTILE_SECRET"
else
    echo
    echo "    WARNING: captcha will be DISABLED — DASHBOARD_TURNSTILE_SECRET is unset."
    echo "    Registration, password-reset and throttled logins will not require a"
    echo "    Turnstile check. Set it (and TURNSTILE_SITE_KEY) in deploy.env."
    echo
fi

if [[ -n "$DASHBOARD_SMTP_HOST" && -n "$DASHBOARD_SMTP_USERNAME" && -n "$DASHBOARD_SMTP_PASSWORD" ]]; then
    echo "    email ENABLED via $DASHBOARD_SMTP_HOST:$DASHBOARD_SMTP_PORT"
    env_line DASHBOARD_SMTP_HOST "$DASHBOARD_SMTP_HOST"
    env_line DASHBOARD_SMTP_PORT "$DASHBOARD_SMTP_PORT"
    env_line DASHBOARD_SMTP_FROM "$DASHBOARD_SMTP_FROM"
    env_line DASHBOARD_SMTP_USERNAME "$DASHBOARD_SMTP_USERNAME"
    env_line DASHBOARD_SMTP_PASSWORD "$DASHBOARD_SMTP_PASSWORD"
else
    echo
    echo "    WARNING: email will be DISABLED — one of HOST/USERNAME/PASSWORD is unset."
    echo "      DASHBOARD_SMTP_HOST     ${DASHBOARD_SMTP_HOST:+set}${DASHBOARD_SMTP_HOST:-<empty>}"
    echo "      DASHBOARD_SMTP_USERNAME ${DASHBOARD_SMTP_USERNAME:+set}${DASHBOARD_SMTP_USERNAME:-<empty>}"
    echo "      DASHBOARD_SMTP_PASSWORD ${DASHBOARD_SMTP_PASSWORD:+set}${DASHBOARD_SMTP_PASSWORD:-<empty>}"
    echo "    Password-reset and verification links will be written to the server log."
    echo "    Set them in deploy.env."
    echo
fi

rsync -avz --chmod=600 -e "$RSYNC_SSH" "$TMP_ENV" \
    "$REMOTE_USER@$REMOTE_HOST:$DASHBOARD_ENV_FILE"
remote chmod 600 "$DASHBOARD_ENV_FILE"
rm -f "$TMP_ENV"

# --- Config -------------------------------------------------------------------
# Seed the file only if absent, then force the deployment-specific keys.
if ! remote test -f "$REMOTE_PATH/dist/game/config/Dashboard.ini"; then
    echo "==> No Dashboard.ini on remote yet, seeding from local template"
    rsync -avz -e "$RSYNC_SSH" dist/game/config/Dashboard.ini \
        "$REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH/dist/game/config/Dashboard.ini"
fi

echo "==> Applying deployment settings to remote Dashboard.ini"
remote sed -i "s|^BindAddress[[:space:]]*=.*|BindAddress = $DASHBOARD_BIND_ADDRESS|" \
    "$REMOTE_PATH/dist/game/config/Dashboard.ini"
remote sed -i "s|^Port[[:space:]]*=.*|Port = $DASHBOARD_PORT|" \
    "$REMOTE_PATH/dist/game/config/Dashboard.ini"
remote sed -i "s|^PublicBaseUrl[[:space:]]*=.*|PublicBaseUrl = $DASHBOARD_PUBLIC_URL|" \
    "$REMOTE_PATH/dist/game/config/Dashboard.ini"
remote sed -i "s|^SiteBaseUrl[[:space:]]*=.*|SiteBaseUrl = $DASHBOARD_SITE_URL|" \
    "$REMOTE_PATH/dist/game/config/Dashboard.ini"
remote sed -i "s|^AllowedOrigins[[:space:]]*=.*|AllowedOrigins = $DASHBOARD_ALLOWED_ORIGINS|" \
    "$REMOTE_PATH/dist/game/config/Dashboard.ini"
# No Smtp* seds: SMTP is environment-only now (written to $DASHBOARD_ENV_FILE
# above). Strip any stale keys a previously deployed ini still carries, so the
# server stops logging "it is IGNORED" errors about them.
remote sed -i "/^Smtp\(Host\|Port\|From\|Username\|Password\|User\)[[:space:]]*=/d" \
    "$REMOTE_PATH/dist/game/config/Dashboard.ini"

# Belt and braces: the server ignores a SessionSecret key in the ini, but its
# presence means one may have been committed at some point.
if remote grep -q '^SessionSecret' "$REMOTE_PATH/dist/game/config/Dashboard.ini" 2>/dev/null; then
    echo "    NOTE: remote Dashboard.ini still contains a SessionSecret key. It is ignored;"
    echo "    remove it, and rotate the value if it was ever committed to git."
fi

# The dashboard opens the same SQLite file as the login/game servers and will
# refuse to start if it is missing, rather than creating an empty one.
if ! remote test -f "$REMOTE_PATH/interlude_classic.db"; then
    echo
    echo "    WARNING: no interlude_classic.db at $REMOTE_PATH."
    echo "    dashboard_api will refuse to start — it must open the SAME database as the"
    echo "    login and game servers. Deploy those first (./deploy.sh)."
    echo
fi

# --- systemd unit -------------------------------------------------------------
echo "==> Installing systemd unit"
TMP_UNIT_DIR="$(mktemp -d)"
# Cleans up both temporaries: bash keeps only ONE EXIT trap, so this replaces
# the TMP_ENV trap set earlier and must therefore cover it too. TMP_ENV holds
# the session secret and SMTP password (mktemp creates it 0600, but leaving it
# behind at all is worse than not).
trap 'rm -rf "$TMP_UNIT_DIR"; rm -f "$TMP_ENV"' EXIT

# WorkingDirectory is $REMOTE_PATH, not dist/game: the config path
# (dist/game/config/Dashboard.ini) and the default database path
# (interlude_classic.db) are both resolved relative to the working directory.
#
# EnvironmentFile supplies DASHBOARD_SESSION_SECRET. Note the unit file itself
# stays free of the secret, so `systemctl cat` and /etc/systemd/system do not
# expose it.
cat >"$TMP_UNIT_DIR/l2-dashboard.service" <<EOF
[Unit]
Description=L2 Rust Web Dashboard API
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$REMOTE_USER
WorkingDirectory=$REMOTE_PATH
EnvironmentFile=$DASHBOARD_ENV_FILE
ExecStart=$REMOTE_PATH/dashboard_api
Restart=on-failure
RestartSec=5
KillSignal=SIGTERM
TimeoutStopSec=30
StandardOutput=append:$REMOTE_PATH/dist/game/log/dashboard.log
StandardError=append:$REMOTE_PATH/dist/game/log/dashboard.log

[Install]
WantedBy=multi-user.target
EOF

rsync -avz -e "$RSYNC_SSH" "$TMP_UNIT_DIR/l2-dashboard.service" \
    "$REMOTE_USER@$REMOTE_HOST:/tmp/"
remote sudo mv /tmp/l2-dashboard.service /etc/systemd/system/
remote sudo systemctl daemon-reload
remote sudo systemctl enable l2-dashboard.service

# --- Restart ------------------------------------------------------------------
echo "==> Restarting l2-dashboard"
remote sudo systemctl restart l2-dashboard.service

# --- Health check -------------------------------------------------------------
# The service can exit *after* systemctl returns success (bad config, missing
# database, missing secret), so check the endpoint rather than trusting restart.
echo "==> Health check"
health_ok=""
for _ in 1 2 3 4 5; do
    sleep 1
    if remote curl -fsS --max-time 3 "http://127.0.0.1:$DASHBOARD_PORT/api/v1/health" >/dev/null 2>&1; then
        health_ok=1
        break
    fi
done

if [[ -n "$health_ok" ]]; then
    echo "    OK — /api/v1/health responded on port $DASHBOARD_PORT"
else
    echo "    FAILED — no response from http://127.0.0.1:$DASHBOARD_PORT/api/v1/health" >&2
    echo "    Recent log:" >&2
    remote tail -n 20 "$REMOTE_PATH/dist/game/log/dashboard.log" >&2 || true
    remote sudo systemctl --no-pager --lines=10 status l2-dashboard.service >&2 || true
    exit 1
fi

# --- Cloudflare Tunnel --------------------------------------------------------
# Installed after the health check, so the tunnel is only ever pointed at an
# API that is already answering locally.
if [[ -n "$CLOUDFLARE_TUNNEL_TOKEN" ]]; then
    if ! remote command -v cloudflared >/dev/null 2>&1; then
        echo "==> Installing cloudflared"
        case "$TARGET_TRIPLE" in
            x86_64-*) cf_arch=amd64 ;;
            aarch64-*) cf_arch=arm64 ;;
            *)
                echo "error: no cloudflared package known for $TARGET_TRIPLE" >&2
                exit 1
                ;;
        esac
        remote curl -fsSL -o /tmp/cloudflared.deb \
            "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${cf_arch}.deb"
        remote sudo dpkg -i /tmp/cloudflared.deb
        remote rm -f /tmp/cloudflared.deb
    fi

    # Reinstall rather than reuse: the token is the whole configuration, and
    # this is the only way to guarantee the running tunnel matches the token in
    # this script. The uninstall is a no-op on a first deploy.
    echo "==> Installing cloudflared service"
    remote sudo cloudflared service uninstall >/dev/null 2>&1 || true
    remote sudo cloudflared service install "$CLOUDFLARE_TUNNEL_TOKEN"
    remote sudo systemctl enable --now cloudflared

    sleep 3
    if remote systemctl is-active --quiet cloudflared; then
        echo "    cloudflared is running"
    else
        echo "    WARNING: cloudflared is not active. Recent log:" >&2
        remote sudo journalctl -u cloudflared --no-pager --lines=15 >&2 || true
    fi

    # End-to-end check through Cloudflare. Warn rather than fail: the public
    # hostname route is configured in the dashboard, not by this script, and
    # DNS may not have propagated yet.
    echo "==> Checking $DASHBOARD_PUBLIC_URL from here"
    if curl -fsS --max-time 15 "$DASHBOARD_PUBLIC_URL/api/v1/health" >/dev/null 2>&1; then
        echo "    OK — reachable through Cloudflare"
    else
        code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 15 "$DASHBOARD_PUBLIC_URL/api/v1/health" || echo 000)"
        echo "    Not reachable yet (HTTP $code)."
        case "$code" in
            521) echo "    521 = Cloudflare cannot reach the origin. Check the tunnel's Public Hostname" ;;
            530) echo "    530 = tunnel not connected, or hostname not routed to this tunnel" ;;
            000) echo "    No response — DNS may not have propagated yet" ;;
        esac
        echo "    Confirm in the dashboard: Zero Trust -> Networks -> Tunnels -> Public Hostname"
        echo "      api.battlecrab.com -> HTTP -> localhost:$DASHBOARD_PORT"
    fi
else
    echo
    echo "    NOTE: CLOUDFLARE_TUNNEL_TOKEN is not set, so no tunnel was configured."
    echo "    Cloudflare connects to origins on port 443, never $DASHBOARD_PORT, so"
    echo "    https://api.battlecrab.com will return 521 until either a tunnel or a"
    echo "    reverse proxy fronts this service. See the header of this script."
    echo
fi

echo "==> Status"
remote sudo systemctl --no-pager --lines=5 status l2-dashboard.service || true

echo "==> Done. Logs: $REMOTE_PATH/dist/game/log/dashboard.log"
echo "    Listening on $DASHBOARD_BIND_ADDRESS:$DASHBOARD_PORT"
if [[ -n "$CLOUDFLARE_TUNNEL_TOKEN" ]]; then
    echo "    Exposed publicly by cloudflared at $DASHBOARD_PUBLIC_URL"
else
    echo "    NOT exposed publicly — no tunnel or reverse proxy is configured."
fi
