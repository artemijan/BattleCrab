# G19 — Abnormal visual effects

The sixth **G19** slice, and the cosmetic counterpart to the five before it:
stun, root, silence, poison and bleed have all worked *mechanically* since the
CC and periodic slices, but the client showed **nothing** on the victim.

Java sources: `AbnormalVisualEffect`, `EffectList.getCurrentAbnormalVisualEffects`
/ `startAbnormalVisualEffect` / `stopAbnormalVisualEffect`,
`Creature.updateAbnormalVisualEffects`, `CharInfo`,
`ExUserInfoAbnormalVisualEffect`, `AdminEffects`' `//ave_abnormal`.

---

## 1. Why this next

With five slices of effect breadth landed, the remaining G19 backlog was
thinning and increasingly belonged elsewhere (`Lethal`/`EnergyAttack`/
`AttackTrait` are damage-formula work that sits better in G20; `Transformation`
is partly G13). Two checks decided it:

- The **geometric affect scopes** deferred back in the first slice
  (`FAN`/`SQUARE`/`RING_RANGE`) are only **5 learnable skills** — confirming
  that deferral rather than reversing it.
- The **abnormal-visual runtime** is the largest item still named explicitly in
  G19's scope, and the roadmap lists it as unblocking a batch of AdminEffects
  handlers.

69 learnable skills carry an `<abnormalVisualEffect>`, and the top values —
`STUN` (289 uses), `DOT_BLEEDING` (203), `DOT_POISON` (147), `ROOT` (91),
`SILENCE` (59), `PARALYZE` (56) — are precisely the effects the previous slices
made work. So this closes the loop on all of them at once.

## 2. The runtime

`abnormal_visual_client_id` maps Java's enum names to client ids (`VP_KEEP`
shares 29 with `VP_UP`, Java's TODO comment and all; an unknown name resolves to
`None` and is simply not drawn). The parsed ids ride on the `Skill`, are stamped
onto each `ActiveBuff`, and are folded on read by
`abnormal::visual_effects` — the same **stamp-and-fold** pattern the CC flags
and `BlockAbnormalSlot` use, so there is again no cached set to invalidate. The
fold de-duplicates: two poisons draw one tint.

## 3. Getting it onto the wire

Both packet sites were stubs:

- `CharInfo` hard-coded an abnormal-visual **count of 0** — so nobody nearby
  ever saw an effect on anyone. It now writes the real count and ids. Since
  `PlayerView` carries no `Buffs`, the list is passed in by the caller.
- `ExUserInfoAbnormalVisualEffect` handled only GM stealth and the transform id.
  It now carries the buff-driven set too, with `STEALTH` appended when invisible
  rather than sent alone — so toggling `//hide` on a stunned GM no longer wipes
  the stun from their own view.

## 4. Only broadcasting on change — and the test that caught it

The first attempt pushed `ExUserInfoAbnormalVisualEffect` on *every* buff
add/remove. That broke an existing positional packet-order test, and the test
was right: **Java pushes the set only from `startAbnormalVisualEffect` /
`stopAbnormalVisualEffect`** — i.e. only when the set actually changed, never
from the generic buff path. A skill with no `<abnormalVisualEffect>` cannot have
changed anything, so it now sends nothing; the expiry path checks whether the
buff being removed carried a visual before bothering.

This is worth remembering: a plausible "refresh on every change" is both
chattier than retail *and* observable, because several tests assert exact packet
sequences.

## 5. `//ave_abnormal`

With the runtime in place, `AdminEffects`' `//ave_abnormal <NAME> [radius]` is
small: it **toggles** a GM-pinned effect (Java's `performAbnormalVisualEffect`
starts it when absent and stops it when present) on the target, on self when
untargeted, or on everyone within `radius`.

Java has no separate storage for these — it mutates the same `EffectList` set
the buffs feed. Because this port computes the buff set rather than storing it,
the pinned ones need their own home: an `AdminVisuals` component, folded first
in `visual_effects` so a pinned effect shows even on a buff-less creature and is
independent of any buff coming or going.

## 6. Tests

Parse assertions against real skills: Shield Stun 92 → `STUN(7)`, Bleed 96 →
`DOT_BLEEDING(1)`, Horror 65 → `TURN_FLEE(32)`, Might 1068 → none; plus the
id-map's unknown-name `None`.

Behaviour: the fold de-duplicating two poisons and clearing per-buff; `CharInfo`
growing by the visual entry once a stun lands; a visual-less buff pushing **no**
`ExUserInfoAbnormalVisualEffect`; and `//ave_abnormal` toggling a pinned effect
on and off, rejecting an unknown name, and coexisting with buff-driven visuals.

## 7. What is still missing

The rest of the AdminEffects AVE subset — `//setteam`, `//settargetable`,
`//set_displayeffect`, `//playmovie`, `//event_trigger` — each needs its own
per-creature state (team id, targetable flag, display effect) plus a packet
field, rather than riding this set; they are now unblocked but not done.
`ExAbnormalStatusUpdateFromTarget` (showing a *target's* abnormals in the target
window) is likewise still open.

Remaining G19 by learnable usage: `Transformation` (32, partly G13),
`MpConsumePerLevel` (11), `EnergyAttack` (9), `Lethal` (9), `StatUp` (9),
`ShieldDefence` (8), `AttackTrait` (7) — several of which are really G20
damage-formula work. Also open: `calcMagicSuccess`, the remaining
`AcquireSkillType`s, and skill enchanting.
