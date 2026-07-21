# G23 slice 9 — boss barks

## The blocker was one function

`npc_say` lived as a `QuestCtx` method, so a boss script couldn't reach it. Its
body only ever needed the world and the speaker — the quest coupling was
incidental, not structural.

Lifted to `helpers::npc_say(world, npc_oid, npc_string_id)`, with `QuestCtx`
delegating. All 113 quest tests pass unchanged, which is the evidence the move
was behaviour-neutral.

Worth noting as a pattern: a helper that "belongs" to one subsystem because
that's where it was first needed is not the same as one that depends on it.
Check the body before assuming a port is blocked.

## Core's lines

| when | line(s) |
|---|---|
| first hit **of a life** | "A non-permitted target has been discovered." + "Intruder removal system initiated." |
| later hits, 1-in-100 | "Removing intruders." |
| death | "A fatal error has occurred." + "System is being shut down..." |

Two details that would be easy to lose:

- **The intro is once per life, not once per server run.** Java sets
  `_firstAttacked = false` in `onKill`, so the next Core greets its killers
  afresh. Without the reset, a Core killed once would stay silent for the
  lifetime of the process — invisible in testing, obvious to players.
- **The taunt is 1-in-100**, not every swing. Forced both ways so the mechanic
  rather than the RNG is what's tested.

## Tests

`core_boss_tests` 6 → 10: the intro plays once and doesn't replay, dying resets
it, the taunt fires on a hit roll and is silent on a miss, and the death lines
are said.

Counted by opcode (`NpcSay` = 0x30) on a real client channel, so the assertions
measure packets actually sent rather than a flag.

## Still open in G23

Baium (787 lines), Valakas (581), Antharas (1056). Orfen's and Queen Ant's own
barks are a small follow-up now that the helper is reachable.
