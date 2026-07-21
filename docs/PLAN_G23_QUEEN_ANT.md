# G23 slice 5 — Queen Ant

The first grand-boss script. The respawn lifecycle is shared (slice 4); what is
hers is the **larva and the nurses**.

## The fight is a priority rule

Six nurses heal, and they heal the **larva first**. A party that leaves the
larva alive is fighting a Queen whose healers are permanently busy elsewhere —
that ordering *is* the encounter, not a detail of it. Killing the larva is what
frees the nurses to be worth interrupting.

Ported as: larva wounded → heal larva (`HEAL1` or `HEAL2` at random); else Queen
wounded → heal Queen (`HEAL1` only). Both tested, including the switch-over when
the larva dies.

## A branch that cannot fire here

Java also skips a nurse whose **leader is the larva** when healing the Queen.
On this dist the larva (29002) declares **no minions** — only the Queen (29001)
has nurses (×6) and royals (×8) — so no nurse can have the larva as a leader and
that branch is unreachable.

Left out rather than written as dead code, and recorded so its absence is
deliberate. Same call as `EffectFlag.FEAR` and `MP_BLOCK` before it.

## Routed through the ordinary cast path

A nurse's heal goes through `npc_cast::start_cast` behind
`check_use_conditions`, so it pays the same MP and honours the same cooldown as
any other NPC skill — rather than being a privileged script effect that ignores
both. (That cooldown gate only started working two slices before this; see the
NPC-reuse fix.)

## The larva is script-spawned

It is not in the Queen's minion table, so nothing but the script would place it.
Spawned on the Queen's spawn, which the lifecycle now calls into.

## A fixture that made three tests vacuous

The first draft wounded the Queen with an absolute `cur_hp = 10_000`.
`add_test_npc` gives every NPC **100 HP regardless of its template**, so that
set HP *above* max — which reads as "not wounded", so no heal was ever
attempted and three assertions could never have passed for the right reason.

Diagnosed by instrumenting the cast rather than guessing; the tests now wound by
**fraction of max**, with the reason recorded at the helper.

## Tests

New `queen_ant_tests` (6): the larva is spawned, nurses heal a wounded Queen,
the larva takes priority (and the Queen goes untended while it lives), killing
the larva frees the nurses, a nurse of another master is not part of this
Queen's rotation, and a dead Queen ends the beat rather than rescheduling
forever.

## Still open in G23

Nine more boss scripts (Antharas 1056 lines, Baium 787, Valakas 581, Orfen 384,
Sailren 326, DrChaos 321, Core 232, Zaken 109). The shared lifecycle and zone
lookup they need are now in place.
