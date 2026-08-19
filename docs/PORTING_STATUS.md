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
| ✅ **Ported** | Gate met and verified. Any remaining gaps are `TODO(<tag>)` markers in the source — there are currently none. |
| ◐ **Partial by design** | Same, but the recorded gaps are numerous or user-visible enough to call out here. **No row is in this state today** — both that were, G24 and G30, turned out to be describing work that had since landed. |
| ⛔ **Out of scope** | Deliberately not ported. Reasons in [§ Out of scope](#out-of-scope). |

**Three sources of truth, in order of reliability.** Prose about what remains is
the least reliable artefact in this repo — it has drifted into fiction twice,
both times claiming work was outstanding that had in fact shipped. Trust, in
this order:

1. **The code.** `grep -rn "TODO(" crates/ --include='*.rs'`.
2. **`deferral_markers_match_the_recorded_inventory`** in
   `crates/tools/tests/coverage_census.rs` — the marker inventory itself, as an
   assertion rather than a document. Adding a gap without recording it there
   fails the build; closing one without taking it off fails too. Its expected
   list held exactly one entry as of 2026-08-18: `antharas-cc`.
3. **[PROGRESS.md](PROGRESS.md)** — the dated journal of what landed and why.
   Narrative, not enforced.

All three answer "what did we *mark*", and none answers "what did we *miss*".
[§ Measured gaps](#measured-gaps--the-axes-nothing-above-measures) is the
counterweight: a set-difference against the Java tree on the axes none of them
reach — client opcodes, the action bar, the handler families, zone types,
datapack files, quest registration and config keys. Read a ✅ below as "the
gate was met and nothing is marked outstanding", not as "nothing is missing".

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
| G7 | Movement & targeting | ✅ | 0 |
| G7.5 | Full single-target skill casting | ✅ | 0 |
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
| G13 | Admin / GM command system | ✅ `AdminCommands.xml` commands dispatched; the absent ones are off-chronicle, dev tooling, architecturally N/A, or **unreachable in Java too** — a claim the measured-gaps audit disputed and then confirmed, see [row 16](#measured-gaps--the-axes-nothing-above-measures) | 0 |
| G13.9 | TODO parity sweep | ✅ | 0 |

### Game server — breadth

| Milestone | Subsystem | Status | Recorded gaps |
|---|---|---|---:|
| G14 | Item stats & equipment combat accuracy | ✅ | 0 |
| G15 | Economy & item actions | ✅ | 0 |
| G15.5 | Teleporters & user commands | ✅ | 0 |
| G15.7 | Crafting & recipes | ✅ | 0 |
| G16 | Character variables, premium & vitality | ✅ | 0 |
| G17 | Sub-classes, class change & nobless | ✅ | 0 |
| G18 | Clans — full | ✅ | 0 |
| G18.6 | Clan academy | ✅ | 0 |
| G19 | Skills & effects breadth | ✅ | 0 |
| G20 | Combat breadth | ✅ | 0 |
| G20.5 | Recommendations | ✅ | 0 |
| G21 | NPC AI & world-content breadth | ✅ | 0 |
| G22 | Quest & script breadth | ✅ | 0 |
| G23 | Grand bosses & raid bosses (all 10) | ✅ | 0 |
| G24 | Castles, sieges & clan halls | ✅ Sieges, castles and clan halls are live, and so are the two things this row long claimed were missing: castle crests (`Castle.showNpcCrest`, `siege::set_show_npc_crest`) and the rentable castle functions (`World.castle_functions` + the chamberlain console). **Territory War is not ported and will not be** — it is off-chronicle for Interlude; see § Out of scope | 0 |
| G24.5 | Boats | ✅ | 0 |
| G25 | Olympiad & hero | ✅ | 0 |
| G26 | Manor & Mammon | ✅ — **Seven Signs does not exist in this dist**; the Interlude Classic build drops the subsystem entirely (no Java class survives), so there was nothing to port | 0 |
| G26.5 | Lottery & Monster Race | ✅ | 0 |
| G27 | Instances | ✅ Engine complete (Olympiad arenas, Frintezza's tomb) | 0 |
| G28 | Events engine & cursed weapons | ✅ | 0 |
| G29 | Summons, pets, servitors, cubics | ✅ | 0 |
| G30 | Mail, community board & party matching | ✅ Mail, party matching and the board all land, the retail forum boards included — ported **to the reference's own depth**, which is shallow: Java itself leaves the `_bbsloc` detail, the Mail/Memo/Friends writes and `getRegionCount` as `// TODO: Implement.`, so the port's silences on those are parity, not gaps | 0 |
| G30.5 | Item auction | ✅ | 0 |
| G31 | Moderation, accounts, petitions & HWID | ✅ | 0 |
| G32 | Fishing | ✅ | 0 |
| G33 | Misc parity & finishing sweep | ✅ All four named slices, plus the `Custom/*.ini` audit below | 0 |
| G34 | Skills, effects & abnormal-state parity (epic) | ✅ **CLOSED** — the skill parser was fail-open; a wrong-behaviour census of every learnable skill went **275 → 11 of 758**, and each of the 11 is a recorded, named out-of-scope item | 0 |

**Total recorded gaps: 1.** Every column above reads 0 because the milestone
sweep closed everything it had marked; the one entry since added belongs to no
milestone. `TODO(antharas-cc)` — the Antharas entry gate reads only the
*party*, while the doc that used to sit there described the command channel
outranking it. **It was found by deduplication, not by a sweep**: the body was
byte-identical to Sailren's honestly-party-only twin, and the claim lived in
prose, where nothing measures. The sweep that began at 180 markers on 2026-08-03 closed the last two on
2026-08-07 — the block list and `Say2`'s jail gate, both opened days earlier by
the world-chat port.

That number counts **markers**, not gaps. A marker inventory can only see work
somebody stopped to mark; what a milestone never looked at is invisible to it,
and to the census beside it. See
[§ Measured gaps](#measured-gaps--the-axes-nothing-above-measures) for a
set-difference audit of the axes neither one covers.

The count is still **enforced**, by
`deferral_markers_match_the_recorded_inventory` in
`crates/tools/tests/coverage_census.rs`, which expects that one entry: a new
`TODO(<tag>)` anywhere under `crates/` fails the build until someone records it
there deliberately. The scan is total — every parseable tag counts, whether
milestone-scoped (`G24`), suffixed (`G9+`, `G24/G26`) or topic-tagged (`pets`,
`manor`). It reported 134 against a real 180 until 2026-08-05, when the scanner
was widened to see the families it had been silently dropping, which is exactly
why the test rather than prose is the authority.

A separate `SKIP(<tag>)` family marks work examined and deliberately *not* done
— dead Java that no route on this dist can reach — and is not counted.

`docs/DEFERRALS.md` used to enumerate the markers alongside a log of how each
was closed. With the inventory empty it had nothing left to list, and it was
deleted on 2026-08-07; recover it the way any retired plan is recovered:

```sh
git log --diff-filter=D --format=%H -1 -- docs/DEFERRALS.md
git show <sha>^:docs/DEFERRALS.md
```

---

## Measured gaps — the axes nothing above measures

**Audited 2026-08-14.** The marker inventory is empty and
`datapack_skill_coverage_census` holds the skill axis to a named residue. Both
are real; neither is a statement about the port as a whole. The census measures
**skills only**, and markers only exist where someone chose to leave one — so
between them they say nothing about client opcodes, the action bar, the handler
families in `dist/game/data/scripts/handlers`, zone types, datapack files,
quest registration or config keys.

This section is that other half: a set-difference of the Java tree against the
port across those axes. It is a snapshot, like the rest of this file, and every
number below names the command that re-derives it in
[§ Re-deriving these numbers](#re-deriving-these-numbers).

Two mechanisms produced most of the list, and both failed **silently** — which
is why none of it ever became a marker:

- **`RequestActionUse` was an allow-list** (`game_loop/servitor/ai.rs`). An
  action id that was not on it returned without a word: no packet, no log line,
  no `TODO`. Its own doc comment said so — *"Other action ids (sit, socials,
  the per-summon skill buttons) are not handled here yet"*. **Fixed on
  2026-08-14**: dispatch is now table-driven off `ActionData.xml` in
  `game_loop/player_actions.rs`, and a handler with no arm logs Java's own
  "couldn't find handler with name" rather than vanishing. Every in-chronicle
  handler behind it landed over 2026-08-14/15 (rows 1, 2 and 3); 13 rows remain,
  all post-Interlude.
- **The config layer reads about half the keys the dist ships.** An unread key
  is not a parse error; it is a value an operator sets in `dist/game/config`
  that nothing consults. Several are instead hardcoded to this dist's value
  inside a comment quoting the key — behaviourally identical *for the shipped
  config*, and inert for anyone who edits it.

### In-chronicle and reachable

The **`#` column is a stable id, not a position**: a row that closes leaves its
number behind rather than renumbering the rest, so the cross-references in
[§ Re-deriving these numbers](#re-deriving-these-numbers) keep pointing at the
same work. Closed rows move to [§ Closed](#closed).

| # | Area | Gap | Evidence | Effect in game |
|---|---|---|---|---|
| 16 | Admin commands | **76** of 458 absent (case-insensitively), and the earlier "~10 against ported systems" was wrong — see below. What is left needs machinery the port does not model: `delete_group` (spawn-territory groups), `instance_spawns`, `event_bypass` (Java routes it into an `Event` *quest script*; the port's events are not scripts), and `instancezone`/`_clear` (whose table is permanently empty on this dist — see `user_commands::instance_zone`) | a diff of `AdminCommands.xml` against the port's dispatch, then each survivor against Java's own registered handlers | Four GM commands, none of them player-facing |
| 18 | Skill census residue | 133 `<effect>` names, 60 `<condition>`, 8 `<targetType>` unhandled; 975 *reachable* skills lose an effect | `datapack_skill_coverage_census` | Listed for completeness: only **11 learnable** skills are affected and each is recorded out of scope above. This axis is the one that is under control |

### Closed

| # | Area | Landed | What it took |
|---|---|---|---|
| 1 | Emotes | 2026-08-14 | `handlers/playeractions/SocialAction` — the 17 plain socials plus Show Off's info re-broadcast, behind Java's `canMakeSocialAction` gate. The three couple socials (16/17/18) carry a `SKIP(census)`: they negotiate over `ExAskCoupleAction`, which no Interlude client can answer |
| 3 | Walk/run | 2026-08-14 | `handlers/playeractions/RunWalk` + `Creature.setRunning(boolean)`, whose `ChangeMoveType` broadcast and `broadcastUserInfo` were the missing half. `Speeds::move_speed` already read the flag, so walking reaches the movement maths with no recalculation |
| 2 | Pet & servitor commands | 2026-08-15 | All nine handlers: `PetHold`/`PetAttack`/`PetStop`/`PetMove`/`UnsummonPet`/`PetSkillUse`, `ServitorMove`, `ServitorMode`, `UnsummonServitor`. Three parts — the order primitives generalised from servitor-scoped to summon-scoped (a pet carries the same `ServitorOf` link), `<skills>` added to the `PetData` loader with Java's `getAvailableLevel` scaling, and `SummonAI._isDefending` |
| 4 | Karma decay | 2026-08-15 | `KarmaData` (`pcKarmaIncrease.xml`, 99 rows), `RateKarmaLost` with Java's `-1 → RateXp` fallback, and `Formulas.calculateKarmaLost` wired into `add_exp_and_sp` behind its three exemptions (cursed weapon, non-negative reputation, PvP zone unless GM) |
| 5 | Siege mercenaries | 2026-08-15 | `MercTicket` + the hired half of `SiegeGuardManager`: the full `<guard>` row (npc, `npcMaxAmount`, `stationary`), the `ConfirmDlg` posting flow with its spacing and cap rules, `castle_siege_guards` rows at `isHired = 1` loaded at boot, the spawn beside the garrison at siege start, and the clear-out on a change of ownership |
| 6 | Item handlers | 2026-08-15 | `SummonItems`, `Book`, `RollingDice`, `PetFood` and `Elixir` (which collapses onto `ItemSkills` the way `ItemSkillsTemplate` already did). The 14 names still unmapped are off-chronicle or unobtainable here — `Appearance`, `EnchantAttribute`, `ChangeAttributeCrystal`, `NicknameColor`, `SpecialXMas`, `Maps`, `Calculator`, `Harvester`, `CharmOfCourage`, `Bypass` and the support boxes are in no shop, drop list or quest reward on this dist; `MercTicket` is row 5 |
| 9 | Observation | 2026-08-15 | `bypasshandlers/Observation` + `Player.enterObserverMode` / `leaveObserverMode`, the `ObservationMode`/`ObservationReturn` packets and the `ObserverReturn` client packet (0xC1, one off row 15's list). Only the Coliseum's three seats are reachable here; all three verbs and the whole 31-row table are ported anyway |
| 10 | Zone types | 2026-08-15 | `MotherTreeZone` (6), `NoStoreZone` (18), `NoSummonFriendZone` (27) and `LandingZone` (69), each with its consumer: the flat regen bonus Java adds "at last", the private-store / workshop / buff-shop refusal, `OpCallPc`'s missing zone legs, and a new `CanUntransform` condition — which is the *only* thing `LandingZone` gates, and could not be ported until the kind existed |
| 11 | `enchantHPBonus.xml` | 2026-08-15 | `EnchantItemHPBonusData` + `MaxHpFinalizer`'s "Apply enchanted item bonus HP" arm: a flat per-piece figure by grade and enchant level, ×1.5 for a one-piece suit, and Java's three excluded slots (necklace, earrings, rings) |
| 13 | AI scripts | 2026-08-15 | `BabyPets` — the three baby pets' 5 s auto-heal, both rolls and both HP gates, on a scheduler chain keyed to the pet — and `OlyBuffer`, the arena vendor's five buffs per NPC |
| 16a | `//zone_visual` | 2026-08-15 | `AdminZone`'s zone visualiser: adena markers every 10 units along each boundary, one arm per zone shape, and a clear that decays exactly the markers it dropped. The rest of row 16 stays open — see its (corrected) row above |
| 12 | Quests | 2026-08-16 | **All 32 portable quests ported.** Q10866, the 31-quest newbie chain (five race lines of six, plus Moon Knight), and the shared `newbie_chain` skeleton the 25 collect quests are tables over. Three of the 35 are **not** quest-scripting work and stay out: Q00933/Q00935 (NPCs never spawned) and Q00500 (needs the unported agathion subsystem). The audit's 36 was 35 | Java's `QUESTS[]` vs `scripts/q*.rs`, then each survivor against the dist's spawns, items and htmls | Q00255 is ported as `tutorial.rs`; the 17 `not_done` stubs are inert in Java too. See below |
| 15 | Client packets | 2026-08-15 | **13 base opcodes implemented, 7 shown to have no behaviour to port.** The 13: clan roster 0x4D, skill list 0x38, `/gmlist` 0x8B, snoop quit 0xB4, rotation 0x5B/0x5C, boat stop 0x76, recipe-shop back 0xC0, title grant 0x0B, html link 0x22, pet fetch 0x98, item preview 0xC7, GM view 0x7E — the last bringing five `GMView*` packets with it. Ex-opcodes untouched (96 of 199 unhandled, overwhelmingly post-Interlude). See below |
| 17 | Buylists | 2026-08-15 | Limited stock in full (`count`/`restock_delay` → `ProductStock` on the world, the buy gate, `decreaseCount`, `BuyListTaskManager`'s restock beat, the sold-out packet filter, and the `buylists` table that had been sitting unused since the baseline migration) — **plus two pricing bugs the row's own premise had missed.** See below |
| 7 | `player_help` bypass | 2026-08-15 | `bypasshandlers/PlayerHelp` — the help book's own page links, with Java's `..` traversal guard and the `#<itemId>` suffix that marks the dialog item-bound so a button inside it does not close the book |
| 8 | `TerritoryStatus` bypass | 2026-08-15 | `bypasshandlers/TerritoryStatus`. The lookup is `findNearestCastle`, **not** the siege zone the NPC stands in — which is what lets a fisherman in the middle of a town answer at all |
| 20 | Item conditions | 2026-08-19 | `ItemTemplate.checkCondition` and the `<cond>` parser under it — the whole tree, all 24 condition kinds, both message forms, and the four call sites Java gates with it. See below |
| 19 | Player level cap | 2026-08-19 | `ExperienceData`'s two `+ 1`s and the `MaximumPlayerLevel` clamp, `PlayerStat.setLevel`'s ceiling, and `AdminLevel`'s accept range. **The row's own figures were wrong on the Java side** — see below |
| 14 | Config | 2026-08-19 | **59** keys still unread, and every one of them accounted for: dead in Java, off-chronicle, or blocked on an unported subsystem. The last live ones — `CorrectPrices`, the birthday trio and the two `PVP.ini` drop lists — landed with it. See below |

**Row 16 mostly corrected itself.** The audit's original figure was 79 missing
of 458 with "~10 against ported systems, so the G13 row's 'all off-chronicle'
claim is wrong". Working the row showed the audit was the thing that was wrong,
in two ways.

The diff was **case-sensitive**. `AdminCommands.xml` declares
`admin_deleteNpcByObjectId`; Java lower-cases every command before dispatch and
the port registers the lower-cased spelling. It was ported all along. 79 → 78.

And a missing command is only a gap if **Java has a handler for it**. Checking
each survivor against the `admincommandhandlers` sources and `MasterHandler`'s
registration list put **34 of the 78** in the "no live handler anywhere" bucket
— XML access-level rows with no implementation, dead in Java too. That bucket
contains every command the audit had called out by name as targeting a ported
system: `//mammon_find`, `//mammon_respawn`, `//set_vitality_level` (whose
handler registers `admin_set_vitality`, a different string) and
`//tvt_add|advance|remove`. None of them exists in Java either. The G13 row's
original claim was right and the audit's correction of it was wrong.

Of the 44 with a live handler, 25 are off-chronicle (fort sieges, fences,
Gracia, elemental `//setl*`, fake players) and 12 are dev tooling or
architecturally N/A (script reloading against compiled-in scripts, runtime
config editing, the fight calculator, the login-server console). That left
**four**: `//zone_visual` and `//zone_visual_clear`, which are ported here, and
the two `//instancezone` commands, which are not.

**Row 14's headline was inflated by the audit's own method, and its real
content is the triage.** The recorded 319 came from grepping the port for
`get_*("Key")` string literals — but the port builds some key names with
`format!`, and all **13 GrandBoss respawn keys** (which the row listed as a
gap) are read that way. They were never missing. The corrected derivation, run
over both literal and formatted reads, gives **298**, now **289** after the
work below.

Of those 289:

- **27 are dead in Java too** — the `Config` field is assigned from the ini and
  read by nothing outside `Config.java`. The clan level 6–11 costs and
  requirements, ability points, dual-class skill-deletion fees, the blood
  oath/alliance points and the non-droppable/pet item lists are all in this
  bucket. Porting them would mean porting nothing.
- **23 are read only by fortress code**, which is off-chronicle here.
- **17 are list-shaped** (`AutoLootItemIds`, `EnchantBlackList`,
  `AltOlyRestrictedItems`, …) and are assigned to their fields through helpers
  the field-mapping pass does not see; they are counted but not yet triaged.
- The remaining **~222** are live and in-chronicle: General 69, Character 61,
  Olympiad 32, Feature 22, Rates 19, NPC 13, Server 6. (Olympiad, Rates,
  Feature and NPC have since been read; General, Character and Server remain.)

**PVP.ini is wired**, closing the nine keys that block hardcoded. Four of them
had real values baked in: `PvPVsNormalTime`/`PvPVsPvPTime` were `const`
tick counts, `ReputationIncrease` was a `const 0`, and `MaxReputation` was a
literal `.min(0)` in the karma-recovery path — the reason reputation can never
go positive on this dist, which now reads as the config saying so.
`CanGMDropEquipment` was an unconditional "a GM never drops"; it is now the
gate Java writes (`!isGM() || KARMA_DROP_GM`), which agrees with the shipped
`False`.

The four anti-feed keys are parsed and left inert on purpose:
`AntiFeedEnable = False`, and Java's `AntiFeedManager` tests it first in every
entry point, so the manager is dead weight on this dist. Carrying the values
makes turning it on a change in one place rather than a hunt.

**Olympiad.ini is wired too** — all 36 keys, and the shape of the problem was
different from PVP's. The port already carried every shipped value as a `const`
with the Java key in its doc comment: that is *how* they were verified against
the dist, and equally why an operator editing `Olympiad.ini` changed nothing.
The season clock (start time, window length, competition days, round length,
validation window), the point economy (start/weekly/max points, the divider,
the rank and hero bonuses, the mark item and rate) and the match rules
(participants needed, weekly cap, battle length) now read the file.

`AltOlyCompetitionDays` needed care: Java's value is `Calendar` numbering with
Sunday = 1, and the port's season clock is 0-indexed, so the shipped `1,7`
means Sunday and Saturday. An off-by-one moves the Olympiad to Monday and
Sunday, so the conversion has its own test.

Five keys are parsed and inert, all for Java's reasons rather than the port's:
the weapon and armour enchant limits are **-1**, which Java reads as "no limit"
rather than comparing against; `AltOlyRestrictedItems` is empty; and
`AltOlyWinReward`/`AltOlyLoserReward` are the literal `None`, which
`parseItemsList` turns into no list at all. They are parsed anyway so the
values are visible and honouring them later is a change at the use site.

**Rates.ini is now fully read, and Feature.ini's live keys with it.** Rates
turned out to be almost entirely neutral: eleven keys ship at ×1, the three
instance rates ship at **-1** — which is Java's "use `RateXp`/`RateSp`"
*sentinel*, not a multiplier — `UseQuestRewardMultipliers` is False (gating the
four per-type quest rates off entirely), and `BossDropEnable` is False (gating
its three companions). Only `EventItemMaxLevelDifference = 9` carries a
non-neutral value, and nothing on this dist configures an event drop for it to
gate. `RateKarmaExpLost` (a PK's death exp loss) and `PetXpRate`/
`SinEaterXpRate` are wired to their consumers; the rest are carried with the
reason each is inert written down.

Feature needed **two behaviours ported**, not just values read.
`Castle.updateClansReputation` had no counterpart at all: a siege ending moved
no clan reputation. It now does, including the detail that decides the case —
the captor gains `min(TakeCastlePoints, what the former owner had *before*
being docked)`, so taking a castle off a bankrupt clan pays nothing. And
`PlayableStat.addReputationToClanBasedOnLevel` — a member levelling up earning
their clan reputation — was missing along with its anti-farm guard
(`LAST_PLEDGE_REPUTATION_LEVEL`, which stops a delevel loop being an infinite
reputation source). Every band is 0 on this dist, so it pays nothing today; it
is implemented rather than stubbed because the config is the only thing making
it inert.

Feature's remaining 38 are exactly the buckets already classified as
not-portable: 21 fortress keys, the 12 dead `ClanLevel6..11` cost/requirement
keys, and five more dead in Java (`FestivalOfDarknessWin`,
`KillBallistaPoints`, `BloodAlliancePoints`, `BloodOathPoints`,
`KnightsEpaulettePoints`).

**NPC.ini is now fully read.** Fifteen keys, and the split was two dead, eight
inert and five with a consumer already sitting in the tree waiting for them:

- **Two are dead in Java**: `DmgPenaltyForLvLDifferences` and
  `CritDmgPenaltyForLvLDifferences` parse into `NPC_DMG_PENALTY` /
  `NPC_CRIT_DMG_PENALTY`, which nothing outside `Config.java` reads. The note
  already in `config/npc.rs` saying so was independently re-derived and holds.
- **Eight are parsed and inert at the shipped values**, each for a reason in
  Java rather than a shortcut here, and each written down at the field:
  `AltMobAgroInPeaceZone = True` makes `AttackableAI`'s peace-zone skip
  unreachable; `GuardAttackAggroMob = False` makes `Monster`'s guard-retaliation
  branch unreachable; `AltAttackableNpcs = True` makes
  `Creature.onForcedAttack`'s `!canBeAttacked()` refusal unreachable;
  `AttackablesCampPlayerCorpses = False` is the behaviour the port already has
  (a dead target leaves the aggro list, so the next think drifts home); and the
  four `Raid{P,M}{Attack,Defence}Multiplier` keys ship at `100`, which Java
  divides by 100 — **×1.0, not ×100**, and reading the raw number would have
  buffed every raid boss a hundredfold.
- **Five are wired.** `MaximumSlotsForPet` replaced a `const 12` in
  `servitor::pet` and a literal `12` in `enter_world` whose comment read "the
  key isn't parsed yet". `SpoiledCorpseExtendTime` and
  `CorpseConsumeSkillAllowedTimeBeforeDecay` are the two halves of the sweeper's
  timing window — a spoiled or seeded corpse now lingers the extra 10 s, and a
  corpse inside the last 2 s of its life refuses the sweep ("the corpse is too
  old") instead of taking it. The four raid stat multipliers now run in
  `npc_finalized_stats` alongside the champion ones, and the two raid respawn
  multipliers scale a DB-backed boss's window before it is rolled.

Three things surfaced while porting it. Java's `_isRaid` is an **instance**
flag, not a template one — `Monster.onSpawn` calls
`setIsRaidMinion(_master.isRaid())`, which writes the same field — so a raid
boss's escort takes the raid multipliers too; `ChampionStatMods` became
`NpcStatMods` to carry both guards independently. `GMViewItemList`'s inventory
limit was documented as the *pet* limit; Java's only live caller passes a
`Player`, so three of the port's four call sites were writing the wrong number
and now send the player's own limit (the fourth, `//show_pet_inv`, is the pet
one and was right). And the first cut of the old-corpse gate read a "no decay
scheduled" corpse as "0 ms left"; Java's `getRemainingTime` answers
`Long.MAX_VALUE` there, which an existing spoil test caught.

**Character.ini is the largest remaining file, and the first cluster is done.**
Re-deriving it gave **82** unread keys, not the 76 on record. Applying the
usual hypothesis check: **8 are dead in Java too** — the ability-point pair,
the three `FeeDelete*Skills`, `MaxNewbieBuffLevel`, and the two mentor
penalties, which are dead twice over since `MentorPenaltyForMenteeLeave`
assigns to the *same field* as `MentorPenaltyForMenteeComplete` (a bug in
Java, not a porting choice). Four more are list-shaped (`SkillReuseList`,
`EnchantBlackList`, `AugmentationBlackList`, `AutoLootItemIds`) and are
counted but not yet triaged. That leaves **70 live**.

Ten of those are now wired — the *caps and clamps*, chosen first because they
are precisely the shape this row's "Effect in game" column describes: a value
hardcoded to this dist's number, several of them inside a comment quoting the
very key that was being ignored.

| key | was | now reads |
|---|---|---|
| `MaxEquipableItemGrade` | `const MAX_EQUIPABLE_ITEM_GRADE = S` | the buy-list loader, and `//reload buylist` re-applies it live |
| `MinAbnormalStateSuccessRate` / `Max…` | `.clamp(10.0, 90.0)` | `LandRateBounds`, threaded into `calcEffectLandRate` |
| `MaximumWarehouseSlotsForDwarf` / `…NoDwarf` | literal `120` / `100` | `warehouse_limit`'s private-warehouse base |
| `AltMaxNumOfClansInAlly` | `const MAX_CLANS_IN_ALLY = 3` | the alliance join gate |
| `AltClanMembersForWar` | `const CLAN_MEMBERS_FOR_WAR = 15` | both clan-war declaration gates |
| `MaxHP` | no cap at all | `calc_max_hp`, with Java's cursed-weapon exemption |
| `MaxSp` | no ceiling at all | `add_exp_and_sp` |
| `MaxRunSpeedSummon` | no cap at all | the summon stat path only — a plain NPC stays uncapped, as in Java |

Three of those were missing behaviour rather than a frozen number: nothing
capped a player's HP, nothing capped SP, and summon speed was unclamped.
`MaxSp` carries a trap worth naming — Java stores
`getLong(...) >= 0 ? value : Long.MAX_VALUE`, so a **negative** value means
*unlimited*; reading it literally would have frozen every character's SP at
zero.

**Cluster 2 — the karma gates and the arrival/teleport protection window (8
keys)** — landed next, and it was mostly *missing behaviour* rather than frozen
numbers. `PlayerSpawnProtection` (600 s) had no counterpart at all: a character
entering the world is meant to be **ignored by aggressive monsters** until their
first deliberate action. That is not invulnerability — Java's `isSpawnProtected`
has four readers, and the two that matter are `Attackable.getHating` (drops the
player from the aggro list) and `Summon.isInvul` (which *does* make the pet
invulnerable meanwhile, an asymmetry with the owner). The window is ended by
`Player.onActionRequest`, which Java calls from exactly five client packets, so
600 is a ceiling on an AFK login rather than ten minutes of safety. Ported as
`game_loop::spawn_protection` with the five packets hooked in one place in
`dispatch`.

`OffsetOnTeleportEnabled`/`MaxOffsetOnTeleport` needed the opposite of the
obvious change. The scatter looks global but Java applies it only where the
*caller* asks (`randomOffset = true`), which on this dist is four places: the
jail zone, a residence-hall teleport zone, the Olympiad observer's return to
`_lastLoc`, and summons following their owner. Folding it into the port's shared
`teleport_player` — 49 call sites — would have scattered every gatekeeper, quest
and `//tp` teleport. It is a separate `teleport_player_scattered`, wired at the
two of the four sites the port has.

Four of the eight are inert *at their shipped values*, and the reason is the
value rather than the port: `AltKarmaPlayerCanTeleport` and
`AltKarmaPlayerCanTrade` are **True** (their guards read `if (!config && …)`,
so they never fire), `AltKarmaPlayerCanBeKilledInPeaceZone` is **False** (the
peace-zone refusal stands, which is what the port already did), and
`PlayerTeleportProtection` is **0**. That last one is worth its own note: it is
a *different rule* from the spawn window despite the matching name — it is real
invulnerability (`Player.isInvul()` ORs it in) — and it is parsed without wiring
the invulnerability, because at 0 the branch cannot fire.

**Cluster 3 — what may be enchanted, and what may be augmented (8 keys)** —
closed two of the four list-shaped keys that had been counted but not triaged.
`EnchantBlackList` is a veto *on top of* the template flag
(`binarySearch(...) < 0 && _enchantable`), not a substitute, and
`AugmentationBlackList` is the last gate in `AbstractRefinePacket.isValid`;
both are now honoured. `DisableOverEnchanting` was already enforced
unconditionally inside the port's `accepts_target` and is now gated on the key.
Three of the eight are inert or unreachable: `AltAllowAugmentTrade` and
`AltAllowAugmentDestroy` both ship **True**, which is the behaviour the port
already had, and `AltAllowAugmentPvPItems` is **unreachable** — its gate is
`item.isPvp() && !config`, and no item in `data/stats/items` declares `is_pvp`
at all.

**`OverEnchantProtection` turned up a live footgun in the dist's own
configuration, and is deliberately not ported literally.** Java infers the
three enchant ceilings from `<enchantRateGroup>` *names*; this dist ships
`ARMOR_GROUP`, `FULL_ARMOR_GROUP`, `FIGHTER_WEAPON_GROUP` and
`MAGE_WEAPON_GROUP`, none of which matches its accessory patterns, so
`_maxAccessoryEnchant` stays at its initial **0** while weapons and armour both
derive 29. With the shipped `OverEnchantProtection = True` and
`OverEnchantPunishment = JAIL`, retail therefore destroys every enchanted ring,
earring and necklace a character owns on login and jails them for it — an
absence of group data read as a configured limit of zero. The sweep is ported;
`max_enchant_for_type2` returns `Option` and answers `None` for a category with
no group data, so those items are left alone. Recorded in
`docs/CUSTOM_DIST_DEVIATIONS.md` with its own guard test.

**Cluster 4 — clan and alliance timers (12 keys)** — was the one that found
real bugs rather than frozen numbers, because two of the port's hardcoded
constants did not match the dist they claimed to quote.

- **Clan dissolution took seven times as long as configured.** The port's
  `CLAN_DISSOLVE_DELAY_MS` was `7 * 86_400_000` and its doc read
  *"`DaysToPassToDissolveAClan` = 7 on this dist"*. The dist ships **1**.
- **Any privileged member could empty the clan warehouse.** Java's two branches
  for `AltMembersCanWithdrawFromClanWH` are not "privilege vs. nothing": with
  the key **on** the gate is `CL_VIEW_WAREHOUSE`, with it **off** — which is
  what ships — only the **clan leader** may withdraw at all. The port
  implemented the *on* branch unconditionally.
- **Command-channel allies could attack each other.** `AltCommandChannelFriends`
  is **True**, and Java checks it immediately after the peace-zone arm of
  `isAutoAttackable`, ahead of the flag/PK arms. The port had no such check, so
  two parties raiding together could hit each other — including inside a PvP
  zone, where Java's ordering makes the channel win.

The four alliance penalties turned out to be four *different* keys distinguished
by the `ally_penalty_type` Java stamps alongside each — `CLAN_LEAVED`,
`CLAN_DISMISSED`, `DISMISS_CLAN`, `DISSOLVE_ALLY`. All four ship as 1 day, which
is precisely why one shared `ALLY_PENALTY_MS` constant went unnoticed; the test
moves each key independently.

Three of the twelve are carried without a consumer, each for a stated reason:
`AltClanLeaderInstantActivation` is **False** and the port already does the
two-step nomination it implies; `LifeCrystalNeeded` is **True** but no entry in
this dist's pledge tree declares required items; and
`AltClanMembersTimeForBonus` (`30mins`, parsed as a duration) feeds
`ClanMember.getOnlineStatus`, and the port does not track per-member online
time.

**Cluster 5 — interruption and fake death (7 keys)** — found one behaviour
missing outright. `BreakStun` ships **True** (Java's own default is `false`, so
this dist opts *into* it) and the port had no stun break at all: a stunned
character stayed stunned for the full duration however hard they were hit. Now
`Formulas.calcStunBreak`, filtered on the `STUN` abnormal type rather than the
`BLOCK_ACTIONS` flag — sleep and paralyze carry the same flag and must survive.

`AltGameCancelByHit` is one string key Java reads twice, into
`ALT_GAME_CANCEL_CAST` and `ALT_GAME_CANCEL_BOW`. `calcAtkBreak` starts at
`init = 0` and refuses outright when it stays there, so with the key set to
neither, damage interrupts nothing. The port hardcoded the `15`, which made a
cast interruptible however the key was set. `EffectTickRatio` and
`FakeDeathDamageStand` were likewise hardcoded — the former as
`const EFFECT_TICK_RATIO_MS = 666`, and it drives both the DoT cadence *and*
the per-tick amount.

Three are carried without a consumer: `FakeDeathUntarget` (**False**, so Java's
sweep clearing the feigning player out of everyone's target slot never runs —
the port has no such sweep, which is the same behaviour),
`PlayerFakeDeathUpProtection` (**0**, the fake-death sibling of
`PlayerSpawnProtection`, so it never arms), and `MaxTriggeredBuffAmount`, which
caps `SkillBuffType.TRIGGER` buffs — a classification the port's buff list does
not carry, so there is no count to cap yet.

**Cluster 6 — cooldowns and what survives a session (8 keys)** — one genuine
gap and one hardcoded gate, with six carried for stated reasons.

`ArmorSetEquipActiveSkillReuse`: completing an armour set grants its active
skills, and Java stamps a reuse on them immediately
(`Inventory.ArmorSetListener` — *"Active, non offensive, skills start with
reuse on equip"*). The port granted them ready to fire, so a set could be
re-equipped to refire them. The skill's own reuse wins where it declares one;
the key is the fallback, and `0` disables the stamp exactly as Java's `> 0`
guard does. Java also tests `player.hasEnteredWorld()` because its inventory
restore runs the same listener during login — the port's path is equip-driven
only, so there is no login pass to exclude.

`StoreCharUiSettings` gated two packets that were hardcoded on. The gate is the
whole reply, not its contents: with the key off Java answers `RequestKeyMapping`
with **nothing** rather than an empty layout, and drops a `RequestSaveKeyMapping`
on the floor.

The other six are carried with the reason each has no consumer.
`EnableModifySkillReuse` is **False** *and* `SkillReuseList` is **empty**, so
the reuse-override map cannot fire from either side (the list now parses through
the shared `id,value;…` helper). `ItemEquipActiveSkillReuse` is the per-item
twin of the armour-set rule, and the port grants no per-item `ON_EQUIP` skills
to stamp. `SummonStoreSkillCooltime` gates `Pet.storeEffect`, and the port does
not persist summon effects across an unsummon at all. `StoreRecipeShopList` is
**False**, which is precisely the port's transient manufacture stores. And
`SubclassStoreSkillCooltime` is a persistence-model difference: Java flushes
cooldowns with `store(...)` just before `resetTimeStamps()` wipes them, while
the port saves memory-first on its own interval and clears `Reuses` at the
switch — same end state, no distinct moment for the flag to gate.

**Cluster 7 — character creation and auto-loot (7 keys)** — found a live
divergence in the drop path. Java's ordinary auto-loot arm reads
`!item.hasExImmediateEffect()`, so **herbs are excluded from plain `AutoLoot`**
and only `AutoLootHerbs` can pick one up. The port applied `AutoLoot` to every
drop, so on this dist — `AutoLoot = True`, `AutoLootHerbs = False` — herbs were
vacuumed into the inventory instead of falling to the ground for the walk-over
pickup the port already implements. The whole predicate is now Java's, including
`AutoLootItemIds`, which it tests *first* so a listed id is looted whatever the
other flags say.

`ForbiddenNames` was missing: character creation checked length and
alphanumerics but not the substring list, so a player could name themselves
something that reads as a server announcement in chat. Java matches
case-insensitively on *containment*, and answers `REASON_INCORRECT_NAME`.

`InitialEquipmentEvent` selects `initialEquipmentEvent.xml` over
`initialEquipment.xml`; the port hardcoded the normal one. The two files are
**byte-identical on this dist**, so nothing changes today — it is wired because
editing the event table is the only reason to have it. The datapack loaders'
config now travels as a `DataOptions` struct rather than a growing positional
argument list.

`StartingLevel` (1) and `StartingSP` (0) sit on Java's `> 1` / `> 0` guards and
so add nothing, and `AutoLootSlotLimit` (**True**) reduces
`validateCapacity` to "quest items against the quest limit, everything else
against the normal one" — already the port's behaviour.

**Cluster 8 — subclasses and skill acquisition (6 keys)** — found a missing
ceiling. `SubClassHolder.MAX_LEVEL` is
`min(MaxSubclassLevel, experienceTable.maxLevel - 1)`, a ceiling a *subclass*
has and the base class does not; the port had none, so a subclass levelled all
the way to the experience table's maximum. `BaseSubclassLevel` was a `const 40`
beside it.

The cap is applied as an exp ceiling, matching the existing idiom — and that
idiom is off by one on purpose: `exp_for_level(N) - 1` stops you at `N - 1`, so
*reaching* 80 means capping at `exp_for_level(81) - 1`. The first cut got that
wrong and capped subclasses at 79; the test pins the exact level and fails
against both that and the missing cap.

The other four are carried with the reason each has no consumer.
`AltSubclassEverywhere` is **True**, so `VillageMaster.checkVillageMaster`
returns `true` outright and any master adds a subclass — the port has no
race/teach-type gate, which is the same behaviour. `BaseDualclassLevel` belongs
to Ertheia's dual class, which has no Interlude counterpart.
`AutoLearnForgottenScrollSkills` is reachable (`AutoLearnSkills` is **True**)
but empty-handed: this dist's base-class trees carry no forgotten-scroll
entries. And `AltTransformationWithoutQuest` guards learning a transformation
skill behind `Q00136_MoreThanMeetsTheEye`, but the port parses no
`transformSkillTree.xml`, so none is learnable.

**Cluster 9 finishes Character.ini (7 keys), and found one more divergence.**
`MaxPersonalFamePoints` is the ceiling `Player.setFame` clamps every write to,
and this dist ships **0** against Java's own default of 100000 — a deliberate
override that *disables* fame, since each award is clamped straight back. The
port added the castle-zone award (125 per 300 s) unclamped, so players banked
fame Java would have zeroed on the spot, and again on the next login, because
even the DB restore goes through `setFame`. Every fame write now goes through
one clamped helper, `//setfame` included.

The other six are carried with reasons: the fortress fame pair is off-chronicle
*and* pays 0; `MaxExpBonus`/`MaxSpBonus` are both 0 behind Java's `> 0` guard;
`NpcTalkBlockingTime` is 0 with every use site guarded the same way; and
`SilenceModeExclude` has no player-facing silence mode to exclude from — the
port models only `General.ini`'s GM startup flag.

### Character.ini's remaining 8 are not portable work

Every key the file ships is now accounted for. `MaximumPlayerLevel` — the ninth,
and the only one that was ever real work — landed with row 19. All eight still
counted are **dead in Java**:

* `AbilityMaxPoints`, `AbilityPointsResetAdena`, the
  three `FeeDelete*Skills`, `MaxNewbieBuffLevel`, and the two mentor penalties,
  which are dead twice over because **both assign to the same field**, so the
  "leave" penalty has never done anything in any Mobius build. They are listed
  in `config/character.rs`'s module header rather than given fields, because a
  field would imply something reads them. The measure counts *string literals*,
  so naming them in prose deliberately does not move the number — loosening it
  to flatter the count would defeat the point of having one.

What was left in row 14 at that point was General (**8**, down from 19 as the
dev/debug and grid clusters landed) and Server (7). Both are now classified
rather than outstanding — see [the closing section](#row-14-closed-and-two-of-its-own-classifications-were-wrong)
below. (This paragraph used to name General twice, once with Character.ini's
description attached to it — a copy-edit slip from the row that closed
Character.ini.)

**The General.ini fall-damage cluster (`EnableFallingDamage`).** The first
General key worked, and the only one on the file that was a *subsystem* rather
than a flag: nothing in the port implemented falling at all. Java drives it
entirely from `ValidatePosition` — `Player.isFalling(z)` prices the drop once,
arms a 1.5 s task, and returns `true` for the next second so position
reconciliation is suppressed while the player is in the air. All three landed
(`game_loop::falling`), plus `Formulas.calcFallDam` and `Stat.FALL`.

Three things the Java side had to be read for rather than assumed:

* **`Stat.FALL` scales the damage, not the height.** Its only effect is named
  `SafeFallHeight` and the datapack comments it as *"Increases the maximum
  height user can fall without taking damage by 60"* — but the safe height is
  `PlayerTemplate.getSafeFallHeight()`, which no stat touches, and the stat is
  read by `calcFallDam` alone. Acrobatics (173) takes a flat 60/100 off the
  damage. Ported as Java computes it, not as the datapack describes it.
* **`getValue(stat, base)` is `mul * base + add`**, not `(base + add) * mul`
  (`Stat.defaultValue`). The two differ whenever a stat carries both kinds of
  modifier, and the test pins a pair rather than a single one so it can tell
  them apart. **This found a live bug one module over**: `water::breath_ms`
  computed the other order, behind a comment saying it was Java's, and its test
  asserted the same wrong expression — a test written from the premise it was
  meant to check. Fixed, along with the duplication behind it: `model::finalize`
  already *was* `Stat.defaultValue`, and three sites were respelling it
  (`breath_ms`, `calc_fall_dam`, `mana_charge_of`); two of them also skipped the
  `//setparam` fixed-value short-circuit `getValue` checks first. All three now
  call it.
* **The safe height is a constant.** `baseSafeFall` appears in **no**
  `stats/chars/*.xml` on this dist, so every class takes Java's literal
  `getInt("baseSafeFall", 333)` default. A loader would have had one possible
  answer.

This also **closes a G34 census exclusion**: `SafeFallHeight` was recorded out
of scope on the grounds that the stat would have no consumer, which was a
statement about the port's state and not a decision. The effect is now
registered and Acrobatics works; `<effect>` residue 133 → 132, learnable
skills affected 10 → 9, and `only_the_recorded_gap_is_reachable_from_a_skill_tree`
is down to `OpSweeper` alone.

**The General.ini skill-check cluster (`SkillCheckEnable`/`Remove`/`GM`).**
`restoreSkills` validates every `character_skills` row against the skill trees
and, on this dist, removes what fails. Porting it needed two corrections to
things around it first, because a check is only as good as the data it judges:

* **Noble skills were persisted instead of derived.** Java's `Player.restore`
  runs `setNoble(nobless == 1)`, which grants the tree with
  `addSkill(skill, false)` — never written. The port granted them nowhere at
  load and relied on the stored rows, so a revoked nobless kept every skill.
  Hero skills had the same shape from the other side: re-derived at enter-world
  *and* persisted, so a hero decrowned while offline kept theirs. Both are now
  derived-only and filtered out of the flush, which is also what makes them
  correctly *illegal* as stored rows.
* **A GM held no condition overrides at load.** Java defaults a GM's
  `cond_override` variable to `getAllExceptionsMask()`; the port started every
  character at 0. `SkillCheckGM` gates on exactly that override, so without the
  fix the third key of the cluster could never matter. One existing test moved
  (it was asserting the old default while testing the toggle).

Three notes on the check itself. `SkillCheckGM` **reads backwards** — Java's
`(!canOverrideCond(SKILL_CONDITIONS) || SKILL_CHECK_GM)` means **False**, the
dist value, *exempts* an override-holder. The level is clamped to the skill's
max before matching, which is what lets an enchanted skill (stored at level
101+) match its tree entry. And the check runs over the **DB rows**, not the
finished skill book: Java iterates its `ResultSet`, so derived grants are never
candidates — judging the book instead deletes the armour-set and noble skills
granted moments earlier.

`fishingSkillTree.xml` and `transformSkillTree.xml` are now parsed **for the
allow-list only**. Neither is teachable here, but Java's check consults both,
and parsing them makes the result correct by construction rather than by an
argument about which paths can produce a row.

**The General.ini GM-restriction cluster (10 keys).** The family that decides
what a GM's `PlayerCondOverride` grid exempts them from — and three of its
members read backwards: Java's guard is `canOverrideCond(...) && !Config.KEY`,
so setting the key **True** is what *stops* the override applying.

`GMSkillRestriction` is **True** on this dist and the port bypassed every skill
condition for every GM, behind a comment asserting the key was off. That comment
was wrong twice over — it also justified reading the access level instead of the
override, which only became distinguishable once the skill-check cluster gave a
GM a real override grid. Both halves are fixed: the gate reads
`canOverrideCond(SKILL_CONDITIONS)`, so `//set_exception 2` now means something.

`GMTradeRestrictedItems` turns out to be **three** exemption shapes on one key:
the undroppable gate and the quest-item gate need `canOverrideCond(DROP_ALL_ITEMS)`
*and* the key; the `TYPE2_QUEST` gate needs the override **alone**; and
`TradeStart`'s item list reads `ITEM_CONDITIONS` rather than `DROP_ALL_ITEMS`,
while `TradeList.addItem` beside it reads plain `isGM()`. Ported as written. The
port had no exemption on any of the four.

`OnlyGMItemsFree` was already implemented and hard-coded to this dist's value
behind a comment quoting the key — the pattern this row named at the outset.
`ServerGMOnly` was a `false` literal beside the note *"wired to config in a
later milestone"*, so an operator who closed the server was still advertised as
open. `GMRestartFighting`, `GMShowAnnouncerName`, `UseSuperHasteAsGMSpeed` and
`DefaultAccessLevel` are wired at their single Java sites.

**`GMDebugHtmlPaths` needed the recipient threaded through the html layer.**
Java's `HtmCache.getHtm(player, path)` carries a player at 132 of its 145 call
sites, and the parameter exists *for this key*: it sends a GM the path of every
dialog they are served. The port's `read_htm(path)` had no recipient, so the
split is now Java's — `read_htm_for`/`read_htm_for_client` where a player is
being served, plain `read_htm` for the loaders and scans that are Java's
`getHtm(null, …)`. ~40 call sites across 28 modules.

**`GMItemRestriction` is the one key with no consumer, and that is recorded
rather than wired.** It gates `ItemTemplate.checkCondition`, and the port
evaluates no item conditions at all: `<cond>` is unparsed (2126 blocks on this
dist — races, level, sex, categoryType), and the Olympiad and event item
restrictions are absent too. There is nothing for the override to bypass. The
field exists so the gate lands with the conditions when they are ported; **item
`<cond>` evaluation is a genuine unported subsystem** and is not counted
anywhere else in this document.

**The General.ini ground-item persistence cluster (11 keys).** Ten wired, one
carried; the headline is that `itemsonground` is no longer an empty table the
baseline migration left behind — the same position `buylists` was in before row
17.

`SaveDroppedItem` and its three companions (`SaveDroppedItemInterval`,
`ClearDroppedItemTable`, `EmptyDroppedItemTableAfterLoad`) are the
`ItemsOnGroundManager` persistence: load at boot, a periodic rewrite, a
shutdown flush, and Java's two truncate paths. All four are **off** on this
dist, so nothing changes today; what an operator turning them on now gets is
ground items that survive a restart.

**The decay clock had to be resumed rather than restarted.** Java compares
timestamps in its sweep and gets this implicitly; the port holds decay as a
scheduler entry in *ticks*, which do not survive a restart. So `drop_time` is
persisted as wall-clock and the remaining lifetime is computed on the way in —
an item that lay 590 s of its 600 s decays 10 s after the restart, not 600.
`-1` keeps Java's "protected, never decays" meaning end to end.

Two things deliberately **not** persisted, matching Java: loot protection
(`_ownerId` and its `ResetOwner` task are in-memory, so a restart frees a
reserved drop) and cursed weapons (`CursedWeaponsManager` owns their row, and
Java's `run()` skips them to avoid a double save).

`MultipleItemDrop` and `OnlyGMItemsFree` before it were both already
implemented and hard-coded to this dist's value behind a comment naming the key.
Reading it exposes Java's off branch, which is not the intuitive one: `break`ing
out of `ItemContainer.addItem`'s loop creates **one instance of 1 and discards
the rest**. It is a lossy setting, not a stacking setting.

`DestroyAllItems` gains its gate *and* a third cond-override
(`DESTROY_ALL_ITEMS`, ordinal 12) — Java's expression exempts an override
holder from the undestroyable half but not from the cursed-weapon half, because
`isCursed` sits outside the parenthesis. `UpdateItemsOnCharStore` had to become
a **flag on the save payload** rather than an empty item list, since the write
is delete-then-reinsert and an empty list means "delete everything".
`DatabaseCleanUp` ports Java's `IdManager` sweep — 50 statements over 43
tables, all 43 of which exist in this schema — as a four-column table plus the
four statements that do not fit the shape.

**One carried:** `LazyItemsUpdate` gates a *write-through* item path the port
does not have. It is memory-first by design, so turning the key on in Java
makes that engine behave more like this one; turning it off cannot make this
one behave like that. `ClanVariablesStoreInterval` was carried for a different
reason — see the field's own note: nothing writes a clan variable on this
chronicle, because the only writer is the post-Interlude clan-reward system.

**The General.ini loaders/HTML cluster (10 keys), and the bug in it.**
`CustomNpcData` is **True** on this dist and the port read `stats/npcs` **without
its `custom/` subdirectory** — `xml_files_in` does not recurse — so 14 templates
never loaded. Two belong to features the port implements: the **TvT event
manager** (70010/70011), so `//event_start TvT` spawned no NPC at all, and
Kadmos (1003000), so `//spawn 1003000` failed.

**A fixture was holding the gap open.** `tvt_tests` registers the manager
template by hand, under a comment saying it does so *"so `event_start`'s spawn
resolves"* — which is what a test looks like when it is compensating for a
loader rather than exercising one. The new test asserts the template against the
**real dist** instead, which is the only thing that could have caught this.

`CustomSkillsLoad` was the same directory-shaped gap one loader over (Ghost
Walking 100000, TvT's spawn protection). `CustomItemsLoad` is wired but inert —
this dist ships no `stats/items/custom/`. `CustomMultisellLoad` and
`CustomBuyListLoad` were already loading their overlays, hardcoded, with the key
quoted in a comment — the fourth and fifth instance of that shape in this row.

**`HtmCache` turns out to describe what the port already does.**
`data::htm_cache`'s header framed per-interaction reading as a deliberate
deviation, on the premise that "Java loads every file at boot". That is only
true of `HtmCache = True`; this dist ships **False**, where Java logs
*"Cache[HTML]: Running lazy cache"* and reads exactly the same way. The port
implements the configured branch — the correction is to the premise, not the
code. The eager branch stays unimplemented and is documented, because it also
changes `Npc.getHtmlPath` into a cache-is-the-oracle lookup.

`HideBypassRemoval` and `CheckHtmlEncoding` are `HtmCache.loadFile`'s two
content keys and now live in `strip_htm` beside the comment/whitespace strip
they run after. The `-h` removal is safe to apply to content because the
**client** consumes that flag: it strips `bypass `/`bypass -h ` before sending,
and Java's `RequestBypassToServer` never sees one.

Two carried with reasons: `CustomTeleportTable` is **dead in Java** (parsed into
a field nothing reads), and `HtmlActionCacheDebug` traces Java's
`validateHtmlAction` cache, which `game_loop::bypass` records as a deliberate
non-port.

**The General.ini feature-gate cluster (10 keys), and a hole in the measure.**
`AllowWarehouse`, `AllowRefund`, `AllowFishing`, `AllowBoat`,
`AllowCursedWeapons`, `AllowDiscardItem`, `BoatBroadcastRadius`, `TradeChat`,
`GlobalChat` and `MinimumChatLevel`. All ship **on**, so nothing changes today;
what an operator gets is a switch that works.

**The measure had a false negative.** `GlobalChat` never appeared in row 14's
unread list, because `"GlobalChat"` occurs as a string literal in
`config::flood_protector` — as the name of a *flood action*, nothing to do with
the `General.ini` key. A literal-scan measure counts any collision as "read", so
the real figure was 38 where the row said 37. This is the third distinct way the
derivation has been wrong (after the `format!` reads and the missing `getByte`),
and the first where it under-reported.

**`TradeChat`/`GlobalChat` are not on/off.** Three values each: `on` is
region-scoped, `global` is server-wide behind the flood protector, and `gm` is
region-scoped **only** for a `CHAT_CONDITIONS` holder — everyone else falls
through every branch and the line vanishes with no message. "Off" has no
spelling of its own; it is whatever matches nothing.

`MinimumChatLevel` gates General, Shout and Whisper with **three different
system messages**, so it cannot be written once and shared. Trade is absent from
that list because it carries its own **hard-coded level 20** — a Java literal,
not a config, and the port had neither gate.

The rest are single-site: the warehouse bypasses refuse as a *bypass* (the
button stays and does nothing), `AllowBoat` stops `BoatManager` registering any
dock at all, `AllowCursedWeapons` makes the manager load nothing, and
`AllowRefund` picks whether a sold stack is filed or destroyed — plus
`Player.hasRefund()`, which ANDs the key in so an operator turning it off hides
a container filled while it was on.

**The General.ini quest cluster (5 keys).** `OrderQuestListByQuestId` sorts the
NPC's quest-choice window by id — and Java's `TreeMap` **also de-duplicates**,
which a sort alone does not: two scripts sharing an id collapse to one entry.

`AutoDeleteInvalidQuestData` turned out to be two behaviours, only one of which
the key controls. A `character_quests` row naming a quest the server no longer
has is dropped from the **live state** either way (Java's `q == null` →
`continue`); the key decides only whether the *row* is deleted. The port did
neither — it loaded the row into the component and wrote it straight back on the
next flush, so a quest renamed between builds left a `QuestState` no code could
reach and no restart could clear.

`AltDevNoQuests` is misnamed: Java returns from
`ScriptEngineManager.executeScriptList()` before loading anything, so it drops
**every** script — AI and events with the quests. Ported as emptying the
registry, which is the same set. `AltDevShowQuestsLoadInLogs` logs one line per
registered quest.

**`StoryQuestRewardBuff` has no consumer in Java either.**
`Quest.giveStoryQuestReward` has **zero callers** — not one class under `java/`,
not one script in the datapack. It is not dead the way a never-read `Config`
field is (the method is a live scripting entry point), but nothing on this
chronicle is a story quest. Recorded in `config::general`'s header rather than
given a field, following `config::character`'s convention, so it stays counted.

**The General.ini instances/zones cluster (5 keys).** `PeaceZoneMode` is three
modes rather than a flag — 0 always, 1 exempts siege participants, 2 disables
peace zones outright — and mode 1 reads `getSiegeState() != 0`, which is **not**
`isInSiege()`. Java sets `_siegeState` for any registered clan member for the
whole siege regardless of where they stand, so a participant is exempt in a town
peace zone miles from the castle; the port's existing `is_in_siege` is
zone-scoped and would have exempted nobody where the mode is interesting. A
separate `has_siege_state` predicate was added for it.

`JailIsPvp` makes the GM prison a combat zone. `EjectDeadPlayerTime` expels a
corpse from an instance after a minute — **the port had no eject at all**, so a
player who died in Frintezza's tomb or an Olympiad arena stayed until the
instance was torn down. Java's cancellation mechanism is worth copying exactly:
it never cancels the task, it schedules one that re-reads `isDead()` when it
fires, so a resurrection, a manual exit and a logout all cancel it without
cancelling anything.

`RestorePlayerInstance` was absent in both directions: logging out inside an
instance neither recorded it nor moved the player to the exit location, so they
woke up at coordinates inside a world that no longer existed. Both branches now
land, and a remembered id whose instance has since been destroyed is consumed
and discarded rather than retried.

**`DefaultFinishTime` has no consumer here or in the datapack.** Java's only
no-arg `finishInstance()` caller is `AbstractInstance.finishInstance(Player)`, a
protected helper of the script framework that **no script on this dist calls**.
The port's instances end through the empty-destroy timer or an explicit
`destroy`, so there is no finish state for the delay to apply to.

**The General.ini dev/debug and grid cluster (11 keys), and the hysteresis the
port never had.** `GridsAlwaysOn`, `GridNeighborTurnOnTime` and
`GridNeighborTurnOffTime` describe Java's `WorldRegion` activation, and the port
had only half of it: the NPC AI recomputed its active set from where players
were standing *this tick*, with no memory. **Both timers were therefore
missing.** The turn-off half is the one with visible behaviour — an NPC walking
home after losing its target froze the instant the player moved two cells away
and resumed when they came back, where Java keeps it thinking for another 90
seconds, which is long enough for the walk to finish. `World` now carries a
`region_activation` map (`activate_at` / `active_until`) that
`refresh_active_regions` maintains and prunes.

`AltDevNoSpawns` early-returns from `spawn_all`, and the `DBSpawnManager` half
Java guards separately falls out of it: the `db_save` lines `boss_respawn`
settles are collected by that same pass. `ShowServerNews` is Java's `else if`
**after** the clan notice, so a member of a clan with a notice enabled never
sees `servnews.htm` however the key is set.

**`AltDevShowScriptsLoadInLogs` is not a synonym for
`AltDevShowQuestsLoadInLogs`.** Java's `Quest(int questId)` registers through
`addQuest` when the id is positive and `addScript` otherwise, into two separate
maps with a key and a wording each — so the scripts key covers exactly the AI
and event classes, the ones an id-keyed listing cannot show. The port has one
registry and had joined both keys to one loop; it now splits on `id() > 0` and
logs "Loaded quest"/"Loaded script" as Java does.

**The four `Debug*Packets` keys carry one documented deviation.** Java logs the
packet's *class* name (`Say2`, `RequestBypassToServer`) and matches
`ExcludedPacketList` against it. The port has no per-packet type — packets are
opcodes dispatched to functions, and the two opcode tables hold 514 constants
between them — so the trace names the **opcode** (`0x49`, `0xD0:0x005F`) and the
exclusion list matches the same text, case-insensitively. A hand-written name
table would be large and would rot silently the first time an opcode moved.

**`DebugUnknownPackets = True` is inert on this dist, and not because of its own
value.** Java nests the unknown-packet branch *inside* the client-packet trace
switch, so with `DebugClientPackets = False` it is unreachable. Reproduced as
written; the port's own unconditional `error!` for an unhandled opcode is kept
as a deliberate deviation, recorded on the config field.

**`Developer` has no consumer.** All seven Java sites gate a `LOGGER.warning`
inside a `catch` block in the admin handlers, every one of them logging an
exception thrown by parsing a malformed command. The port validates its
arguments and answers with the same usage message Java sends after logging, so
there is no exception to gate. Recorded in `config::general`'s header rather
than given a field, following `config::character`'s convention.

## Row 14 closed, and two of its own classifications were wrong

The row measured **65** unread keys and sorted them into buckets. Working the
buckets found that two of them held live work, and closing the row meant
porting that rather than re-describing it. The count is now **59**, and the
difference is the six keys that turned out to matter.

**`CorrectPrices` was read in one loader and hardcoded in the other.** The buy
lists floored an npc-served price at the item's sell value with the key baked
to `True`; the multisells did not implement the second half at all — Java also
raises a single-adena-ingredient entry's *cost* to the total sell value of what
it hands out, chance-weighted (`sellValue * chance / 100`), which is what keeps
a 1 %-drop list from costing the jackpot. Both halves now read the key.

On the shipped datapack the key corrects **nothing**: no npc-served buy-list
line undercuts its item's sell value, and none of the 4971 single-ingredient
multisell entries is underpriced. That is asserted rather than assumed — one
test per loader compares the catalogue parsed both ways and expects them
identical — and the mechanism itself is pinned by unit tests instead of by the
data.

**The birthday gift was a whole task, not a key.** `AltBirthdayGift`,
`AltBirthdayMailSubject` and `AltBirthdayMailText` feed
`taskmanager/tasks/TaskBirthday`, which Java registers as a global task at
`06:30:00` — the slot the port's daily reset already owns. It mails every
character a present on the anniversary of its creation, and three details are
easy to miss:

* it **catches up**, walking day by day from the task's last activation to
  today, so a server that was down over a birthday still sends the gift. That
  makes it the first reader of the `DAILY_TASK_RESET` stamp the daily reset has
  been writing all along;
* the recipient does **not** have to be online — the query is over the
  `characters` table and the gift is a mail row;
* a 29-February character is paid on the 28th in non-leap years, which Java
  spells out and the port ports.

The shipped `AltBirthdayMailText` uses **neither** `$c1` nor `$s1` despite the
ini's own comment documenting both, so the mail goes out impersonally until an
operator edits it. The substitutions are implemented and tested anyway.

**`ListOfNonDroppableItems` and `ListOfPetItems` were filed as dead in Java and
are not.** Both feed `Player.onDieDropItem`: a dying PK never scatters the 30
ids on the first list (adena, the Interlude hero weapons, the hero circlet, the
newbie gear) or the 12 on the second (collars, pet armour). The port's
death-drop filter had every other leg of Java's condition and not these two, so
a PK's corpse handed a hero weapon to whoever killed them. They live on
`config::rates` beside `MinimumPKRequiredToDrop` — `PVP.ini`'s keys are split
by consumer here — sorted at load, because Java binary-searches them.

Porting that leg also found a **dead branch in Java**: the same filter skips
"the control item of the active pet" by comparing `_pet.getControlObjectId()`,
an id from the world pool starting at `0x10000000`, against `itemDrop.getId()`,
a template id of at most five digits. They can never be equal. A summoned pet's
collar is kept off the ground by `ListOfPetItems` instead, which is what
actually contains 2375.

### The 59 that remain, and why none of them is portable work

| File | Unread | What they are |
|---|---:|---|
| Feature.ini | 38 | 19 fortress fees + 2 fort reputation awards (off-chronicle), 12 clan level 6-11 costs/requirements and 5 others (`BloodOathPoints`, `BloodAlliancePoints`, `FestivalOfDarknessWin`, `KillBallistaPoints`, `KnightsEpaulettePoints`) that **no code outside `Config.java` reads** |
| Character.ini | 8 | all dead in Java — ability points (2), the skill-deletion fees (3), `MaxNewbieBuffLevel`, and the two mentor penalties, which are dead twice over because both assign to the *same* field (`MENTOR_PENALTY_FOR_MENTEE_COMPLETE` is assigned from `MentorPenaltyForMenteeLeave` on the next line) |
| Server.ini | 7 | infrastructure: `Driver` (a JDBC class name; this server is `sqlx`), `MySqlBinLocation` (mysqldump, Windows-only, and `BackupDatabase = False`), the two HWID keys (no `RequestHardWareInfo` on this chronicle, and both off), `PrecautionaryRestartCpu`/`Memory` (the manager is created only `if PRECAUTIONARY_RESTART_ENABLED`, **False** here) and `PrecautionaryRestartChecks` (no reader in Java) |
| General.ini | 4 | `Developer`, `LogAutoAnnouncements`, `StoryQuestRewardBuff` — no reader in Java — plus `BookmarkConsumeItemId`, which Java *does* read but which needs the teleport-bookmark subsystem (four client packets on Ex `0x4E`, none dispatched here) |
| NPC.ini | 2 | `DmgPenaltyForLvLDifferences` / `CritDmgPenaltyForLvLDifferences`: parsed by Java, read by nothing |

Each figure above was re-derived from `Config.java` against the port rather
than carried forward; the command is in
[§ Re-deriving these numbers](#re-deriving-these-numbers). Every "no reader"
claim was checked with a `Config.FIELD` grep over `java/` **and**
`dist/game/data/scripts/`, not assumed from the key's name.

**Row 19 closed, and the row's own arithmetic was wrong — in Java's favour by
one.** It recorded the Java cap as 79. It is **80**, the Interlude cap, and the
missing step is in `Config.load`:

```java
PLAYER_MAXIMUM_LEVEL = characterConfig.getByte("MaximumPlayerLevel", (byte) 90);
PLAYER_MAXIMUM_LEVEL++;
```

The key is read and then *incremented on the next line*, so a shipped
`MaximumPlayerLevel = 80` reaches `ExperienceData` as **81**. There it meets a
second `+ 1`: `MAX_LEVEL = maxLevel + 1` (86 from the table), clamped to that
81. `MAX_LEVEL` therefore names the row **above** the highest attainable level,
which is what makes the exp cap — `getExpForLevel(MAX_LEVEL) - 1` — stop a
character one point short of level 81. Java's own boot log prints
`MAX_LEVEL - 1` as "Max Player Level" for the same reason.

Both `+ 1`s are kept where Java keeps them:
`CharacterConfig::maximum_player_level` stores the incremented value and says
so, and `ExperienceData::max_level` is `MAX_LEVEL` rather than the attainable
cap — so every consumer spells the cap `max_level - 1`, exactly as the Java
expressions do. Storing either one "helpfully" pre-decremented would put an
off-by-one into every use site instead of one place.

Three mechanisms, not one. The exp cap alone would not have held:

* **The exp cap** (`progression::add_exp_and_sp`) already read `max_level`, so
  fixing what that means moved it from 84 to 80 on its own.
* **Row loading stops at the cap.** Java `break`s out of the row loop at the
  first `level > PLAYER_MAXIMUM_LEVEL`, so `_expTable` holds 1..=81 and
  `getExpForLevel` of anything past it answers with row 81. The port loaded all
  86 rows and its `exp_for_level` clamped to the table's *end* — which, past
  the last row, was reading a zero it had padded the vector with.
* **`PlayerStat.setLevel` clamps at `MAX_LEVEL - 1`**, and the paths that set a
  level outright — `//set_level`, the community board's delevel, the subclass
  swap — never go through `addExp` at all. The clamp is `death::cap_level`,
  applied inside `set_level` (Java's own funnel) rather than at each caller, and
  read directly by `//set_level` so the exp it stores is the one that goes with
  the level `setLevel` will settle on.

`AdminLevel` came with it: `//set_level`'s accept range is
`1..=ExperienceData.getMaxLevel()`, lowered to `MaxSubclassLevel` while a
subclass is active, and a value outside it is **refused with a message** rather
than clamped — while a value inside it is still put through `setLevel`, so
asking for 81 lands on 80. The port had neither the range nor the narrowing.

`maxPetLevel` is parsed by Java in the same function (`min(86 + 1,
MAX_LEVEL + 1)` = 82) and is named in prose rather than given a field: its only
readers are `Summon.getExpForThisLevel`/`getExpForNextLevel`, and no species'
`PetData` table comes close to 81, so the species table is the binding
constraint on both sides — which is what the port already reads.

**This also closes `MaximumPlayerLevel`**, the last unread key in
`Character.ini` and the one row 14 was holding open pending this decision.

**Row 20 closed, and the two conditions that look unported are Java's own
constants.** `<cond>` now parses (`data::item_cond`) and evaluates
(`game_loop::items::conditions`). The parse is checked against the datapack
rather than against itself: `the_dist_parses_to_the_blocks_the_datapack_declares`
asserts **2126** blocks and a per-attribute tally — races 822, flyMounted 450,
categoryType 261, level 218, sex 149, levelRange 100, pledgeClass 73,
cloakStatus 49, instanceId 42, castle 29, isHero 29, SiegeZone 22,
class_id_restriction 20, insideZoneId 18, subclass 17, pkCount 16, clanHall 8,
chaotic 6, MinimumVitalityPoints 4, isOnSide 4, isClanLeader 2, vehicleMounted
2, fort 1, and the one `<target levelRange>` — each figure derived from the XML
independently of the parser, so a dropped arm shows up as a drop in one row.

Four things the Java side had to be read for rather than assumed:

* **`<cond>` in `stats/skills` is dead data.** 13 skill files carry blocks in
  the same syntax. `DocumentBase` — the only thing that parses `<cond>` — has
  exactly one subclass in the whole Java tree, `DocumentItem`; `SkillData`
  reads the newer `<condition name="…">` form instead. Those blocks are inert
  on both sides, which is why row 20 is an *item* row and closing it does not
  touch the skill census.
* **The effector is not always the player, and the conditions split three ways
  on that.** `ConditionPlayerRace` tests `isPlayer()` and so refuses a pet
  outright; most read `getActingPlayer()`, which for a summon is its **owner**;
  and `ConditionCategoryType` reads `getId()` — a player's *class* id but a
  summon's *npc* id. That last one is the entire mechanism behind
  `categoryType="STRIDER"` on a saddle, and it is why `RequestPetUseItem`
  evaluates against the pet.
* **`cloakStatus` and `fort` are constants here, not narrowings.**
  `Inventory.canEquipCloak()` forwards to `PlayerStat._cloakSlot`, whose only
  setter has **no caller anywhere in the Java tree** — so the 49 items gated on
  it are unwearable in Java too. `fort="-1"` asks the clan for `getFortId()`,
  and there are no fortresses on this chronicle. Both are ported as the
  expressions Java evaluates rather than left to fail open, so they answer the
  same way for the same reason.
* **`pkCount` is a maximum.** `getPkKills() <= value` — read as a minimum, the
  16 items using `pkCount="0"` would be gated exactly backwards.

Two gates rode along with the function because they are inside it: the Olympiad
refusal for hero and `is_oly_restricted` items (with the equip/use wording
split, and `_heroItem` computed from the id range 6611-6621 / 9388-9390 / 6842
rather than read from a flag) and the event-restricted refusal.

**The other half is `checkItemRestriction`**, which re-judges worn gear and
strips what stopped qualifying. Java calls it from nine places; the seven that
are in-chronicle are wired — `rewardSkills`, `onPlayerKill`, `setPledgeClass`
(the port's scattered assignments, at the clan join/leave/level/leader sites),
`onTeleported`, the clan-leader handover, and siege end. The Olympiad's
`portPlayerToArena` call needs no wiring of its own: the port enters the arena
through `teleport_player`, so `on_teleported` fires it at the same moment, with
the match already registered. `PetInventory.restore`'s validation pass runs at
**summon** instead of at load, because the port keeps the pet inventory on the
owner — there is no pet to judge the gear against until then.

Porting it also closed three holes on the pet path, all of them the same
handler's other gates: `RequestPetUseItem` never ran `checkCondition`; its
`useItem` refuses an equippable item carrying **no** conditions at all (pet gear
is *defined* by being gated), where the port had been wearing anything
equippable the pet window offered; and `isForNpc()` — `for_npc`, declared by 508
items and Java's first check on the packet — was not modelled, so the window
accepted any item. And `force_new_leader` never re-derived either player's
pledge class, which Java's `setNewLeader` does on the line above the
`checkItemRestriction` this row added.

**Row 12 closed, and porting it found a live inventory divergence.** The
recorded figure was 36; the arithmetic gives **35** (53 ids absent, minus
Q00255 which is ported as `tutorial.rs`, minus the 17 `not_done` stubs).
Checking each survivor against the datapack took three more off the
quest-scripting list:

- **Q00933 / Q00935** (Dungeon of Abyss wings) — NPCs 31774–31777 are declared
  as templates but appear in **no spawn file**. Nobody to talk to.
- **Q00500 Brothers Bound in Chains** — reachable in Java (the Penitent's
  Manacles grant skill 55701 → `SummonAgathion npcId 9021`), but gated
  end-to-end on the player having that agathion, and agathions are an unported
  subsystem `cubic_data.rs` already names as deferred (166 `SummonAgathion`
  skills). It also needs the daily-quest reset. A subsystem gap wearing a
  quest's clothes.

The other **32 are ported**. The 25 collect quests are tables over a shared
`scripts/newbie_chain` skeleton — five race lines running the same script with
different ids — and the five "Future \<race\>" capstones share a second one.
Moon Knight and Q10866 are bespoke enough to stay hand-written.

**The divergence.** Q11000's Rolento hands over items 49559 and 49560, which
this datapack does not declare, and Gudz then gates on holding both — so the
quest stalls at cond 8 in Java. It did **not** stall here: the port's shared
add-item path reads the template with `.unwrap_or(false)` for stackability and
creates the item anyway, where Java's `ItemContainer.addItem` logs
`Invalid ItemId` and returns null. Any script giving an id the datapack lacks
was minting a phantom item.

The fix is scoped to `QuestCtx::give_items` rather than the shared path. The
shared version is what loot, admin grants, lottery and the rest go through, and
tightening it failed 26 tests whose fixtures rely on the leniency — that is a
core-invariant change with its own blast radius, not part of porting a quest
chain, and it is left for a decision of its own. Validating the quest path
alone failed five, all genuine fixture debt (a script handing out an item the
test world never declared), and those five now declare them.

Two datapack facts recorded so they are not re-investigated as porting bugs:

- **Q11000 Moon Knight cannot be finished**, in Java or here, for the reason
  above. Everything up to cond 8 is real content; the port reproduces the dead
  end rather than inventing the two items.
- **Item 49772 (Scroll of Blood Melody) does not exist**, and five quests award
  it on completion. It is only ever `giveItems`, never checked, so those quests
  finish with the reward silently absent — in Java as here.

And one Java bug reproduced: `Q11006`'s `a_cleric.html` sets cond **5**, the
wizard's cond, while Zigaunt (the cleric trainer) answers only at cond 6. A
cleric is served by Parina and Zigaunt's page is unreachable. Both pay the same
reward, so the quest still completes.

**Row 15 split almost exactly in half, and the dead half had to be proved
dead.** Twenty base opcodes were listed as unhandled. Thirteen were real and
are ported; the other **seven have no behaviour to port**, and each needed the
Java side read rather than the port's absence noted:

- `MoveWithDelta` (0x52) — `runImpl` is the comment `// TODO this`.
- `RequestPledgeExtendedInfo` (0x66) — empty `runImpl`.
- `GameGuardReply` (0xCB) — hashes the reply into `_isAuthedGG`, whose getter
  `isAuthedGG()` has **no callers**. Doubly inert: `GameGuardQuery` is never
  sent, so the client is never asked in the first place.
- The three clan-war replies (0x04/0x06/0x08) — every one returns unless
  `getActiveRequester()` is set, and nothing in the clan-war path sets it
  (`onTransactionRequest` is called only by trade, duel, party room, MPCC and
  friend invites). The declarations act unilaterally. Both reachable routes to
  `ClanWarState.MUTUAL` — declaring back, and five kills — were already ported
  in the 0x03 handler and `clan_war_on_kill`, so the replies add only a branch
  Java cannot legitimately reach.
- `RequestChangePetName` (0x93) — refuses when the pet already has a name, and
  `Npc.getName()` returns `getTemplate().getName()` (neither `Pet` nor `Summon`
  overrides it, so the `_name` that `setName` writes is never read). All 873
  pet templates here have a name, so **no pet can ever be renamed in Java**.

Two of the thirteen corrected notes already in the tree. `flags::register_gm`
claimed the `hidden` flag was inert because "every `getAllGms` call site passes
`includeHidden = true`" — `AdminData.sendListToPlayer` passes
`player.isGM()`, so a plain player's `/gmlist` filters on it. And because
`GMStartupAutoList = False` here, *every* GM is flagged hidden, which is why a
player's `/gmlist` correctly answers "there are no GMs currently visible" with
a GM standing beside them. `gm_util`'s "no `//gmlist` consumer yet" is now
false too.

`RequestGMCommand` (0x7E) was the largest single item: five new `GMView*`
packets on top of the two already ported. Its `isGM() && allowAltG()` gate
reduces to `isGM()` on this dist — only levels 70 and 100 are GMs and both
carry `allowAltg`, so the seven other `allowAltg` levels never reach it.

**Row 17's real headline was not limited stock.** The row was recorded as
"limited stock untracked, castle-guard price scaling skipped". Both were true,
but three of Java's rules live in the **`Product` constructor** rather than in
`BuyListData.parseDocument`, and reading only the parser had missed two of
them.

The larger one: `_price = (price < 0) ? item.getReferencePrice() : price`. A
bare `<item id="2505" />` is not a product without a price — it sells at the
item's own reference price. The port kept the parser's -1 and
`RequestBuyItem` refused the purchase, so **3079 of the 8198 product lines on
the npc-served lists were unbuyable**: 38 % of the merchant catalogue, whole
shops at a time. Cooper in Gludin sold nothing at all.

The second: `Config.CORRECT_PRICES` floors a declared price at the item's sell
value, but Java's condition ends `&& (buyList.getNpcsAllowed() != null)`. The
GM-shop lists under `custom/` have no `<npcs>` block, and that clause is the
only reason their `price="0"` lines stay free. Applying the floor
unconditionally, as the port did, put a price on **2691** GM-shop lines.

Neither was reachable from the audit's framing, and neither is about limited
stock. The row's original two items were real and are also done: 1928
limited-stock lines across 147 files now track a count and restock on
`BuyListTaskManager`'s schedule, and `RateSiegeGuardsPrice` scales the eleven
`CASTLE_GUARD` items — a ×1 identity on this dist, so the *stale* half of that
note was the stated reason ("no sieges", written before G24), not the skip.
Java's `MAX_EQUIPABLE_ITEM_GRADE` filter came along with it; it drops five S80
lines from one GM list and nothing else here.

One Java bug is deliberately **not** reproduced. `BuyList.writeImpl` writes
`_list.size()` as the entry count and *then* skips every sold-out product, so
the packet claims more items than it carries and the client parses into the
following bytes. The port counts after filtering. This is the rare case where
matching Java would mean emitting a malformed packet rather than a different
game rule, and the test asserts the declared count matches the bytes written.

**A datapack quirk found while porting row 13, recorded so it is not
re-investigated as a bug.** Every `Heal` skill on this dist writes its power as
`<effect name="Heal"><item>power</item></effect>` with no value anywhere.
Java's `SkillData` parses a nested `<item>` into a *list*, not a parameter, so
`params.getDouble("power", 0)` returns **0** — in Java as well as here. A heal's
whole amount is therefore `sqrt(2 · mAtk)` plus the shot bonus. The port is at
parity; the `Heal { power: 0.0 }` it produces looks like a parse failure and is
not one.

**Row 11 was one unread file and one missing loop.** Every enchanted armour
piece in the game was worth exactly its unenchanted stats — a +12 set silently
short by hundreds of HP. Java's exclusion list is by **body part**, not item
kind, which is the detail worth keeping: `ItemKind::Armor` covers jewellery too,
so testing the kind alone would have paid a bonus on a +12 ring.

The Olympiad wrinkle is parity rather than a gap: Java reads
`getOlyEnchantLevel()`, which caps the enchant during a match at
`AltOlyArmorEnchantLimit` — **-1 (no limit) on this dist**, so the capped and
raw levels are the same number here and the finalizer needs no match context.

**Row 9 needed a second observer mode, not a bigger one.** The port already had
the Olympiad's spectator (`olympiad::observer`), which is scoped into a match
instance and answers `ExOlympiadMode`. Java shares one `_observerMode` flag
between the two but keeps two enter/leave pairs and two *client* packets, so
folding the tower's viewer into the Olympiad component would have had the wrong
packet strand whichever viewer answered second. Two components, as Java has two
paths.

The gate that makes it more than a teleport is `Action.runImpl`'s
`inObserverMode()` — a spectator clicks nothing. Both sites that need it
(`Action` and `ValidatePosition`) already carried "no observer mode yet" notes.

**Row 10's four kinds were dropped at load, and the loader said so.**
`kind_from_type` returns `None` for an unported kind "so mixed files can be read
without pulling in unported behaviour" — correct as a policy, and it meant every
rule keyed on those four was unenforced with nothing to show for it. Two of them
had already been noticed from the other side: `private_store.rs` and
`sell_buffs.rs` each carried a comment reading "a zone kind this port does not
load", and `conditions.rs`'s `call_pc` one reading "the port has neither zone
kind". Three comments describing the same missing four lines of parser.

The `u8` membership mask has been full since `Swamp` took the last bit, so all
four are geometry-queried like `NoLanding` and `Fishing` — which is what they
want anyway: three are asked at the moment of an action, and the mother tree
needs the *zone* (for its bonuses) rather than a flag.

Not loaded: `ssq.xml`'s ten further `MotherTreeZone`s. They are the Seven Signs
main event's (`ssq_main_event_*`), and this dist has no Seven Signs.

**Rows 7 and 8 were two dead buttons on 346 files between them**, and both are
~40 lines. The reason they sat unported is visible in the audit's own framing:
they were recorded as "bypass handlers", a category that reads like plumbing,
when what they actually are is the in-game help book (92 pages, reached from the
`Book` item handler row 6 restored) and the "who owns this land and what does it
tax" button on every fisherman, pet manager and warehouse keeper in the game.

**Row 5 finished what row 6 started.** `MercTicket` was the one reachable item
handler row 6 left behind, because it is not really an item handler: it is the
front door of a subsystem. The `<siegeGuards>` loader had been reading only
`itemId → castleId` — enough for the pickup refusal that already existed, not
enough to know *which* guard a ticket posts or how many of it a castle may
field. `siege/capture.rs`'s own comment ("which it always is until the mercenary
system lands") is now stale in the good way: postings are cleared there for
real.

SKIP(census): Java's `spawnMercenary` also spawns a 3-second preview of the
guard at the moment of posting (`scheduleDespawn(3000)`). The siege-start spawn
is the real one and is unaffected.

**Row 6 was mis-titled in this table until it was worked.** It read as a tail of
small conveniences — dice, a readable book, pet food — and the count of affected
items was the least interesting thing about it. **Every pet collar on this dist,
the Wolf Collar included, declares `handler="SummonItems"`**, and that name fell
through `ItemHandler`'s match to `None`, whose arm is `{}`. Using a collar
therefore did nothing at all: the entire pet system — summoning, feeding,
levelling, evolution, all of G29 and all of row 2 above — had **no entry point
from the client**. Every test that summoned a pet did it by setting
`pending_pet_collar` by hand, so the gap was invisible from inside the suite.
The collar path now runs end-to-end, under a test that drives the real
`UseItem` dispatch and fails without the one-line mapping.

Rows 1 and 3 fell out of the same change that made the dispatch table-driven,
which is why they closed together: the handlers are ~30 lines each, and what was
actually missing was a router that could reach them.

**Row 2 carried a behaviour change worth knowing about.** Java's `Summon` is not
an `Attackable`: `SummonAI.onEvtAttacked` retaliates only in the `ServitorMode`
*defending* stance, and a fresh summon starts passive. The port ran every summon
through the ordinary monster damage reaction, so summons and pets **always**
fought back — there was no stance for `ServitorMode` to select. That gate is now
in `combat::damage::npc_receive_damage`, which makes the toggle mean something
and matches retail: your pet does not pick fights until you tell it to. The
damage tally is still kept either way, because kill credit reads it.

Only 13 rows in `ActionData.xml` are left without an arm, and all four handlers
behind them are post-Interlude: `AirshipAction` (4), `TacticalSignTarget` (4),
`TacticalSignUse` (4), `TeleportBookmark` (1).

**Row 4 is where row 19 came from.** Java's karma table stops at
`MaximumPlayerLevel` and `getMultiplier` unboxes straight out of the map, so it
would throw for any level past it — which never happens there, because the same
config also caps the attainable level one short of it. Checking that the port
had the same guarantee is what showed it does not: it reads `maxLevel` raw and
reads `MaximumPlayerLevel` nowhere. The lookup therefore answers from the row the
file declares (1–99) and falls back to the highest row below, so decay keeps
working across the port's wider range; the cap itself is left alone as row 19,
because narrowing it is a live-server decision, not a porting one.

Covered by `game_loop/tests/player_actions_tests.rs`, the pet/servitor order
tests at the end of `game_loop/tests/servitor_tests.rs`, and the karma-decay
section of `game_loop/tests/pvp_kill_tests.rs`.

### Measured, and correctly out of scope

Not gaps — the audit reached them and they are off-chronicle, config-disabled,
or unimplemented in Java too. Recorded so the next audit need not re-derive it:
fort siege, territory war, Gracia/Hellbound, elemental attributes,
Sayune/shuttles/airships, mentor, commission, the 9 daily-mission handlers,
prime shop, beauty shop, appearance stones, Seven Signs (`SSQZone`, 41 zones),
96 wired Ex opcodes, and 13 base opcodes that are `null` in Java's own enum
(`SOCIAL_ACTION`, `CHANGE_MOVE_TYPE`, `CHANGE_WAIT_TYPE`, `REQUEST_EVALUATE`,
`REQUEST_MAGIC_LIST`, `NET_PING`, `REQUEST_SSQ_STATUS`, `REQUEST_BUY_PROCURE`
among them). `PetSkillData.xml` is unread but nearly irrelevant here: of its
1046 npc ids only 8 are reachable from an Interlude-range summon skill.
`FakePlayers`, `OfflinePlay`, `.lang` and `.changepassword` are all disabled in
this dist's config. The 17 `quests/not_done` classes load in Java and do
nothing, so their absence is parity.

Four families came back **clean**: chat handlers (all 13 registered ones land),
community-board handlers, user commands, and target handlers.

### Re-deriving these numbers

```sh
# rows 1, 2, 3 — action-bar handlers the dist declares, against the arms in
# `game_loop/player_actions.rs::dispatch`
grep -oE 'handler="[A-Za-z]+"' dist/game/data/ActionData.xml | sort | uniq -c | sort -rn

# row 6 — item handlers the dist actually uses, by item count
grep -rhoE 'name="handler"[[:space:]]+val="[A-Za-z]+"' dist/game/data/stats/items/*.xml \
  | sed 's/.*val="//;s/"//' | sort | uniq -c | sort -rn

# rows 7, 8 — how much HTML reaches an unported bypass
grep -rl player_help     dist/game/data/html | wc -l
grep -rl TerritoryStatus dist/game/data/html | wc -l

# row 10 — zone types in use, against `kind_from_type`
grep -rhoE 'type="[A-Za-z]+"' dist/game/data/zones/*.xml | sort | uniq -c | sort -rn

# row 12 — quests Java loads, against the ported scripts
grep -oE 'Q[0-9]{5}_[A-Za-z0-9]+\.class' \
  ../interlude_classic/dist/game/data/scripts/quests/QuestMasterHandler.java \
  | sed 's/_.*//;s/^Q//' | sort -u > /tmp/jq.txt
ls crates/gameserver/src/scripts/ | grep -E '^q[0-9]' | sed 's/_.*//;s/^q//' | sort > /tmp/rq.txt
# 53 rows. Discount Q00255 (ported as `tutorial.rs`, not `q00255_*.rs`) and
# the 17 `not_done` stubs, which load but do nothing in Java either, to reach
# **35** — the row's headline said 36.
comm -23 /tmp/jq.txt /tmp/rq.txt

# …then the part the id diff cannot tell you. For each survivor, check its
# NPCs are actually spawned and its items actually exist:
grep -rl 'id="31774"' dist/game/data/spawns/    # Q00933: no output = unreachable
grep -l  '<item id="49559"' dist/game/data/stats/items/*.xml   # Q11000: none

# row 14 — the strict form, per file. Java's `Config` reads keys through a
# parser variable per ini; map variable → file, intersect with the port's
# string literals, and what is left is unread. (The coarse form below
# over-reports badly; it is kept because it needs no script.)
python3 - <<'EOF'
import re, collections, glob
src = open('../interlude_classic/java/org/l2jmobius/Config.java').read()
parsers = dict(re.findall(r'final PropertiesParser (\w+) = new PropertiesParser\((\w+)\)', src))
consts = dict(re.findall(r'(?:public|private) static final String (\w+) = "\./config/([^"]+)"', src))
reads = collections.defaultdict(set)
for var, const in parsers.items():
    for m in re.finditer(re.escape(var) + r'\.get\w+\("([^"]+)"', src):
        reads[consts.get(const, const)].add(m.group(1))
seen = set()
for p in glob.glob('crates/**/*.rs', recursive=True):
    seen |= set(re.findall(r'"([A-Za-z0-9_]+)"', open(p, errors='replace').read()))
for f in ('Character.ini','General.ini','Server.ini','Feature.ini','NPC.ini','PVP.ini'):
    print(f, sorted(k for k in reads[f] if k not in seen))
EOF

# …and the two `CorrectPrices` measurements: how much of the shipped
# catalogue either half of the key actually moves. Both are zero, which is
# what `the_shipped_catalogue_is_the_same_either_way` and its multisell twin
# assert from the parser's own output.

# row 14 — coarse form, and **it over-reports**. A literal-only scan of the
# port misses every key it builds with `format!` (all 13 GrandBoss respawn
# keys, the flood-protector block, the PvP colour ladder), so the real figure
# is smaller than this prints. The strict form intersects with the keys Java's
# `Config` actually parses, narrows to the ten core .ini files, and expands
# the port's `format!` patterns before subtracting.
grep -rhoE '^[A-Za-z0-9_]+[[:space:]]*=' dist/game/config/*.ini dist/game/config/Custom/*.ini \
  | sed 's/[[:space:]]*=//' | sort -u > /tmp/ini.txt
grep -rhoE 'get_[a-z_]+\("[A-Za-z0-9_]+"' crates/ --include='*.rs' \
  | sed 's/.*("//;s/"//' | sort -u > /tmp/read.txt
comm -23 /tmp/ini.txt /tmp/read.txt | wc -l   # 862 of 1342 shipped keys

# …and the count is only the start. A key is a gap only if Java *reads* the
# field it fills; check each survivor's `Config.FIELD` against the tree:
grep -rn 'Config\.CLAN_LEVEL_6_COST' ../interlude_classic/java | grep -v Config.java  # none

# row 20 — `<cond>` blocks in the item datapack, and the attributes they use
grep -c '<cond' dist/game/data/stats/items/*.xml | awk -F: '{n+=$2} END {print n}'   # 2126
grep -h -A4 '<cond' dist/game/data/stats/items/*.xml \
  | grep -oE '<(player|target)[^>]*' | grep -oE '[a-zA-Z_]+=' | sort | uniq -c | sort -rn
# …and the same figures out of the *parser*, which is what
# `the_dist_parses_to_the_blocks_the_datapack_declares` asserts.

# row 16 — admin commands
grep -oE 'command="[a-zA-Z_0-9]+"' dist/game/config/AdminCommands.xml \
  | sed 's/.*command="//;s/"//' | sort -u > /tmp/ja.txt
grep -rhoE '"admin_[a-z_0-9]+"' crates/ --include='*.rs' | tr -d '"' | sort -u > /tmp/ra.txt
comm -23 /tmp/ja.txt /tmp/ra.txt

# row 17 — buy-list product lines, by whether they declare a price and a stock
grep -rhoE '<item [^>]*/>' dist/game/data/buylists/*.xml dist/game/data/buylists/custom/*.xml \
  | wc -l                                              # 12062 product lines
grep -rhoE '<item [^>]*/>' dist/game/data/buylists/*.xml | grep -vc 'price='   # 3079 bare
grep -rlE 'count="' dist/game/data/buylists/*.xml | wc -l                      # 147 files
grep -rhoE 'count="[0-9]+"' dist/game/data/buylists/*.xml | wc -l              # 1928 lines
```

Row 15 needs the opcode value, not the name — the two trees name the same
packet differently often enough that a name diff lies. Parse `(0x..,` out of
`network/ClientPackets.java`, **discarding the `null`-handler entries**, and
match against the `pub const …: u8` table in `network/client_packets.rs`. The
first cut of this audit skipped that filter and reported 34 missing where the
real figure is 21.

Two further corrections came out of working it. The port's arms are not all in
`dispatch.rs` — `PROTOCOL_VERSION` is handled a layer down in
`network/connection.rs`, so a `dispatch.rs`-only scan over-reports by one (21,
not the 20 recorded). And a missing arm is only a gap if Java's handler *does*
something: seven of the twenty turned out to be empty, unreachable, or gated on
state nothing sets. Re-run the scan over both files:

```sh
# every cop:: name referenced by either dispatch layer, resolved to its opcode
grep -rhoE 'cop::[A-Z_0-9]+' crates/gameserver/src/game_loop/dispatch.rs \
  crates/gameserver/src/network/connection.rs | sort -u
```

Rows 16 and 17 are the reason the commands above are a *starting* point.
A set difference over a datapack is a hypothesis: it tells you a name is
absent, not that a behaviour is. Row 16's survivors had to be checked against
Java's own handler registry — 34 of 78 turned out to be dead there too — and
row 17's two largest findings were not in the XML at all, but in the `Product`
constructor that reads it. Both times the correction came from reading the
**Java** side more carefully, not the port.

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
