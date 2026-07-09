# Login Server — Java → Rust Parity Checklist

Every file under `java/org/l2jmobius/loginserver` (63 files) accounted for.
Status: ✅ ported · 🔵 folded into another module · ⏭️ intentionally dropped
(per decisions in [JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md)) ·
💀 dead code in Java (unreferenced).

Shared crypto/config/db/network live in the `commons` crate and are reused by
the game server phase.

## Bootstrap & core

| Java | Status | Rust |
|---|---|---|
| `LoginServer.java` | ✅ | `main.rs` (boot order, scheduled restart, shutdown + SQLite backup) |
| `LoginController.java` | ✅ | `controller.rs` (actor) + `context.rs` (key caches) + `dao.rs` |
| `GameServerTable.java` | ✅ | `gs_table.rs` + controller GS state |
| `GameServerThread.java` | ✅ | `gs_link/connection.rs` |
| `GameServerListener.java` + `FloodProtectedListener.java` | ✅ | `gs_link/listener.rs` |
| `SessionKey.java` | ✅ | `session.rs` |
| `model/data/AccountInfo.java` | ✅ | `session.rs` |
| `HackingException.java` | 🔵 | folded into disconnect handling (no exception type needed) |

## Crypto

| Java | Status | Rust |
|---|---|---|
| `crypt/BlowfishEngine.java` | 🔵 | `commons::crypt::NewCrypt` (std `Blowfish<LE>`) |
| `crypt/NewCrypt.java` | ✅ | `commons::crypt::new_crypt` |
| `network/ScrambledKeyPair.java` | ✅ | `commons::crypt::scrambled_keypair` + `raw_rsa` (GS-link 512-bit) |
| `network/LoginEncryption.java` | ✅ | `network/encryption.rs` |

All four verified byte-for-byte against golden vectors from the Java classes
(`tools/vector-dump`, `commons/tests/golden_vectors.rs`).

## Enums

| Java | Status | Rust |
|---|---|---|
| `enums/LoginFailReason.java` | ✅ | `enums.rs::LoginFailReason` |
| `enums/PlayFailReason.java` | ✅ | `enums.rs::PlayFailReason` |
| `enums/AccountKickedReason.java` | ✅ | `enums.rs::AccountKickedReason` |
| `enums/LoginResult.java` | 🔵 | `controller::AuthOutcome` |
| `network/ConnectionState.java` | ✅ | `enums.rs::ConnectionState` |
| `network/GameServerPacketHandler.GameServerState` | 🔵 | `gs_link/connection.rs` local enum |

## Client packets (client ↔ LS)

| Java | Status | Rust |
|---|---|---|
| `AuthGameGuard` / `GGAuth` | ✅ | handshake (M2) |
| `RequestAuthLogin` | ✅ | `client_connection::request_auth_login` (M3) |
| `RequestCmdLogin` | ✅ | cmd-line login path (M5) |
| `RequestServerList` → `ServerList` | ✅ | M4, with char counts (M5) |
| `RequestServerLogin` → `PlayOk`/`PlayFail` | ✅ | M4 |
| `RequestPIAgreementCheck` → `PIAgreementCheck` | ✅ | M5 |
| `RequestPIAgreement` → `PIAgreementAck` | ✅ | M5 |
| `Init` / `LoginFail` / `LoginOk` / `AccountKicked` | ✅ | `network/server_packets.rs` |
| `LoginClientPacket` / `LoginClientPackets` / `LoginServerPackets` / `LoginPacketHandler` / `LoginServerPacket` | 🔵 | dispatch in `client_connection.rs` (opcode+state match) |
| `serverpackets/LoginOtpFail` | 💀 | never referenced in Java — not ported |

## GS-link packets (GS ↔ LS)

| Java (GS→LS) | Status | Rust |
|---|---|---|
| `BlowFishKey` | ✅ | key exchange (M4) |
| `GameServerAuth` | ✅ | registration (M4) |
| `PlayerInGame` | ✅ | M4 |
| `PlayerLogout` | ✅ | M4 |
| `ChangeAccessLevel` | ✅ | M4 |
| `PlayerAuthRequest` | ✅ | M4 |
| `ServerStatus` | ✅ | M4 |
| `PlayerTracert` | ✅ | M5 |
| `ReplyCharacters` | ✅ | M5 |
| `RequestTempBan` | ✅ | M5 |
| `ChangePassword` | ✅ | M5 |

| Java (LS→GS) | Status | Rust (`gs_link/packets.rs`) |
|---|---|---|
| `InitLS` / `LoginServerFail` / `AuthResponse` | ✅ | M4 |
| `PlayerAuthResponse` / `KickPlayer` / `RequestCharacters` | ✅ | M4/M5 |
| `ChangePasswordResponse` | ✅ | M5 |

## Intentionally dropped

| Java | Status | Reason |
|---|---|---|
| `ui/Gui.java`, `ui/frmAbout.java` | ⏭️ | Swing GUI dropped (decision #10) — headless |

## Notable behavior preserved 1:1

- Wrong password → `REASON_ACCESS_FAILED` (not `USER_OR_PASS_WRONG`).
- Banned IP → `LoginFail(REASON_NOT_AUTHED)` in place of `Init`.
- `ServerList` waits up to 500 ms (10×50 ms) for `ReplyCharacters`.
- `RequestTempBan` passes the absolute ban-end timestamp as a duration to
  `addBanForAddress` (a Java quirk, kept identical).
- Double login (LS or GS) → `ACCOUNT_IN_USE` + kick of the prior session.

## Verification

- 15 unit tests (crypto, config, db, packet, subnet, hexid).
- 6 golden-vector parity tests vs. the Java crypto classes.
- 20 integration tests: handshake, auth, GS link, M5 packets — all driving the
  real wire protocol (client- and GS-side crypto simulated).
- 1 live end-to-end test (`live_e2e.rs`, `--ignored`) confirmed against the
  unmodified Java game server: it registered as "Server 1: Bartz" and a client
  logged through the Rust LS to `PlayOk`.
