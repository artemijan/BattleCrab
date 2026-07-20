# G19 — AttackTrait skill effect

## Why this slice

The last item standing on the learnable-skill ranking that started this
string of G19 slices, set aside three times running as "needs the whole
`TraitType` system." Investigating it properly (rather than deferring a
fourth time) found the real scope was much smaller than feared — and, more
interestingly, that the effect is **inert on the real Java server too**.

## What Java does

`AttackTrait.instant` merges a `Map<TraitType, Float>` onto the caster via
`CreatureStat.mergeAttackTrait` (additive per trait, removed the same way on
buff exit). All 7 learnable instances — Detect Insect/Beast/Animal/Dragon/
Plant Weakness (75/80/87/88/104), Eye of Hunter/Slayer (359/360) — use only
the `*_WEAKNESS` category of `TraitType` (`BUG_WEAKNESS`, `ANIMAL_WEAKNESS`,
…), which is where this stopped looking like "the whole trait system" and
started looking like a single self-contained effect: `TraitType` also
carries weapon-type traits (`SWORD`, `DAGGER`, …) and status-resist traits
(`POISON`, `HOLD`, …) that no learnable `AttackTrait` skill touches.

The consuming formula, `Formulas.calcWeaknessBonus`, only applies a bonus
when **both** sides have a matching trait: `attacker.hasAttackTrait(trait)`
*and* `target.getStat().hasDefenceTrait(trait)`. Grepping the entire Java
tree and datapack for `mergeDefenceTrait` turns up exactly one call site —
its own definition in `CreatureStat.java`. No NPC template, no skill, no
other Java class ever calls it. So `target.hasDefenceTrait(anyWeakness)` is
**always false** for every creature in this build, `calcWeaknessBonus`'s
loop body never executes, and the multiplier stays at its initial `1`.
**Casting "Detect Beast Weakness" changes nothing observable, even on the
reference server it was ported from.** A faithful port is exactly as inert.

## What landed

- **`SkillEffect::AttackTrait`** (`model/skill.rs`, a unit variant — unlike
  `DefenceTrait`/`VampiricAttack` there's no per-trait data worth storing,
  since nothing would ever read it) + the `"AttackTrait"` parse arm
  (`data/skill_data.rs`), dropping the per-trait param map rather than
  keeping it.
- **Lands as an icon-only timed buff**, joining `DefenceTrait`/
  `VampiricAttack`/`AttackAttribute`/… in the `has_iconless_buff` guard and
  the instant-loop no-op arm — before this slice the effect name wasn't
  recognized at all, so the whole skill silently produced an empty effect
  list and never landed (no buff, no icon, nothing), which is a real
  regression from Java's own actual behavior (the buff *does* show and
  expire on retail — it just doesn't *do* anything).

## Collateral improvement: `NpcTemplate.race` now parses every category

Understanding `calcWeaknessBonus` required checking whether `<race>` (already
parsed, but collapsed to `None` for every non-playable value since the
Newbie Guide slice that added it only needed the six playable races) was
secretly the missing `DefenceTrait` link. It isn't — but the investigation
surfaced a real, low-risk gap worth closing while there: Java's `Race` enum
is **one shared enum** for players (`HUMAN`, `ELF`, …) *and* every creature
category (`UNDEAD`, `BEAST`, `ANIMAL`, …), and this port's `Race`/`parse_race`
only ever kept the first six. Extended both to the full 26-member enum
(matching Java's ordinals exactly — no renumbering, so nothing downstream
that already reads `Player.race`/the first six is affected), which:

- Fixed `data::npc_data::race_tests::parses_playable_races_from_dist`'s own
  assertion that undead "is not a playable race" (true) into the correct one
  — it *is* a `Race`, just not a *playable* one — `Some(Race::Undead.
  ordinal())` rather than `None`.
- Costs nothing today (nothing reads the new values yet) but means the day
  NPC-side `DefenceTrait` data *does* land, the race data this effect would
  actually need to check against is already there.
- Verified safe: the only consumer of `NpcTemplate.race` besides the parser
  itself is the Newbie Guide's own-race gate (`npc_race() != Some(player_
  race())`), which only ever runs against guide NPCs (always a real playable
  race) — a monster's race value changing from `None` to `Some(<category>)`
  can't affect that comparison, since player race ordinals (0-6) and monster
  category ordinals (7-25) never overlap.

## Test

`skills_tests::attack_trait_lands_as_an_icon_only_buff` — real dist data
(skill 80 "Detect Beast Weakness"): casting it lands exactly one buff, which
expires after its 600 s `abnormalTime`. No damage assertion — there's
nothing to assert, faithfully.

## Deferred (not this slice)

- A real per-creature attack-trait accumulator and `calcWeaknessBonus`
  wiring — only worth building once NPC-side `DefenceTrait`/creature-category
  resistance data exists to check against; nothing to wire it to today.
- The weapon-type and status-resist halves of `TraitType` (`SWORD`, `POISON`,
  …) — no learnable `AttackTrait` skill on this dist uses them.
