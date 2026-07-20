# G19 — Transformation skill effect

## Why this slice

The 2026-07 audit ranked unported G19 effects by *learnable-skill* count, not
raw instance count. `DefenceAttribute` topped that list (33 learnable skills)
but is Kamael-era elemental-attribute content, explicitly out of scope for
this Interlude/Classic build (`ROADMAP.md`'s scope gate). `Transformation` is
next (32 learnable skills, 306 instances): the "Transform &lt;Monster&gt;"
scroll family — Grail Apostle, Unicorn, Lilim Knight, Golem Guardian, Inferno
Drake, Dragon Bomber (541-558, three tiers each), Onyx Beast, Doom Wraith,
Zaken, Anakim, Venom, Gordon, Ranku, Kechi, Demon Prince, Heretic, Veil
Master, Saber Tooth Tiger, Oel Mahum, Doll Blader (617-674) — real,
player-reachable content (well-known low-level fun/PvP items on retail
Interlude), and squarely in scope.

The admin `//transform`/`//ride_*` runtime (G13.B) already carries the state
machine this needs: `Player.transform_id`/`transform_display_id`,
`TransformData` (174 `data/stats/transformations/*.xml` templates), and the
apply/remove logic in `game_loop::admin::transforms`. This slice wires the
*skill-cast* path into that existing plumbing rather than building a second
one.

## What landed

- **`SkillEffect::Transform { transformation_id }`** (`model/skill.rs`) +
  the `"Transformation"` parse arm (`data/skill_data.rs`, reads
  `<transformationId>`).
- **Cast-time gate** (`game_loop::skills::cast::use_magic_on`), porting
  `ConditionPlayerCanTransform`: refuses a `Transformation` cast while
  already transformed (SM 2058 `YOU_ALREADY_POLYMORPHED_AND_CANNOT_
  POLYMORPH_AGAIN`), in water (SM 2060), or with a cursed weapon equipped (no
  SM, matching Java's silent `else`). A horse/bike mount collapses into the
  "already transformed" leg on this port, since `//ride_horse`/`//ride_bike`
  are themselves transforms (G13.B) — no separate mount state to check, unlike
  Java's `isMounted()`. Not ported: the sitting and registered-on-event legs
  — neither state is modeled on this port (`TODO(G19)`/`TODO(G28)`); dead
  casters are already refused earlier in the same function for every skill.
- **`admin::transforms` split into state/broadcast halves**
  (`apply_transform_state`/`remove_transform_state`, both now `pub(crate)`)
  so the skill-effect path can mutate transform state without a duplicate
  `UserInfo` broadcast: `apply_continuous_effects`' buff-landing path already
  sends `UserInfo`/`CharInfo` for the buff carrying this effect, so only the
  transform-specific extras (`ExUserInfoAbnormalVisualEffect` + refreshed
  `SkillList`, via the new `refresh_transform_visuals`) are added on top.
- **Revert on `BuffExpire`** (`handle_buff_expire`): looks up the expiring
  buff's skill effects, and if it's a `Transform`, calls
  `remove_transform_state` before the generic buff removal, then
  `refresh_transform_visuals` after the shared `broadcast_user_info` call —
  same one-broadcast discipline as apply. Since the death path
  (`game_loop::death`) already routes every stripped buff through
  `handle_buff_expire`, transformations correctly revert on death too, with
  no separate death-specific hook needed.

## Test

`skills_tests::transformation_skill_polymorphs_and_reverts_on_expiry` — real
dist data (skill 618 "Transform Doom Wraith" → `transformationId` 2, transform
2's Male template grants skill 586 "Rolling Attack"): cast lands, transform id
+ display id + granted skill + run speed all update from one `TRANSFORM` buff;
a second cast while transformed is refused with SM 2058 and never starts a
`Casting`; `handle_buff_expire` reverts all of it. Uses the full real datapack
(`GameData::load_from`), not a synthetic template — a `for_test()` player has
an empty `player_templates`, so a level-1 dummy char computes 0 max HP and
`checkUseConditions`' HP precheck silently refuses every cast.

## Deferred (not this slice)

- Sitting / registered-on-event cast refusal legs (no modeled state yet).
- `DispelBySlot`'s Java `AbnormalType.TRANSFORM` special case (dispelling a
  transform via a cleanse skill) — still explicitly omitted, per the existing
  comment on `SkillEffect::DispelBySlot`.
- The transform template's own combat-stat overrides beyond speed (Java's
  deeper `//transform` integration gap, noted in `admin/transforms.rs`'s
  module doc, predates this slice and isn't specific to the skill-cast path).
