# G19 — DamageBlock skill effect

## Why this slice

Next on a fresh ranking sweep after `AttackTrait` closed out the previous
batch: `DamageBlock` (5 learnable, 84 skills, 162 instances — the highest
raw count left, since a skill carries two separate `<effect>` elements, one
per block kind). Two existing TODOs already pointed at it —
`HealPercent`'s and `Lethal`'s doc comments both note "Java also skips this
while `effected.isHpBlocked()` — not gated, since that effect isn't ported
yet" — so this slice closes those too.

## What Java does

`DamageBlock.getEffectFlags()` — a pure state-flag effect, no `onStart`/
`onExit` logic — contributes `EffectFlag.HP_BLOCK` or `MP_BLOCK` depending on
its `type` param. The five learnable instances (Celestial Shield 1418,
Flames of Invincibility 1427, Dance of Medusa 367, Sonic Barrier 442, Force
Barrier 443) are all short (10-30s) full-invulnerability shields — each
skill carries *two* `DamageBlock` elements, one `BLOCK_HP` and one
`BLOCK_MP`.

`HP_BLOCK` has a real, single choke-point consumer:
`CreatureStatus.reduceHp`/`PlayerStatus.reduceHp` — `if
(creature.isHpBlocked() && !(isDOT || isHPConsumption)) return;` — refusing
essentially **all** incoming HP-reducing damage (auto-attack, every skill
effect, damage zones) except a DoT tick or a skill's own HP cost.

`MP_BLOCK` is a repeat of the `AttackTrait`/`MAX_MOMENTUM`/
`INSTANT_KILL_RESIST` pattern this run of slices keeps finding: `isMpBlocked()`
is defined but **has zero callers anywhere in the Java tree** (grepped
exhaustively). No MP-drain path checks it. It's dead code on the reference
server too.

## What landed

- **`effect_flag::HP_BLOCK`/`MP_BLOCK`** (`model/skill.rs`) +
  **`SkillEffect::DamageBlock { block_hp, block_mp }`** (one per `<effect>`
  instance, matching Java's per-instance shape) + the `"DamageBlock"` parse
  arm (`data/skill_data.rs`, reading the `type` string param directly —
  `param()`'s `f64` parser doesn't apply here).
- **`game_loop::abnormal::is_hp_blocked`** — the buff-flag read, mirroring
  `is_muted`/`is_debuff_blocked`'s exact shape.
- **The real choke point**: `game_loop::combat::apply_physical_damage`
  (already the single function every damage path funnels through — auto-attack,
  every instant-damage `SkillEffect`, DoT ticks, damage zones) gained an
  `is_dot: bool` parameter and an early `is_hp_blocked` return, mirroring
  Java's `reduceHp` gate exactly. Threading it: `apply_skill_damage` (9 call
  sites — 8 pass `false`, only the `DamOverTimeTick` handler passes `true`)
  and the two other `apply_physical_damage` callers (auto-attack: `false`;
  the damage-zone tick in `effect_zones.rs`: also `false` — Java's
  `DamageZone` calls the plain `reduceCurrentHp` overload, `isDOT` defaults
  `false`, so a damage zone genuinely *is* blocked by `HP_BLOCK`, unlike an
  abnormal-effect DoT).
- **Closed both existing TODOs**: `HealPercent`'s positive (heal) branch and
  `Lethal` both now bail on `is_hp_blocked`. `HealPercent`'s negative
  (damage) branch already got this for free, since it routes through the
  same `apply_skill_damage`/`apply_physical_damage` choke point.
- **Not wired**: `MP_BLOCK` — no consumer, matching Java's own dead
  `isMpBlocked()`.

## Test

`skills_tests::damage_block_refuses_incoming_hp_damage_except_a_dot` — real
dist data (skill 1418 "Celestial Shield", self-cast via `targetType TARGET`):
the buff lands with both `HP_BLOCK` and `MP_BLOCK` set; a huge non-DoT hit
through `apply_physical_damage` changes nothing; a DoT-flagged hit still
lands, exercising the one exemption end to end.

## Deferred (not this slice)

- `MP_BLOCK` — no Java consumer to port.
- The `isHPConsumption` exemption — a skill's own HP cost never routes
  through the damage choke point on this port (a direct `Vitals` mutation at
  cast time), so there was nothing to gate in the first place.
