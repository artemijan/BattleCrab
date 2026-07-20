# G19 — TriggerSkillByAttack

## Why this slice

The ranking (now excluding enchant-route instances, per the level-gating slice)
left a four-way tie at 3 learnable: `TriggerSkillByAttack`, `ReflectSkill`,
`BlockMove`, `TwoHandedBluntBonus`. Grouping by prefix again:

| cluster | learnable | skills |
|---|---|---|
| **`Trigger*`** | **5** | **362** |
| `TwoHanded*` | 5 | 22 |
| `Reflect*` | 3 | 54 |
| `BlockMove` | 3 | 21 |

`Trigger*` and `TwoHanded*` tie on learnable count, but `Trigger*` covers
**362 skills** to `TwoHanded*`'s 22 and is a genuinely new capability — "landing
a hit can cast another skill" — that nothing on this port could express.

The three learnable carriers are Sword/Blunt Weapon Mastery 205, Dagger Mastery
209 and Dance of Shadows 366. Each is a passive/dance whose *on-hit half* did
nothing before this slice.

## Scope decision

Java's handler takes **15 parameters**. Checking what the reachable content
actually sets narrowed that sharply — all three carriers use the same subset:

```xml
<attackerType>Creature</attackerType>  <minDamage>1</minDamage>
<chance>50</chance>                    <targetType>SELF</targetType>
<isCritical>true</isCritical>          <allowWeapons>SWORD,BLUNT</allowWeapons>
<skillId>5604</skillId>                <skillLevel>1</skillLevel>
```

No `triggerSkills` ladder, no `skillLevelScaleTo`, no `min`/`maxAttackerLevel`,
no `allowSkillAttack`/`allowReflect` overrides. So the port implements that
subset and keeps Java's defaults for the rest, rather than building 15 params
of machinery for content that doesn't exist here.

## What Java does

The effect subscribes to `OnCreatureDamageDealt`. Its gates, in order:
`chance != 0`, a valid skill, **`isCritical == event.isCritical()`**,
`allowSkillAttack`/`allowNormalAttack` vs whether a skill caused the hit,
`allowReflect`, `attacker != target`, attacker level bounds,
`damage >= minDamage`, `Rnd.get(100) > chance` bails, and the `allowWeapons`
mask against the equipped weapon.

Then it resolves `targetType` and casts — but only past a refresh guard:

```java
if (buffInfo == null || buffInfo.getSkill().getLevel() < triggerSkill.getLevel())
    SkillCaster.triggerCast(attacker, target, triggerSkill);
```

`triggerCast` bypasses cast time, MP and reuse.

**The subtle one: `isCritical` is an equality test, not a minimum.** An
`isCritical=false` trigger fires only on *non*-crits. Dance of Shadows 366
ships one of each, so reading it as "crits also count" would silently double
it.

## What landed

- **`SkillEffect::TriggerSkillByAttack`** with the seven fields the reachable
  content uses, plus the parse arm (including `allowWeapons` → a `WeaponType`
  mask, reusing the existing `weapon_condition_passes`).
- **`effects::fire_attack_triggers`**, called from
  `combat::handle_attack_hit` after the damage lands — the port's normal-attack
  choke point, which already carries `damage` and `crit`.
- The refresh guard, the party/self target split (an unpartied caster is a
  party of one, as `skills::affect` already treats it), and `triggerCast`'s
  bypass — the trigger goes straight through `apply_skill_effects`.

**Implementation note.** Java attaches a listener when the carrying skill
starts. These carriers are *passives*, whose effects this port folds into
`StatModifiers` rather than keeping as a live list, so there is nothing to
subscribe. Instead the attacker's skill book is scanned at hit time — a handful
of `HashMap` lookups per swing. If that ever shows up in a profile it should
become a cached index like `NpcAiSkillIndex`; it is not a behavioural
difference.

## Tests

`game_loop::tests::trigger_skill_tests` (9):

- `is_critical_matches_the_hit_exactly` — both directions of the equality trap
  above, which is the single easiest thing to get wrong here.
- `a_hit_below_min_damage_does_not_fire` — including that the floor itself
  qualifies (Java's check is `<`).
- `an_already_active_trigger_is_not_recast` — the refresh guard; without it a
  fast weapon would re-apply the buff every swing.
- `a_zero_chance_trigger_never_fires`, `a_self_hit_never_triggers`,
  `an_attacker_without_the_carrier_triggers_nothing` — the remaining bails.
- `real_dist_carriers_parse` — the real parameters, *and* that 205/209 have no
  trigger at level 8, which double-checks the previous slice's `fromLevel="9"`
  gating from a different angle.
- `the_triggered_skills_carry_real_effects` — 5603 grants a 5-second
  `FatalBlowRate`, so the trigger has a visible result rather than landing an
  empty buff.

## Deferred (not this slice)

- **The sibling triggers** — `TriggerSkillByMagicType` (1 learnable),
  `TriggerSkillByDamage` (1), and the five 0-learnable ones
  (`BySkill`, `ByAvoid`, `ByKill`, `BySkillAttack`, `ByDeathBlow`,
  `ByHpPercent`). They share this shape and should reuse `fire_attack_triggers`'
  structure when they land.
- **`triggerSkills` ladders**, `skillLevelScaleTo`, attacker-level bounds,
  `attackerType` filtering, `allowReflect` — no learnable skill sets any of
  them.
- **`allowSkillAttack`** — would need the same hook on the skill-damage path;
  no reachable carrier enables it.
