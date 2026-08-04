# Recorded deferrals — the `TODO(G<N>)` inventory

Every milestone row in [PROGRESS.md](PROGRESS.md) is ✅ or an explicit
scope-out. That is true, and it is also **not the whole picture**: a milestone
is marked complete when its *gate* is met, and each one shipped with a handful
of narrow behaviours deferred and marked at the site. There are **140** such
markers. A reader looking only at the status table cannot see them.

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
`crates/gameserver/src/data/skill_data.rs`. It moves only when someone changes
it deliberately: adding a gap without recording it fails, and closing one
without updating the number fails too — the same two-way discipline G34's
close-out gate uses.

## Inventory

| marker | count | files |
|---|---:|---|
| `TODO(G33)` | 15 | `config/offline_trade.rs`, `data/item_data.rs`, `game_loop/admin/editchar.rs`, `game_loop/admin/skills.rs`, +9 more |
| `TODO(G24)` | 14 | `game_loop/admin/castle.rs`, `game_loop/clan_hall_auction.rs`, `game_loop/clans.rs`, `game_loop/combat.rs`, +4 more |
| `TODO(G34)` | 13 | `data/skill_data.rs`, `game_loop/death.rs`, `game_loop/npc_cast.rs`, `game_loop/skills/cast.rs`, +2 more |
| `TODO(G30)` | 12 | `config/community_board.rs`, `data/skill_data.rs`, `game_loop/community_board.rs`, `game_loop/multisell.rs`, +2 more |
| `TODO(G19)` | 11 | `game_loop/admin/effects.rs`, `game_loop/skill_enchant.rs`, `game_loop/skills/cast.rs`, `game_loop/skills/effects.rs`, +4 more |
| `TODO(G22)` | 11 | `game_loop/area_npcs.rs`, `game_loop/tamed_beast.rs`, `scripts/feedable_beasts.rs`, `scripts/forge_of_the_gods.rs`, +5 more |
| `TODO(G28)` | 9 | `game_loop/admin/cursed_weapons.rs`, `game_loop/cursed_weapon.rs`, `game_loop/events/tvt.rs`, `model/cursed_weapon.rs` |
| `TODO(G21)` | 8 | `data/npc_ai_skills.rs`, `game_loop/admin/cursed_weapons.rs`, `game_loop/npc_ai.rs`, `game_loop/npc_cast.rs` |
| `TODO(G20)` | 5 | `data/skill_data.rs`, `game_loop/combat.rs`, `game_loop/duel.rs`, `game_loop/skills/effects.rs`, +1 more |
| `TODO(G23)` | 5 | `game_loop/bypass.rs`, `game_loop/grand_boss.rs`, `game_loop/target.rs`, `game_loop/valakas.rs` |
| `TODO(G27)` | 4 | `game_loop/admin/instance.rs`, `game_loop/duel.rs`, `game_loop/user_commands.rs` |
| `TODO(G29)` | 4 | `game_loop/admin/mounts.rs`, `game_loop/death.rs`, `game_loop/tests/servitor_tests.rs` |
| `TODO(G-pvp)` | 3 | `data/skill_data.rs`, `game_loop/skills/effects.rs`, `model/skill.rs` |
| `TODO(G14)` | 2 | `config/general.rs`, `model/mod.rs` |
| `TODO(G15)` | 2 | `game_loop/items.rs`, `game_loop/skills/effects.rs` |
| `TODO(G17)` | 2 | `game_loop/subclass.rs` |
| `TODO(G18)` | 2 | `game_loop/death.rs`, `game_loop/pvp.rs` |
| `TODO(G18.6)` | 2 | `game_loop/academy.rs`, `game_loop/clans.rs` |
| `TODO(G26.5)` | 2 | `game_loop/lottery.rs`, `game_loop/monster_race.rs` |
| `TODO(G7.5)` | 2 | `data/skill_data.rs` |
| `TODO(G-later)` | 1 | `network/server_packets/manor.rs` |
| `TODO(G15.5)` | 1 | `game_loop/options.rs` |
| `TODO(G24.5)` | 1 | `game_loop/boats.rs` |
| `TODO(G25)` | 1 | `game_loop/olympiad.rs` |
| `TODO(G32)` | 1 | `game_loop/fishing.rs` |
| `TODO(G7)` | 1 | `data/player_template.rs` |

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

## Regenerating

```sh
grep -rn -oE 'TODO\(G[0-9._a-z-]*\)' crates/ | \
  awk -F: '{print $3}' | sort | uniq -c | sort -rn
```
