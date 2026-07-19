# G17 slice 6 — occupation change (`Player.setClassId`)

Sixth G17 slice. The class-change *mechanic*, which G17's roadmap line says can
land ahead of G22's occupation quests.

## The bug this fixes — one my own subclass work created

`//setclass` did:

```rust
p.class_id = class_id;
p.base_class_id = class_id;   // unconditional
```

Before subclasses existed that was harmless: there was only ever one slot. Once
slices 2–5 landed, using `//setclass` **while standing on a subclass would
rewrite the character's base class** — silently changing what the character
fundamentally *is*, with no error and no way back.

Java doesn't do that. `Player.setClassId` updates *the active slot*: on a
subclass it's `getSubClasses().get(_classIndex).setClassId(id)`, and
`_baseClass` is only touched on the base slot.

I introduced the hazard by adding subclasses without revisiting the existing
class-change path. Worth noting as a pattern: **when a new axis (here, "which
slot am I on") appears, every existing writer of the affected field becomes
suspect**, not just the new code.

## What landed

`subclass::set_class_id` — the shared mechanic:

- Base slot → `class_id` **and** `base_class_id` move.
- Subclass slot → only `class_id` and that slot's stored class move; the slot
  is re-persisted so the advancement survives a restart.
- `rewardSkills()` + stat recompute + status/UserInfo/SkillList refresh (via
  the existing `set_level` path).
- The **class-change flash** — Java's `broadcastPacket(new MagicSkillUse(this,
  5103, 1, 0, 0))` — to everyone nearby, including the player.

`//setclass` is rewired onto it, so the GM command and any future quest-driven
advancement share one implementation.

## Tests

5 added (26 in the subclass suite): base-slot advancement moves the base class;
**subclass advancement leaves the base class alone** and records the new class
on the slot; that survives a switch away and back; an unknown class id is
rejected with nothing moved; and the visual effect is broadcast.

I verified the key test actually catches the old behaviour by temporarily
restoring the unconditional assignment — it fails, then passes on revert. A
regression test that has never been seen to fail is only a guess.

**774 lib tests; all 9 targets green.**

## Deliberate narrowings (`TODO(G17)`/`TODO(G22)` at the site)

- The clan-side effects Java fires on a *third*-occupation change
  (`PledgeShowMemberListDelete`/`Update` broadcasts) aren't sent.
- Party window refresh on class change.
- The occupation **quests** that normally drive this are G22; this is the
  mechanic they'll call.

## Remaining in G17

- `character_skills_save` (buff restore across logout) is still index-0 only.
- Certification skills: **struck** — no data on this dist.
