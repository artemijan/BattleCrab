# G19 — StatByMoveType + the player regen stat pipeline

## Why this slice

The ranking sweep after `Fear` left a three-way tie at 4 learnable skills:
`StatByMoveType` (51 skills), `MagicalAttackMp` (23) and `SilentMove` (19).
Everything above them is out of scope — `DefenceAttribute` (31, Kamael
elemental attributes) and the `Summon`/`SummonCubic`/`SummonNpc` family
(24/12/9, all G29).

`StatByMoveType` won on two counts:

1. **Two of its four skills are 100% dead.** Vital Force 148 and Clear Mind
   1297 carry *only* `StatByMoveType`, so they parsed to an empty effect list
   and were dropped whole — passives that did precisely nothing. (`SilentMove`'s
   four all carry other ported effects, so they at least land.)
2. **It exposed a much bigger gap behind it.** `StatByMoveType`'s payload is
   almost entirely `REGENERATE_HP_RATE`/`REGENERATE_MP_RATE`, and it turns out
   `regen_player` never read `StatModifiers` at all.

### The gap behind the gap

The effect ranking counts *unported effect names*. `HpRegen`/`MpRegen`/`CpRegen`
are in `EFFECT_REGISTRY`, so they count as ported — but nothing consumed the
stats they pumped. `game_loop::regen::regen_player` computed:

```rust
let regen = t.base_hp_regen(p.level) * level_mod * con_bonus * STANDING_STILL_REGEN_MULTIPLIER;
```

— no `mul`, no `add`, no move-type term, and a hard-coded standing multiplier.
So the real scope here is **25 learnable skills**, not 4:

| effect | learnable | examples |
|---|---|---|
| `MpRegen` | 12 | Focus Mind 191, Mana Recovery 214, Armor Mastery 142 |
| `HpRegen` | 8 | Regeneration 1044, Song of Life 265, Relax 226 |
| `StatByMoveType` | 4 | Vital Force 148, Esprit 171, Acrobatic Move 225, Clear Mind 1297 |
| `CpRegen` | 1 | Victories of Pa'agrio 1414 |

This is the same "parsed but unconsumed" shape as `ShieldDefenceRate` and
`PhysicalAttackRange` in earlier slices — worth noting that the ranking script
is structurally blind to it.

## What Java does

`StatByMoveType` is a three-field effect (`<stat>`, `<type>`, `<value>`) whose
`onStart` calls `CreatureStat.mergeMoveTypeValue(stat, type, value)` — writing
into `_moveTypeStats`, a map kept **separate** from the ordinary add/mul maps.
It is read back at finalize time:

```java
public static double defaultValue(Creature creature, Stat stat, double baseValue) {
    final double mul = creature.getStat().getMulValue(stat);
    final double add = creature.getStat().getAddValue(stat);
    return (mul * baseValue) + add + creature.getStat().getMoveTypeValue(stat, creature.getMoveType());
}
```

Because it is read against `creature.getMoveType()` *at read time*, the value
swings as the player stands/walks/runs with no stat recompute anywhere.

`Creature.getMoveType` is `isMoving() && isRunning()` → `RUNNING`,
`isMoving()` → `WALKING`, else `STANDING`; `Player` overrides it to return
`SITTING` while seated.

All three regen finalizers then share this block verbatim:

```java
if (player.isSitting())       baseValue *= 1.5; // Sitting
else if (!player.isMoving())  baseValue *= 1.1; // Staying
else if (player.isRunning())  baseValue *= 0.7; // Running
```

**Walking falls through all three branches and gets no multiplier at all** — so
walking regen (×1.0) is *worse* than standing still (×1.1). That is Java as
written, and it is now pinned by a test.

## What landed

- **`MoveType`** (`model/stats.rs`) + **`Stat::from_xml`** for the four stat
  names a `<stat>` element actually uses in this dist.
- **`StatModifierEffect.move_type: Option<MoveType>`** — the qualifier rides on
  the existing effect struct, so the whole buff pipeline (landing, stacking,
  removal, passive folding at `Player::from_char`) needed no changes at all.
  Always additive: `mergeMoveTypeValue` has no percent form, so `mode` is not
  consulted on that path.
- **`StatModifiers.by_move_type`** + `move_type_value()` — Java's separate
  `_moveTypeStats` map, deliberately *not* folded into `add` (that would apply
  the bonus in every locomotion state rather than the one it names).
  `apply_modifier` routes; the two rebuild-from-scratch sites clear it too.
- **`regen::move_type_of`** and **`regen::movement_regen_multiplier`**, and
  `regen_player` rewritten to end in Java's `Stat.defaultValue` shape
  (`mul * base + add + moveTypeValue`) for all three of HP/MP/CP. This is what
  makes the 21 `HpRegen`/`MpRegen`/`CpRegen` skills mean anything, and it
  retired a stale `TODO(G7)` claiming sitting/moving states didn't exist.
- **Evasion**: Acrobatic Move's `EVASION_RATE`-while-`RUNNING` folds in at
  `combat::combatant()`, the per-attack snapshot — *not* the cached
  `CombatStats`, which would need invalidating on every start and stop of
  movement. That matches Java, where the finalizer runs on demand.

## Tests

`game_loop::tests::move_type_tests` (9):

- `movement_regen_multipliers_match_java` — the four multipliers, including an
  explicit assertion that walking < standing.
- `move_type_follows_movement_and_run_flag` — the `getMoveType` derivation.
- `regen_rate_tracks_the_move_type` — end-to-end rate ratios (1.1 / 1.0 / 0.7).
  Before this slice all three were equal.
- `hp_regen_stat_modifiers_now_reach_the_regen_tick`,
  `mp_regen_stat_modifiers_now_reach_the_regen_tick` — the pumped-but-unread
  fix, for both `Diff` and `Per` modes.
- `move_type_effects_route_to_their_own_map` — qualified effects must not land
  in `add`.
- `stat_by_move_type_applies_only_in_its_own_state` — a `RUNNING` bonus applies
  running and not standing. Deliberately the *reverse* slope of the movement
  multiplier, so the multiplier alone can't satisfy it.
- `real_dist_stat_by_move_type_skills_parse` — all four learnable skills with
  their real level-1 values (Vital Force 1.9/1.9, Esprit 2.5/1.8, Clear Mind
  3.2 walking + 2.6 standing, Acrobatic Move 4.0 evasion).
- `vital_force_passive_folds_into_by_move_type` — the passive path, asserting
  the entry does *not* leak into `add`.

Note: the rate tests need a world with the **real** player templates loaded —
`GameData::for_test`'s synthetic ones have a zero `baseHpRegen`, which would
make every ratio assertion pass vacuously. `regen_world()` exists for that.

## Deferred (not this slice)

- **`MoveType::Sitting`** — sitting isn't modeled on this port (`TODO(G29)`).
  Parsed and stored, so the one dist skill that uses it (13200, a non-learnable
  belt item) round-trips; it starts applying with no further work once sitting
  lands.
- **The zone/residence multipliers** in `RegenHPFinalizer` (clan hall, castle,
  fort, Mother Tree, siege) — those need residence functions, not this slice.
- **`MagicalAttackMp` and `SilentMove`**, the other two of the tied cluster.
