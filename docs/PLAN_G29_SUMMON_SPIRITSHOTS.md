# G29 slice 30 — summon spiritshots (G29 complete)

The last open item, and it turned out not to be blocked.

## "Needs pets to cast first" was wrong

Slices 18 and 29 both closed noting spiritshots were blocked on pets being able
to cast. Checking instead of assuming: `npc_ai_tick`'s summon branch already
runs `think_attack` for a summon in Attack intention, and `think_attack` calls
`npc_cast::try_cast`. **Summons have been casting since G21.** 53 distinct
active skills exist across the 56 pet species.

Fourth carried-forward note wrong on inspection this milestone. The check cost
one grep; the note had deferred a whole slice.

## The mirror of slice 18, with one real difference

Soulshots are charged before a **swing** and spent by a landed hit. Spiritshots
are charged before a **cast** and spent by the cast itself — so the charge lives
in `npc_cast::start_cast` and the spend in the effect path, not the attack loop.
Java splits them the same way (`Summon.doCast` → `rechargeShots(false, true,
false)`).

Cost is the pet level's `spiritshot_count`, parsed back in slice 18 and unread
until now.

## The same `Player`-only gate, again

`apply_skill_effects` read the spiritshot flags off `crate::model::Player`, so
an NPC caster silently got no bonus — the identical shape as the soulshot gate
in slice 18 and the `ChargedShots`-on-`Creature` mismatch behind it. Third
instance of that pattern in this subsystem.

Blessed Beast Spiritshots do not exist on this dist, so only the ×2 tier is
reachable; the ×4 branch stays player-only.

## Tests

`servitor_tests` 126 → 130.

- A summon charges from its owner at the level's cost.
- A charged spiritshot **roughly doubles the summon's magic damage** — measured
  by running the same cast twice, charged and not, rather than asserting a
  flag.
- One cast spends it; the next is unshotted.
- A **physical** skill does not burn a magic shot.

## G29 complete

Summon, feed, persist, exp, stats, death, revive, decay, regen, soulshots,
spiritshots, equipment, reconnect (pet + servitor), buff persistence, commanded
skills, cubics.

Struck as not-on-this-chronicle, each verified against the datapack: agathions
(166 skills, 0 learnable), pet evolution (no item handler), master-buff
inheritance (Freya-era).
