# G23 slice 1 — the raid curse

G23's gate is *"a raid boss spawns on schedule, applies raid curse, and its
state persists."* Checking before planning — the lesson from G20.5, whose row
sat at ⏳ while the work was already done — **two of the three clauses were
already met**: `boss_respawn` (built during G21) covers scheduled respawn and
`npc_respawns` persistence for all 225 `dbSave` spawns.

Raid curse had **zero references** in the port. This is that clause.

## An anti-farming rule, not a difficulty one

A player **more than 8 levels above** a raid boss is punished for interfering.
It exists to stop a high-level character trivialising a raid for a low-level
party — which is why it fires on *helping*, not only on attacking.

Two skills, both already in the datapack with effects the port already had:

| skill | effects | duration | trigger |
|---|---|---|---|
| 4215 `RAID_CURSE` | `Mute` + `PhysicalMute` | 3600 s | casting a **good** skill nearby |
| 4515 `RAID_CURSE2` | `BlockActions` | 120 s | attacking it, or a **bad** skill nearby |

That the *silence* lasts an hour and the *petrification* two minutes looks
inverted until you read the intent: the long one punishes buffing from safety,
the short one punishes a mistake in melee.

## Two trigger sites, and the ordering is explicit

- **Damage** (`Attackable.reduceCurrentHp`) — hooked **after** the damage block,
  because Java's own comment says *"In retail you deal damage to raid before
  curse."* The hit that earns the curse still lands.
- **Post-cast** (`Creature`'s tail block) — scans every attackable within
  `ALT_PARTY_RANGE`, so a high-level player buffing a low-level party from
  outside the fight is caught. The damage-side check never sees that case.

The boss must be **in combat** for the cast-side rule; casting near an idle
boss is free, which is what keeps ordinary travel past a spawn point safe.

## Details worth pinning

- `giveRaidCurse()` is true for a raid/grand boss **and for a raid minion**,
  which inherits the answer from its master — so a boss's adds curse too.
- The boundary is Java's `> level + 8`, i.e. nine levels above. Written that
  way deliberately: "improving" it to `>= 9` reads identical and is the same
  value, but `> 8` is what the Java says and the constant is named `LEVEL_GAP`
  to stop it drifting. A test pins that exactly 8 is *not* cursed.
- The **boss** is the caster of the curse, so the debuff's landing rate reads
  the boss's level rather than the victim's own.
- `DisableRaidCurse` (false on this dist) is honoured, not assumed.

## Tests

New `raid_curse_tests`, 7. The boundary both ways, ordinary monsters not
cursing, the config flag, both skill kinds on the cast side, an idle boss
cursing nobody, and an **end-to-end** hit through `apply_physical_damage` that
asserts both the curse landed *and* the damage was dealt — the helper being
correct proves nothing if the damage path never calls it.

## Still open in G23

Boss zones + entry conditions, chaos target swaps, minion waves, raid points.
