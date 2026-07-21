# G29 slice 29 — `ServitorSkillUse`

The summon's action-bar buttons: the owner presses one, the **servitor** casts
it.

## What was dead

`ActionData.xml` ships **105** `ServitorSkillUse` rows, each binding an action
id to a skill id. The port's `handle_request_action_use` matched three
hard-coded ids (hold/attack/stop) and returned early on everything else, so
every one of those buttons did nothing.

Reachability, measured before building: **13** of the 105 name a skill that one
of the six summonable servitors on this dist actually has. The rest bind
summons from later chronicles. Thirteen live buttons across the Summoner's
whole pet roster is worth the slice; 105 would have been the wrong number to
quote.

## The loader was already there, half-built

`data/action_data.rs` existed but kept only the id list — enough for
`ExBasicActionList`, discarding `handler` and `option`. Widened to keep the
servitor bindings, which is why this is a lookup rather than 105 match arms.

## A guard that matters

`ActionData.xml` binds buttons for **every** summon in the game, so most rows
name a skill this particular servitor has never had. Casting one anyway would
let any summon borrow any other summon's abilities — a Kai the Cat using a
Spectral Lord's attack.

So the skill must be in the servitor's own `skill_list`, at the level it knows.
Java gets this for free (it looks the skill up on the summon); the port has to
check explicitly because it resolves through the action table first.

## Ordered casts obey the same rules

The cast goes through `npc_cast::start_cast`, the same path the AI uses, behind
the same `check_use_conditions` gate — so a commanded skill pays the same MP,
respects the same mutes, and honours the same cooldown as one the servitor chose
itself. (That gate only started working two slices ago; see the NPC-reuse fix.)

## Tests

`servitor_tests` 123 → 126.

- A servitor casts the skill its button names, verified through the scheduler
  to the effect actually landing.
- It refuses a skill it does not have.
- The **real** `ActionData.xml` binds action 1000 → skill 4079 and 32 → 4230,
  and a non-servitor action binds nothing — a fixture cannot catch a parse
  regression here.

The cast test was confirmed to fail with `start_cast` disabled.

## G29 status

The summon subsystem is complete for this chronicle: summon, feed, persist,
exp, stats, death, revive, decay, regen, shots, equipment, reconnect, buff
persistence, and now commanded skills. Remaining: **pet spiritshots**, which
need pets to cast before the magic half of the shot bonus has anywhere to
apply. Struck as not-on-this-chronicle: agathions, pet evolution, master-buff
inheritance.
