# G19 — Skill enchanting

The last player-facing G19 headline. Two slices:

- **Slice 1 (this one): the sub-level data foundation.** Skill sub-levels
  parse and resolve, the enchant-cost table loads, and the route registry
  knows what every skill can enchant into. No packets yet.
- **Slice 2: the flow.** `ExEnchantSkillList/Info/InfoDetail` +
  `RequestExEnchantSkill`, the success roll, SP/item consume, sub-level
  persistence on `SkillBook`/`character_skills`, re-learn on relog.

## Why now

413 dist skills declare enchant routes, **20 learnable** (Sonic Storm, Force
Storm, Thunder Storm, Rage, Sacrifice, Dance of Medusa, Curse Gloom, …) —
the biggest reach left in G19 after the attributes slice. The system is live
end-to-end in this dist's Java: `EnchantSkillGroups.xml` (30 enchant levels,
Giant's Codex items that really drop), the ex-packet family (0x0E/0x0F/0x43),
and computed sub-level rows in the skill XMLs.

## The Java data model

- A skill's enchanted variants are **sub-levels**: routes at 1001–1020,
  2001–2020, (rarely) 3001–3020 — up to three routes of +1…+20 each.
- Skill XML rows carry `fromSubLevel`/`toSubLevel` bounds on `<value>` rows
  and on whole `<effect>`s. Row text can be a **computed expression**:
  `{base + base / 100 * subIndex}` — evaluated at parse (exp4j) with
  variables `base` (the same level's non-sub value), `index`
  (`level − fromLevel + 1`), `subIndex` (`subLevel − fromSubLevel + 1`).
- `SkillData` pre-builds an instance per (id, level, subLevel) and registers
  each in `EnchantSkillGroupsData`'s route map; `Skill.isEnchantable` = has
  routes.
- `EnchantSkillGroups.xml`: per enchant level 1–30 — SP cost, success chance
  and required items, each by type (`NORMAL`/`BLESSED`/`CHANGE`/`IMMORTAL`).

## Port (slice 1)

1. **Parser**: collect `<value fromLevel toLevel fromSubLevel toSubLevel>`
   rows per field into `sub_rows` instead of today's silent mis-key into the
   level-0 slot (a latent bug: the last sub row's `{…}` text clobbers the
   field's scalar fallback). Same for effect params. Effect-level sub gating
   (`applies_at`) already exists — the `SUB_LEVEL = 0` constant becomes a
   parameter.
2. **Expression evaluator**: a small pure recursive-descent parser for
   `+ − * / ( )` + the three variables — the whole grammar the dist uses
   (grepped; no functions, no comparisons).
3. **Resolution**: `SkillData.get_enchanted(id, level, sub)` — pre-built like
   Java into a second map keyed `(id, level, sub)`; `get()` stays untouched
   (sub 0). Route registry `enchant_routes(id, level) → &[(route_base,
   max_sub)]`.
4. **`EnchantSkillGroupsData`**: load the 30-level cost table.
5. Census tests against the dist: Sonic Storm +1 power = `base + base/100`,
   route 3's magic-crit row, Heal's 2001-route effect swap, group 1 = 90%
   NORMAL + Superior Giant's Codex 30297.

## Deferred to slice 2

Everything the client sees, the enchant transaction, persistence, and
casting an enchanted skill from the bar (SkillBook sub-levels).
