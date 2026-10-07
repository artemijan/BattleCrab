# Shared by deploy.sh and deploy-dashboard.sh: the env file, SSH to each
# service's host, and the topology — which services share a machine, and the
# addresses the internal channels use between them. Sourced, never run.
#
# Each service may live on its own host (LOGIN_HOST, GAME_HOST,
# DASHBOARD_HOST, each defaulting to REMOTE_HOST). Whether two of them share a
# machine is decided by the machine itself, not by the names: two hosts with
# the same /etc/machine-id are one box reached two ways, and are treated as
# one. Co-located services talk over loopback, as they always have. Services
# on different machines talk over a private network (docs/MONITORING.md §4):
#
#   LOGIN_PRIVATE_ADDRESS  the login machine's private address — where the
#                          game server's link (LoginPort) and the dashboard's
#                          monitor/status polls reach it
#   GAME_PRIVATE_ADDRESS   the game machine's private address — where the
#                          dashboard's monitor polls reach it
#
# Each is required only when some other service is on another machine, and
# must be a private-range address: these ports have no authentication.

# shellcheck shell=bash

# --- Env file -------------------------------------------------------------------
deploy_load_env() {
    ENV_FILE="${1:-$SCRIPT_DIR/deploy.env}"
    if [[ ! -f "$ENV_FILE" ]]; then
        echo "error: env file not found: $ENV_FILE" >&2
        echo "create it from deploy.env.example before deploying." >&2
        exit 1
    fi
    # shellcheck disable=SC1090
    source "$ENV_FILE"

    : "${REMOTE_USER:?REMOTE_USER must be set in $ENV_FILE}"
    : "${REMOTE_PATH:?REMOTE_PATH must be set in $ENV_FILE}"
    LOGIN_HOST="${LOGIN_HOST:-${REMOTE_HOST:-}}"
    GAME_HOST="${GAME_HOST:-${REMOTE_HOST:-}}"
    DASHBOARD_HOST="${DASHBOARD_HOST:-${REMOTE_HOST:-}}"
    local v
    for v in LOGIN_HOST GAME_HOST DASHBOARD_HOST; do
        if [[ -z "${!v}" ]]; then
            echo "error: $v is not set, and there is no REMOTE_HOST to default it to ($ENV_FILE)." >&2
            exit 1
        fi
    done
    SSH_PORT="${SSH_PORT:-22}"

    SSH_OPTS=(-p "$SSH_PORT")
    RSYNC_SSH="ssh -p $SSH_PORT"
    if [[ -n "${SSH_KEY:-}" ]]; then
        SSH_OPTS+=(-i "${SSH_KEY/#\~/$HOME}")
        RSYNC_SSH="$RSYNC_SSH -i ${SSH_KEY/#\~/$HOME}"
    fi
}

# --- SSH ------------------------------------------------------------------------
# `on HOST cmd args...` runs one command on HOST. ssh flattens argv into a
# single string with plain spaces before handing it to the remote shell, so any
# arg containing a space (the sed scripts) would be word-split remotely unless
# it is re-quoted here.
on() {
    local host="$1" cmd
    shift
    printf -v cmd '%q ' "$@"
    ssh "${SSH_OPTS[@]}" "$REMOTE_USER@$host" "$cmd"
}

# `at HOST PATH` is an rsync destination on HOST.
at() {
    printf '%s@%s:%s' "$REMOTE_USER" "$1" "$2"
}

# `push HOST rsync-args... DEST-PATH` rsyncs to DEST-PATH on HOST.
push() {
    local host="$1"
    shift
    local args=("$@")
    local last=$((${#args[@]} - 1))
    local dest="${args[$last]}"
    unset "args[$last]"
    rsync -avz -e "$RSYNC_SSH" "${args[@]}" "$(at "$host" "$dest")"
}

# `set_ini HOST FILE KEY VALUE` sets KEY in FILE on HOST, appending it when the
# file lacks it. A plain sed against a missing key changes nothing and still
# exits 0, which is how a deploy once reported success while leaving a setting
# at its default.
set_ini() {
    # shellcheck disable=SC2016
    on "$1" sh -c '
        if grep -q "^$1[[:space:]]*=" "$3"; then
            sed -i "s|^$1[[:space:]]*=.*|$1 = $2|" "$3"
        else
            printf "%s = %s\n" "$1" "$2" >>"$3"
        fi' _ "$3" "$4" "$2"
}

# `ini_value FILE KEY` reads KEY from a local ini file.
ini_value() {
    sed -n "s/^$2[[:space:]]*=[[:space:]]*//p" "$1" | tail -n 1 | tr -d '\r'
}

# --- Remote prerequisites and target -------------------------------------------
# rsync on each distinct host.
deploy_require_rsync() {
    local host
    for host in "$@"; do
        if ! on "$host" command -v rsync >/dev/null 2>&1; then
            echo "==> rsync not found on $host, installing"
            on "$host" sudo apt-get update -qq
            on "$host" sudo apt-get install -y rsync
        fi
    done
}

triple_of() {
    local arch
    arch="$(on "$1" uname -m)"
    case "$arch" in
        x86_64) echo x86_64-unknown-linux-gnu ;;
        aarch64 | arm64) echo aarch64-unknown-linux-gnu ;;
        *)
            echo "error: cannot map $1's arch '$arch' to a Rust target triple; set TARGET_TRIPLE in $ENV_FILE" >&2
            return 1
            ;;
    esac
}

# One build serves every host, so they must agree on the target. Pinning
# TARGET_TRIPLE skips the probe (and its round-trips) entirely.
deploy_resolve_triple() {
    if [[ -n "${TARGET_TRIPLE:-}" ]]; then
        return
    fi
    local host triple
    for host in "$@"; do
        triple="$(triple_of "$host")" || exit 1
        if [[ -z "${TARGET_TRIPLE:-}" ]]; then
            TARGET_TRIPLE="$triple"
            echo "==> Detected $host -> $TARGET_TRIPLE"
        elif [[ "$triple" != "$TARGET_TRIPLE" ]]; then
            echo "error: $host is $triple but another host is $TARGET_TRIPLE; one build cannot serve both." >&2
            exit 1
        fi
    done
}

# --- Topology -------------------------------------------------------------------
# What identifies the machine behind HOST: its machine-id, or its hostname
# where there is none.
machine_of() {
    on "$1" sh -c 'cat /etc/machine-id 2>/dev/null || hostname' | tr -d '[:space:]'
}

# Loopback, RFC 1918, RFC 6598 (100.64/10, Tailscale) and IPv6 unique-local —
# the same ranges the servers accept for their internal channels
# (commons::network::internal).
is_private_address() {
    local a="$1" o1 o2 rest
    case "$a" in
        127.* | 10.* | 192.168.*) return 0 ;;
        fc* | fd* | FC* | FD* | ::1) return 0 ;;
    esac
    IFS=. read -r o1 o2 rest <<<"$a"
    [[ "$o1" =~ ^[0-9]+$ && "$o2" =~ ^[0-9]+$ ]] || return 1
    if [[ "$o1" == 172 ]] && ((o2 >= 16 && o2 <= 31)); then return 0; fi
    if [[ "$o1" == 100 ]] && ((o2 >= 64 && o2 <= 127)); then return 0; fi
    return 1
}

require_private_address() {
    local name="$1" why="$2"
    local value="${!name:-}"
    if [[ -z "$value" ]]; then
        echo "error: $name must be set: $why." >&2
        echo "       Use that machine's private-network address (LAN, VPC, WireGuard/Tailscale)." >&2
        exit 1
    fi
    if ! is_private_address "$value"; then
        echo "error: $name = $value is not a private-network address." >&2
        echo "       The internal ports have no authentication and must never face the internet." >&2
        exit 1
    fi
}

# Sets LOGIN_MACHINE / GAME_MACHINE / DASHBOARD_MACHINE, then every address the
# services use to reach each other:
#
#   GS_LINK_ADDRESS        login's LoginHostname (where it listens for game
#                          servers) and game's LoginHost (where it connects)
#   LOGIN_CHANNEL_ADDRESS  where login binds its monitor and status channels,
#                          and where the dashboard reaches them
#   GAME_CHANNEL_ADDRESS   the same for the game server's monitor channel
deploy_resolve_topology() {
    LOGIN_MACHINE="$(machine_of "$LOGIN_HOST")"
    if [[ "$GAME_HOST" == "$LOGIN_HOST" ]]; then
        GAME_MACHINE="$LOGIN_MACHINE"
    else
        GAME_MACHINE="$(machine_of "$GAME_HOST")"
    fi
    if [[ "$DASHBOARD_HOST" == "$LOGIN_HOST" ]]; then
        DASHBOARD_MACHINE="$LOGIN_MACHINE"
    elif [[ "$DASHBOARD_HOST" == "$GAME_HOST" ]]; then
        DASHBOARD_MACHINE="$GAME_MACHINE"
    else
        DASHBOARD_MACHINE="$(machine_of "$DASHBOARD_HOST")"
    fi
    local m
    for m in LOGIN_MACHINE GAME_MACHINE DASHBOARD_MACHINE; do
        if [[ -z "${!m}" ]]; then
            echo "error: could not identify the machine behind ${m%_MACHINE} (no machine-id or hostname)." >&2
            exit 1
        fi
    done

    if [[ "$LOGIN_MACHINE" == "$GAME_MACHINE" ]]; then
        GS_LINK_ADDRESS=127.0.0.1
    else
        require_private_address LOGIN_PRIVATE_ADDRESS "the game server ($GAME_HOST) is on another machine than login ($LOGIN_HOST)"
        GS_LINK_ADDRESS="$LOGIN_PRIVATE_ADDRESS"
    fi
    if [[ "$LOGIN_MACHINE" == "$DASHBOARD_MACHINE" ]]; then
        LOGIN_CHANNEL_ADDRESS=127.0.0.1
    else
        require_private_address LOGIN_PRIVATE_ADDRESS "the dashboard ($DASHBOARD_HOST) is on another machine than login ($LOGIN_HOST)"
        LOGIN_CHANNEL_ADDRESS="$LOGIN_PRIVATE_ADDRESS"
    fi
    if [[ "$GAME_MACHINE" == "$DASHBOARD_MACHINE" ]]; then
        GAME_CHANNEL_ADDRESS=127.0.0.1
    else
        require_private_address GAME_PRIVATE_ADDRESS "the dashboard ($DASHBOARD_HOST) is on another machine than the game server ($GAME_HOST)"
        GAME_CHANNEL_ADDRESS="$GAME_PRIVATE_ADDRESS"
    fi

    # Ports as the repo configures them: the deploy syncs these very files.
    LOGIN_LINK_PORT="$(ini_value dist/login/config/LoginServer.ini LoginPort)"
    LOGIN_STATUS_PORT="$(ini_value dist/login/config/LoginServer.ini InternalStatusPort)"
    LOGIN_MONITOR_PORT="$(ini_value dist/login/config/Monitor.ini InternalMonitorPort)"
    GAME_MONITOR_PORT="$(ini_value dist/game/config/Monitor.ini InternalMonitorPort)"

    deploy_check_database
}

# SQLite is one file on one disk: login, game and dashboard can share it only
# on one machine. A database server (the URL in each service's ini) lifts that.
deploy_check_database() {
    DB_URL="$(ini_value dist/login/config/LoginServer.ini URL)"
    local f url
    for f in dist/game/config/Server.ini dist/game/config/Dashboard.ini; do
        url="$(ini_value "$f" URL)"
        if [[ "$url" != "$DB_URL" ]]; then
            echo "error: $f has URL = $url, but LoginServer.ini has $DB_URL." >&2
            echo "       All three services must open the same database." >&2
            exit 1
        fi
    done
    DB_IS_SQLITE=""
    if [[ "$DB_URL" == jdbc:sqlite:* || "$DB_URL" == sqlite:* ]]; then
        DB_IS_SQLITE=1
        if [[ "$LOGIN_MACHINE" != "$GAME_MACHINE" || "$LOGIN_MACHINE" != "$DASHBOARD_MACHINE" ]]; then
            echo "error: the services are on more than one machine, but the database is SQLite" >&2
            echo "       ($DB_URL): one file on one disk, which they can only share on one machine" >&2
            echo "       — SQLite over a network filesystem is not safe. Point URL in" >&2
            echo "       LoginServer.ini, Server.ini and Dashboard.ini at a database server first." >&2
            exit 1
        fi
    fi
}

# " (same machine as NAME)" when machine A is machine B.
same_note() {
    [[ "$1" == "$2" ]] && printf '  (same machine as %s)' "$3"
    return 0
}

deploy_print_topology() {
    echo "==> Topology"
    echo "    login      $REMOTE_USER@$LOGIN_HOST:$REMOTE_PATH"
    echo "    game       $REMOTE_USER@$GAME_HOST:$REMOTE_PATH$(same_note "$GAME_MACHINE" "$LOGIN_MACHINE" login)"
    local note
    note="$(same_note "$DASHBOARD_MACHINE" "$GAME_MACHINE" game)"
    [[ -z "$note" ]] && note="$(same_note "$DASHBOARD_MACHINE" "$LOGIN_MACHINE" login)"
    echo "    dashboard  $REMOTE_USER@$DASHBOARD_HOST:$REMOTE_PATH$note"
    echo "    game -> login link    $GS_LINK_ADDRESS:$LOGIN_LINK_PORT"
    echo "    dashboard -> login    $LOGIN_CHANNEL_ADDRESS:$LOGIN_MONITOR_PORT (monitor), $LOGIN_CHANNEL_ADDRESS:$LOGIN_STATUS_PORT (status)"
    echo "    dashboard -> game     $GAME_CHANNEL_ADDRESS:$GAME_MONITOR_PORT (monitor)"
}
