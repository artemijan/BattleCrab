# G29 slice 4 — Servitor lifecycle (upkeep, expiry, leash, logout)

## Why this next

With slices 1-3 a servitor exists, follows, attacks and is visible to everyone.
What it could not do was **end**: the lifetime was recorded and displayed but
never enforced, the upkeep item was parsed and never charged, and logging out
left an ownerless NPC in the world.

## What Java does

`Servitor.run()` is a fixed **5-second** task (`usedtime = 5000`) doing four
things in order:

1. `_lifeTimeRemaining -= 5000`; dead or despawned → cancel the task.
2. `< 0` → `YOUR_SERVITOR_PASSED_AWAY` + `unSummon`.
3. Every `_consumeItemInterval`, `destroyItemByItemId` the upkeep item →
   `A_SUMMONED_MONSTER_USES_S1` on success, or
   "since you do not have enough items to maintain the servitor's stay" +
   `unSummon` on failure.
4. `SetSummonRemainTime(lifeTime, remaining)`, then — *"using same task to
   check if owner is in visible range"* — a **2000-unit leash** that forces
   `AI_INTENTION_FOLLOW`.

The consume interval defaults to **240 s** (60 for siege weapons), which is
what nearly every summon on this dist runs on.

## What landed

- **`ScheduledTask::ServitorLifeTick`**, armed at summon and rescheduling
  itself — Java's `_summonLifeTask`, with the same "dead or gone → stop"
  contract the DoT chain already uses.
- All four steps above, including `SetSummonRemainTime` (0xD1, new).
- **The leash**, which matters more than it looks: an ordered attack clears the
  follow flag, so without it a servitor sent at a distant target would simply be
  abandoned there.
- **Unsummon on the owner leaving the world**, wired into
  `net::store_and_remove_player` so it covers logout *and* disconnect.

## An honest narrowing

Java stores a servitor in `CharSummonTable` on logout and restores it on
reconnect (`RestoreServitorOnReconnect`). Persistence is a later slice, so for
now the servitor simply goes away with its owner. That is a behaviour
difference, not a bug — and it is strictly better than the alternative this
slice replaced, which was leaking an ownerless NPC into the world.

## Tests

`servitor_tests` is now 27. The eight new ones cover: expiry (including that it
survives one tick early), no-expiry servitors never being reaped, the upkeep
item being charged, running out of it dismissing the servitor, no-upkeep
servitors never being charged, the leash pulling an attacking servitor back,
logout taking it with you, and a dead servitor ending the tick chain rather
than rescheduling forever.

## Still open in G29

Master-buff inheritance on spawn, servitor skills (the `ServitorSkillUse`
actions), exp/level and summon points; servitor persistence across reconnect;
then **pets** — the second gate clause — plus cubics and agathions.
