# Remaining-work Roadmap (G14 → G33)

Master plan for the rest of the Java→Rust port: every Java game-server subsystem
not yet built, broken into dependency-ordered milestones. Supersedes the old
single "G14 Long tail & parity sweep" catch-all in
[PLAN_GAME_SERVER.md](PLAN_GAME_SERVER.md) §6. Live status lives in
[PROGRESS.md](PROGRESS.md).

**Where we are:** G0–G13 done (login complete; game through enter-world, items,
stats/skills, combat/AI, social, quests/clan-creation, static world, and the
full admin framework + all *portable* admin handlers incl. mounts, transforms,
mob-group AI, and the geo-editor). What's left is **subsystem breadth** — the
whole-feature systems the vertical slices deferred.

**The admin lens.** The 202 still-unimplemented `//` commands are almost all
*gated-but-bodiless* because their backing subsystem doesn't exist yet. Each
milestone below lists the admin handler(s) it unblocks, so "finish the admin
system" and "reach Java parity" are the same backlog.

**Definition of done (per milestone):** faithful port of the Java behavior,
verifiable against the live Java server on the same DB/client, with the milestone
gate met. Same vertical-slice-then-breadth discipline used through G13.

---

## Milestone map

| # | Milestone | Track | Unblocks (admin) | Depends on |
|---|-----------|-------|------------------|------------|
| G14 | Item stats & equipment combat accuracy | Foundations | `//setparam` | — |
| G15 | Economy & item actions | Foundations | — | G14 |
| G16 | Character variables, premium & vitality | Foundations | `//premium*` `//pccafepoints` `//primepoints` `//set_vitality_level` | — |
| G17 | Sub-classes, class change & nobless | Progression | `//setnoble` `//setsubclass` (editchar) | G22¹ |
| G18 | Clans — full | Progression | `//clan_*` `//pledge` `//add_clan_skill` | G15 |
| G19 | Skills & effects breadth | Combat | `//ave_abnormal` `//setteam` `//settargetable` `//para` `//playmovie` … (AdminEffects) | — |
| G20 | Combat breadth | Combat | — | G14, G19 |
| G21 | NPC AI & world-content breadth | Combat | `//scan` extras, guard/faction | G20 |
| G22 | Quest & script breadth | Content | `//quest_*` `//charquestmenu` `//setcharquest` `//reload` (scripts) | G17, G19 |
| G23 | Grand bosses & raid bosses | End-game | `//grandboss` (AdminGrandBoss) | G21 |
| G24 | Castles, sieges, clan halls & territory war | End-game | `//siege`/AdminFortSiege, `//castle`, `//clanhall`, territory war | G18, G21 |
| G25 | Olympiad & hero | End-game | AdminOlympiad, `//saveolymp` `//endolympiad` `//sethero` `//givehero` `//settruehero` | G17 |
| G26 | Seven Signs, Manor & Mammon | End-game | `//manor`, `//mammon_*` | G24, G15 |
| G27 | Instances | End-game | AdminInstance, AdminInstanceZone | G21 |
| G28 | Events engine & cursed weapons | End-game | AdminEvents, `//tvt_*`, AdminCursedWeapons | G20 |
| G29 | Summons, pets, servitors, cubics, agathions | Support | AdminEditChar summon/pet subcommands | G19, G20 |
| G30 | Mail, community board & party matching | Support | AdminBBS | G18 |
| G31 | Moderation, accounts, petitions & HWID | Support | AdminPunishment, AdminLogin, AdminHwid, AdminPetition, AdminFakePlayers, editchar find_ip/dualbox/tracert | IP plumbing |
| G32 | Fishing | Support | — | G19 |
| G33 | Misc parity & finishing sweep | Finishing | AdminFightCalculator, AdminRepairChar, AdminPForge, AdminMissingHtmls, AdminPcCondOverride, `//geosave` serializer | (last) |

¹ G17's occupation *quests* need G22, but the class-change *mechanics* can land
first; nobless status can be admin-set before the nobless quest exists.

**Out of scope (present in the datapack, not Interlude Classic):**
`AdminGraciaSeeds`, ADMIN HELLBOUND, `AdminElement` (Gracia/Hellbound/elemental
attributes are Kamael-era content). Also out: `tools/` ports, MariaDB/Postgres,
Swing UI (per [PLAN_GAME_SERVER.md](PLAN_GAME_SERVER.md) §11).

---

## Track A — Foundations (high leverage; do first)

### G14 — Item stats & equipment combat accuracy
Parse the `<stats>` block every `stats/items/*.xml` carries (currently skipped —
combat runs on naked class values). Wire weapon/armor bonuses into P/M-Atk,
P/M-Def, accuracy, evasion, crit, attack speed; port `calcShldUse` (shield
defence), `ArmorSetData` (set bonuses + `getArmorMinEnchant` for the UserInfo
enchant byte), and the `SHOTS_BONUS` dynamic stat. **Gate:** equipped gear
changes the UserInfo stat block to the retail values; a shield blocks; a full
armor set grants its bonus. **Unblocks:** `//setparam` (fixed-stat editing);
accurate everything downstream.

### G15 — Economy & item actions
The itemcontainer breadth G5 deferred: private/clan warehouse + freight; private
stores (sell/buy/manufacture/package) + offline stores; player-to-player trade;
ground drop/pickup (`ItemsOnGroundManager`, herbs); `multisell`/`sell` bypasses;
crystallization; enchant scrolls (safe/normal/blessed + `EnchantResult`);
augmentation / life stones + variation; the rest of `handlers/itemhandlers/*`
(dyes/scrolls/`<cond>` gating). **Gate:** warehouse round-trip, a private store
sells to another client, trade completes, an item enchants and can break, loot
drops to the ground and is picked up. **Deps:** G14 (enchant/augment stat
effects).

### G16 — Character variables, premium & vitality
`GlobalVariablesManager` + a per-character key/value store (`character_variables`
table). On top of it: premium accounts (+ `ExVitalityEffectInfo` bonuses),
PC-café points, prime points, full vitality (points ↔ level, peace-zone regen,
item consumption), henna/dye symbols on the character sheet. **Gate:** a
premium flag and vitality level survive relog; henna changes stats.
**Unblocks:** `//premium*`, `//pccafepoints`, `//primepoints`,
`//set_vitality_level`.

---

## Track B — Progression & clans

### G17 — Sub-classes, class change & nobless
Occupation change (1st/2nd/3rd) through the village-master flow; subclass
add/change/level with the class-skill retable; certification skills; nobless
status + tiara. The class-change *mechanic* + admin set can land before the
occupation *quests* (G22). **Gate:** a character changes class and gets the new
skill tree; a subclass can be added and switched. **Unblocks:** `//setnoble`,
fuller `//setclass`, `//setsubclass`.

### G18 — Clans (full)
Everything past G11's creation slice: invite/join/leave/oust/dissolve; clan
level-up + reputation; sub-pledges (royal guard / order of knights) + academy;
clan skills + `PledgeSkillList`; crests (pledge/ally/large); notices; clan
warehouse; clan wars; alliances; the `PledgeInfo`/`PledgeStatusChanged`/RELATION
breadth. **Gate:** form a clan, invite members, level it, learn a clan skill,
declare war, form an ally. **Unblocks:** `//clan_*`, `//pledge`,
`//add_clan_skill`/`//give_clan_skills`. **Deps:** G15 (clan warehouse).

---

## Track C — Combat, skills & AI breadth

### G19 — Skills & effects breadth
Grow `EFFECT_REGISTRY` toward the 369 Java effect classes and the 230-entry
`Stat` enum on demand; toggle-type skills; the remaining `AcquireSkillType`s
(pledge/transform/transfer/subclass/collect/…); `calcMagicSuccess`
(`ALT_GAME_MAGICFAILURES`); AoE affect scopes (only `SINGLE` resolves today);
buffs/effects on NPC targets; the **abnormal-visual-effect** runtime + per-
creature team / targetable / display-effect state; `ExAbnormalStatusUpdateFrom
Target`. **Gate:** a debuff lands on a mob, an AoE nuke hits a cluster, a toggle
skill switches on. **Unblocks:** the AdminEffects AVE subset (`//ave_abnormal`,
`//setteam`, `//settargetable`, `//para*`, `//bighead`, `//playmovie`,
`//set_displayeffect`, `//event_trigger`), `//switch_gm_buffs`.

### G20 — Combat breadth
`PhysicalAttack`-type skills; bows/crossbows (arrows, reuse gauge); dual-weapon
split hits; polearm sweep; PvP auto-attack + the karma/PK/flag consumers; overhit
XP; the `SHOTS_BONUS` dynamic value; the rest of `isMovementDisabled`
(root/immobilize). **Gate:** a bow attack consumes an arrow, a polearm hits a
line, PvP flagging drives auto-attack, a physical skill lands. **Deps:** G14, G19.

### G21 — NPC AI & world-content breadth
NPC skill casting (`AISkillScope` lists); minions; guard/faction/clan-help aggro
(needs karma); NPC pathfinding (chase/return-home + closest-reachable grid, the
G7.85 worker for NPCs) and NPC regen; ground drops + spoil/sweep; `DBSpawnManager`
persistence (raid HP across restart); `HtmCache`; walker routes; the other ~33
zone types (damage/effect/boss/jail/water-breath/no-store/arena…) + fence checks
+ the `ValidatePosition` door-exploit tail. **Gate:** a mob casts, a guard aggros
a PK, a spoiled corpse can be swept, a boss keeps its HP across restart.
**Deps:** G20.

---

## Track D — Content

### G22 — Quest & script breadth
The remaining ~188 quests, ~14 village-master scripts and ~81 `ai/` scripts;
daily quests (`restartTime`); the tutorial (Q00255); `onFirstTalk`; the
quest-window guards; `validateHtmlAction`; the remaining bypass families
(multisell/sell already partly in G15). Script hot-reload backs `//reload`.
**Gate:** the quest/AI parity checklist is green; a representative quest of each
kind (one-time, repeatable, daily, class-transfer, instance) completes.
**Unblocks:** `//quest_info`/`//quest_reload`/`//script_load`/`//script_unload`,
`//charquestmenu`/`//setcharquest`, `//reload`. **Deps:** G17, G19.

---

## Track E — End-game systems (each unblocks a C-group handler)

### G23 — Grand bosses & raid bosses
Boss zones + entry conditions; respawn windows (`GrandBossManager` /
`RaidBossSpawnManager`); boss AI (chaos target swaps, raid curse, minion waves);
raid points; DB persistence of boss state/HP. **Gate:** a raid boss spawns on
schedule, applies raid curse, and its state persists. **Unblocks:**
`//grandboss`. **Deps:** G21.

### G24 — Castles, sieges, clan halls & territory war
Castle ownership + taxes + functions; the siege engine (registration, siege
zones, control towers, flags, siege guards/mercenaries); fort sieges; clan-hall
auction + siege; territory war. **Gate:** a siege can be scheduled, fought, and
change castle ownership; a clan hall can be bought at auction. **Unblocks:**
AdminFortSiege (`//siege*`), `//castle`, `//clanhall`, territory war commands.
**Deps:** G18 (clans), G21.

### G25 — Olympiad & hero
Olympiad registration/matches/points/rank; the hero system (monthly heroes, hero
skills/weapons/aura, monument). **Gate:** register for Olympiad, run a match,
compute heroes at period end. **Unblocks:** AdminOlympiad, `//saveolymp`,
`//endolympiad`, `//sethero`/`//givehero`/`//settruehero`. **Deps:** G17
(nobless).

### G26 — Seven Signs, Manor & Mammon
Seven Signs cycle (competition/seal periods, Festival of Darkness) + its castle
and dungeon effects; the manor system (seed sowing, crop harvest, castle manor
production/procure); the Mammon merchants (Blacksmith/Merchant of Mammon).
**Gate:** a manor seed can be sown and harvested; the Seven Signs period
advances. **Unblocks:** `//manor`, `//mammon_find`/`//mammon_respawn`. **Deps:**
G24 (siege/castle tie-in), G15 (manor economy).

### G27 — Instances
`InstanceManager` + instance worlds; instance zones; reenter timers; instance-
scoped spawns/doors/reset; the party-enter flow. **Gate:** a party enters an
instance, clears it, and is bound by the reenter timer. **Unblocks:**
AdminInstance, AdminInstanceZone (`//instance*`, `//instancezone`). **Deps:** G21.

### G28 — Events engine & cursed weapons
The event framework (`AbstractEvent` + `EventManager`) with a representative
event (TvT); cursed weapons (Zariche/Akamanah) lifecycle via
`CursedWeaponsManager` (drop, pickup, transformation, karma, decay). **Gate:** a
TvT event runs start-to-finish; a cursed weapon can be dropped and equipped.
**Unblocks:** AdminEvents, `//tvt_*`, AdminCursedWeapons. **Deps:** G20.

---

## Track F — Social, comms, moderation & support

### G29 — Summons, pets, servitors, cubics, agathions
Summon skills + servitor AI; pets (summon items, food/feed, pet inventory,
persistence, evolution); cubics; agathions; the pet/servitor party-window
packets. **Gate:** summon a servitor that follows and attacks; summon a pet, feed
it, and it persists. **Unblocks:** AdminEditChar `//summon_info`/`//show_pet_inv`/
`//summon_setlvl`/`//unsummon`, `//fullfood`. **Deps:** G19, G20.

### G30 — Mail, community board & party matching
`MailManager` (compose/read/attachments/return); `communitybbs` (the BBS pages);
party matching rooms; command channels (MPCC); block list (wired into every
whisper/trade/invite check); tactical signs. **Gate:** send mail with an
attachment; open the community board; create a matching room. **Unblocks:**
AdminBBS. **Deps:** G18 (clan board).

### G31 — Moderation, accounts, petitions & HWID
Per-client IP plumbing (needed by several); punishment/jail (`PunishmentManager`
+ chat/jail/ban types) + say filter/chat bans; petitions (`PetitionManager`);
account/login admin control (the login-link `//setaccess`/ban relay, `//gm*`
account ops); HWID tracking; fake players. **Gate:** jail a player, file and
answer a petition, ban via the login link. **Unblocks:** AdminPunishment,
AdminLogin, AdminHwid, AdminPetition, AdminFakePlayers, and editchar
`//find_ip`/`//find_dualbox`/`//tracert`. **Deps:** IP plumbing.

### G32 — Fishing
Fishing skill + rods/lures/bait; the fishing minigame (`FishingManager`); fish
tables; the fishing championship. **Gate:** cast, hook, and land a fish.
**Deps:** G19.

---

## Track G — Finishing

### G33 — Misc parity & finishing sweep
The residuals: game-time clock (CharSelected/UserInfo use 0 today);
`AutoSaveManager` periodic save cadence; precautionary/scheduled restart +
deadlock detector; offline-trader restore; the `//geosave` binary-region
serializer; `NpcNameLocalisationData`/multilang; remaining packets and the last
data loaders; the niche admin tools (AdminFightCalculator, AdminRepairChar,
AdminPForge, AdminMissingHtmls, AdminPcCondOverride); Dockerfile parity. Close
with the file-by-file parity checklist ([PLAN_GAME_SERVER.md](PLAN_GAME_SERVER.md)
§8). **Gate:** parity checklist complete.

---

## Suggested sequencing

The tracks are ordered by leverage and dependency, but a few notes:

1. **G14 first, always.** Item `<stats>` unblock accurate combat, shields, sets,
   and the enchant/augment effects G15 needs — everything downstream is more
   faithful once it lands.
2. **G16 is cheap and unblocks 4 admin handlers** (premium/vitality/points) — a
   good quick win alongside G14/G15.
3. **G19 (effects breadth) is the long pole for combat and content** — grow it
   continuously; G20/G21/G22/G28/G29 all pull from it.
4. **End-game (G23–G28) can be reordered by product priority** — they're
   independent of each other (only shared dep is G18/G21). Pick by what the
   server operator wants live first (sieges vs olympiad vs instances).
5. **G31 needs IP plumbing** — a small cross-cut worth doing early if dualbox/
   moderation tooling is wanted sooner.
