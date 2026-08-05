# Recorded deferrals — the `TODO(<tag>)` inventory

Every milestone row in [PROGRESS.md](PROGRESS.md) is ✅ or an explicit
scope-out. That is true, and it is also **not the whole picture**: a milestone
is marked complete when its *gate* is met, and each one shipped with a handful
of narrow behaviours deferred and marked at the site. There are **147** such
markers — the sum of the inventory below, and of the expected list the
`deferral_markers_match_the_recorded_inventory` test holds the code to. A reader
looking only at the status table cannot see them.

Because every milestone row is now ✅, this residue **is** the remaining work on
the port. The list below is the backlog, not a footnote to it.

This file is that missing half — generated from the code, not written by hand,
so it cannot drift into fiction the way prose does.

## Why this exists

Two documentation bugs found on 2026-08-03 prompted it, both the same shape:

- G33's plan listed "Slice 3 — periodic autosave" as pending work that had in
  fact been complete and tested for milestones.
- The `Custom/*.ini` row claimed "the 16 unported operator features" when all
  17 were ported, consumed and tested. Two separate documents had drifted to
  the same wrong claim, and one of them had been *re-introduced* from memory.

Both rotted in the safe direction — the code was ahead of the notes. A sweep of
the five `TODO(G<N>)` markers PROGRESS claimed but the code did not contain
found the same: all five were done (the manual-learn `<item>` cost, the
HATE-effect aggro skip, academy-graduation rewards, sub-unit tabs, ally
crests). Prose about what is left is the least reliable artefact in the repo.
Markers in the code are the reliable one, so they are what this file counts.

## What a marker means

Per `CLAUDE.md`: when a port intentionally skips part of the Java behaviour,
leave a `TODO(G<N>): …` at the exact site naming the Java source of the skipped
behaviour. A marker is therefore **a deliberate, recorded gap** — not a bug and
not a reminder to tidy. `TODO(G24)` on a closed G24 means "G24 shipped its gate
and left this specific thing", which is the intended end state, not a
contradiction.

The count is asserted by `deferral_markers_match_the_recorded_inventory` in
`crates/gameserver/src/data/skill_data/coverage_census.rs`. It moves only when
someone changes it deliberately: adding a gap without recording it fails, and
closing one without updating the number fails too — the same two-way discipline
G34's close-out gate uses.

Markers come in two families. **Milestone tags** (`G<N>`) hang off a shipped
milestone's gate. **Topic tags** (lowercase — `pets`, `manor`, `radar`, …) are
gaps that never had a milestone to hang off; they are deferrals just the same.
A tag must parse as alphanumerics plus `. - _ + ? /` — no spaces, no `<`, no
quotes — or the scanner cannot see it. That is also why prose in a `.rs` file
must never spell a *parseable* tag: it would be counted as a marker.

### `TODO` versus `SKIP`

A `TODO(<tag>)` is **work that is missing**. A `SKIP(<tag>)` is **work that was
examined and deliberately not done**, where the reason is a property of the
datapack rather than of the port — Java code that no route on this dist can
reach, content from a later chronicle, a branch whose other end does not exist.
`SKIP` is not counted by the inventory, because counting it would inflate the
backlog with entries nobody should ever action.

The distinction is load-bearing in one direction only: **downgrading a real gap
to `SKIP` hides it**, and the inventory will not catch that. So a `SKIP` must
carry the evidence in the comment — which page fails to offer the button, which
NPC id is registered nowhere — not merely the assertion. `academy.rs`'s
squad-skills note is the model: it names the item ids, the clan level and the
chronicle marker that put the content out of reach.

When a `SKIP` marks dead Java that was nonetheless ported verbatim, say so and
say what would have to change to revive it. Half-restoring a two-ended dead
route is how you get a trap: quest 415's alternate ending consumes Rosheek's
letter and hands out no recommendation, so wiring up its entry page without
also registering its NPCs would strand the player.

## Inventory

| marker | count | files |
|---|---:|---|
| `TODO(G33)` | 15 | `config/offline_trade.rs`, `data/item_data.rs`, `game_loop/admin/editchar.rs`, `game_loop/admin/skills.rs`, `game_loop/admin/transforms.rs`, `game_loop/chat.rs`, `game_loop/combat/intent.rs`, `game_loop/death/restart.rs`, `game_loop/dispatch.rs`, `game_loop/offline_trade.rs`, `game_loop/party.rs`, `game_loop/position.rs`, `game_loop/servitor.rs` |
| `TODO(G24)` | 14 | `game_loop/admin/castle.rs`, `game_loop/clan_hall_auction.rs`, `game_loop/clans/membership.rs`, `game_loop/combat/intent.rs`, `game_loop/siege.rs`, `game_loop/target.rs`, `model/skill.rs`, `scripts/q00234_fates_whisper.rs` |
| `TODO(G34)` | 13 | `data/skill_data/build.rs`, `game_loop/death/player_death.rs`, `game_loop/npc_cast.rs`, `game_loop/skills/cast.rs`, `game_loop/skills/conditions.rs`, `game_loop/skills/effects/control.rs`, `game_loop/skills/effects/mod.rs`, `game_loop/skills/effects/ticks.rs`, `game_loop/skills/instant.rs` |
| `TODO(G30)` | 12 | `config/community_board.rs`, `data/skill_data/coverage_census.rs`, `game_loop/community_board.rs`, `game_loop/multisell.rs`, `model/skill.rs`, `network/client_packets.rs` |
| `TODO(G19)` | 11 | `game_loop/admin/effects.rs`, `game_loop/skill_enchant.rs`, `game_loop/skills/cast.rs`, `game_loop/skills/effects/mod.rs`, `game_loop/tests/mana_restore_tests.rs`, `model/mod.rs`, `model/skill.rs`, `network/enter_world.rs` |
| `TODO(G22)` | 11 | `game_loop/area_npcs.rs`, `game_loop/tamed_beast.rs`, `scripts/feedable_beasts.rs`, `scripts/forge_of_the_gods.rs`, `scripts/primeval_isle.rs`, `scripts/q00224_test_of_sagittarius.rs`, `scripts/q00227_test_of_the_reformer.rs`, `scripts/q00230_test_of_the_summoner.rs`, `scripts/sin_eater.rs` |
| `TODO(G28)` | 9 | `game_loop/admin/cursed_weapons.rs`, `game_loop/cursed_weapon.rs`, `game_loop/events/tvt.rs`, `model/cursed_weapon.rs` |
| `TODO(G21)` | 8 | `data/npc_ai_skills.rs`, `game_loop/admin/cursed_weapons.rs`, `game_loop/npc_ai.rs`, `game_loop/npc_cast.rs` |
| `TODO(G20)` | 5 | `data/skill_data/build.rs`, `game_loop/combat/attack.rs`, `game_loop/duel.rs`, `game_loop/skills/effects/mod.rs`, `model/formulas.rs` |
| `TODO(G23)` | 5 | `game_loop/bypass.rs`, `game_loop/grand_boss.rs`, `game_loop/target.rs`, `game_loop/valakas.rs` |
| `TODO(G27)` | 4 | `game_loop/admin/instance.rs`, `game_loop/duel.rs`, `game_loop/user_commands.rs` |
| `TODO(G29)` | 4 | `game_loop/admin/mounts.rs`, `game_loop/death/rewards.rs`, `game_loop/tests/servitor_tests.rs` |
| `TODO(G-pvp)` | 3 | `data/skill_data/build.rs`, `game_loop/skills/effects/mod.rs`, `model/skill.rs` |
| `TODO(G14)` | 2 | `config/general.rs`, `model/mod.rs` |
| `TODO(G15)` | 2 | `game_loop/items.rs`, `game_loop/skills/effects/gathering.rs` |
| `TODO(G17)` | 2 | `game_loop/subclass.rs` |
| `TODO(G18)` | 2 | `game_loop/death/rewards.rs`, `game_loop/pvp.rs` |
| `TODO(G18.6)` | 2 | `game_loop/academy.rs`, `game_loop/clans/membership.rs` |
| `TODO(G21+)` | 2 | `model/skill.rs`, `scripts/q00414_path_of_the_orc_raider.rs` |
| `TODO(G26.5)` | 2 | `game_loop/lottery.rs`, `game_loop/monster_race.rs` |
| `TODO(G29+)` | 2 | `model/components.rs`, `network/enter_world.rs` |
| `TODO(G7.5)` | 2 | `data/skill_data/build.rs` |
| `TODO(login-playauth)` | 2 | `tests/e2e_create.rs` |
| `TODO(D4)` | 1 | `dashboard_api: routes/status.rs` |
| `TODO(G-later)` | 1 | `network/server_packets/manor.rs` |
| `TODO(G?)` | 1 | `model/mod.rs` |
| `TODO(G13+)` | 1 | `scripts/q00416_path_of_the_orc_shaman.rs` |
| `TODO(G15.5)` | 1 | `game_loop/options.rs` |
| `TODO(G19+)` | 1 | `data/skill_data/build.rs` |
| `TODO(G24.5)` | 1 | `game_loop/boats.rs` |
| `TODO(G24/G26)` | 1 | `scripts/castle_chamberlain.rs` |
| `TODO(G25)` | 1 | `game_loop/olympiad.rs` |
| `TODO(G32)` | 1 | `game_loop/fishing.rs` |
| `TODO(G35)` | 1 | `commons/src/audit.rs` |
| `TODO(G7)` | 1 | `data/player_template.rs` |
| `TODO(G9+)` | 1 | `data/skill_data/mod.rs` |
| `TODO(skill-see-range)` | 1 | `game_loop/skills/cast.rs` |

## Closed

Markers retired, newest first. A row here is a marker that left the code *and*
the inventory in the same commit — the two-way discipline in both directions.

| date | marker | what closed it |
|---|---|---|
| 2026-08-05 | `TODO(manor)` ×2 (`game_loop/manor.rs`) | Both real, both closed. **Persistence:** Java saves per action when `AltManorSaveAllActions` is on and otherwise runs a `storeMe` autosave every `AltManorSavePeriodRate` hours *plus* one on shutdown — the port had `store_manor` but called it only at rollover, and the shutdown sweep (which saves players, bosses, olympiad, cursed weapons) did not include the manor. All three paths ported, with the new `AltManorSavePeriodRate` key. **Weight/capacity:** added `weight::validate_weight` / `validate_capacity` / `slots_needed` mirroring `PlayerInventory`, and wired them into `RequestBuySeed` in **Java's order** — weight, then slots, then adena, so an overloaded pauper is told about the weight. |
| 2026-08-05 | `TODO(frintezza-4b)` ×1 (`game_loop/frintezza.rs`) | The marker was right, and **undercounted**: Java calls `playRandomSong` at *four* sites — the intro, both Scarlet morphs, and the 90 s timer — and the port only had the timer. Only the first morph carried a marker. Split `handle_song` (timer: play + re-arm) from `play_song` (Java's `playRandomSong`), because calling the timer entry point from a morph would have given each morph its own duplicate 90 s timer. Also cleared a stale doc comment claiming the 5008 debuff was unported, contradicted by the code 15 lines below it. |
| 2026-08-05 | `TODO(saga)` ×1 (`scripts/saga.rs`) | **The marker named something that does not exist.** Neither `givePormanders` nor `SkillTransfer` appears anywhere in this dist's Java or datapack. Reading what the saga quests *actually* do at the transfer exposed a real bug instead: the port cast `4339` (quest 235's elixir flash) via `cast_visual`, which emits two *self*-casts, where all 40 sites across the 31 saga quests broadcast `MagicSkillUse(npc, player, 5103, 1, 1000, 0)`. Fixed with `cast_visual_at`. Note the transfer legitimately produces **two** 5103 casts — `Player.setClassId` broadcasts its own self-cast first — which the test now pins by caster/target rather than by skill id. |
| 2026-08-05 | `TODO(q214-gargoyle-name)` ×1 (`scripts/q00214_trial_of_the_scholar.rs`) | Settled on **"Reinforced Gargoyle"** — the name three of the five disagreeing surfaces already used, including the client's own `ItemName` table. The operator renamed the client `QuestName` entry; `30612-04.html` was aligned to match and the constants renamed `ENCHANTED_*` → `REINFORCED_*`. Recorded in [CUSTOM_DIST_DEVIATIONS.md](CUSTOM_DIST_DEVIATIONS.md), because a re-sync from the Java reference dist would silently restore the retail wording. Sabotage-verified. |
| 2026-08-05 | `TODO(sieges)` ×1 (`network/server_packets/residence.rs`) | A **subsystem-level claim** that G24 falsified — the header said "sieges aren't ported yet", so the world-map overlay reported every castle unowned. Everything it needed already existed (`world.castles`, the clan `castle_id` back-reference, `castle::tax_percent`, `world.sieges`). `ExShowCastleInfo` now carries owner, tax, siege date and side. The **fortress** overlay stays static, and that is not a deferral: fort sieges are an explicit scope-out, so no fort on this dist can have an owner — the header now says that instead of implying deferred work. |
| 2026-08-05 | `TODO(quests)` ×1 (`scripts/q00641_attack_sailren.rs`) | **Stale.** "Gated until Q00126_TheNameOfEvil2 is ported" — it is ported *and* registered, the gate already calls `other_quest_completed`, and the test exercises both branches. Nothing to do but delete the claim. |
| 2026-08-05 | `TODO(reco)` ×1 (`game_loop/reco.rs`) | **Not a gap** — retagged `SKIP(fake-players)`. Recommending a `fakePlayerTalkable` NPC belongs to a Mobius `config/Custom/*` feature that ROADMAP.md scopes out except where an operator enables it, and this dist ships `EnableFakePlayers = False`. Reviving it means porting `FakePlayerData`/`FakePlayerInfo` first, not adding a branch. |
| 2026-08-05 | `TODO(login-playauth)` ×2 — **not closed, corrected** (`tests/e2e_create.rs`) | The recorded cause was false. It claimed `RequestServerLogin` answers PlayFail instead of PlayOk; instrumenting the handshake shows **PlayOk on both logins** and the whole login half completing cleanly. The real failure is on relogin: the list loads and `CharSelectionInfo` is sent, then `handle_request_restart` runs *unprompted* (the test never sends 0x57, and that handler has one non-test caller), which reloads the list and puts a second `CharSelectionInfo` where the client expects `CharSelected`. The exchange never resynchronises, so the test **hangs** rather than failing an assertion — which is why the wrong guess survived: nothing contradicted it. Marker rewritten with the walkthrough; still open, still `#[ignore]`. |
| 2026-08-05 | `TODO(pets)` ×5 (`scripts/q00421_little_wings_big_adventure.rs`) | Java's `npc.setTarget(x); npc.doCast(skill)` — **real** casts (root, DoT, debuff), not visuals. Added `QuestCtx::npc_cast`, which routes through the same `npc_cast::start_cast` the boss AIs use, plus `npc_hp_ratio` for the HP-gated arm. Dryad Root is cast at **level 33**, not 1; the level sets the root's strength and Java's `SkillHolder` carries it. The test initially passed with the casts silently doing nothing, because the fixture uses `SkillData::empty()` and an unregistered skill id makes `npc_cast` return false — see the note in `tests/mod.rs`. |
| 2026-08-05 | `TODO(cinematic)` ×1, `TODO(cosmetic)` ×1 (`scripts/q00235_mimirs_elixir.rs`, `q00125_the_name_of_evil_1.rs`) | Both wanted one `MagicSkillUse` from a named caster at a named target, which `cast_visual` could not express (it emits *two* self-casts). Added `QuestCtx::cast_visual_at`. The Q125 marker **undercounted its own gap threefold**: Java casts the pillar flourish at all three Kaimu (Ulu cond 5, Balu 6, Chuta 7) and only Ulu carried a marker — the other two were missing it with nothing to say so. |
| 2026-08-05 | `TODO(newbie-guide)` ×2 (`scripts/q00261_collectors_dream.rs`, `q00276_totem_of_the_hestui.rs`) | Ported `giveNewbieReward` for real. The marker's stated blockers were half-right: `ExShowScreenMessage` existed but only in its TEXT form, so the NpcString variant was added (4155, "Last duty complete"). `GUIDE_MISSION` still has no reader — the mission-list UI is unported — but it is written anyway because it **persists**, so credit earned now survives until that UI lands. Two traps found while wiring: it must be a *player* variable, not a `QuestState` one (both callers `exit_quest`, which would drop it on the very turn-in that earned it), and Java's `getString(key, null)` check means an absent variable and a stored 0 take different branches. |
| 2026-08-05 | `TODO(dead)` ×6 (`scripts/q00414`, `q00415`, `q00416`) | **Not gaps.** These mark Java branches that were ported verbatim but that no route on this dist can reach — no page offers the entry html, and the NPCs are registered nowhere. Retagged `SKIP(dead)`; see the `TODO` versus `SKIP` note above. Nothing changed in behaviour, and the comments' warnings against half-restoring a two-ended dead route are kept intact. |
| 2026-08-05 | `TODO(manor)` ×3 (`game_loop/skills/effects/gathering.rs`) | Same shape as the soul-crystal set: the ids (889/890/891) were in `commons::system_messages` all along, absent only from the hand-maintained `sm_ids`. Wiring them surfaced two things the markers did not mention — Java **party-broadcasts** the sow result (`party.broadcastPacket`), and the success leg also plays `ITEMSOUND_QUEST_ITEMGET`, which this port dropped silently. Both ported. |
| 2026-08-05 | `TODO(soul-crystal)` ×4 (`scripts/q00350_enhance_your_weapon.rs`) | **Half stale.** The `sm_ids` claim was true — that hand-maintained list had no soul-crystal entries — but `commons::system_messages` has carried all four ids (974/975/976/978) the whole time. Added them to `sm_ids` and wired Q350's three sites. The fifth marker with this tag was *not* closed: it sits in `skills/cast.rs`, describes `onSkillSee` breadth rather than anything about crystals, and was retagged `skill-see-range`. |
| 2026-08-05 | `TODO(radar)` ×3 (`scripts/q00348_an_arrogant_search.rs`) | **Stale.** They said the radar pings were unported; `QuestCtx::add_radar` / `add_quest_radar` / `clear_radar` all exist and Q211/Q214 use them. Wired Q348's two sites to the helpers. Closing them first exposed a real bug in `add_radar` itself — see the commit before. |

**First use, 2026-08-03.** Triaging the 15 `TODO(G33)` markers against the code
found two describing work that is **already done**:

- the delegated clan-leader transfer, delivered by
  `daily_tasks::clan_leader_apply` on the weekly reset — exactly as Java gates
  it on Wednesday. Marker removed (15 → 14).
- the auto-play/auto-potions voiced commands, which landed with the
  `Custom/*.ini` audit. That marker also covered `.premium` and `.password`,
  which really are unported, so it was **narrowed** rather than deleted.

A third, item expiry, is a *real* deferral that now records why: all 3230
time-limited items on this dist are id ≥ 10015 — entirely post-Interlude — so
nothing reachable here expires and the timer would have no consumer.

**Second pass, same day: G24, G22 and G21.** Five more markers described
finished work.

- Two siege **module headers** (`game_loop/siege.rs`, `model/siege.rs`) still
  called the battlefield "a later milestone" — control and flame towers, siege
  guards, the siege zone and its PvP, siege flags, teleport and
  ownership-on-victory all landed with G24. A third header
  (`admin/castle.rs`) already said so, which is how the contradiction showed.
- `model/castle.rs` listed residential skills as deferred; they landed with
  G24 (`clans::grant_residential_skills_to_clan`). Castle *functions* and
  crests really are still out, so that header was narrowed rather than cleared.
- `model/skill.rs` said `RESURRECTION_SPECIAL` "has no source yet" — it landed
  in **this repo's own G34 S4.16**, and the flag sits 111 lines below the
  comment denying it. Obsoleted by work in the same session that later wrote
  this file.
- `data/npc_ai_skills.rs` said "no Resurrection effect is ported yet". One is
  (G17's `revive_request`); the real reason the AI never revives is that **no
  NPC on this dist carries a resurrect skill**, which is a datapack fact and is
  now what the comment says.

**Third pass: G30, G28, G19.** Two more, and a distinction worth keeping.

- The **TvT module header** still called per-kill scoring, respawn and winner
  rewards "slices 3–4". All three landed: `on_player_death` is called from
  `death::player_do_die` for every death, and `reward_team` pays the winners.
  What is genuinely left — party/command-channel grouping, the logout forfeit
  listener, and Java's immobilise + skill-lock during the freeze — is now what
  the header says.
- The **`Lethal` effect's** deferral list named two subsystems as missing that
  have since landed: `abnormal::is_hp_blocked` (G19's `DamageBlock`) and
  `effects::calc_counter_attack` (G34 S4.4). **The gaps are real — the lethal
  arm does not consult either — but the stated reasons were wrong.** That is a
  different failure from a stale marker and a more insidious one: the deferral
  looks justified by an absence that no longer exists, so nobody re-examines
  it. Corrected to say what is actually left: wiring, not absence.
- The cursed-weapon markers (drop-on-PK-death, the "hungry" HP drain) were
  checked and are **accurate** — neither is implemented.

**Fourth pass: G20, G23, G25, G27, G29 and the small groups.** Four more, all
of the same kind — a marker naming a *subsystem* as absent when that subsystem
had since landed:

- Two sites (`skills/effects.rs`'s fear check, `skills/cast.rs`'s `isPlayable`
  narrowing) said "servitors are `TODO(G29)`". Servitors landed **with G29**.
  Neither site had a gap at all: Java's summon leg folds into the same branch,
  so the comments described a missing subsystem that exists and a gap that
  never was.
- Two more (`model/mod.rs`, twice) said "Olympiad crowning is unported
  (TODO(G25))". It is ported — `olympiad::crown` calls
  `admin::hero::set_hero`, and the period end clears it.

That is the failure this file exists to catch: markers rot in the code the same
way prose rots in the docs, and a marker describing done work is worse than no
marker — it makes finished work look outstanding.

**Fifth pass: the module headers.** The rule the fourth pass produced —
subsystem-level claims rot, itemised ones do not — was used to aim this one.
Of the 22 markers sitting in `//!` headers, two were stale, and both were
exactly that shape:

- `scripts/oly_manager.rs` said the class leaderboards, the point→mark exchange
  and the reward multisell "need persistence/scoring that lands in later
  slices". All three landed with G25 and are dispatched in that same file —
  `rank_detail`, `calculate_points_done`, and `showEquipmentReward` calling
  `multisell::separate_and_send`.
- `scripts/alliance_master.rs` said both of its buttons "log-drop until G18
  lands", the alliance system being "G18 work". G18 landed; `bypass.rs` routes
  `create_ally` and `dissolve_ally` to `clans::handle_create_ally` /
  `handle_dissolve_ally`.

**Sixth pass: the remainder, line by line.** Six more, and one of them cuts
against the pattern:

- `db.rs` said `storeCharSub`/`storeEffect` "need systems that don't exist yet
  (subclasses, buff restore on login)". Both exist — subclasses with G17, buff
  restore with G19's relative `remaining_time` rows.
- `items.rs` narrowed item use with "no pets (none exist yet), no Olympiad
  guard (no Olympiad)". Both subsystems landed (G29, G25); the path simply
  never routed to them, which is a different and smaller statement.
- `skills/effects.rs`'s sweep said "item weight/slot limits aren't modeled".
  Weight is modelled (`game_loop::weight`, plus G34 S4.1's `WEIGHT_LIMIT` /
  `WEIGHT_PENALTY`); the sweep just does not consult it.
- `model/mod.rs` carried `TODO(G21): cursedOnLogin`; `cursed_weapon::
  on_enter_world` (G28) does exactly that.
- **`death.rs` claimed shadow items were "absent from this dist". They are
  not — 295 sit in the Interlude id range, and `items.rs` already models their
  mana.** Correcting that exposed a **real bug**: the drop filter implements
  every leg of Java's `isShadowItem() || isTimeLimitedItem() || !isDropable()
  || ADENA || TYPE2_QUEST` **except `isShadowItem()`**, so a shadow item can
  drop on death here when Java would keep it. **Fixed** rather than recorded:
  the filter now carries the missing leg, reading the *instance's* `mana_left`
  (Java `Item._mana >= 0`) rather than the template, because two copies of one
  item id can differ. Sabotage-verified.

That last one is the counter-example worth recording: a **false** claim of
absence hid a live gap, where every other stale marker had merely overstated
one. The first draft of this pass removed the line as "not a real deferral" —
reading the filter, rather than the comment above it, is what caught that.

**Tally across six passes: 21 of ~134 markers examined were stale or
misjustified (16 %)**, every one understating progress. The dominant failure is
not "this small thing is still missing" — those held up almost perfectly — but
*"subsystem X is not ported"* written before X landed and never revisited.
Module headers are where those live, which makes them the place to look first.

## How to read the big ones

- **`TODO(G24)` (14)** — sieges shipped their gate; what is deferred is narrow
  and named: castle crests, the members-inside-the-zone fame task,
  `Castle.removeUpgrade()`, `ItemAction`'s mercenary-ticket refusal, a
  scheduled hit-time delay.
- **`TODO(G33)` (15)** — the optional tooling tail: `//geosave`, the niche
  admin tools, scheduled restart, multilang, Dockerfile parity. One is not
  optional but *impossible*: the GM Sayune click-to-move mode
  (`game_loop/position.rs`) needs `ExFlyMove`/`ExFlyMoveBroadcast`
  (`0xFE:0xE8` / `0xFE:0x108`), Ertheia-era opcodes with no counterpart in the
  Interlude protocol — the hop is ported with a `FlyToLocation(DUMMY)`
  substitute.
- **`TODO(G34)` (13)** — the skill epic's recorded residue, each argued in
  `PLAN_G34_SKILL_PARITY.md`: A3's `isSelfContinuous` icon rule (no effector on
  `ActiveBuff` to test), `nextAction=CAST` (needs an intention queue),
  `calcCrit`'s level-78 branch, the fort gate on `OpenDoor`, raid-minion
  detection for `Bluff`. Plus one added since: `BlockActions.onStart` ports
  Java's `startParalyze` *cast* abort but not its `abortAttack()` leg — a
  scheduled `AttackHit` has no cancel handle in this port, so a stun landing
  between a swing's start and its hit tick still lets that hit land
  (`game_loop/skills/effects.rs`, `apply_block_actions_interrupt`).

**Seventh pass, 2026-08-05: the counter could not see 46 of its own subjects.**
Every pass above triaged markers the inventory *knew about*. This one audited
the inventory itself, and found the number it asserts — 134 — was never the
number in the code. It was 180.

The scan looked for the literal `TODO(G` and accepted only alphanumerics, `.`,
`-` and `_` in a tag. Two families fell through:

- **Nine milestone markers** written with a `+`, `?` or `/` suffix — `G9+`,
  `G13+`, `G19+`, `G21+` (×2), `G29+` (×2), `G?`, `G24/G26`. Every one is a
  deliberate, correctly-placed deferral. The tag simply failed the character
  allowlist, so it was dropped in silence.
- **Thirty-seven topic-tagged markers** — `pets`, `manor`, `soul-crystal`,
  `radar`, `dead`, `login-playauth` and the rest — which never matched the
  `TODO(G` prefix at all, so no version of this file ever listed them.

The `Regenerating` recipe below had the identical blind spot (it required a
`)` immediately after `[0-9._a-z-]*`), which is why the doc and the test agreed
with each other and both disagreed with the code. **Two artefacts derived from
the same wrong assumption do not cross-check each other** — that is the lesson
worth keeping, and it is a sharper version of the one this file opens with.

The fix widens the scan to every `TODO(<tag>)` and moves the burden onto the
allowlist: `+ ? /` are now legal tag characters, `<`, quotes and whitespace are
not. One marker was renamed to comply (`frintezza slice 4b` → `frintezza-4b`;
a tag cannot contain spaces). The widened scan immediately caught a parseable
tag written inside a doc comment in its own source file, which is the intended
behaviour and the reason that constraint is documented above.

No marker was closed by this pass and none was stale — 134 → 180 is a
correction to the *count*, not a regression in the code. The port did not get
worse; the ledger got honest.

## Regenerating

```sh
grep -rn -oE 'TODO\([A-Za-z0-9][A-Za-z0-9._/+?-]*\)' crates/ | \
  awk -F: '{print $3}' | sort | uniq -c | sort -rn
```
