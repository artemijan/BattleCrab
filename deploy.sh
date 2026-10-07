#!/usr/bin/env bash
# Cross-compiles loginserver + gameserver for the remote host and deploys them
# over SSH as systemd services (start-on-boot, graceful SIGTERM stop with a
# 30s grace period before SIGKILL).
#
# `l2r-migrate` ships alongside them and runs against the remote database while
# the services are stopped, so a schema change cannot reach production as code
# without the database following it.
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
# The login and game servers may run on separate machines (LOGIN_HOST,
# GAME_HOST; both default to REMOTE_HOST). deploy-lib.sh works out which
# services share a machine and the private addresses between them, and this
# script writes those into each server's config after syncing it.
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

# shellcheck source=deploy-lib.sh
source "$SCRIPT_DIR/deploy-lib.sh"
deploy_load_env "${1:-}"

if ! command -v cargo-zigbuild >/dev/null 2>&1; then
    echo "error: cargo-zigbuild not found. Install with: cargo install cargo-zigbuild && brew install zig" >&2
    exit 1
fi

deploy_resolve_topology
deploy_print_topology

# --- Remote prerequisites ---------------------------------------------------
if [[ "$GAME_MACHINE" == "$LOGIN_MACHINE" ]]; then
    SERVER_HOSTS=("$LOGIN_HOST")
else
    SERVER_HOSTS=("$LOGIN_HOST" "$GAME_HOST")
fi
deploy_require_rsync "${SERVER_HOSTS[@]}"

# --- Resolve target triple ------------------------------------------------
deploy_resolve_triple "${SERVER_HOSTS[@]}"
if ! rustup target list --installed | grep -qx "$TARGET_TRIPLE"; then
    echo "error: rustup target $TARGET_TRIPLE not installed. Run: rustup target add $TARGET_TRIPLE" >&2
    exit 1
fi

# --- Build ------------------------------------------------------------------
echo "==> Building loginserver + gameserver + l2r-migrate for $TARGET_TRIPLE (release)"
cargo zigbuild --release --target "$TARGET_TRIPLE" -p loginserver -p gameserver -p migration

BIN_DIR="target/$TARGET_TRIPLE/release"

# --- Remote layout ------------------------------------------------------------
echo "==> Ensuring remote directories"
on "$LOGIN_HOST" mkdir -p "$REMOTE_PATH/dist/login/log"
on "$GAME_HOST" mkdir -p "$REMOTE_PATH/dist/game/log"

# --- Sync binaries + datapack -------------------------------------------------
# rsync's default temp-file-then-rename means an in-flight service keeps
# running the old (now-unlinked) inode until it's restarted below — safe to
# sync while the services are up.
#
# l2r-migrate goes to the login host, which is where migrations run.
echo "==> Syncing binaries"
push "$LOGIN_HOST" "$BIN_DIR/loginserver" "$BIN_DIR/l2r-migrate" "$REMOTE_PATH/"
push "$GAME_HOST" "$BIN_DIR/gameserver" "$REMOTE_PATH/"

echo "==> Syncing dist/login and dist/game"
# ipconfig.xml (external IP) and Dashboard.ini (secrets) are per-environment
# with no fixup below — never overwrite a live one; *.db* and log/ are runtime
# state, not datapack.
#
# LoginServer.ini, Server.ini and both Monitor.ini ARE synced. Their only
# per-environment keys are the addresses the topology decides, which the
# set_ini calls below rewrite right after this sync. Excluding the whole file
# instead used to strand every other setting at whatever was seeded on the
# first deploy — that is how the remote ended up running AutoCreateAccounts =
# True while the repo said False, since a missing/stale key silently falls back
# to its built-in default.
push "$LOGIN_HOST" \
    --exclude='*.db' --exclude='*.db-shm' --exclude='*.db-wal' \
    --exclude='log/*' \
    dist/login/ "$REMOTE_PATH/dist/login/"
push "$GAME_HOST" \
    --exclude='config/ipconfig.xml' \
    --exclude='config/Dashboard.ini' \
    --exclude='*.db' --exclude='*.db-shm' --exclude='*.db-wal' \
    --exclude='log/*' \
    dist/game/ "$REMOTE_PATH/dist/game/"

if ! on "$GAME_HOST" test -f "$REMOTE_PATH/dist/game/config/ipconfig.xml"; then
    echo "==> No ipconfig.xml on $GAME_HOST yet, seeding from local template"
    push "$GAME_HOST" dist/game/config/ipconfig.xml "$REMOTE_PATH/dist/game/config/ipconfig.xml"
    echo "    WARNING: edit $REMOTE_PATH/dist/game/config/ipconfig.xml on $GAME_HOST —"
    echo "    it currently has the dev LAN address, set gameserver address to $GAME_HOST"
    echo "    (or the box's public IP) before external clients can connect."
fi

# The addresses between services, whatever the dev-checked-in defaults
# (LoginServer.ini's `*`, Server.ini's LAN IP) say: loopback where two services
# share a machine, the private address where they don't (deploy-lib.sh).
echo "==> Applying topology to the server configs"
set_ini "$LOGIN_HOST" "$REMOTE_PATH/dist/login/config/LoginServer.ini" LoginHostname "$GS_LINK_ADDRESS"
set_ini "$GAME_HOST" "$REMOTE_PATH/dist/game/config/Server.ini" LoginHost "$GS_LINK_ADDRESS"
set_ini "$LOGIN_HOST" "$REMOTE_PATH/dist/login/config/LoginServer.ini" InternalStatusBindAddress "$LOGIN_CHANNEL_ADDRESS"
set_ini "$LOGIN_HOST" "$REMOTE_PATH/dist/login/config/Monitor.ini" InternalMonitorBindAddress "$LOGIN_CHANNEL_ADDRESS"
set_ini "$GAME_HOST" "$REMOTE_PATH/dist/game/config/Monitor.ini" InternalMonitorBindAddress "$GAME_CHANNEL_ADDRESS"

if [[ -n "$DB_IS_SQLITE" ]] && ! on "$LOGIN_HOST" test -f "$REMOTE_PATH/interlude_classic.db"; then
    echo "    NOTE: no interlude_classic.db found at $REMOTE_PATH — create it with"
    echo "    \`l2r-migrate up -u jdbc:sqlite:$REMOTE_PATH/interlude_classic.db\` (docs/DATABASE.md)"
    echo "    before login will work."
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

install_unit() {
    local host="$1" unit="$2"
    push "$host" "$TMP_UNIT_DIR/$unit" /tmp/
    on "$host" sudo mv "/tmp/$unit" /etc/systemd/system/
    on "$host" sudo systemctl daemon-reload
    on "$host" sudo systemctl enable "$unit"
}
install_unit "$LOGIN_HOST" l2-loginserver.service
install_unit "$GAME_HOST" l2-gameserver.service

# --- Restart ---------------------------------------------------------------
# Stop, migrate, start — rather than two `systemctl restart`s. The schema has
# to move while nothing is holding the database: `l2r-migrate` rebuilds tables,
# and a server that booted against the old shape would go on serving it.
#
# Migrating here is not optional bookkeeping. A schema change ships as code the
# moment the binaries sync, and until this step existed the remote database was
# never migrated at all — which is how `grandboss_data` was left on a column
# type the new code could not decode, and the game server booted with no grand
# bosses and only a warning to show for it (#19).
#
# The game server stops first, so it is never left up against a login server
# that is already down.
echo "==> Stopping services for the migration"
on "$GAME_HOST" sudo systemctl stop l2-gameserver.service
on "$LOGIN_HOST" sudo systemctl stop l2-loginserver.service

if [[ -z "$DB_IS_SQLITE" ]]; then
    # A database server: no file to copy. Back it up with its own tools.
    echo "==> Applying database migrations ($LOGIN_HOST)"
    if ! on "$LOGIN_HOST" "$REMOTE_PATH/l2r-migrate" up -u "$DB_URL"; then
        echo "" >&2
        echo "error: migration failed. Both services are STOPPED and were not started." >&2
        echo "       Each migration is applied in a transaction; verify the database before" >&2
        echo "       restarting: sudo systemctl start l2-loginserver (on $LOGIN_HOST), then" >&2
        echo "       sudo systemctl start l2-gameserver (on $GAME_HOST)." >&2
        exit 1
    fi
elif on "$LOGIN_HOST" test -f "$REMOTE_PATH/interlude_classic.db"; then
    # One rolling copy, overwritten each deploy: the last known-good state from
    # before the most recent schema change, without unbounded disk creep.
    echo "==> Backing up the database"
    on "$LOGIN_HOST" cp "$REMOTE_PATH/interlude_classic.db" \
        "$REMOTE_PATH/interlude_classic.db.pre-migrate.bak"
    echo "==> Applying database migrations"
    # `set -e` would abort here with both services still stopped and nothing
    # said about it, so the failure explains itself. Stopping is the right
    # outcome — booting against a database whose schema is half-moved is worse
    # than being down — but the operator has to be told that is where they are.
    if ! on "$LOGIN_HOST" "$REMOTE_PATH/l2r-migrate" up -u "jdbc:sqlite:$REMOTE_PATH/interlude_classic.db"; then
        echo "" >&2
        echo "error: migration failed. Both services are STOPPED and were not started." >&2
        echo "       Each migration is applied in a transaction, so the database is" >&2
        echo "       either untouched or fully migrated — but verify before restarting." >&2
        echo "       Backup from just before this run:" >&2
        echo "         $REMOTE_PATH/interlude_classic.db.pre-migrate.bak" >&2
        echo "       Once resolved, bring the services back up with:" >&2
        echo "         sudo systemctl start l2-loginserver l2-gameserver" >&2
        exit 1
    fi
else
    echo "    no interlude_classic.db on remote — skipping migrations"
fi

echo "==> Starting l2-loginserver ($LOGIN_HOST)"
on "$LOGIN_HOST" sudo systemctl start l2-loginserver.service
sleep 2
echo "==> Starting l2-gameserver ($GAME_HOST)"
on "$GAME_HOST" sudo systemctl start l2-gameserver.service

echo "==> Status"
on "$LOGIN_HOST" sudo systemctl --no-pager --lines=5 status l2-loginserver.service || true
on "$GAME_HOST" sudo systemctl --no-pager --lines=5 status l2-gameserver.service || true

echo "==> Done."
echo "    Logs (rotating, JSON, 14-day retention; rotation dates the real files,"
echo "    so tail the stable symlink rather than a dated name):"
echo "      $GAME_HOST:$REMOTE_PATH/dist/game/log/game_server.json"
echo "      $LOGIN_HOST:$REMOTE_PATH/dist/login/log/login_server.json"
echo "    Warnings and errors only:"
echo "      $GAME_HOST:$REMOTE_PATH/dist/game/log/game_server_error.log"
echo "      $LOGIN_HOST:$REMOTE_PATH/dist/login/log/login_server_error.log"
echo "    Startup output, panics and pre-subscriber messages go to the journal:"
echo "      journalctl -u l2-gameserver -f"
echo "    Verbosity, rotation and retention: dist/{game,login}/config/Logging.ini"
