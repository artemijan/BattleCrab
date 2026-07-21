# NPC skill cooldowns never applied (G21 bug, found by the G29 sweep)

Closing the `Creature`-vs-`Player` sweep's last two probes (`Reuses`,
`TargetRef`) turned up a bug that has nothing to do with summons.

## The bug

`set_skill_reuse` records a cooldown through:

```rust
if let Some(reuses) = world.objects.get_component_mut::<Reuses>(&object_id) { … }
```

Players are given `Reuses` at load. **NPCs never were.** So for an NPC the write
was a silent no-op — and `npc_cast::check_use_conditions` reads the same
component with `if let Some(...)`, treating an absent one as *ready*.

Both halves fail open. **NPC skill cooldowns never applied at all**: a mob could
re-cast a 10-second skill as fast as its AI ticked, and the only thing pacing it
was the AI think interval.

G21 landed NPC skill casting across 4831 templates, and the reuse plumbing was
written and called correctly — it just wrote into a component nobody had
attached. Nothing failed; the feature simply had no effect.

## The fix

Attach `Reuses` on **first use** rather than at spawn. Only NPCs that actually
cast pay for the map, which matters when the world holds ~34.9k of them and the
vast majority never cast anything.

## Why two tests

The obvious test — "the cooldown was recorded" — is only half. The check reads
the *same* component, so a test stopping there would not notice if the gate
itself were bypassed. The second test drives `check_use_conditions` across the
boundary: ready → refused → ready again after expiry.

Recording and enforcing are separate failures, and here both were broken by the
same missing component.

## A note on `if let Some(...)` writes

Both sides of this bug are the same shape: a `get_component_mut` write guarded
by `if let Some`, which silently does nothing when the component is absent.
That is fine when presence is an invariant and dangerous when it is an
assumption. It has now caused two bugs in this port (this, and
`add_components` on an unspawned id in the cubic slice) — worth a look wherever
a write is conditional on a component the caller did not create.

## Tests

`servitor_tests` 109 → 111. npc_cast/raid/boss/combat/skills groups re-run clean
(12/14/18/39/75).

## Sweep closed

`Reuses` — bug found and fixed. `TargetRef` — NPCs do not carry one; summon
targeting is passed explicitly through `servitor_attack`, so there is nothing
to resolve. The `Creature`-vs-`Player` sweep is now complete.
