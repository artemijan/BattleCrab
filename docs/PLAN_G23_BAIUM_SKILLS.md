# G23 slice 13 — Baium's skill selection

`manageSkills`, which completes Baium's combat behaviour.

## Two mechanics beyond "pick a skill"

**The rotation.** After acting on the top threat, Java knocks it down to **500**
seventy percent of the time. That is what stops Baium tunnelling the single
biggest damage dealer for the whole fight — the next-highest gets a turn. It
reads like bookkeeping and is the reason the encounter moves.

**The widening pool.** The skill list grows as he weakens:

| health | options before the fallback |
|---|---|
| above 75% | Energy Wave, Earthquake |
| above 50% | + Group Hold |
| above 25% | + Thunderbolt |
| below 25% | all four |

Each is an independent 10% roll taken **in order**, with the basic attack as the
fallback — so a party watches his repertoire open up as the fight goes on. The
same shape as his threat weighting from slice 12, which is presumably deliberate
design rather than coincidence.

## Pruning is targeting

A threat entry whose attacker has **died or gone beyond 9000 units** is zeroed.
That is not tidiness: it is how a fled or dead player stops holding a slot, and
it can change who Baium attacks — a test seeds the highest threat on a corpse
and the second on someone who ran, and asserts he turns on the third, *lowest*,
attacker.

## Tests

`baium_tests` 8 → 14. Rolls are forced throughout, so each test isolates one
decision: the highest threat is chosen, the decay knocks it below its rival,
pruning redirects him, the first option of each HP band is revealed, every roll
missing falls back to the basic attack, and an empty table yields no action.

The band test asserts the **first option of each band** rather than "some skill
was chosen" — that is what distinguishes a correctly-ordered ladder from four
skills in a bag.

## Baium is complete

Archangels, the strider debuff, the threat table, and skill selection. The
remaining G23 work is Valakas's entry flow and Antharas, both gated on
`SpecialCamera`.
