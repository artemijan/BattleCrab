# Login Server Parity Checklist

M5 acceptance from [PLAN_LOGIN_SERVER.md](PLAN_LOGIN_SERVER.md): every Java
login-server file, marked **ported** / **dropped by decision** / **n/a**.
Rust locations are relative to `crates/loginserver/src` unless noted.

## Core classes

| Java file | Status | Rust location / note |
|---|---|---|
| `LoginServer.java` | ✅ ported | `main.rs` (bootstrap order kept; GUI branch dropped; scheduled restart = exit code 2; shutdown backup for SQLite) |
| `LoginController.java` | ✅ ported | `controller.rs` actor + `context.rs` (key caches) + `dao.rs` (SQL); purge task → per-session 5-min deadline in `network/client_connection.rs` |
| `GameServerTable.java` | ✅ ported | `gs_table.rs` (servername.xml, gameservers table, hexid BigInteger semantics, subnet addresses); GS RSA keys in `context.rs` |
| `GameServerThread.java` | ✅ ported | `gs_link/connection.rs` (task per GS, state machine, payload crypt) |
| `GameServerListener.java` + `FloodProtectedListener.java` | ✅ ported | `gs_link/listener.rs` (per-IP flood rules, removeFloodProtection on close) |
| `LoginController` bans + `banned_ip.cfg` loader | ✅ ported | `controller.rs` + `ban_file.rs` (subnet ban forms incl.) |
| `SessionKey.java` | ✅ ported | `session.rs` (incl. show-licence-dependent equality in PlayerAuthRequest) |
| `model/data/AccountInfo.java` | ✅ ported | `session.rs` |
| `HackingException.java` | n/a | Java-ism (exception as control flow); Rust uses close paths |
| `LoginServer` GUI (`ui/Gui.java`, `ui/frmAbout.java`) | 🚫 dropped | decision #10 — headless |

## Crypt

| Java file | Status | Rust location |
|---|---|---|
| `crypt/BlowfishEngine.java` | ✅ ported | `commons/crypt/new_crypt.rs` (= `Blowfish<LE>`, golden-vector verified) |
| `crypt/NewCrypt.java` | ✅ ported | `commons/crypt/new_crypt.rs` (checksum, XOR pass) |
| `network/ScrambledKeyPair.java` | ✅ ported | `commons/crypt/scrambled_keypair.rs` (golden-vector verified) |
| GS-link RSA (512-bit, `BigInteger.toByteArray`) | ✅ ported | `commons/crypt/raw_rsa.rs` |
| SHA-1+Base64 password hash | ✅ ported | `commons/crypt/password.rs` (golden-vector verified) |

## Network core

| Java | Status | Rust |
|---|---|---|
| Async-mmocore (`commons/network/*`, 18 files) | ✅ replaced | `commons/network/` tokio: framing, PacketReader/Writer (UTF-16LE strings); fairness/drop-policy/buffer-pool deferred to GS phase by plan §4 |
| `LoginEncryption.java` | ✅ ported | `network/encryption.rs` (static+XOR first packet, checksum after) |
| `ConnectionState.java`, enums (4 files) | ✅ ported | `enums.rs` |
| `LoginPacketHandler.java` + `LoginClientPackets.java` | ✅ ported | state-checked dispatch in `network/client_connection.rs` |
| `GameServerPacketHandler.java` | ✅ ported | state machine in `gs_link/connection.rs` |
| `LoginClient.java` | ✅ ported | `ClientSession` + connection task (banned-IP pre-Init refusal, kick channel, onDisconnection cleanup) |

## Client packets (8)

| Java | Status | Note |
|---|---|---|
| `AuthGameGuard.java` | ✅ | session-id check → GGAuth |
| `RequestAuthLogin.java` | ✅ | both 128/256-byte layouts, Java trim |
| `RequestCmdLogin.java` | ✅ | gated on EnableCmdLineLogin, user@0x40/pass@0x60 |
| `RequestServerList.java` | ✅ | loginOk pair check; char counts incl. 500 ms ReplyCharacters wait |
| `RequestServerLogin.java` | ✅ | licence-dependent key check, lastServer update, PlayOk/PlayFail |
| `RequestPIAgreementCheck.java` | ✅ | ShowPIAgreement config |
| `RequestPIAgreement.java` | ✅ | ack echo |
| `LoginClientPacket.java` (base) | n/a | base-class mechanics live in the dispatch loop |

## Server packets (13)

`Init`, `GGAuth`, `LoginOk`, `LoginFail`, `AccountKicked`, `ServerList`
(incl. char-count block), `PlayOk`, `PlayFail`, `PIAgreementCheck`,
`PIAgreementAck` — ✅ all in `network/server_packets.rs`.
`LoginOtpFail` — 🚫 not sent by any Interlude flow (OTP is a later-chronicle
feature); add if a client ever elicits it.
`LoginServerPacket.java` (base) — n/a.

## GS→LS packets (12)

| Java | Status | Note |
|---|---|---|
| `BlowFishKey.java` | ✅ | RSA decrypt, leading-zero strip, key switch |
| `GameServerAuth.java` | ✅ | hexid match / alternative-id / register-new + DB insert |
| `PlayerInGame.java` | ✅ | also frees LS slot per Java `addAccountOnGameServer` |
| `PlayerLogout.java` | ✅ | |
| `ChangeAccessLevel.java` | ✅ | accounts.accessLevel update |
| `PlayerAuthRequest.java` | ✅ | session-key match incl. licence variant |
| `ServerStatus.java` | ✅ | all attribute types; global LS status override |
| `PlayerTracert.java` | ✅ | pcIp/hop1..4 update |
| `ReplyCharacters.java` | ✅ | char counts → ServerList (deletion timestamps read+discarded, as unused by Interlude ServerList) |
| `RequestTempBan.java` | ✅ | account_data ban_temp + IP ban (Java quirk kept: absolute timestamp passed as duration) |
| `ChangePassword.java` | ✅ | verify/update/respond via hosting GS |
| `RequestSendMail` (0x09) | n/a | commented out in Java too |

## LS→GS packets (7)

`InitLS`, `LoginServerFail`, `AuthResponse`, `PlayerAuthResponse`,
`KickPlayer`, `RequestCharacters`, `ChangePasswordResponse` — ✅ all in
`gs_link/packets.rs`.

## Infrastructure

| Java | Status | Note |
|---|---|---|
| `Config.java` (login section) + `PropertiesParser` | ✅ | `config.rs` + `commons/config.rs` (env-var override scheme kept) |
| `DatabaseFactory`/dialect/queries | ✅ SQLite-only | `commons/db.rs` + `dao.rs` (decision #9) |
| `DatabaseBackup.