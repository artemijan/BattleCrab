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

### Slice 2 — Ban + chat-ban + party-ban
- BAN (account/char → disconnect + refuse login relay), CHAT_BAN (block chat
  send), PARTY_BAN. `//ban`/`//unban`/`//chatban`/`//chatunban` + expiry.

### Slice 3 — Petitions
- `PetitionManager` (in-memory sessions) + the client packets
  (`RequestPetition`/`Cancel`/`Feedback`) + `AdminPetition` (view/accept/reject/
  reset). **Gate:** a player files a petition, a GM answers it.

### Slice 4 — Account/login admin + IP tools
- `AdminLogin` (ban/unban via the login-link relay, `//gm*` account ops) —
  **gate: "ban via the login link"** — plus editchar `//find_ip` /
  `//find_dualbox` / `//tracert` off `Session.addr`.

### Slice 5 (polish) — HWID + fake players
- `AdminHwid`, `AdminFakePlayers`, GM `//snoop`. Subject to the scope gate
  (HWID needs client plumbing that may be stubbed).

## Watch-list
- A punishment's `key` is the affected value (char name / account / ip / hwid);
  matching a player checks all four affects.
- Jail must survive relog (persisted) — re-apply on enter-world, like the
  hero-crown re-apply.
- Petitions are transient (not the `punishments` DB) — only feedback persists.
