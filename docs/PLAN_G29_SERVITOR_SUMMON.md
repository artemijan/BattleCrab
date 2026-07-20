# G29 slice 1 — Servitor summoning & lifecycle

## Why this first

G29's gate is *"summon a servitor that follows and attacks; summon a pet, feed
it, and it persists."* The servitor half starts here because `Summon` is also
the **single biggest unported effect on the whole ranking** — 24 learnable
skills (Summon Dark Panther 283, Summon Kat the Cat 1111, Summon Shadow 1128,
the golems, …), every one of which currently casts and produces nothing.

## Design decision: a servitor is an NPC with an owner

Java models `Summon` as its own `Creature` subclass. This port instead marks an
ordinary NPC entity with a [`ServitorOf`] component. A servitor already *is*
"a template, stats, a position and an AI" — everything the NPC entity provides —
and the genuinely new state is only the owner link, the summoning skill and the
lifetime. That keeps servitors inside the existing spawn, region-index,
visibility and combat machinery rather than duplicating it.

`Player.getServitors()` becomes a scan rather than a cached index: a player has
at most one servitor on this dist, so there is nothing to iterate.

## What landed

- **`SkillEffect::Summon { npc_id, life_time, consume_item_id, consume_item_count }`**
  + the parse arm. `npcId` is declared **per skill level**, so each level
  summons a stronger template — which is why the id lives on the effect rather
  than being derived from the skill.
- **`game_loop::servitor`** — `summon_servitor`, `unsummon_servitor`,
  `servitor_of`.
  - Re-casting **swaps** rather than stacking, matching Java's
    `getServitors().forEach(unSummon)` before the new spawn.
  - `lifeTime <= 0` is Java's no-expiry case (`Integer.MAX_VALUE`, commented
    "Classic hack. Resummon upon entering game."); a positive value becomes an
    absolute deadline tick.
  - The servitor comes out at full HP/MP.
- **`PetSummonInfo`** (`PET_INFO`, 0xB2) — the ~50-field flat packet the
  *owner* sees. The servitor's remaining lifetime rides in the fed/max-fed pair,
  which is what draws the summon's time bar.

## Deliberately **not** in this slice

Stated plainly because they are visible gaps, not oversights:

- **Other players cannot see the servitor.** That needs `SummonInfo` (0x8B), a
  338-line *masked* packet — a slice of its own, and the port's masked-packet
  work (`UserInfo`/`CharInfo`) is where the bit-order traps live.
- **No follow, no attack.** The servitor stands where it was summoned. This is
  the other half of G29's gate and the natural next slice; the NPC AI already
  has chase/attack primitives to build on, but a servitor takes orders from its
  owner rather than from an aggro list, so it needs its own intention source.
- **No lifetime expiry tick, no item consumption.** The deadline is recorded
  and shown to the client but nothing yet enforces it.
- **No unsummon on logout/death**, no persistence across sessions.
- Master-buff inheritance on spawn (Java copies the owner's non-bad buffs),
  exp/level, and summon points.

## Tests

`game_loop::tests::servitor_tests` (9), including
`resummoning_replaces_rather_than_stacks` (the swap semantics),
`life_time_zero_means_no_expiry`, `an_npc_cannot_summon`, and two real-dist
tests pinning the per-level `npcId` and that all the sampled learnable summon
skills parse to a usable effect.
