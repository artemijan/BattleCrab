# G29 slice 28 — a servitor's buffs survive a relog

Completes slice 27: the servitor came back, but everything cast on it was
dropped.

## A mislabelled TODO, corrected

The remaining-work list called this *"servitor master-buff inheritance"* — a
Freya-era mechanic where a summon inherits its owner's buffs. That is **not**
what `SummonEffectTable` is. It persists the summon's **own** buffs across a
relog (`character_summon_skills_save`), which is a different and much more
useful thing on this chronicle.

Third mislabelled carried-forward note in this milestone (after the corpse
"24 hours" and the "no regen fields" claim). Reading the Java before scheduling
the work keeps being worth more than the note that scheduled it.

## Why it matters here specifically

Slice 19 turned on the Summoner support kit — Servitor Haste, Wind Walk, Magic
Boost, the four class buffs. A summoner spends real time and MP on those. Slice
27 then restored the servitor on reconnect **stripped of all of them**, which is
arguably worse than not restoring it: the player has to re-buff without even
being told why.

## Reuse, not reimplementation

- `SkillBuffRow` is reused verbatim — a servitor's buff has the same
  relative-remaining-time semantics as the player's own, frozen while offline.
- Restoring goes through `restore_persisted_buffs`, the same function the
  player's login path uses. A servitor's buff cannot drift from a player's.
- `ORDER BY buff_index` on load, so buffs come back in the order they were
  applied — which matters once the buff-slot cap is involved.

Expired buffs are filtered at capture, so relogging can't resurrect a buff that
had already run out.

## Best-effort writes, again

Both new statements use the same tolerance as slice 27's: a missing
`character_summon_skills_save` must not abort the character's whole save. The
lesson from last slice applied without having to relearn it.

## Tests

`servitor_tests` 121 → 123, `char_persistence` extended.

- A buffed servitor comes back **still buffed** — asserted on the buff's actual
  effect (run speed), not on the row's presence.
- An expired buff is not carried across.
- The real-schema round trip carries the buff and its remaining time.

## Still open in G29

Pet spiritshots (needs pets to cast) and `ServitorSkillUse`. **Master-buff
inheritance is struck** — it is not a mechanic on this chronicle.
