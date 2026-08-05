# Porting status — what is ported, what is partial, what is not

One table for the whole Java→Rust port. It replaces the 172 per-feature
`PLAN_*.md` documents that used to live in this directory: those were written
*before* each feature was built, so once the feature landed they described the
past in the future tense. What survives them is here.

## How to read this

A milestone is marked ✅ when its **gate** was met and verified against the Java
server on the same database and client. That is not the same as "every line of
Java behaviour is reproduced" — each milestone shipped with a handful of narrow
behaviours deliberately skipped and marked at the exact site in the code with a
`TODO(G<N>)` comment naming the Java source. So there are three states, not two:

| State | Meaning |
|---|---|
| ✅ **Ported** | Gate met and verified. Any remaining gaps are recorded markers, listed in [DEFERRALS.md](DEFERRALS.md). |
| ◐ **Partial by design** | Same, but the recorded gaps are numerous or user-visible enough to call out here. |
| ⛔ **Out of scope** | Deliberately not ported. Reasons in [§ Out of scope](#out-of-scope). |

**Three sources of truth, in order of reliability.** Prose about what remains is
the least reliable artefact in this repo — it has drifted into fiction twice,
both times claiming work was outstanding that had in fact shipped. Trust, in
this order:

1. **The code.** `grep -rn "TODO(" crates/ --include='*.rs'`.
2. **[DEFERRALS.md](DEFERRALS.md)** — the marker inventory, generated from the
   code and held to it by the `deferral_markers_match_the_recorded_inventory`
   test in `crates/gameserver/src/data/skill_data/coverage_census.rs`. Adding a gap without
   recording it fails the build; closing one without updating the count fails
   too.
3. **[PROGRESS.md](PROGRESS.md)** — the dated journal of what landed and why.
   Narrative, not enforced.

This file is a snapshot; it is not machine-checked. Where it states a number,
the command that regenerates it is given.

---

## Subsystem status

Every milestone from G0 to G34 is ✅. The "recorded gaps" column is the count of
`TODO(G<N>)` markers attributed to that milestone — **not** a list of missing
features but of named, deliberate narrowings inside shipped ones.

Regenerate the counts with:

```sh
grep -rhoE 'TODO\([A-Za-z0-9][A-Za-z0-9._/+?-]*\)' crates/ --include='*.rs' | sort | uniq -c | sort -rn
```

### Login server

| Milestone | Subsystem | Status | Recorded gaps |
|---|---|---|---:|
| M0–M5 | Authentication, account handling, server registration, the game↔login link | ✅ Feature-complete, interop-verified against the unmodified Java **game** server | 0 |

### Game server — foundations

| Milestone | Subsystem | Status | Recorded gaps |
|---|---|---|---:|
| G0 | Scaffold & boot | ✅ | 0 |
| G1 | Client link & cipher parity | ✅ | 0 |
| G2 | Login-link + auth | ✅ | 0 |
| G3 | Character selection & persistence | ✅ | 0 |
| G4 | Enter world (Player, HP/MP, `UserInfo`, the enter-world burst) | ✅ | 0 |
| G5 | Items & inventory | ✅ | 0 |
| G6 | Stats, skills & effects (engine) | ✅ | 0 |
| G7 | Movement & targeting | ✅ | 1 |
| G7.5 | Full single-target skill casting | ✅ | 2 |
| G7.8 | Geodata & position validation | ✅ | 0 |
| G7.85 | Pathfinding (dedicated path-worker thread) | ✅ | 0 |
| G7.9 | Region-grid visibility & scoped broadcasting | ✅ | 0 |
| G8 | Static world content — 34.9k NPC spawns | ✅ | 0 |
| G9 | Combat & AI | ✅ | 0 |
| G9.5 | ECS stage 2 — split components, one world | ✅ | 0 |
| G9.6 | Macros & panel shortcuts | ✅ | 0 |
| G10 | Social systems — chat, party, friends | ✅ | 0 |
| G11 | Scripting engine + quests | ✅ | 0 |
| G12 | Static world breadth — zones, all 1180 doors, static objects | ✅ | 0 |
| G13 | Admin / GM command system | ✅ **361 of 443** `AdminCommands.xml` commands dispatched; the 82 absent are all off-chronicle, dev tooling, architecturally N/A, or unreachable in Java too — see below | 1 |
| G13.9 | TODO parity sweep | ✅ | 0 |

### Game server — breadth

| Milestone | Subsystem | Status | Recorded gaps |
|---|---|---|---:|
| G14 | Item stats & equipment combat accuracy | ✅ | 2 |
| G15 | Economy & item actions | ✅ | 2 |
| G15.5 | Teleporters & user commands | ✅ | 1 |
| G15.7 | Crafting & recipes | ✅ | 0 |
| G16 | Character variables, premium & vitality | ✅ | 0 |
| G17 | Sub-classes, class change & nobless | ✅ | 2 |
| G18 | Clans — full | ✅ | 2 |
| G18.6 | Clan academy | ✅ | 2 |
| G19 | Skills & effects breadth | ✅ | 11 |
| G20 | Combat breadth | ✅ | 5 |
| G20.5 | Recommendations | ✅ | 0 |
| G21 | NPC AI & world-content breadth | ✅ | 8 |
| G22 | Quest & script breadth | ✅ | 11 |
| G23 | Grand bosses & raid bosses (all 10) | ✅ | 5 |
| G24 | Castles, sieges & clan halls | ◐ Sieges, castles and clan halls are live; **castle crests, castle functions and territory war are not** | 14 |
| G24.5 | Boats | ✅ | 1 |
| G25 | Olympiad & hero | ✅ | 1 |
| G26 | Manor & Mammon | ✅ — **Seven Signs does not exist in this dist**; the Interlude Classic build drops the subsystem entirely (no Java class survives), so there was nothing to port | 0 |
| G26.5 | Lottery & Monster Race | ✅ | 2 |
| G27 | Instances | ✅ Engine complete (Olympiad arenas, Frintezza's tomb) | 4 |
| G28 | Events engine & cursed weapons | ✅ | 9 |
| G29 | Summons, pets, servitors, cubics | ✅ | 4 |
| G30 | Mail, community board & party matching | ◐ Board home/buffer/gatekeeper/premium land; the **forum boards** (`_bbstop`/post/region/notice) are not ported | 12 |
| G30.5 | Item auction | ✅ | 0 |
| G31 | Moderation, accounts, petitions & HWID | ✅ | 0 |
| G32 | Fishing | ✅ | 1 |
| G33 | Misc parity & finishing sweep | ✅ All four named slices, plus the `Custom/*.ini` audit below | 15 |
| G34 | Skills, effects & abnormal-state parity (epic) | ✅ **CLOSED** — the skill parser was fail-open; a wrong-behaviour census of every learnable skill went **275 → 11 of 758**, and each of the 11 is a recorded, named out-of-scope item | 13 |

The per-milestone column above counts only tags of the exact form `TODO(G<N>)`.
Two more milestone markers are not milestone-*scoped* — `TODO(G-pvp)` (3) and
`TODO(G-later)` (1) — and a further nine use a `+`, `?` or `/` suffix (`G9+`,
`G13+`, `G19+`, `G21+` ×2, `G29+` ×2, `G?`, `G24/G26`) that this column does
not break out. Alongside them sit **3 topic-tagged** markers (`pets`,
`manor`, `newbie-guide`, `login-playauth`, …) which belong to no milestone at
all. A separate `SKIP(<tag>)` family marks work examined and deliberately not
done — dead Java no route on this dist can reach — and is deliberately *not*
counted here; see [DEFERRALS.md](DEFERRALS.md).

**Total recorded gaps: 134.** Enumerated in [DEFERRALS.md](DEFERRALS.md) and
asserted by the test named above — if this number and that file disagree, the
file is right. It read 134 until 2026-08-05, when the scanner was widened to
see the suffixed and topic-tagged families it had been dropping in silence; see
that file's seventh pass. Because every milestone row here is ✅, these 134 are
what is actually left to do.

---

## Out of scope

Not gaps. These were decided against, and the decision is not expected to be
revisited.

| Not ported | Why |
|---|---|
| Gracia / Hellbound content, elemental item attributes, `AdminGraciaSeeds`, `AdminElement` | Kamael-era content. This is an **Interlude** server. |
| Sayune, shuttles, airships | Same — post-Interlude travel systems. |
| Fort sieges, territory war, siegable clan halls, fences | Off-chronicle for this build. |
| MariaDB / PostgreSQL backends | The Java dist ships SQLite here; one backend, one dialect. |
| The Java Swing server UI | A GUI on a headless server process. Dropped by decision #10 (see [JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md)). |
| Java's `tools/` tree | Replaced, not ported — see [`crates/tools`](../crates/tools/README.md), which answers datapack questions by calling the server's own geo engine. |
| Runtime script loading (`//script_load`, `//quest_reload`, `//script_dir`) | Architecturally N/A: the port compiles its scripts in, so there is no runtime loader to drive. |
| 11 named learnable skills (G34's residue) | Each recorded with its reason at the close of the G34 epic. |

**Settled: the Mobius `Custom/*.ini` scope gate.** The roadmap deferred these as
out of scope "except any the operator explicitly enables". The audit ran, and
the answer is that **all 17 features this dist enables are ported, consumed and
tested** — not 16, a figure two documents drifted to and one re-introduced from
memory. The three whose consumers are least obvious, recorded so the next audit
need not re-derive them: L2Walker protection in `game_loop/chat.rs`, the
private-store spacing rule in `game_loop/private_store.rs`, and the boss spawn
announcement in `model/npc.rs`. One file, `Custom/PcCafe.ini`, ships and looks
authoritative but is **dead in Java itself** — `Config.java` never opens it; the
live PC-cafe keys are in `PremiumSystem.ini`. A shipped ini proves nothing on
its own; check that Java parses *and* consumes it before calling it a gap.

**Known forward decision: the dashboard coin shop.** Deliberately out of the
dashboard's v1 because it cannot be built inside that design's two-table
constraint — it needs a wallet plus an append-only ledger, a delivery queue the
game server drains through its *normal* item-add path (writing items straight
into the DB is clobbered by autosave), and an idempotency key so a retried
delivery is a no-op. When it lands, the single-database decision has to be
revisited; adding the tables to the game DB is preferred, because the debit and
the enqueue then commit in one transaction. Full reasoning in
[DASHBOARD.md](DASHBOARD.md) §7.

---

## Retired plans

The 172 planning documents below were deleted once their work shipped. Nothing
is lost — each is one `git show` away, and the record of what actually landed
(which is not always what the plan proposed) is in [PROGRESS.md](PROGRESS.md),
dated and keyed by the same milestone.

```sh
# find the commit that deleted a plan, then read it at its last living revision
git log --diff-filter=D --format=%H -1 -- docs/PLAN_G19_FEAR.md
git show <sha>^:docs/PLAN_G19_FEAR.md
```

One plan was kept rather than retired: [DASHBOARD.md](DASHBOARD.md) (formerly
`PLAN_DASHBOARD.md`) is the design reference for a subsystem that is still
evolving, and code comments point at it.

| Retired plan | Subject | Milestone |
|---|---|---|
| `PLAN_LOGIN_SERVER.md` | Implementation Plan — Login Server | M0–M5 (master) |
| `PLAN_GAME_SERVER.md` | Implementation Plan — Game Server | G0–G13 (master) |
| `PLAN_ECS_STAGE2.md` | ECS stage 2: split components, one world | G9.5 |
| `PLAN_MACROS_SHORTCUTS.md` | Macros & panel shortcuts | G9.6 |
| `PLAN_G10_SOCIAL.md` | Social systems (vertical slice: chat + party + friends) | G10 |
| `PLAN_G11_QUESTS_CLANS.md` | Scripting engine + quests (+ clans via bypass) | G11 |
| `PLAN_G12_STATIC_WORLD_AND_CONTENT_BREADTH.md` | Static world (zones & doors) + script/content breadth (framework extension) | G12 |
| `PLAN_G13_ADMIN.md` | Admin / GM command system | G13 |
| `PLAN_G13_B_LOGIN.md` | G13.B-login — GM login state, hero aura & admin menu | G13 |
| `PLAN_BUFF_PERSISTENCE.md` | Buff persistence across relog (`character_skills_save`, `restore_type = 0`) | G13.9 |
| `PLAN_G13_9_TODO_SWEEP.md` | TODO Parity Sweep | G13.9 |
| `PLAN_G15_7_CRAFTING.md` | Crafting & recipes | G15.7 |
| `PLAN_G16_ADMIN_POINTS.md` | Main-menu admin commands: points, premium & spawn lists | G16 |
| `PLAN_G16_HENNA.md` | Henna / dye symbols | G16 |
| `PLAN_G16_VITALITY.md` | Character variables, vitality & premium effects | G16 |
| `PLAN_G17_NOBLESS.md` | nobless | G17 |
| `PLAN_G17_OCCUPATION_CHANGE.md` | occupation change (`Player.setClassId`) | G17 |
| `PLAN_G17_SKILL_COOLDOWNS.md` | skill cooldowns per class index | G17 |
| `PLAN_G17_SUBCLASSES.md` | subclasses | G17 |
| `PLAN_G17_SUBCLASS_HENNA_SHORTCUTS.md` | per-subclass hennas and shortcuts | G17 |
| `PLAN_G17_SUBCLASS_SKILLS.md` | per-subclass skill books | G17 |
| `PLAN_G17_VILLAGE_MASTER_SUBCLASS.md` | the village-master subclass flow | G17 |
| `PLAN_G18_CLANS.md` | G18: Clans (full) | G18 |
| `PLAN_G19_ABNORMAL_RESIST.md` | Abnormal resistance, blocking & probabilistic dispel | G19 |
| `PLAN_G19_ABNORMAL_STATES.md` | Abnormal-state flags & crowd control | G19 |
| `PLAN_G19_ABNORMAL_VISUALS.md` | Abnormal visual effects | G19 |
| `PLAN_G19_ATTACK_TRAIT.md` | AttackTrait skill effect | G19 |
| `PLAN_G19_ATTRIBUTES.md` | Elemental attributes (`calcAttributeBonus`) | G19 |
| `PLAN_G19_CC_BREADTH.md` | CC breadth: mute, debuff-block, control-block, target-cancel | G19 |
| `PLAN_G19_CONFUSE.md` | Confuse + RandomizeHate | G19 |
| `PLAN_G19_CRITICAL_DAMAGE.md` | Critical damage stats | G19 |
| `PLAN_G19_DAMAGE_BLOCK.md` | DamageBlock skill effect | G19 |
| `PLAN_G19_DEFENCE_CRITICAL_RATE.md` | DefenceCriticalRate | G19 |
| `PLAN_G19_DISPEL_CATEGORY.md` | DispelByCategory skill effect (the "Cancel" family) | G19 |
| `PLAN_G19_EFFECTS.md` | Skills & effects breadth: affect scopes & toggles | G19 |
| `PLAN_G19_EFFECT_LEVEL_GATING.md` | Per-effect level gating (`fromLevel`/`toLevel`/`subLevel`) | G19 |
| `PLAN_G19_EFFECT_SCOPES.md` | Effect scopes (`<selfEffects>`, `<pveEffects>`, `<pvpEffects>`) | G19 |
| `PLAN_G19_ENEMY_NOT_TARGET.md` | TargetType::EnemyNot | G19 |
| `PLAN_G19_ENLARGE_SLOT.md` | EnlargeSlot skill effect | G19 |
| `PLAN_G19_FATAL_BLOW_RATE.md` | FatalBlowRate skill effect | G19 |
| `PLAN_G19_FEAR.md` | Fear skill effect | G19 |
| `PLAN_G19_FORCE_CHARGES.md` | Force/charges resource (FocusMomentum + EnergyAttack) | G19 |
| `PLAN_G19_GEOMETRIC_SCOPES.md` | Geometric affect scopes: FAN / FAN_PB / SQUARE / SQUARE_PB / RING_RANGE | G19 |
| `PLAN_G19_GROUND_CHANNELING.md` | GROUND casts + skill channeling (Volcano family) | G19 |
| `PLAN_G19_HATE_EFFECTS.md` | hate-manipulation skill effects (GetAgro/AddHate/DeleteHate/DeleteHateOfMe) | G19 |
| `PLAN_G19_HEAL_PERCENT.md` | HealPercent skill effect | G19 |
| `PLAN_G19_LETHAL.md` | Lethal skill effect | G19 |
| `PLAN_G19_MAGICAL_ATTACK_MP.md` | MagicalAttackMp (MP drain) | G19 |
| `PLAN_G19_MANA_RESTORE.md` | MP restoration family | G19 |
| `PLAN_G19_MP_CONSUME_PER_LEVEL.md` | MpConsumePerLevel skill effect | G19 |
| `PLAN_G19_PERIODIC_EFFECTS.md` | Periodic HP/MP effects, healing modifiers & CP | G19 |
| `PLAN_G19_PHYSICAL_ATTACK_RANGE.md` | PhysicalAttackRange skill effect | G19 |
| `PLAN_G19_REFLECT_BLOCKMOVE.md` | ReflectSkill + BlockMove | G19 |
| `PLAN_G19_RESIST_DD_MAGIC.md` | ResistDDMagic (MAGIC_SUCCESS_RES) | G19 |
| `PLAN_G19_RESURRECTION.md` | Resurrection | G19 |
| `PLAN_G19_SHIELD_DEFENCE.md` | ShieldDefence / ShieldDefenceRate skill effects | G19 |
| `PLAN_G19_SILENT_MOVE_FAKE_DEATH.md` | SilentMove + FakeDeath | G19 |
| `PLAN_G19_SKILL_ENCHANT.md` | Skill enchanting | G19 |
| `PLAN_G19_STAT_BY_MOVE_TYPE.md` | StatByMoveType + the player regen stat pipeline | G19 |
| `PLAN_G19_SYMBOLS.md` | SummonNpc symbols (EffectPoint totems) | G19 |
| `PLAN_G19_TRANSFORMATION.md` | Transformation skill effect | G19 |
| `PLAN_G19_TRIGGER_SKILL_BY_ATTACK.md` | TriggerSkillByAttack | G19 |
| `PLAN_G19_TWO_HANDED_BONUS.md` | TwoHandedBluntBonus / TwoHandedSwordBonus | G19 |
| `PLAN_G20_DEATH_DROPS.md` | Death item drops (the karma penalty) | G20 |
| `PLAN_G20_DUELS.md` | Duels (1v1) | G20 |
| `PLAN_G20_MELEE_VARIANTS.md` | Multi-hit melee: dual weapons and the polearm sweep | G20 |
| `PLAN_G20_OVERHIT.md` | Over-hit: bonus XP for overshooting a killing blow | G20 |
| `PLAN_G20_PVP_KILLS.md` | PvP kill consequences: counters, karma and zone exemptions | G20 |
| `PLAN_G20_RANGED.md` | Ranged attacks: bows, crossbows and ammunition | G20 |
| `PLAN_G21_BOSS_PERSISTENCE.md` | raid-boss persistence (`DBSpawnManager`) | G21 |
| `PLAN_G21_DAMAGE_SWAMP_ZONES.md` | `DamageZone` + `SwampZone` | G21 |
| `PLAN_G21_EFFECT_ZONES.md` | `EffectZone` + per-zone `type=` parsing | G21 |
| `PLAN_G21_GUARD_AGGRO.md` | town-guard PK aggro + faction help calls | G21 |
| `PLAN_G21_MINIONS.md` | minions (`MinionList`) | G21 |
| `PLAN_G21_NPC_CASTING.md` | NPC skill casting | G21 |
| `PLAN_G21_NPC_PATHFINDING.md` | NPC pathfinding | G21 |
| `PLAN_G21_NPC_REGEN.md` | NPC HP/MP regeneration | G21 |
| `PLAN_G21_NPC_SKILL_REUSE.md` | NPC skill cooldowns never applied (G21 bug, found by the G29 sweep) | G21 |
| `PLAN_G21_TARGET_RECONSIDER.md` | `skillTargetReconsider` (support mobs help their pack) | G21 |
| `PLAN_G21_WALKER_ROUTES.md` | NPC walking routes | G21 |
| `PLAN_G22_AI_OTHERS.md` | G22 `ai/others` NPC scripts (remaining-ports audit row 5) | G22 |
| `PLAN_G22_ALLIANCE_MASTER.md` | AllianceMaster, closing the village-master group | G22 |
| `PLAN_G22_DARK_ELF_CLASS_TRANSFER.md` | Dark Elf first-class transfer + a second class-corruption fix | G22 |
| `PLAN_G22_DWARF_CLASS_TRANSFER.md` | Dwarf first-class transfers | G22 |
| `PLAN_G22_DWARF_SECOND_CLASS.md` | Dwarf second-class transfers | G22 |
| `PLAN_G22_ELF_HUMAN_CLASS_TRANSFER.md` | Elf/Human first-class transfers | G22 |
| `PLAN_G22_ELF_HUMAN_SECOND_CLASS.md` | Elf/Human second-class transfers | G22 |
| `PLAN_G22_ELVEN_PATH_QUESTS.md` | the Elven first-occupation quests | G22 |
| `PLAN_G22_FIRST_CLASS_TRANSFER_TALK.md` | FirstClassTransferTalk | G22 |
| `PLAN_G22_ORC_DARKELF_SECOND_CLASS.md` | Orc and Dark Elf second-class transfers | G22 |
| `PLAN_G22_PATH_ARTISAN.md` | Path of the Artisan | G22 |
| `PLAN_G22_PATH_DARKELF_1.md` | Path of the Palus Knight / Path of the Assassin | G22 |
| `PLAN_G22_PATH_DARKELF_2.md` | Path of the Dark Wizard / Path of the Shillien Oracle | G22 |
| `PLAN_G22_PATH_ELVEN_ORACLE.md` | Path of the Elven Oracle | G22 |
| `PLAN_G22_PATH_ELVEN_WIZARD.md` | Path of the Elven Wizard | G22 |
| `PLAN_G22_PATH_HUMAN_KNIGHT.md` | Path of the Human Knight | G22 |
| `PLAN_G22_PATH_ORC_MONK.md` | Path of the Orc Monk | G22 |
| `PLAN_G22_PATH_ORC_RAIDER.md` | Path of the Orc Raider | G22 |
| `PLAN_G22_PATH_ORC_SHAMAN.md` | Path of the Orc Shaman | G22 |
| `PLAN_G22_PATH_SCAVENGER.md` | Path of the Scavenger (the last Path quest) | G22 |
| `PLAN_G22_PATH_WARRIOR_ROGUE.md` | Path of the Warrior / Path of the Rogue | G22 |
| `PLAN_G22_PATH_WIZARD_CLERIC.md` | Path of the Human Wizard / Path of the Cleric | G22 |
| `PLAN_G22_WOLF_PET.md` | PLAN G22 — Q00210 Obtain a Wolf Pet | G22 |
| `PLAN_G23_ANTHARAS.md` | Antharas's minion waves | G23 |
| `PLAN_G23_ANTHARAS_CINEMATIC.md` | Antharas's entry cinematic | G23 |
| `PLAN_G23_ANTHARAS_DEATH_TAIL.md` | PLAN G23 — Antharas death tail (exit cube + zone cleanup) | G23 |
| `PLAN_G23_ANTHARAS_ENTRY.md` | Antharas's entry flow wired (Heart of Warding → WAITING → spawn) | G23 |
| `PLAN_G23_ANTHARAS_GATE.md` | Antharas's entry gate | G23 |
| `PLAN_G23_ANTHARAS_SKILLS.md` | Antharas's skill ladder, and the caller neither boss had | G23 |
| `PLAN_G23_BAIUM.md` | Baium (archangels + strider debuff) | G23 |
| `PLAN_G23_BAIUM_SKILLS.md` | Baium's skill selection | G23 |
| `PLAN_G23_BAIUM_THREAT.md` | Baium's threat table | G23 |
| `PLAN_G23_BOSS_BARKS.md` | boss barks | G23 |
| `PLAN_G23_BOSS_IDS.md` | the boss-id audit | G23 |
| `PLAN_G23_CORE.md` | Core | G23 |
| `PLAN_G23_DR_CHAOS.md` | Dr. Chaos (the paranoia transformation) | G23 |
| `PLAN_G23_GRANDBOSS_LIFECYCLE.md` | the grand-boss respawn lifecycle | G23 |
| `PLAN_G23_ORFEN.md` | Orfen (and Zaken for free) | G23 |
| `PLAN_G23_QUEEN_ANT.md` | Queen Ant | G23 |
| `PLAN_G23_RAID_CURSE.md` | the raid curse | G23 |
| `PLAN_G23_RAID_POINTS.md` | raid points | G23 |
| `PLAN_G23_SCRIPT_ZONES.md` | `ScriptZone` support | G23 |
| `PLAN_G23_SPECIAL_CAMERA.md` | `SpecialCamera` | G23 |
| `PLAN_G23_VALAKAS.md` | Valakas (attack rules) | G23 |
| `PLAN_G23_VALAKAS_CINEMATIC.md` | Valakas's entry cinematic | G23 |
| `PLAN_G23_VALAKAS_DEATH_TAIL.md` | PLAN G23 — Valakas death tail (exit cubes + zone clear) | G23 |
| `PLAN_G23_VALAKAS_ENTRY.md` | Valakas's entry flow wired (Klein → Heart of Volcano → cinematic) | G23 |
| `PLAN_G24_SIEGE_SCHEDULE.md` | the automatic siege schedule | G24 |
| `PLAN_G25_OLYMPIAD.md` | G25 Grand Olympiad & Hero | G25 |
| `PLAN_G26_5_LOTTERY_RACE.md` | G26.5 Lottery & Monster Race | G26.5 |
| `PLAN_FRINTEZZA.md` | Frintezza (Last Imperial Tomb) instanced encounter | G27 |
| `PLAN_G27_INSTANCES.md` | G27 Instances | G27 |
| `PLAN_G28_CURSED_WEAPONS.md` | PLAN G28 — Cursed weapons: the autonomous gameplay loop | G28 |
| `PLAN_G28_EVENTS_ENGINE.md` | G28 Events engine (TvT), the second half of G28 | G28 |
| `PLAN_G29_ACTING_PLAYER_AUDIT.md` | the `getActingPlayer()` audit, part 2 | G29 |
| `PLAN_G29_ACTING_PLAYER_AUDIT_3.md` | closing the `getActingPlayer()` audit | G29 |
| `PLAN_G29_CLIENT_GAPS.md` | features that landed but never reached the client | G29 |
| `PLAN_G29_CUBICS.md` | cubics | G29 |
| `PLAN_G29_PET_DATA.md` | PetData (the pet-template foundation) | G29 |
| `PLAN_G29_PET_DEATH.md` | pet death | G29 |
| `PLAN_G29_PET_DECAY.md` | pet corpse decay | G29 |
| `PLAN_G29_PET_EQUIP.md` | pet equipment | G29 |
| `PLAN_G29_PET_EXP.md` | pet experience and levelling | G29 |
| `PLAN_G29_PET_FEEDING.md` | pet feeding | G29 |
| `PLAN_G29_PET_PERSISTENCE.md` | pet persistence | G29 |
| `PLAN_G29_PET_REGEN.md` | pet regeneration | G29 |
| `PLAN_G29_PET_REVIVE.md` | pet resurrection | G29 |
| `PLAN_G29_PET_SHOTS.md` | summon shots (Beast Soulshot) | G29 |
| `PLAN_G29_PET_STATS.md` | per-level pet stats | G29 |
| `PLAN_G29_PET_SUMMON.md` | Pet summoning | G29 |
| `PLAN_G29_RECONNECT_RESUMMON.md` | a pet that was out at logout comes back | G29 |
| `PLAN_G29_SERVITOR_AI.md` | Servitor follow & attack | G29 |
| `PLAN_G29_SERVITOR_LIFECYCLE.md` | Servitor lifecycle (upkeep, expiry, leash, logout) | G29 |
| `PLAN_G29_SERVITOR_RECONNECT.md` | servitor reconnect | G29 |
| `PLAN_G29_SERVITOR_SKILL_USE.md` | `ServitorSkillUse` | G29 |
| `PLAN_G29_SERVITOR_SUMMON.md` | Servitor summoning & lifecycle | G29 |
| `PLAN_G29_SUMMON_BUFF_INFO.md` | summon buffs reach the client | G29 |
| `PLAN_G29_SUMMON_BUFF_PERSIST.md` | a servitor's buffs survive a relog | G29 |
| `PLAN_G29_SUMMON_INFO.md` | SummonInfo (other players can see a servitor) | G29 |
| `PLAN_G29_SUMMON_KILL_CREDIT.md` | a summon's kill credits its owner | G29 |
| `PLAN_G29_SUMMON_PVP_FLAG.md` | a summon's attack flags its owner | G29 |
| `PLAN_G29_SUMMON_SPIRITSHOTS.md` | summon spiritshots (G29 complete) | G29 |
| `PLAN_G29_SUMMON_TARGET.md` | the `SUMMON` target type | G29 |
| `PLAN_G30_MAIL_PARTY_MATCHING.md` | Mail & Party Matching (the milestone's missing half) | G30 |
| `PLAN_G30_5_ITEM_AUCTION.md` | G30.5 Item Auction | G30.5 |
| `PLAN_G31_MODERATION.md` | G31 Moderation, accounts, petitions & HWID | G31 |
| `PLAN_G33_AUTO_PLAY.md` | auto play (`Custom/AutoPlay.ini`) | G33 |
| `PLAN_G33_CUSTOM_INI_AUDIT.md` | the `Custom/*.ini` enable-flag audit | G33 |
| `PLAN_G33_MISC_PARITY.md` | G33 Misc parity & finishing sweep | G33 |
| `PLAN_SHADOW_WEAPONS.md` | Shadow Weapon Exchange Coupons + shadow-item mana ✅ (2026-08-01) | G33 |
| `PLAN_G34_SKILL_PARITY.md` | PLAN_G34 — Skills, effects & abnormal-state parity (epic) | G34 |
| `PLAN_ORM_MIGRATION.md` | SeaORM 2 migration (models crate + Rust migrations + DAO layer) | infra |
