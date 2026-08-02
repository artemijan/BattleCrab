# G21 slice 1 — NPC skill casting

First slice of **G21 (NPC AI & world-content breadth)**. Until now a monster
could only swing its weapon: every NPC skill in the datapack was inert. This
slice ports `AttackableAI`'s "Cast skills" block and the AI skill bucketing at
the tail of `NpcData.parse`, so mobs cast.

## Why this first

Surveyed the dist rather than working down the roadmap's list:

| Fact | Number |
|---|---|
| NPC templates with ≥1 *active* (castable) skill | **4831** (Python survey) / **5013** (the built index) |
| Distinct active skill ids attached to NPCs | 1564 |
| NPC skill attachments whose effects are **fully** ported | 73 % |
| …**partially** ported (some effects no-op) | 9 % |
| …unported / effect-less | 18 % |

73 % coverage means most casts land real behaviour today; the rest animate and
no-op, which is the port's existing convention for unported effects. The two
survey numbers agreeing within ~4 % also cross-validates the NPC parser against
an independent read of the XML — a gap there would have meant dropped skill
lists (the failure mode of the old nested-`<minions>` bug).

G21's gate is *"a mob casts, a guard aggros a PK, a spoiled corpse can be
swept, a boss keeps its HP across restart."* Spoil/sweep already landed in
G15; this slice is the first gate clause.

## What landed

**Bucketing** — `data/npc_ai_skills.rs`. `NpcAiSkillIndex` is built once at
`GameData::load_from` (both loaders must be done first, so `skill_data` and
`npc_data` are now bound to locals before the struct literal). Each non-passive
template skill is classified by a straight transcription of Java's `else if`
ladder into `AiSkillScope`s. **The branch order is load-bearing**: a continuous
skill takes the first arm and never reaches the ATTACK arm even when it also
carries a damage effect. There's a test pinning exactly that.

**`Skill.is_continuous`** — the Rust `OperateType` collapses `A1`/`A2` into
`Active` (the cast pipeline treats them alike), so continuity had been lost.
Rather than proxy it off `abnormal_time`, it's now read from the raw
`operateType` string (`A2..A6`/`DA2..DA5`, Java `SkillOperateType.isContinuous`).

**`<ai type>` + skill chances** — `NpcTemplate.ai_type` (`AiType`), plus
`min/max_skill_chance` (Java defaults 7/15, absent from this dist).

**The ladder** — `game_loop/npc_cast.rs`, hooked into `think_attack` *before*
the chase/swing tail, so a mob that starts a spell neither chases nor swings
that think. Gate: `(!moving && hasSkillChance()) || aiType == MAGE`. Order:
heal → self-buff → immobilize a moving target → mute a casting one →
short-range → long-range → general.

**NPC-side `startCasting`** — cast timing from the NPC's own finalized
`CombatStats` (`m_atk_spd / 333`, `p_atk_spd / 300`) rather than the player
path's class-template + DEX/WIT route; `magic_skill_use_raw` broadcast (the
`&Player`-taking builder doesn't apply); `NpcAi.cast_seq` as the NPC
counterpart of `Player.cast_seq`. The cast then rides the **existing shared**
`handle_skill_launch` → `handle_skill_finish` path, which turned out to be
almost entirely caster-agnostic already.

## Two things the shared path assumed about players

1. **MP would have been charged twice.** `handle_skill_finish` consumes
   `mp_consume` at landing. `start_cast` therefore charges only
   `mp_initial_consume`, exactly like the player path.
2. **`effects.rs` hard-`expect`ed a `Player` on the caster** in five places
   (for the caster's name, and the level feeding `levelMod` in the physical
   formula) — an NPC cast panicked the server. Replaced with
   `caster_display_name` / `caster_level`, which resolve an NPC through its
   template. Caught by the one behaviour test that ran a cast end-to-end
   through the real tick loop rather than calling `try_cast` directly; the
   nine tests that stopped at "did a cast start?" all passed while this
   panic was live. **Worth remembering: assert through the real path at least
   once per slice.**

## Deliberate narrowings (all `TODO(G21)` at the site)

- **`skillTargetReconsider`** — Java re-picks heal/buff targets across the
  caster's faction. No faction/clan-help plumbing yet, so heal and buff resolve
  to the caster itself (what a solo mob would pick anyway).
- **`AIType.ARCHER` kite move** and the raid target-chaos shuffle.
- **`SUICIDE`** — ported for shape, never taken: no skill in this dist declares
  `isSuicideAttack` (self-destruct mobs here are script-driven).
- **`RES`** — no resurrection effect is ported, so the bucket stays empty and
  mobs never revive their dead.
- **Sleep** — Java splits `SLEEP` (no range scope) from `BLOCK_ACTIONS`/`ROOT`
  (with one). The port parses stun *and* sleep to `BlockActions`, so the two
  arms collapse and the SLEEP arm is unreachable.

## Tests

11 new in `game_loop/tests/npc_cast_tests.rs` — four on bucketing (including
the ordering invariant and "passive stat holders are never castable"), six on
the ladder (mage casts and deals damage end-to-end; a moving fighter doesn't;
no recast of a held abnormal; no cast without MP; heal skipped at full HP and
certain below 33 %; no double cast), and one against the **real datapack**
asserting the index covers >4000 templates and that nothing passive was
bucketed.

**635 lib tests green**, `char_persistence` 7/7, `e2e_create` 1/1.

## Next in G21

The remaining gate clauses and breadth, roughly in value order:

- **Guard/faction aggro + clan-help calls** — the second gate clause, and it
  unblocks `skillTargetReconsider` above.
- **Minions** — templates parse them; nothing spawns them.
- **`DBSpawnManager`** — raid-boss HP across restart (fourth gate clause).
- **NPC pathfinding** (the G7.85 worker for NPCs) and NPC regen.
- **The other ~33 zone types**, fences (`FenceData`), `HtmCache`, walker
  routes, `CreatureSeeTaskManager`.

## Follow-up (2026-08-02): the ladder was starved of rolls

The hook described above — "*hooked into `think_attack` before the chase/swing
tail*" — sat **below** a `Busy swinging` early return that this slice
inherited:

```rust
if attack_state.attack_end_tick > now { return; }   // ← not in Java
```

Java's `thinkAttack` has no mid-swing gate. The refusal lives in
`Creature.doAutoAttack`, behind `isAttackDisabled()` = `isAttackingNow() ||
isDisabled()`, so a mob whose swing is winding down keeps thinking: faction
call, movement, and the cast ladder all still run; only the next swing is
dropped.

With the gate on top, the only think that ever reached the ladder was the one
`ScheduledTask::NpcAttackReady` fires at the swing's end — one
`hasSkillChance()` roll per swing instead of one per second. The visible
casualty was Porta (20213): rolls ~18 s apart against a 6 s Stun (4073) reuse
meant the `SHORT_RANGE` rung always had Stun available, so the `GENERAL` rung
holding its signature Summon (4161) was never reached. 3000 s of simulated
melee: 131 stuns / 8 summons before, 264 / 44 after.

`thinkAttack`'s actual first line was missing too — `if ((npc == null) ||
npc.isCastingNow()) return;`. Without it the 1 s think landing inside a 2 s
cast fell through to the swing tail and the mob attacked mid-cast.

Both are now `think_attack`'s: an `isCastingNow` return at the top, and a
`mid_swing` flag that gates **only** the `do_auto_attack` call at the tail.
