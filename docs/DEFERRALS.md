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
| `TODO(G24)` | 14 | `game_loop/admin/castle.rs`, `game_loop/clan_hall_auction.rs`, `game_loop/clans.rs`, `game_loop/combat.rs`, +6 more |
| `TODO(G33)` | 15 | `config/offline_trade.rs`, `data/item_data.rs`, `game_loop/admin/editchar.rs`, `game_loop/admin/skills.rs`, `game_loop/position.rs`, +9 more |
| `TODO(G22)` | 11 | `game_loop/area_npcs.rs`, `game_loop/tamed_beast.rs`, `model/skill.rs`, `scripts/feedable_beasts.rs`, +6 more |
| `TODO(G30)` | 12 | `config/community_board.rs`, `data/skill_data.rs`, `game_loop/community_board.rs`, `game_loop/multisell.rs`, +2 more |
| `TODO(G34)` | 12 | `data/skill_data.rs`, `game_loop/death.rs`, `game_loop/npc_cast.rs`, `game_loop/skills/cast.rs`, +2 more |
| `TODO(G19)` | 11 | `game_loop/admin/effects.rs`, `game_loop/skill_enchant.rs`, `game_loop/skills/cast.rs`, `game_loop/skills/effects.rs`, +4 more |
| `TODO(G21)` | 9 | `data/npc_ai_skills.rs`, `game_loop/admin/cursed_weapons.rs`, `game_loop/npc_ai.rs`, `game_loop/npc_cast.rs`, +1 more |
| `TODO(G28)` | 9 | `game_loop/admin/cursed_weapons.rs`, `game_loop/cursed_weapon.rs`, `game_loop/events/tvt.rs`, `model/cursed_weapon.rs` |
| `TODO(G20)` | 5 | `data/skill_data.rs`, `game_loop/combat.rs`, `game_loop/duel.rs`, `game_loop/skills/effects.rs`, +1 more |
| `TODO(G23)` | 5 | `game_loop/bypass.rs`, `game_loop/grand_boss.rs`, `game_loop/target.rs`, `game_loop/valakas.rs` |
| `TODO(G29)` | 5 | `game_loop/admin/mounts.rs`, `game_loop/skills/cast.rs`, `game_loop/skills/effects.rs`, `game_loop/tests/servitor_tests.rs` |
| `TODO(G25)` | 4 | `game_loop/olympiad.rs`, `model/mod.rs`, `scripts/oly_manager.rs` |
| `TODO(G27)` | 4 | `game_loop/admin/instance.rs`, `game_loop/duel.rs`, `game_loop/user_commands.rs` |
| `TODO(G-pvp)` | 3 | `data/skill_data.rs`, `game_loop/skills/effects.rs`, `model/skill.rs` |
| `TODO(G18)` | 3 | `game_loop/death.rs`, `game_loop/pvp.rs`, `scripts/alliance_master.rs` |
| `TODO(G-later)` | 2 | `db.rs`, `network/server_packets/manor.rs` |
| `TODO(G14)` | 2 | `config/general.rs`, `model/mod.rs` |
| `TODO(G15)` | 2 | `game_loop/items.rs`, `game_loop/skills/effects.rs` |
| `TODO(G17)` | 2 | `game_loop/subclass.rs` |
| `TODO(G18.6)` | 2 | `game_loop/academy.rs`, `game_loop/clans.rs` |
| `TODO(G26.5)` | 2 | `game_loop/lottery.rs`, `game_loop/monster_race.rs` |
| `TODO(G7.5)` | 2 | `data/skill_data.rs` |
| `TODO(G15.5)` | 1 | `game_loop/options.rs` |
| `TODO(G24.5)` | 1 | `game_loop/boats.rs` |
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

That is the failure this file exists to catch: markers rot in the code the same
way prose rots in the docs, and a marker describing done work is worse than no
marker — it makes finished work look outstanding. Across two passes, **7 of 32
markers examined were stale (22 %)**, every one of them understating progress.

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
- **`TODO(G34)` (12)** — the skill epic's recorded residue, each argued in
  `PLAN_G34_SKILL_PARITY.md`: A3's `isSelfContinuous` icon rule (no effector on
  `ActiveBuff` to test), `nextAction=CAST` (needs an intention queue),
  `calcCrit`'s level-78 branch, the fort gate on `OpenDoor`, raid-minion
  detection for `Bluff`.

## Regenerating

```sh
grep -rn -oE 'TODO\(G[0-9._a-z-]*\)' crates/ | \
  awk -F: '{print $3}' | sort | uniq -c | sort -rn
```
