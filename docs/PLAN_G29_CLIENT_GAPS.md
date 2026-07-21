# G29 slice 10 — features that landed but never reached the client

The cubics slice found `CharInfo` hard-coding `0` for the cubic count, making a
summoned cubic invisible to everyone else — the second bug of that exact shape
after G19's abnormal visuals. So this slice ran the check deliberately instead
of waiting to trip over a third one.

## The sweep

Grep every packet builder for a hard-coded `0` written into a length/count
field, then ask: **does the feature behind that count exist yet?**

| site | verdict |
|---|---|
| `party.rs` summon count | **live gap** — pets/servitors exist since slices 1–8 |
| `enter_world.rs` subclass count | **live gap** — subclasses landed in G17 |
| `enter_world.rs` henna count | dead code — superseded by the real `HennaInfo`; deleted |
| `clan.rs` squad-skill count | correct — sub-pledges genuinely unmodelled |
| `manor.rs` castle count | correct — no castles |
| `enter_world.rs` bookmark count | correct — teleport bookmarks unported |

Two real regressions. Both are the same failure mode: a packet written early,
stubbed honestly at the time, and never revisited when the milestone that
filled it in actually landed. Nothing failed, nothing logged — the feature just
silently didn't reach the client.

## Party-window summons

`PartySmallWindowAll` now writes Java's per-summon block (pet first, then
servitors): object id, `npcId + 1000000` (the client's summon-template space),
the 1=pet/2=servitor discriminator, name, HP/MP and level.

## `ExSubjobInfo`

Java's `_subs.add(0, new SubInfo(player))` puts the **base class first**, then
one row per subclass — so **the count is never 0**, even for a character that
has never subclassed. The port wrote a flat `0`, so the client's class list was
empty for everyone. `SubclassType`: 0 = BASECLASS, 1 = DUALCLASS, 2 = SUBCLASS;
Interlude has no dual class, so subclass rows are always type 2.

## The `SummonRef` refactor

`member_view` takes `&World`, but `pet_of`/`servitor_of` swept the store, which
needs `&mut World` (the ECS builds its `QueryState` mutably). Rather than widen
the packet path to `&mut`, the owner now holds the link:

```rust
struct SummonRef { servitor: Option<i32>, pet: Option<i32> }
```

This is **closer to Java**, not further from it — `Player.getPet()` is a field
read, not a world scan. It is also O(1) instead of a full sweep per call.

The ids are **validated on read** (the helpers check the entity still exists),
so a despawn path that forgets to clear the link yields `None` rather than a
dangling reference. Every spawn/despawn goes through one `set_summon_link`
helper so the link cannot be updated in only one direction, and a test asserts
unsummoning clears it.

## Tests

`servitor_tests` 51 → 54, `subclass_tests` +1. Party-window rows for both a
servitor and a pet (with the right discriminator each), link clearing on
unsummon, and the `ExSubjobInfo` row count including the base class.

All 51 pre-existing servitor tests passed unchanged through the `SummonRef`
refactor, which is the evidence that swapping the lookup strategy didn't change
behaviour.

## Worth keeping as a habit

**A stubbed count is a promise to come back.** When a milestone lands a feature,
grep the packet builders for the count that feature feeds. Three of these have
now been found by accident; this is the first one found on purpose.
