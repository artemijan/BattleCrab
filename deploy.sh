#!/usr/bin/env bash
# Cross-compiles loginserver + gameserver for the remote host and deploys them
# over SSH as systemd services (start-on-boot, graceful SIGTERM stop with a
# 30s grace period before SIGKILL).
#
# The services log to the journal, NOT to a file: each server owns its own
# rotating files through commons::logging (config/Logging.ini), so the unit
# only needs to catch stdout/stderr — startup output, panics and anything
# written before the subscriber exists. Redirecting stdout to a file here as
# well would recreate the unbounded log this replaced.
#
# Config comes from an env file (default: deploy.env next to this script,
# override with $1). The env file is gitignored — see deploy.env.example for
# the variables it must define. This script is committed and holds no secrets.
#
# Cross-compilation uses cargo-zigbuild (https://github.com/rust-cross/cargo-zigbuild):
#   cargo install cargo-zigbuild && brew install zig
#   rustup target add <target-triple>
#
# The remote user needs passwordless sudo for `tee` into /etc/systemd/system
# and `systemctl`, e.g. via /etc/sudoers.d/l2-deploy:
#   debian ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/systemd/system/l2-*.service, /usr/bin/systemctl

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

SSH_OPTS=(-p "$SSH_PORT")
RSYNC_SSH="ssh -p $SSH_PORT"
if [[ -n "${SSH_KEY:-}" ]]; then
    SSH_OPTS+=(-i "${SSH_KEY/#\~/$HOME}")
    RSYNC_SSH="$RSYNC_SSH -i ${SSH_KEY/#\~/$HOME}"
fi

remote() {
    # ssh flattens argv into a single string with plain spaces before handing
    # it to the remote shell, so any arg containing a space (e.g. the sed
    # script below) gets word-split remotely unless we re-quote it here.
    local cmd
    printf -v cmd '%q ' "$@"
    ssh "${SSH_OPTS[@]}" "$REMOTE_USER@$REMOTE_HOST" "$cmd"
}

echo "==> Target: $REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH"

# --- Remote prerequisites ---------------------------------------------------
if ! remote command -v rsync >/dev/null 2>&1; then
    echo "==> rsync not found on remote, installing"
    remote sudo apt-get update -qq
    remote sudo apt-get install -y rsync
fi

# --- Resolve target triple ------------------------------------------------
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

if ! command -v cargo-zigbuild >/dev/null 2>&1; then
    echo "error: cargo-zigbuild not found. Install with: cargo install cargo-zigbuild && brew install zig" >&2
    exit 1
fi
if ! rustup target list --installed | grep -qx "$TARGET_TRIPLE"; then
    echo "error: rustup target $TARGET_TRIPLE not installed. Run: rustup target add $TARGET_TRIPLE" >&2
    exit 1
fi

# --- Build ------------------------------------------------------------------
echo "==> Building loginserver + gameserver for $TARGET_TRIPLE (release)"
cargo zigbuild --release --target "$TARGET_TRIPLE" -p loginserver -p gameserver

BIN_DIR="target/$TARGET_TRIPLE/release"

# --- Remote layout ------------------------------------------------------------
echo "==> Ensuring remote directories"
remote mkdir -p \
    "$REMOTE_PATH/dist/login/log" \
    "$REMOTE_PATH/dist/game/log"

# --- Sync binaries + datapack -------------------------------------------------
# rsync's default temp-file-then-rename means an in-flight service keeps
# running the old (now-unlinked) inode until it's restarted below — safe to
# sync while the services are up.
echo "==> Syncing binaries"
rsync -avz -e "$RSYNC_SSH" \
    "$BIN_DIR/loginserver" "$BIN_DIR/gameserver" \
    "$REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH/"

echo "==> Syncing dist/login and dist/game"
# ipconfig.xml (external IP) and Dashboard.ini (secrets) are per-environment
# with no fixup below — never overwrite a live one; *.db* and log/ are runtime
# state, not datapack.
#
# LoginServer.ini and Server.ini ARE synced. Their only per-environment keys
# are LoginHostname/LoginHost, which the sed below rewrites to 127.0.0.1 right
# after this sync. Excluding the whole file instead used to strand every other
# setting at whatever was seeded on the first deploy — that is how the remote
# ended up running AutoCreateAccounts = True while the repo said False, since a
# missing/stale key silently falls back to its built-in default.
rsync -avz -e "$RSYNC_SSH" \
    --exclude='*.db' --exclude='*.db-shm' --exclude='*.db-wal' \
    --exclude='log/*' \
    dist/login/ "$REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH/dist/login/"
rsync -avz -e "$RSYNC_SSH" \
    --exclude='config/ipconfig.xml' \
    --exclude='config/Dashboard.ini' \
    --exclude='*.db' --exclude='*.db-shm' --exclude='*.db-wal' \
    --exclude='log/*' \
    dist/game/ "$REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH/dist/game/"

if ! remote test -f "$REMOTE_PATH/dist/game/config/ipconfig.xml"; then
    echo "==> No ipconfig.xml on remote yet, seeding from local template"
    rsync -avz -e "$RSYNC_SSH" \
        dist/game/config/ipconfig.xml \
        "$REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH/dist/game/config/ipconfig.xml"
    echo "    WARNING: edit $REMOTE_PATH/dist/game/config/ipconfig.xml on the remote —"
    echo "    it currently has the dev LAN address, set gameserver address to $REMOTE_HOST"
    echo "    (or the box's public IP) before external clients can connect."
fi

# Game and login server run on the same box and talk over loopback for the
# GS-link, regardless of what the dev-checked-in defaults (LoginServer.ini's
# `*`, Server.ini's LAN IP) say.
remote sed -i 's/^LoginHostname[[:space:]]*=.*/LoginHostname = 127.0.0.1/' \
    "$REMOTE_PATH/dist/login/config/LoginServer.ini"
remote sed -i 's/^LoginHost[[:space:]]*=.*/LoginHost = 127.0.0.1/' \
    "$REMOTE_PATH/dist/game/config/Server.ini"

if ! remote test -f "$REMOTE_PATH/interlude_classic.db"; then
    echo "    NOTE: no interlude_classic.db found at $REMOTE_PATH — first boot needs a"
    echo "    database provisioned (see dist/db_installer/dumps/) before login will work."
fi

# --- systemd units -------------------------------------------------------------
echo "==> Installing systemd units"
TMP_UNIT_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_UNIT_DIR"' EXIT

cat >"$TMP_UNIT_DIR/l2-loginserver.service" <<EOF
[Unit]
Description=L2 Rust Login Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$REMOTE_USER
WorkingDirectory=$REMOTE_PATH
ExecStart=$REMOTE_PATH/loginserver
Restart=on-failure
RestartSec=5
KillSignal=SIGTERM
TimeoutStopSec=30
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

cat >"$TMP_UNIT_DIR/l2-gameserver.service" <<EOF
[Unit]
Description=L2 Rust Game Server
After=network-online.target l2-loginserver.service
Wants=network-online.target

[Service]
Type=simple
User=$REMOTE_USER
WorkingDirectory=$REMOTE_PATH/dist/game
ExecStart=$REMOTE_PATH/gameserver
Restart=on-failure
RestartSec=5
KillSignal=SIGTERM
TimeoutStopSec=30
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

rsync -avz -e "$RSYNC_SSH" "$TMP_UNIT_DIR/l2-loginserver.service" "$TMP_UNIT_DIR/l2-gameserver.service" \
    "$REMOTE_USER@$REMOTE_HOST:/tmp/"
remote sudo mv /tmp/l2-loginserver.service /tmp/l2-gameserver.service /etc/systemd/system/
remote sudo systemctl daemon-reload
remote sudo systemctl enable l2-loginserver.service l2-gameserver.service

# --- Restart --------------------------------------------------------------
# `systemctl restart` stops (SIGTERM, then up to TimeoutStopSec=30 before
# SIGKILL, per the unit above) before starting the new binary.
echo "==> Restarting l2-loginserver"
remote sudo systemctl restart l2-loginserver.service
sleep 2
echo "==> Restarting l2-gameserver"
remote sudo systemctl restart l2-gameserver.service

echo "==> Status"
remote sudo systemctl --no-pager --lines=5 status l2-loginserver.service l2-gameserver.service || true

echo "==> Done."
echo "    Logs (rotating, JSON, 14-day retention; rotation dates the real files,"
echo "    so tail the stable symlink rather than a dated name):"
echo "      $REMOTE_PATH/dist/game/log/game_server.json"
echo "      $REMOTE_PATH/dist/login/log/login_server.json"
echo "    Warnings and errors only:"
echo "      $REMOTE_PATH/dist/game/log/game_server_error.log"
echo "      $REMOTE_PATH/dist/login/log/login_server_error.log"
echo "    Startup output, panics and pre-subscriber messages go to the journal:"
echo "      journalctl -u l2-gameserver -f"
echo "    Verbosity, rotation and retention: dist/{game,login}/config/Logging.ini"
