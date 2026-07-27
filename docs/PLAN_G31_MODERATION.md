# PLAN — G31 Moderation, accounts, petitions & HWID

GM moderation tooling. **Gate:** jail a player, file and answer a petition, ban
via the login link. **Deps:** per-client IP (already on `Session.addr`).

## What already exists (verified)

- `Session.addr: SocketAddr` (+ `addr()`) — the per-client IP the find-IP tools
  need. accessLevel + `//gm`/`set_access` already in `admin/moderation.rs`
  (kick, disconnect, gmchat, changelvl).
- No punishment / petition / jail / HWID system yet.

## Java sources

- **Punishment** (`model/punishment/*`, `instancemanager/PunishmentManager` 147,
  `PunishmentTask` 263): `PunishmentType` = BAN / CHAT_BAN / PARTY_BAN / JAIL;
  `PunishmentAffect` = ACCOUNT / CHARACTER / IP / HWID; DB table `punishments`
  (`key`, `affect`, `type`, `expiration`, `reason`, `punishedBy`). Effect via
  punishment handlers (`JailHandler`/`ChatBanHandler`/`BanHandler`).
- **Jail**: a `JailZone` in `data/zones/gm_room.xml`; jail-in
  `(-114356, -249645, -2984)`, release `(17836, 170178, -3507)`.
- **Petitions** (`Petition` 178, `PetitionManager` 460): in-memory (only
  `petition_feedback` persists post-resolution). Client packets
  `RequestPetition` (content + type 1–9), `RequestPetitionCancel`,
  `RequestPetitionFeedback`; GM handles via `AdminPetition`.
- **Admin handlers** (`AdminPunishment` 426, `AdminPetition` 130, `AdminLogin`
  251, `AdminHwid` 64, `AdminFakePlayers` 79).

## Slice breakdown

### Slice 1 — Punishment foundation + jail  ✅ LANDED (gate: "jail a player" met)
- `model/punishment.rs`: `PunishmentType`/`PunishmentAffect` enums + a
  `Punishment { key, affect, type, expiration, reason, by }` + the
  `PunishmentManager` runtime on `World` (keyed set, `is_punished` lookups).
- DB: `punishments` load at boot (`DbEvent::PunishmentsLoaded`) + writes
  (`DbCommand::{StorePunishment, DeletePunishment}`); expiring rows dropped.
- `ZoneKind::Jail` (parse `gm_room.xml`'s `JailZone`) + the jail in/out locs.
- Jail apply/remove: a `jailed` flag (block actions/movement like the observer
  invul pattern), teleport to jail on apply, release on end; the JailZone
  keep-in enforcement.
- `AdminPunishment` `//jail <name> [minutes]` / `//unjail <name>` (character
  affect). Expiry via `ScheduledTask::PunishmentExpire`.
- **Gate for the slice:** a GM jails a player (teleported + blocked), the row
  persists, and `//unjail` (or expiry) releases them.

### Slice 2 — Ban + chat-ban + party-ban  ✅ LANDED
- BAN (account/char → disconnect on start + character-select login gate),
  CHAT_BAN (block `Say2` unless `.`-prefixed + chat-block icon), PARTY_BAN
  (block `RequestJoinParty` for a banned requestor or target). Generic
  `start_punishment`/`stop_punishment` engine dispatching per-type onStart/onEnd
  effects. Admin `//ban_char`/`//ban_acc`/`//ban_chat`/`//ban_party` (+`//un*`),
  with the un-commands taking a name **or** a raw char id (no offline name→id
  table). Expiry via the shared `PunishmentExpire` timer.

### Slice 3 — Petitions  ✅ LANDED (gate: file + answer a petition met)
- `PetitionManager` (in-memory `pending`/`completed`, no persistence) + the
  client packets (`RequestPetition` 0x89 / `Cancel` 0x8A / `Feedback` 0xC9) +
  petition consultation chat (`Say2` PETITION_PLAYER/GM → both participants) +
  `AdminPetition` (`//view_petitions`/`view_petition`/`accept`/`reject`/`reset`).
  Only feedback persists (`petition_feedback` via `StorePetitionFeedback`).

### Slice 4 — Account/login admin + IP tools  ✅ LANDED (gate: ban via login link met)
- The login-link account-ban relay (Java `Player.setAccountAccesslevel` →
  `LoginServerThread.sendAccessLevel` → `ChangeAccessLevel` 0x04):
  `LoginLinkCommand::SetAccountAccessLevel`, `//login_ban` (relay level −1 +
  kick online sessions on that account) / `//login_unban` (level 0). Plus the
  editchar IP tools off `Session.addr`: `//find_ip <ip>`, `//find_dualbox [n]`,
  `//tracert <name>` (peer-address only — Java's route-trace needs client
  plumbing the port lacks).

### Slice 5 (polish) — snoop + HWID  ✅ LANDED (fake players deferred)
- **`//snoop`**: `Player.snoop_listeners`/`snooped`, the `Snoop` packet (0xDB),
  and a `broadcast_snoop` hook in `Say2` that mirrors a snooped player's chat.
- **HWID**: `RequestHardWareInfo` (ex 0xAE) → a `HardwareInfo` on
  `World.hwids` (keyed by client id, cleared on disconnect); the HWID
  punishment affect now matches (ban/jail), the character-select gate + a
  post-enter `on_hwid_received` re-check enforce it, and `//hwid`/`//hwinfo`
  displays it. `EnableHardwareInfo = False` on this dist, so it is dormant
  until enabled — ported per [[l2r-config-disabled-still-port]].
- **Deferred:** `AdminFakePlayers` (`//fakechat`) needs the whole fake-player
  subsystem (`FakePlayerData` + `FakePlayerChatManager` + fake-NPC spawns),
  which the port lacks — a separate content system, not moderation. Its own
  milestone, not G31.

## Watch-list
- A punishment's `key` is the affected value (char name / account / ip / hwid);
  matching a player checks all four affects.
- Jail must survive relog (persisted) — re-apply on enter-world, like the
  hero-crown re-apply.
- Petitions are transient (not the `punishments` DB) — only feedback persists.
