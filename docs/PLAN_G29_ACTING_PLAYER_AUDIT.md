# G29 slice 23 — the `getActingPlayer()` audit, part 2

Two more live bugs from the same root: Java expresses a rule as "do X to
`getActingPlayer()`", and the port tested "is the actor a player" instead.

## PK/karma: a pet kill was a free kill

`Player.doDie`'s reputation block reads `killer.getActingPlayer()`. The port
gated on `has_component::<Player>(&killer_oid)`, so **a summon killing a player
produced no PK counter and no karma for the owner**.

Same exploit shape as the flagging gap in slice 21, and worse in consequence:
flagging affects who may retaliate, this affects whether murder is punished at
all. Set your pet on someone and walk away clean.

## Duels: a summon could really kill

`duel_lethal_guard` exists to hold one invariant — *a duel never kills*, the
loser stops at 1 HP. It began `if !are_dueling(world, attacker, target)`, and a
summon carries no `DuelRef`, so its blow was not recognised as duel damage and
**slipped straight past the cap**. Dueling someone while your pet was out could
kill them for real.

The guard now resolves the attacker first. Note this is a function whose entire
purpose is an invariant, and the invariant was violable by an actor the guard
never considered — worth remembering when auditing other guards.

## A test that was wrong about the post-condition

The duel test first asserted the opponent sits at **1 HP**. It doesn't: capping
ends the duel, and ending it runs `restorePlayerConditions`, which heals both
sides. 1 HP is a transient intermediate the test happened to name.

Corrected to assert the observable outcome — *the opponent survived*. Asserting
an intermediate value couples a test to the order of operations inside the code
it tests, and it would have failed the next time restore moved.

## Tests

`servitor_tests` 105 → 107; duel/pvp/death/combat/quest groups re-run clean
(11/12/26/39/113).

## Audit status

Now checked and fixed: PvP flagging (21), reward attribution (22), PK/karma
(23), duel damage (23). **Four for four** — every `getActingPlayer()` site
probed so far was a live bug in the port.

That hit rate suggests the remaining unchecked sites deserve the same
treatment rather than being assumed fine: clan-war kill counting
(`atWarWith(killer.getActingPlayer())`, Player.java:5371), and the
`OnAttackableKill` event's `killer.isSummon()` flag.

## Still open in G29

`Reuses`/`TargetRef` summon probes, `PET_EQUIP` paperdoll, pet spiritshots,
evolution, reconnect resummon, servitor master-buff inheritance,
`ServitorSkillUse`.
