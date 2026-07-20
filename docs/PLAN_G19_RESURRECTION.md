# G19 — Resurrection

## Why this slice

The in-scope ranking is down to a 2-learnable tail, so this one was picked on
**player-visible value** rather than count. `Resurrection` is 2 learnable
skills (Resurrection 1016, Mass Resurrection 1254) across 39 skills, but it is a
headline mechanic: without it nobody can be raised, and every death is a walk
back from town.

## What Java does

`Resurrection.instant` does **not** revive. It calls `Player.reviveRequest`,
which stores the proposal and puts a `ConfirmDlg` on the corpse's screen; the
revive happens in `reviveAnswer` when they accept.

```java
_revivePower = Formulas.calculateSkillResurrectRestorePercent(power, reviver);
restoreExp   = round(((_expBeforeDeath - getExp()) * _revivePower) / 100);
```

`calculateSkillResurrectRestorePercent` scales the declared power by the
reviver's WIT — with a quirk: once the bonus has already added more than 20, it
adds a **further flat +20**, so high-WIT revivers jump rather than scale
smoothly. Clamped to `[base, 90]`, and short-circuited at 0 and 100.

## Two prerequisites this needed

- **`TargetType::PcBody`** — Resurrection 1016 targets `PC_BODY`, a dead
  *player* corpse. The port had `NpcBody` (for Sweeper) but no player
  equivalent, so the skill could not even resolve a target. Added, and joined to
  the same "dead targets allowed" exemption.
- **Pre-death XP.** Java keeps `_expBeforeDeath` and subtracts; the port now
  records the *difference* directly (`Player.lost_exp_on_death`) in
  `apply_death_exp_penalty_ex`, which already computes it. That is the only
  quantity a resurrection reads, so storing the delta is simpler and cannot
  drift from the penalty that produced it.

## What landed

- `SkillEffect::Resurrection { power, hp_percent, mp_percent, cp_percent }`
  and `TargetType::PcBody`, with their parse arms.
- `death::revive_request` — the proposal: `isResurrectionBlocked` gate, the
  "already been proposed" refusal that stops two clerics racing, the WIT-scaled
  restore percent, and the `ConfirmDlg`.
- `death::handle_revive_answer` — consumes the proposal, re-checks the corpse
  is *still* dead (they may have used "to village" while the dialog sat on
  screen), and on accept calls…
- `death::do_revive_with` — Java's `doRevive(power)`: the skill's own HP/MP/CP
  percentages **override** the config respawn defaults (a zero means "leave
  what the config gave", matching Java's `if (reviveHp > 0)` guards), plus the
  XP restore.
- `DlgAnswer` dispatch now offers the packet to the revive flow first, which
  reports whether the reply was its own; the admin-confirm flow keeps it
  otherwise. `an_unrelated_answer_is_not_claimed` pins that hand-off.

## Tests

`game_loop::tests::resurrection_tests` (10). Notable:

- `restore_percent_matches_javas_formula_including_the_plus_twenty_jump` — the
  quirk above, both sides of the threshold.
- `a_proposal_does_not_revive_by_itself` — the whole shape of the mechanic.
- `a_second_proposal_is_refused_while_one_is_pending`.
- `accepting_after_already_respawning_does_nothing` — the re-check, without
  which a player could take the XP back twice.
- `an_unrelated_answer_is_not_claimed` — the shared-packet hand-off.
- `level_one_resurrection_restores_no_xp` — both skills declare `power = 0` at
  level 1, so real data exercises the formula's short-circuit.

## Deferred (not this slice)

- **Pets** — Java's `reviveRequest` has an `isPet` branch throughout; servitors
  are `TODO(G29)`.
- **`Charm of Courage`** — a different dialog and a self-revive path; the item
  isn't modelled.
- **`BLOCK_RESURRECTION`** — the gate is ported, but no learnable skill on this
  dist grants the flag (4 non-learnable ones do), so nothing reachable trips it.
- **Mass Resurrection's affect scope** — it is `SELF` + a party scope, so it
  currently proposes to the caster only; the party fan-out rides on
  `skills::affect` and should be checked when party-scoped instants get
  attention.
