# G13 — Admin / GM command system

Status: **in progress** — G13.A (framework) landed; **G13.B ~220 portable
handlers landed** (B1–B7: character/skill/item/spawn/movement/GM-util/world/
vitality, plus read-only geo queries and the `//admin` HTML menu). Remaining is
dedicated subsystem work, not handler bodies: mounts + transforms
(`AdminRide`/`AdminTransform` — touch byte-verified UserInfo/CharInfo),
`AdminMobGroup` (controllable-mob group AI), the `AdminGeodata` editor/save
commands, plus the enumerated blocked subcommands (clan-skills, fences, the
AVE/team/targetable effect subset, IP/dualbox, premium/prime/pc-cafe, and the
G13.C sieges/olympiad/instances/… families). This doc is the plan of record;
the G13 section of [PROGRESS.md](PROGRESS.md) records what actually shipped.

Carved out of the old G13 "long tail & parity sweep" catch-all (which named
"admin commands" as one of its absorbed items). The catch-all is renumbered to
**G14** so the admin system gets its own milestone, per the user request for a
faithful full port of the Java admin framework.

## 0. Scope philosophy

The Java datapack has **458 admin commands across 81 handler classes**
(`dist/game/config/AdminCommands.xml`, `data/scripts/handlers/admincommandhandlers/*`).
"Full admin system as in Java" cannot mean "all 458 commands run" in one gate,
because a large fraction of them drive subsystems this server has not ported
yet (sieges, olympiad, instances, cursed weapons, events, territory war,
grand bosses, petitions, hwid tracking…). A command that toggles a castle
siege has nothing to call.

So the milestone is scoped as **the complete framework + every handler whose
backing subsystem already exists**, with the subsystem-blocked handlers
enumerated and deferred to land *with* their subsystems (naturally G14 work).
This is the maximal faithful admin port the current server can support, and
the framework is built to Java parity so that adding a deferred handler later
is a pure "write the handler body" change — no engine work.

Concretely:
- **G13.A** — the framework (access levels, gating, dispatch, confirm flow,
  GM state, behavior-flag integration, name colors). Full parity.
- **G13.B** — all handlers whose subsystems exist (~55% of commands). Faithful
  ports of the Java handler behavior.
- **G13.C** — the subsystem-blocked handlers. Enumerated here, registered in
  `AdminCommands.xml` gating (so access checks are already correct), but their
  bodies land with their subsystems.

## 1. The Java system (what we are porting)

Four pieces, all in the reference tree:

| Java file | Role |
|---|---|
| `model/AccessLevel.java` | One access level: `level`, `name`, name/title color, `childAccess` (parent→child privilege chain), `isGM`, and behavior flags: `allowPeaceAttack`, `allowFixedRes`, `allowTransaction`, `allowAltG`, `giveDamage`, `takeAggro`, `gainExp`. |
| `model/AdminCommandAccessRight.java` | Per-command gate: `command`, required `accessLevel` (default 7), `confirmDlg`. `hasAccess(charLevel)` = exact match **or** char level's childAccess chain contains the command's level. |
| `data/xml/AdminData.java` | Loads `config/AccessLevels.xml` (9 levels: Banned −1 … Master 100) and `config/AdminCommands.xml` (458 rights). `hasAccess`/`requireConfirm`; auto-grants an undefined command to the master level; tracks the live GM list (`_gmList`, hidden flag). |
| `handler/AdminCommandHandler.java` | Registry `command → IAdminCommandHandler`. `useAdminCommand(player, full, useConfirm)`: `isGM` gate → resolve handler → `hasAccess` → optional `ConfirmDlg` → run (Java wraps in a threadpool task; single-threaded here, run inline). GMAudit hook. |

**Dispatch paths (both):**
- `SendBypassBuildCmd` (opcode **0x74**, IN_GAME) — the `//command` bar →
  `useAdminCommand(player, "admin_" + cmd, true)`.
- `RequestBypassToServer` (0x23) with an `admin_` prefix — the HTML admin
  menus' buttons → same entry (currently deferred in `game_loop/bypass.rs`).
- `DlgAnswer` (confirm reply) → re-invokes the stored command with
  `useConfirm = false`.

**Data files (loaded unedited from `dist/game`):**
`config/AccessLevels.xml`, `config/AdminCommands.xml`, plus the admin HTML
menus under `data/html/admin/`.

## 2. Rust starting point (what exists)

- **No access-level concept on `Player`.** The DB column *is* loaded
  (`character.rs::access_level`) but never propagated to the in-game `Player`
  or consulted anywhere.
- **0x74 is unhandled** — `//commands` hit the `_ => error!` arm today.
- **`admin_` bypass is explicitly deferred** in `game_loop/bypass.rs`.
- Handler style here is plain functions in `game_loop/*` dispatched from
  `dispatch.rs` (not a registry object); static data loads via
  `GameData::load_from` in `data/mod.rs`.
- The behavior-flag consumers already exist and currently have no GM exception:
  peace-zone attack refusal (`net.rs`/zones, G12), aggro (`game_loop`, G9), XP
  gain (G9), trade/transaction (not yet a system).

## 3. Phase G13.A — Framework (parity)

The engine. No gameplay command bodies; this is what every command runs on.

1. **`data/admin_data.rs`** — port `AccessLevel` + `AdminCommandAccessRight` +
   an `AdminData` loader. Parse `config/AccessLevels.xml` and
   `config/AdminCommands.xml`; hold `access_levels: HashMap<i32, AccessLevel>`
   and `command_rights: HashMap<String, AdminCommandAccessRight>`; expose
   `has_access(cmd, level)`, `require_confirm(cmd)`, `get_access_level(i32)`,
   `master_access_level()`. Faithful childAccess chain walk + the
   undefined-command master auto-grant. Wire into `GameData` as `admin`.
2. **`Player.access_level: i32`** — add the field; populate in `from_char`
   from `CharData::access_level`. Add `Player::access_level_def(&GameData) ->
   &AccessLevel` and `Player::is_gm(&GameData) -> bool` (resolves through the
   level table, matching Java's `Player.isGM()` = `getAccessLevel().isGm()`).
3. **Behavior-flag integration** — thread the `AccessLevel` flags into the
   consumers that already exist so a GM behaves as in Java:
   `allowPeaceAttack` (peace-zone attack gate), `giveDamage`/`takeAggro`
   (combat/aggro), `gainExp` (XP award), `allowTransaction`/`allowFixedRes`/
   `allowAltG` (stubbed where the consumer doesn't exist yet, with a TODO).
4. **Name/title color** — feed `AccessLevel.nameColor`/`titleColor` into the
   UserInfo/CharInfo builders (visible in-client confirmation the levels
   loaded).
5. **GM registry** — an `AdminData`-equivalent live GM set (`gm_list` with a
   hidden flag) populated on enter-world / cleared on logout; backs
   `//gmliston`/`//gmlistoff` and GM-only broadcasts.
6. **Dispatch** — register `SEND_BYPASS_BUILD_CMD = 0x74`; add
   `game_loop/admin/mod.rs::use_admin_command(world, client_id, full, use_confirm)`
   doing `is_gm` → command lookup → `has_access` → confirm-or-run. Route the
   `admin_` prefix in `bypass.rs` to the same entry. Port `GMAudit` logging.
7. **Confirm flow** — port `ConfirmDlg` (server) + `DlgAnswer` (client 0x?? —
   verify opcode) + the `PlayerAction::AdminCommand` pending-state, so
   `confirmDlg="true"` commands prompt first and execute on "yes". (This is
   full parity; not deferred — several destructive commands rely on it.)
8. **Command registry** — `command → fn(world, client_id, args, player_oid)`
   as a `match`/table in `game_loop/admin/`, mirroring the existing dispatch
   style rather than introducing a registry object.

**G13.A gate:** a character with DB `accessLevel = 100` types `//` a
no-op diagnostic command (e.g. `//serverinfo`) and it runs; a `accessLevel = 0`
character is refused with the Java "no access rights" message; a
`confirmDlg` command prompts and only runs on confirm; a GM's name renders in
the configured color.

## 4. Phase G13.B — Portable handlers (subsystems exist)

Faithful ports of the Java handler bodies, grouped. Command counts are from
`AdminCommands.xml`. Each maps to a subsystem already in the tree.

**B1 — Character, vitals & combat** (`AdminHeal` 1, `AdminRes` 2, `AdminKill` 2,
`AdminLevel` 2, `AdminExpSp` 3, `AdminEditChar` 37, `AdminInvul` 4,
`AdminEffects` 35, `AdminBuffs` 9, `AdminSkill` 17, `AdminHide` 1) — vitals,
revive, combat, level/exp, buffs/effects, skill grant, invul/hide. `EditChar`
is the big one (37 subcommands: set stats, class, name/title, karma, pvp, etc.)
and lands incrementally.

**B2 — Items** (`AdminCreateItem` 5, `AdminDestroyItems` 4, `AdminEnchant` 16) —
inventory + enchant systems exist.

**B3 — Spawns & NPCs** (`AdminSpawn` 19, `AdminDelete` 2, `AdminSummon` 1,
`AdminMobGroup` 17, `AdminScan` 2, `AdminTarget` 1) — spawn/despawn, targeting,
scan; mob-group AI leans on the G9 AI.

**B4 — Movement** (`AdminTeleport` 23, `AdminSpeed`/gmspeed 5, `AdminRide` 9,
`AdminTransform` 3) — teleport/recall/goto, gmspeed/superhaste, mounts,
transforms (mount/transform data present).

**B5 — GM utility & comms** (`AdminAdmin` menu subset, `AdminMenu` 10,
`AdminHtml` 2, `AdminGm` 1, `AdminGmChat` 3, `AdminTargetSay` 1,
`AdminAnnouncements` 4, `AdminMessages` 1, `AdminOnline` 1, `AdminServerInfo` 1,
`AdminKick` 2, `AdminDisconnect` 1, `AdminChangeAccessLevel` 1, `AdminShutdown`
3, `AdminReload` 1, `AdminDebug` 1, `AdminTest` 2, `AdminFightCalculator` 3,
`AdminMissingHtmls` 3, `AdminCamera` 1) — the admin HTML menu, GM toggle/chat,
broadcasts, session control, `//setaccess` (writes the DB `accessLevel` and,
like Java, relays through the login link's `ChangeAccessLevel`), shutdown,
diagnostics.

**B6 — World features (G12 systems)** (`AdminZone` 3, `AdminDoorControl` 5,
`Fences` 5, `AdminGeodata` 27, `AdminGeoEditor` 4, `AdminPathNode` 1,
`AdminShop` 2, `AdminPledge` 1, `AdminClan` 4, `AdminShowQuest` 2,
`AdminQuest` 6) — zones, doors, fences, geodata/pathnode inspection, shop,
clan/pledge admin, quest inspection/reset.

Handlers whose subsystem is only *partially* present (e.g. `AdminVitality` 5,
premium/prime/pc-cafe player-var commands) are ported to the extent the
underlying field exists and TODO-stubbed otherwise.

## 5. Phase G13.C — Deferred handlers (blocked on unported subsystems)

Registered in `AdminCommands.xml` gating (access checks already correct) but
bodies land with their subsystems. Grouped by blocker:

| Blocked handler(s) | Cmds | Blocking subsystem |
|---|---|---|
| `AdminFortSiege`, ADMIN CH SIEGE, `AdminCastle`, `AdminClanHall`, ADMIN TERRITORY WAR | 33 | Castles / sieges (not ported) |
| `AdminOlympiad` (+ saveolymp/endolympiad in AdminAdmin) | — | Olympiad |
| `AdminInstance`, `AdminInstanceZone` | 8 | Instance system |
| `AdminEvents`, ADMIN TVT EVENT | 9 | Event engine |
| `AdminCursedWeapons` | 6 | Cursed weapons |
| `AdminGrandBoss` | 5 | Grand boss / raid manager |
| `AdminManor`, ADMIN MAMMON | 3 | Manor / seven signs |
| `AdminPetition` | 6 | Petition system |
| `AdminLogin` | 6 | Login/account control |
| `AdminHwid` | 2 | HWID tracking |
| `AdminFakePlayers` | 1 | Fake players |
| `AdminBBS` | 1 | Community board (G10-deferred) |
| `AdminPunishment` / ADMIN BAN | 3 | Punishment/jail system |
| `AdminGraciaSeeds`, ADMIN HELLBOUND, `AdminElement` | 12 | Non-Interlude content (may be dropped, not ported) |
| `AdminRepairChar`, `AdminPForge`, ADMIN COND EXCEPTIONS | 7 | Niche / DB-tool / debug (low priority) |

## 6. Milestone gate (live client)

1. A DB-`accessLevel=100` character issues `//` commands from at least each
   B1–B6 group and observes the real effect (heal restores HP, spawn creates a
   visible NPC, teleport moves the character, createitem adds an item, etc.).
2. A `accessLevel=0` character is refused every admin command with the Java
   message; a mid-tier level passes only the commands its childAccess chain
   covers.
3. A `confirmDlg` command (e.g. `//stopallbuffs`) prompts and only runs on
   confirm.
4. The admin HTML menu (`//admin`) opens and its buttons route through the
   `admin_` bypass to the same handlers.
5. `//setaccess` promotes/demotes a target character, persists to the DB, and
   the change survives relog.
6. GM name/title color renders; `//gmliston`/`//hide` toggle GM visibility.
7. A subsystem-blocked command (e.g. `//siege`) is *gated correctly* (access
   check passes for a master) but answers the "not implemented" path rather
   than crashing — proving G13.C is wired, just bodiless.

## 7. Deliberate deviations

- Java runs each command on a threadpool task (freeze protection); the game
  loop is single-threaded, so commands run inline. The >5s "took N ms" notice
  is dropped (no long-running admin work in the ported set).
- `validateHtmlAction` (the sent-action anti-cheat registry) stays unported,
  consistent with the G11 bypass decision; admin menu buttons re-resolve
  through the same range/target checks the handlers already apply.
- `GMAudit` writes to a log line, not the Java per-GM audit file, unless the
  file format is later needed.

## 8. Testing

- **Access math** — unit tests over `has_access` with hand-traced Java values:
  exact-match, childAccess-chain pass/fail across the 9 real levels, the
  undefined-command master auto-grant, `require_confirm`.
- **Data loaders** — load the real `AccessLevels.xml`/`AdminCommands.xml` from
  `dist/game`; assert level count (9) and command count (458), spot-check a few
  rights (`admin_heal` 100, default 7).
- **Dispatch/gating** — synthetic-world tests: 0x74 and `admin_` bypass both
  reach `use_admin_command`; gm vs non-gm vs mid-tier gating; confirm round
  trip (ConfirmDlg out → DlgAnswer in → command runs).
- **Handlers** — synthetic-world integration per B-group over the real tick
  systems (heal restores HP, spawn appears in knownlist, teleport updates
  position + broadcasts, createitem shows in ItemList, setaccess round-trips
  the DB).
- **Blocked path** — a G13.C command for a master answers the not-implemented
  path without panicking.
