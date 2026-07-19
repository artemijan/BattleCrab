# G22 slice 8 — AllianceMaster, closing the village-master group

`AllianceMaster` is 67 Java lines, the smallest of the 16 and the last one
left. **Port now has 16 of 16 village-master scripts.**

## The script is one guard

`onTalk` always opens `9001-01.htm`. `onEvent` echoes the requested page back
unless the player has no clan, in which case it serves `9001-04.htm` ("You must
be in Clan"):

```java
if (!"9001-01.htm".equals(event) && (player.getClan() == null))
    return "9001-04.htm";
return event;
```

**The asymmetry is the whole script and is easy to "fix" away.** The menu is
explicitly excluded from the gate, so a clanless player *does* see the two
buttons and only learns they can't use them after clicking. Gating `onTalk`
too — which reads like the tidier behaviour — would change what retail shows.
There's a 6-case test pinning both halves: clanless gets the menu but is
refused on `-02`/`-03`, a clan member gets all three.

Pages are numbered against a **virtual NPC id** (`9001-NN.htm`), like
`ClanMaster`'s `9000`. No real master ships a page; the test asserts that so the
virtual id can't be "corrected" to a per-NPC name that would 404. Same 60 NPCs
as `ClanMaster`, deliberately — both attach to every village master.

## The dialog works; both buttons are inert

Worth stating plainly rather than burying: this slice makes the *dialog* work,
not alliances. `9001-02.htm` posts `npc_%objectId%_create_ally $name` and
`9001-03.htm` posts `npc_%objectId%_dissolve_ally` — both
`VillageMaster.onBypassFeedback` verbs in Java, **neither routed here**. The
alliance system as a whole is G18 (`ally_id`/`ally_name` exist only as a DB
column list and a "when the alliance system lands" comment in
`server_packets/clan.rs`).

I checked the failure mode rather than assuming it: unrouted `npc_` verbs hit
the bypass router's fallback arm, which `warn!`s and drops. So the buttons are
inert and *greppable at runtime*, not silently swallowed. A `TODO(G18)` at the
module head names both verbs.

Shipping a dialog whose buttons don't act is the established convention here —
`ClanMaster` already ships with `learn_clan_skills`/`multisell` unrouted — but
it is the same shape as the dead-button bugs this port has hit repeatedly
(`Chat <page>`, the race-track gatekeeper), so it is documented as a known gap
rather than left to be rediscovered as one.

## Tests

3 added, all passing on first run: the menu opens for a clanless player
(byte-compared against the dist page); the 6-case clan/page gate matrix; and a
page-existence sweep that also asserts no per-NPC pages exist.

Also added `QuestCtx::has_clan` (`Player.getClan() != null`; clan id 0 is the
no-clan sentinel), alongside the existing `is_clan_leader`.

## Status

**The village-master group is complete — 16 of 16.** G22 continues with ~188
quests, ~81 `ai/` scripts, daily quests (`restartTime`), tutorial Q00255 and
`//reload`.
