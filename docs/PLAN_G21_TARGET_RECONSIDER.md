# G21 slice 8 — `skillTargetReconsider` (support mobs help their pack)

Eighth G21 slice. A healer mob now heals whoever in its pack is worst off, and
a buffer buffs its faction-mates — instead of both only ever targeting
themselves.

## Why this over the rest of the tail

Slice 1 shipped NPC casting with an explicit narrowing: heal and buff resolved
to the caster, because the port had no faction data. Slice 2 added it. Sizing
what that unblocks against the other remaining G21 items:

| Item | Weight | Verdict |
|---|---|---|
| `skillTargetReconsider` | **1040** NPCs with a buff-bucket skill, **305** with a heal-bucket one | **Picked** |
| Walker routes | 14 routes, **12** NPCs (`TownNpcWalkers.xml`) | Small but real |
| `DamageZone` / `SwampZone` | 15 live between them | Small |
| `FenceData` | **1** fence, named `"demo"` | Effectively nothing |
| `HtmCache` | 2629 `.htm` files, already read at runtime | Caching, no behaviour |
| `CreatureSeeTaskManager` | Trigger for AI *scripts* | No script engine yet |

`FenceData` is the ConditionZone lesson again: a whole subsystem whose dist
content is a single placeholder.

## What landed

`skill_target_reconsider(world, npc, skill, inside_cast_range)`:

- **Bad skill** → candidates are the caster's own aggro list.
- **Good skill** → nearby faction-mates plus itself. A **heal** picks the
  lowest HP percentage; anything else picks at random.
- Range is `castRange + collisionRadius` when `insideCastRange`, else Java's
  flat `2000` (its own source carries a "TODO need some forget range" there).

The heal step now also rolls its `(100 - hpPercent) * 1.5` chance against the
**chosen target's** HP, not the caster's — which is what makes a healer reliably
top up a dying pack-mate.

## A deliberate deviation, and why

Java's good-skill candidate set is `getVisibleObjectsInRange(npc,
Creature.class, range)` — *every* nearby creature. Its `checkSkillTarget` only
rejects auto-attackable targets **inside the `isContinuous()` branch**, and a
heal is not continuous. Read literally, a healer mob would heal the wounded
**player** fighting it.

That is almost certainly unintended and would read as a port bug in-game, so
the candidate set here is scoped to the caster's faction (`shares_clan_with`)
plus itself. The scoping makes the AI do *less* than Java, never more — the
safe direction for behaviour I can't verify against a live server. There's a
test named `a_wounded_player_is_never_healed` pinning it, and a `TODO(G21)` at
the site.

## The bug this surfaced in slice 1

`check_skill_target` carried:

```rust
if !(skill.is_debuff || skill.is_bad()) && target_oid != npc_oid { return false; }
```

Java's actual test is `target.isAutoAttackable(caster)` — refuse a *good*
continuous skill on an **enemy**. Encoding it as "not self" was indistinguishable
while heal and buff were self-only, and became wrong the moment reconsider
landed: it silently blocked every buff on a faction-mate. Caught by
`a_buff_goes_to_a_faction_mate_that_lacks_it`, which failed with *no cast at
all*. Now `is_auto_attackable_by_npc` (players yes, NPCs no).

Worth remembering: **a narrowing that is currently indistinguishable from the
real rule will silently become a bug when the thing it was narrowed around
arrives.** Slice 1's comment said "same reconsider narrowing as heal", which
described the intent but not the trap.

## Tests

9 in `game_loop/tests/target_reconsider_tests.rs`: heals a wounded mate rather
than itself; picks the worst-off of several; still self-heals when it's the
worst off; ignores another faction; **never heals the player**; ignores an ally
beyond the reconsider range; a full-HP pack falls through to the buff step; a
buff goes to the mate lacking it; and a clanless mob (most of the datapack)
keeps the old self-only behaviour.

**712 lib tests green**, `char_persistence` 7/7, `e2e_create` 1/1. The one build
warning (`is_in_duel`) predates this work.

## Next in G21

The tail is now genuinely small: walker routes (12 NPCs), `DamageZone`/
`SwampZone` (15 live zones), `HtmCache` (caching only), and
`CreatureSeeTaskManager` (needs the script engine). `FenceData` is one demo
fence and not worth porting on this dist.
