# G19 — CC breadth: mute, debuff-block, control-block, target-cancel

The fifth **G19** slice. It completes the crowd-control family started in
[PLAN_G19_ABNORMAL_STATES.md](PLAN_G19_ABNORMAL_STATES.md) (stun/root) and
[PLAN_G19_ABNORMAL_RESIST.md](PLAN_G19_ABNORMAL_RESIST.md) (resisting it).

Java sources: `handlers/effecthandlers/{Mute,PhysicalMute,DebuffBlock,
BlockControl,TargetCancel}.java`, `SkillCaster.checkDoCastConditions` (the mute
gate), `Formulas.calcMagicAffected` (the debuff-block bail),
`clientpackets/UseItem` (the control-block gate), `Creature.isMuted`/
`isPhysicalMuted`/`isDebuffBlocked`/`isControlBlocked`.

---

## 1. Why this next

By learnable-skill usage these were the cheapest remaining wins, because the
`effect_flag` infrastructure from the stun/root slice already existed — each is
a flag plus one gate:

| learnable skills | effect | real skills |
|---|---|---|
| 10 | `TargetCancel` | Trick 11, Switch 12, Aura Flash 1417 |
| 9 | `BlockControl` | Horror 65, Curse Fear 1169, Turn Undead 1400 |
| 6 | `Mute` | Seal of Silence 1246, Curse of Doom 1336 |
| 6 | `DebuffBlock` | Mystic Immunity 1411, Celestial Shield 1418 |
| 4 | `PhysicalMute` | Shield Slam 353, Heroic Grandeur 1375 |

## 2. Flags and gates

Four flags join `BLOCK_ACTIONS`/`ROOTED` in `effect_flag`, each with the gate
Java puts it behind:

| flag | gate | effect |
|---|---|---|
| `MUTED` | `checkDoCastConditions` | **magic** skills refused |
| `PHYSICAL_MUTED` | same | **non-magic** skills refused |
| `DEBUFF_BLOCK` | `calcMagicAffected` | incoming debuffs fail outright |
| `BLOCK_CONTROL` | `UseItem` | item use refused |

The mute pair is mutually exclusive in effect — a silence must not block a
physical skill, and a physical mute must not block a spell — which is asserted
in both the parse and behaviour tests. Java exempts **static** skills
(`magic_type == 2`) from both, and that is ported.

`DEBUFF_BLOCK` refuses the debuff *outright*, with no roll — it is a hard bail
in Java, ahead of the resistance multiplier this port added last slice. Buffs
are unaffected, and self-cast is exempt on the same `target != attacker` test
the resist roll uses.

`BLOCK_CONTROL` is the one deliberate narrowing: Java's "out of control" state
also covers summon/mob control, which needs G29. The only ported consumer is
the item-use gate, which is what `UseItem` checks.

## 3. Side effects on landing

`Mute.onStart` aborts the victim's current cast — otherwise a silence landing
mid-cast would let that cast finish, which is exactly the case silence exists to
prevent. **Raid bosses are immune** to this (Java's `isRaid()` bail); the flag
still applies, only the interrupt is skipped. That is what stops a single
silence from neutering a raid, and it is tested against a raid-flagged NPC.

Unlike a stun, a mute leaves movement alone.

`TargetCancel` is instant and chance-rolled (`chance`, default 100): it drops
the victim's target — through `set_target(None)`, so the `TargetUnselected`
broadcast that clears the client's selection ring goes out — and aborts their
attack and cast. A 0 % variant is tested to prove the roll is consulted.

## 4. Tests

Parse assertions against the real skills (`skill_data`): Seal of Silence 1246
(`MUTED`, and asserted *not* `PHYSICAL_MUTED`), Shield Slam 353, Mystic
Immunity 1411, Horror 65, Trick 11.

Behaviour (`game_loop/tests/abnormal_tests.rs`, now 15 cases, 7 new): mute
blocking magic while physical mute does not and vice versa; the mute
interrupting an in-flight cast; raid immunity to that interrupt; debuff-block
refusing a stun while still admitting a buff; control-block refusing item use;
target-cancel clearing the target and aborting the cast; and a 0 %
target-cancel doing nothing.

## 5. What is still missing

`Fear` (9 learnable skills) is the notable CC hold-out — it needs forced flee
movement in the AI, which is a movement-system change rather than a flag, so it
belongs with the NPC AI breadth of G21. `AbnormalShield` (Java's
`getAbnormalShieldBlocks`, the counter consulted just before `DEBUFF_BLOCK`) has
no ported source. Java's wider `BLOCK_CONTROL` semantics wait on G29.

The remaining G19 backlog by learnable usage: `Transformation` (32, partly
G13), `MpConsumePerLevel` (11), `EnergyAttack` (9), `Lethal` (9), `StatUp` (9),
`ShieldDefence` (8), `AttackTrait` (7) — several of these (`Lethal`,
`EnergyAttack`, `AttackTrait`) are really damage-formula work and may sit better
in G20. Also open: the geometric affect scopes, `calcMagicSuccess`, the AVE
runtime, and skill enchanting.
