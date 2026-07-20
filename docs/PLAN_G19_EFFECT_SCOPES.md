# G19 — Effect scopes (`<selfEffects>`, `<pveEffects>`, `<pvpEffects>`)

## Why this slice

Found by the previous slice: Vengeance 368's `BlockMove` sat in `<selfEffects>`
and silently never loaded, because **the parser read only the default
`<effects>` block**. Measured across the datapack:

| scope | skills | learnable |
|---|---|---|
| `selfEffects` | 91 | 7 |
| `endEffects` | 58 | 1 |
| `pvpEffects` | 38 | 1 |
| `pveEffects` | 33 | 1 |
| `channelingEffects` | 24 | 4 |
| `startEffects` | 3 | 0 |

~14 learnable skills — more than any remaining effect entry (3), and silent
breakage rather than a missing feature, which is the same reasoning that made
the level-gating slice the right call.

Every one of the seven `<selfEffects>` carriers holds an **already-ported**
effect (`Speed`, `FocusMomentum`, `BlockMove`, `PhysicalEvasion`,
`FatalBlowRate`), so this is pure plumbing with immediate payoff — six skills
gained a real self-buff.

## What Java does

`EffectScope` maps XML node names to scopes, and `Skill` keeps one effect list
per scope:

- **GENERAL** (`effects`) → the target. Already ported.
- **SELF** (`selfEffects`) → a separate `applyEffects(caster, caster, …)` after
  the target loop, so a skill can buff its caster while debuffing its target.
- **PVE** / **PVP** → applied to the *same* target as GENERAL, selected by
  `effector.isPlayable() && effected.isAttackable() ? PVE : effector.isPlayable() && effected.isPlayable() ? PVP : null`.
- **START** / **END** / **CHANNELING** → cast-start, buff-end and channelling
  hooks this port doesn't have.

## What landed

- `EffectScope` in the parser; any `<*Effects>` element opens the effect
  section and stamps its scope on every effect inside. Unsupported scopes parse
  as `Other` and are **dropped rather than merged** — merging would apply them
  at the wrong time, which is worse than not having them.
- `Skill` gained `self_effects`, `pve_effects`, `pvp_effects`.
- `skills::cast` applies the SELF list to the caster after the target loop, and
  appends the matchup list to each target via `matchup_effects`.

### `impl Default for Skill`

Adding `Skill` fields had broken every exhaustive struct literal twice now —
`magic_critical_rate` churned 15 test files and was backed out partly for that
reason. This slice needed three more fields, so it invests in a `Default` impl
and converts the literals to `..Default::default()`.

Two defaults are load-bearing rather than zero: `activate_rate: -1` and
`reuse_delay_group: -1` are Java's "absent" sentinels, and gates test for them
explicitly (a skill with `activate_rate == -1` always lands and is never
reflected). `skill_default_uses_javas_sentinels` pins both.

**Honest note on how this went:** the conversion took several passes. Automated
brace-matching mangled four files and they were reverted from git; two of them
(`skills_tests.rs`, `mod.rs`) were finished by adding the three fields
explicitly instead, because the automated approach kept mis-matching nested
literals. The result compiles and passes, but the mixed style — 20 files on
`..Default::default()`, two with explicit fields — is a deliberate stopping
point rather than a tidy end state.

## A latent flake this slice exposed

`confuse_tests::a_confused_mob_turns_on_a_bystander` failed in the sweep. The
cause was **not** this slice's changes: `apply_skill_effects` charges an
unconditional per-cast magic-crit `roll(1000)` *before* any effect runs, so the
`forced_rolls.extend([0, 1])` the `Confuse` slice added never actually pinned
the candidate index — it fell through to the real RNG and the assertion was a
coin flip. It happened to pass for two slices until an unrelated change shifted
the draw.

Fixed by forcing all three rolls (`[0, 0, 1]`) with the ordering documented at
the call site, and verified stable over five consecutive runs.

**Lesson: when forcing rolls, account for rolls charged by the machinery around
the code under test, not just by the code itself.**

## Tests

`game_loop::tests::effect_scope_tests` (8), plus
`reflect_tests::vengeance_block_move_loads_from_its_self_effect_scope` — the
placeholder the previous slice left, now flipped from "still unread" to
asserting the effect loads into `self_effects` and is *not* merged into the
general list.

Notable: `unsupported_scopes_are_dropped_not_merged` (Anchor 1170's
`<endEffects>` must not leak into a live list) and
`a_self_effect_lands_on_the_caster` (the target must *not* get it).

## Deferred (not this slice)

- **`startEffects` / `endEffects` / `channelingEffects`** — they need cast-start,
  buff-end and channelling hooks the port lacks. 6 learnable skills between
  them; `channelingEffects` (4) is the biggest and depends on channelling.
- Java's `stopSkillEffects` refresh before a SELF application (a re-cast
  currently stacks through the ordinary buff pipeline instead).
