# G29 slice 3 — SummonInfo (other players can see a servitor)

## Why this next

Slice 1 left a glaring multiplayer gap: a servitor was visible **only to its
owner**. In a game where summons fight alongside you, that is worse than the
lifecycle gaps also outstanding, so it went first.

## It was far cheaper than it looked

The Java class is 338 lines and I sized it as "its own slice, and masked
packets are where this port's bit-order traps live." That estimate was too
pessimistic: `SummonInfo` uses the **same `NpcInfoType` 37-bit mask format** the
port already implements for `npc_info`, including the `masks::add_mask` /
`contains_mask` helpers and the two-block size accounting. The real work was
the summon-specific component set, not the mask machinery.

Worth recording as a calibration note: check whether a big Java packet shares a
format the port already has before pricing it.

## What differs from `npc_info`

| | `NpcInfo` | `SummonInfo` |
|---|---|---|
| opcode | 0x0C | **0x8B** |
| `TITLE` | template title, conditional | **always**, and carries the **owner's name** |
| `PVP_FLAG` | absent | **always** |
| `NAME` | when server-side named | when `displayId != id` |
| `SUMMONED` | — | marks the spawn animation |
| `RELATIONS` | 0 | the owner's relation to the viewer |

The owner's name in the title slot is what draws the "of X" label under a
summon — the field most likely to be wired to the wrong string, so it has its
own test that searches the encoded packet for the name.

## What landed

- `server_packets::summon_info` — the masked packet, mirroring the existing
  `npc_info` structure.
- `visibility::send_summon_info`, used at **both** introduction points (enter
  world and the region-delta path) so a servitor walking into view is
  introduced the same way as one already there. It returns whether the object
  was a servitor, so the caller falls through to `npc_info` for everything else.
- `servitor::broadcast_summon_info` on summon, with the spawn animation.
- The **owner is excluded everywhere** — they have the `PetInfo` view, and Java
  splits the two the same way in `Summon.sendInfo`.

## Left at Java's defaults

`relation` is 0: the per-viewer PvP relation isn't resolved at this call site
yet. Also clan crests (the owner's), team, reputation, water/fly, enchant and
transformation — none modelled for NPCs on this port.

## Tests

`servitor_tests` is now 19. The three new ones:
`other_players_are_sent_summon_info_and_the_owner_is_not` (both directions —
the owner must *not* get the bystander packet either),
`summon_info_carries_the_owners_name`, and
`a_servitor_entering_view_is_introduced_as_a_summon` (the delta path, which is
a separate call site and could easily have been missed).

## Still open in G29

Lifetime expiry, item consumption, unsummon on logout/death, persistence,
master-buff inheritance, servitor skills, exp/level, summon points — then pets
(the second gate clause), cubics and agathions.
