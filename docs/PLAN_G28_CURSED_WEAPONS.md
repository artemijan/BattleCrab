# PLAN G28 — Cursed weapons: the autonomous gameplay loop

**Status:** landed (branch `feat/g28-cursed-weapons`). The cursed-weapon half of
the G28 gate ("a cursed weapon can be dropped and equipped") is met. The events
engine (TvT / `AbstractEvent`) half of G28 is still pending.

## What existed before

The **activation engine** was already ported (branch `feat/g21-…`, in
`game_loop/admin/cursed_weapons.rs`): `activate` (equip + transform + skill +
karma swap + full heal + announce + DB `saveData`) and `end_of_life` (restore
the wielder, strip the weapon, announce, clear the DB row, reset), driven only
by the `//cw_*` GM commands. `CursedWeapons.xml` was loaded into
`world.cursed_weapons` at boot. But nothing made a cursed weapon enter or leave
the world through *play* — the whole `CursedWeaponsManager.checkDrop` /
`CursedWeapon.dropIt` / pickup / `RemoveTask` loop was a `TODO(G21)`.

## What this slice adds — `game_loop/cursed_weapon.rs`

The autonomous loop, wired to the two real entry points:

1. **Drop on monster death** — `on_monster_killed`, called from
   `death::npc_do_die` (right after `calculate_rewards`). Port of
   `CursedWeaponsManager.checkDrop` → `CursedWeapon.checkDrop`/`dropIt`:
   - Gated exactly as Java: the killer's **acting player** (a pet's kill counts
     for the owner) must be a real player who isn't already cursed; the victim
     must be an ordinary monster (`is_monster() && !is_raid()` and not a
     `FeedableBeast` — Java also excludes `Defender`/`Guard`/`FortCommander`,
     which aren't `is_monster()` here).
   - Each not-in-world weapon rolls `Rnd.get(100000) < dropRate` independently;
     the first hit drops and breaks.
   - `drop_weapon` spawns the ground item with a **new `DropSource::CursedWeapon`**
     that is exempt from `ItemsAutoDestroy` (Java's `_item.setDropTime(0)`) — the
     weapon's own life task owns its removal. Broadcasts `ExRedSky(10)` +
     `Earthquake`, sets `is_dropped`, records the ground item oid, arms the life
     task at **`now + duration` (the full 300 min, per `checkDrop`)**, and
     announces `S2_WAS_DROPPED_IN_THE_S1_REGION`.

2. **Pickup activates the curse** — `try_pickup`, intercepted inside
   `ground_items::pickup_ground_item` (via `is_dropped_cursed`). Port of
   `CursedWeaponsManager.activate`:
   - Un-cursed picker → get-item animation + despawn, then reuse
     `admin::cursed_weapons::activate` to curse them.
   - Already-cursed picker → the newly grabbed weapon is consumed (resets to
     not-in-world), matching Java erasing the duplicate. `TODO(G28)`: Java also
     grants the wielded weapon a stage-kill bonus (`increaseKills`) — deferred
     with the kill-count level-up.

3. **Expiry** — `handle_expiry`, dispatched from the game loop on
   `ScheduledTask::CursedWeaponExpiry` (armed by `arm_expiry`). Port of
   `CursedWeapon.RemoveTask`: only end-of-lifes when `now >= end_time` (a
   re-armed/superseded timer no-ops via that guard). A dropped weapon despawns
   its ground item first; an activated one is stripped from its wielder — both
   fall through to `end_of_life`.

New model field `CursedWeapon.dropped_item_oid` (runtime only, cleared by
`reset`) ties the state to its ground entity so expiry can despawn it. New SM id
`S2_WAS_DROPPED_IN_THE_S1_REGION = 1817`.

## Deferred (`TODO(G28)`, a follow-up slice)

- **Kill-count level-up** — `increaseKills` / stage bonus (the weapon growing
  stronger as its wielder kills), and the already-cursed-picker stage bonus.
- **"Hungry" decay** — the periodic HP/time drain (`durationLost` polling).
- **Drop on PK death** — `dropIt(killer)` when the cursed wielder is killed.
- **Login restore** — `cursedOnLogin` re-applying a weapon across relog, with
  the `_endTime -= durationLost*60000` session penalty.
- **Region name** — the announce `SysString(0)` renders blank until MapRegion
  carries its sysstring id.
- **Pickup end_time preservation** — Java keeps the drop's `end_time` on pickup
  (total on-ground + wielded life is one `duration`); reusing `activate` resets
  the clock to `now + duration`, granting back the (short) ground-lying time.

## Tests — `game_loop/tests/cursed_weapon_tests.rs` (10)

Drop hit/miss, cursed-killer and raid-kill exclusions, pickup-through-the-real-
`pickup_ground_item`-entry (curse + persist), already-cursed consume, ungrabbed-
drop expiry (despawn + DB clear + announce), premature-timer no-op, the
scheduled task firing through the loop dispatch, and the death-path wire via
`npc_do_die`. All three wires (death hook, pickup interception, scheduler
dispatch) sabotage-verified.
