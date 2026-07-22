# G19 — Elemental attributes (`calcAttributeBonus`)

## Why this slice

The largest remaining G19 item by learnable reach, hiding in plain sight:
**`DefenceAttribute` carries 33 learnable skills** (the whole Resist
Fire/Water/Wind/Earth + Divine/Dark Protection + elemental Surrender family)
and **`AttackAttribute` 7 more** (Holy Weapon 1043, Holy Blade 196, Dance of
Light 277, Dark Form 423, the BD/SWS elemental Seeds 1285–1287) — ~40
learnable skills, all inert today because the port computes no elemental
term anywhere.

The earlier "elemental attributes are Kamael-era, out of scope" note covered
**item attribute enchanting** (`AdminElement`). The *combat* term is live in
this dist's Java: `Formulas.calcAttributeBonus` multiplies blow damage, magic
damage, debuff land rate, counter-attack damage, and the
`PhysicalAttack`/`EnergyAttack`/`Lethal` handlers all call it. Volcano casts
FIRE 20 today and the port ignores it.

## Java model

- `attackAttribute = attacker.getAttackElementValue(type) + skill.attributeValue`
  where the per-element value is `Stat.FIRE_POWER`… (fed by `AttackAttribute`
  effects; weapon attribute holders don't exist on this dist's items).
- `defenceAttribute = target.getDefenseElementValue(type)` = `Stat.FIRE_RES`…
  — base from the **creature template** (`AttributeFinalizer`: NPC templates
  declare `<attribute><defence fire="20" …/>`; player templates none) plus
  the `DefenceAttribute` effect merges.
- With no skill attribute, the attacker's strongest POWER stat picks the
  element ("temp fix" block in `CreatureStat.getAttackElement`) — so Holy
  Weapon colors even an attribute-less *skill* holy. Skill-less call sites
  (plain auto-attacks) never invoke the bonus in this build.
- The multiplier: `diff = attack − defence`;
  `> 0 → min(1.025 + √(diff³/2)·0.0001, 1.25)`,
  `< 0 → max(0.975 − √(−diff³/2)·0.0001, 0.75)`, else 1.

## Port

1. **Model**: `Element` enum (Fire/Water/Wind/Earth/Holy/Dark) with
   `power_stat()`/`res_stat()`; 12 new `Stat` variants;
   `Skill.attribute_type: Option<Element>` + `attribute_value: i32`.
2. **Parse**: skill `<attributeType>`/`<attributeValue>`;
   `DefenceAttribute`/`AttackAttribute` effects → one `StatModifier` per
   element in the (possibly comma-separated) `attribute` param, mode DIFF —
   after that, player-side plumbing is free (the buff→`StatModifiers`
   rebuild is generic). NPC templates: `<attribute><defence …/>` →
   `base_element_res: [i32; 6]`, `<attribute><attack type value/>` →
   `base_attack_element`.
3. **Read side**: `attack_element_value`/`defense_element_value` helpers —
   players read `StatModifiers`; NPCs read template base + a fold over
   active buffs (NPCs keep no `StatModifiers`; same fold-on-read shape as
   the abnormal flags), so Day of Doom's −50s bite mobs too.
4. **Formulas**: pure `calc_attribute_bonus(attack, defence)`; multiplied
   into the `MagicalAttack` family, `PhysicalAttack`, `EnergyAttack`, the
   blow family, `Lethal`'s chance, and a new `element_mod` factor in
   `calc_effect_land_rate` (Java: `rate = baseMod · elementMod · … ·
   buffDebuffMod`, clamped after).

## Out of scope (as before)

Item attribute holders/enchanting (`AdminElement`, no items on this dist
declare them), `calcCounterAttack`'s attribute term (vengeance reflection is
narrowed in the port), the skill-less auto-attack path (Java never applies
the bonus there).

## Tests

1. Formula unit: diff 0 → 1.0; +20 → ≈1.031; −20 → ≈0.969; caps 1.25/0.75
   pinned at huge diffs.
2. Dist parse: Volcano FIRE/20; Holy Weapon → `AttackAttribute HOLY +20` as
   a `HolyPower` StatModifier; Surrender to Fire's multi-element debuff;
   totem 13028's template defences (20 across the board).
3. Behavior: a FIRE nuke does less damage to a fire-resistant mob than to a
   neutral one (template res); a `DefenceAttribute` debuff on the mob raises
   that damage back; Holy Weapon on the caster raises an attribute-less
   skill's damage vs a holy-weak target (strongest-POWER election).
4. Land rate: the elementMod factor moves `calc_effect_land_rate`.
