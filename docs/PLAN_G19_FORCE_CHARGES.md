# G19 — Force/charges resource (FocusMomentum + EnergyAttack)

## Why this slice

Next after `TargetType::EnemyNot`: `AttackTrait` (7 learnable) was set aside
again — it needs a whole `TraitType` attacker-bonus/weakness system this port
doesn't have. `EnergyAttack` (9 learnable, 16 instances) was the other
candidate flagged two slices ago as needing the Dwarf/Human-Fighter "Force"
(`Player.charges`) resource first — this slice builds that resource and both
effects that touch it.

This isn't a niche mechanic: **Sonic Focus → Sonic Blaster/Buster** (and the
Orc/Dark Elf **Force Burst/Storm/Blaster** equivalents) are core early
warrior-class skills. 9 `EnergyAttack` skills (Double Sonic Slash 5, Sonic
Blaster 6, Sonic Storm 7, Sonic Buster 9, Force Burst 17, Force Storm 35,
Force Blaster 54, Triple Sonic Slash 261, Hurricane Assault 284) and 6
`FocusMomentum` skills (Sonic Focus 8, Focus Force 50, Sonic Rage 345, Raging
Force 346, …) are learnable. Before this slice every one of them parsed to an
empty effect list — the Force-builder skills did nothing, and the
Force-spending attacks were silent no-ops. It also closes three existing
`TODO`s: `PhysicalSoulAttack`/`MagicalSoulAttack`/`SoulBlow`'s doc comments
already said "×1 until charges are modeled" — those are follow-on work, not
touched here, but the resource they were waiting on now exists.

## What Java does

`Player._charges` (an `AtomicInteger`, never persisted — matches every other
transient combat resource on this port). Two producers:

- `FocusMomentum.instant()`: `+amount` (default 1), capped at
  `min(maxCharges, getValue(MAX_MOMENTUM, 8))`. **`MAX_MOMENTUM` is never set
  anywhere in this datapack** (no skill/item/npc stat touches it), so the
  hardcoded fallback `8` this call site passes is the *real* cap on this
  build, not a simplification — confirmed by grepping the whole datapack.
  Already at the cap: refused with SM 324, no gain. Otherwise SM 323
  (`"Your force has increased to level $s1"`) + `EtcStatusUpdate` (redraws
  the Force icon/count).
- `GetMomentum` (a periodic +1-per-tick toggle variant): only one skill in
  the whole datapack uses it (10200-10299 range, not learnable) — not
  ported. Its own Java default (`getValue(MAX_MOMENTUM, 0)`) would cap it at
  0 anyway with nothing else setting the stat, so it's effectively dead code
  in this datapack regardless.

One consumer: `EnergyAttack.instant()` — `charge = min(chargeConsume,
player.charges)`, `decreaseCharges(charge)` (which can only fail by asking
to remove more than the player has — never true here since `charge` is
pre-clamped — so the "unsuitable terms" refusal path is dead code in
practice, and isn't ported). Damage: `77 · ((pAtk · levelMod) + power) /
(pDef · pDefMod) · ssMod · critMod · weaponTraitMod · generalTraitMod ·
weaknessMod · attributeMod · (1 + charge · 0.1) · pvpPveMod`.

## What landed

- **`Player.charges: i32`** (`model/mod.rs`), transient, default 0 — no DB
  column, matching Java. Java's 10-minute idle-decay
  (`ResetChargesTask`/`restartChargeTask`) is **not** ported; TODO left at
  the field.
- **`SkillEffect::FocusMomentum { amount, max_charges }`** — gain, capped at
  `max_charges.min(8)`, SM 323/324 + `EtcStatusUpdate`.
- **`SkillEffect::EnergyAttack { power, critical_chance, p_def_mod,
  charge_consume }`** — spend + damage, sharing `PhysicalAttack`'s `77·((pAtk
  · levelMod) + power) / (pDef · pDefMod)` core and its established
  simplifications (no weapon/general trait, weakness, attribute or PvP/PvE
  multiplier terms — none of those are modeled anywhere on this port), times
  the new `1 + charge · 0.1` boost. `charge_consume` reads a **skill-level**
  `<chargeConsume>` tag (a sibling of `<targetType>`), not a child of the
  `<effect name="EnergyAttack">` element — Java's effect constructors
  receive the skill's whole merged param set, and this is the one field this
  port needed from outside the effect's own XML block.
- **`EtcStatusUpdate` (0xF9) now carries real charges** instead of a
  hardcoded 0 — `etc_status_update` gained a leading `charges: i32`
  parameter; all three call sites (`helpers::send_etc_status_update`,
  `expertise::refresh_expertise_penalty`, the enter-world initial burst)
  updated. This is what draws/updates the Force-count icon.

## Test

Two tests, both against the real dist datapack:

- `skills_tests::focus_momentum_builds_force_and_refuses_past_the_cap` —
  Sonic Focus (8, level 1, max 1 charge): first cast lands (0 → 1, already
  at the level-1 cap, SM 324); recasting at the cap gains nothing.
- `skills_tests::energy_attack_spends_charges_for_bonus_damage` — Sonic
  Blaster (6, level 1: power 369, `chargeConsume` 2) against a real dist
  monster (Gremlin, 20001) with 5 pre-set charges: spends 2 (5 → 3), deals
  `77·((pAtk·levelMod)+369)/pDef · 1.2` damage (forced no-crit via
  `world.forced_rolls`), matching the formula exactly.

**Debugging note for future NPC-damage tests in this file:** the second test
initially read 0 damage dealt despite the effect computing the right number
internally — `game_loop::combat::is_npc_oid` gates the whole damage-receiver
routing on `object_id >= FIRST_NPC_OBJECT_ID` (`0x4000_0000`), and an
arbitrary low test object id like `90001` silently fails that check and
routes to the (nonexistent) player-damage path instead, a no-op. Existing
tests use the shared `NPC_OID` constant (`= FIRST_NPC_OBJECT_ID`) plus an
offset for exactly this reason; a hand-picked small id works for target
*selection*/*refusal* tests (as the `TargetType::EnemyNot` slice's own tests
happened to, since they never exercised the damage path) but silently breaks
anything that needs the NPC to actually take damage.

## Deferred (not this slice)

- `AttackTrait` (7 learnable) — still needs the `TraitType` system.
- Java's 10-minute charge-decay task.
- `GetMomentum` — dead code in this datapack (see above), not ported.
- Wiring the new `energyChargesBoost`-style charge bonus into
  `PhysicalSoulAttack`/`MagicalSoulAttack`/`SoulBlow` (their own `×1`
  stand-ins, called out in their doc comments) — the resource they needed
  now exists, but updating those effects is separate follow-on work.
