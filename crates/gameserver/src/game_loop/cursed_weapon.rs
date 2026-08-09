//! Cursed weapons — the autonomous gameplay loop (G28, PLAN_G28_CURSED_WEAPONS.md).
//!
//! The activation engine (`activate` / `end_of_life`) lives in
//! [`super::admin::cursed_weapons`], where the `//cw_*` GM commands drove it
//! first; this module wires the parts that make a cursed weapon enter and leave
//! the world through *play*: a slain monster has a tiny chance to **drop** one
//! (`CursedWeaponsManager.checkDrop` → `CursedWeapon.checkDrop`), a player who
//! **picks it up** becomes cursed (reusing `activate`), and the weapon
//! **expires** when its life runs out — the `RemoveTask` deadline for both the
//! un-grabbed drop and the wielder — and a wielder who relogs comes back
//! **still cursed** ([`on_enter_world`]). The kill-count stage-up
//! ([`increase_kills`]), the per-kill time decay in its tail, and
//! drop-on-PK-death ([`on_wielder_death`]) all landed with that follow-up
//! slice. There is no HP decay to port: Java's only HP touch is the full heal
//! `activate` gives the new wielder.

use crate::game_loop::guard::position;
use crate::game_loop::helpers::send_to_client;
use crate::model::Player;
use crate::model::components::{Position, SkillBook};
use crate::model::inventory::Inventory;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::admin::cursed_weapons::{activate, end_of_life, idx_by_item, now_millis};
use super::ground_items::{DropSource, despawn_ground_item, spawn_ground_item};
use crate::game_loop::helpers::region_cell_of;

const TICKS_PER_SECOND: u64 = 10;
const MILLIS_PER_MINUTE: i64 = 60_000;
/// Java `CursedWeapon.dropRate` is out of 100000 (config comment "100000 for
/// 100%"), so a value of 50 is 0.05%.
const DROP_RATE_SCALE: i32 = 100_000;

/// `CursedWeaponsManager.checkDrop(attackable, player)` — a monster slain by a
/// player may drop a not-yet-in-world cursed weapon. No-op unless the killer is
/// a real, un-cursed player and the victim is an ordinary monster (Java
/// excludes `Defender`/`Guard`/`GrandBoss`/`FeedableBeast`/`FortCommander`).
pub(crate) fn on_monster_killed(world: &mut World, monster_oid: i32, killer_oid: i32) {
    if world.cursed_weapons.is_empty() {
        return;
    }
    // Every not-in-world weapon rolls; the first to hit drops (Java breaks).
    let candidates: Vec<usize> = (0..world.cursed_weapons.len())
        .filter(|&i| !world.cursed_weapons[i].is_active())
        .collect();
    if candidates.is_empty() {
        return;
    }

    let killer = super::pvp::acting_player(world, killer_oid);
    let eligible_killer = world
        .objects
        .get_component::<Player>(&killer)
        .is_some_and(|p| p.cursed_weapon_equipped_id == 0);
    if !eligible_killer {
        return;
    }
    // Ordinary monster only — `is_monster()` covers the Monster subtree
    // (including raids/feedable beasts), so subtract the excluded kinds.
    let ordinary = world
        .objects
        .get_component::<crate::model::npc::Npc>(&monster_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.is_monster() && !t.is_raid() && t.type_name != "FeedableBeast");
    if !ordinary {
        return;
    }
    let Some(pos) = position(world, monster_oid) else {
        return;
    };

    for idx in candidates {
        let drop_rate = world.cursed_weapons[idx].drop_rate;
        if world.roll(DROP_RATE_SCALE) < drop_rate {
            drop_weapon(world, idx, killer, pos.x, pos.y, pos.z);
            break;
        }
    }
}

/// `CursedWeapon.dropIt(attackable, player)` + the `checkDrop` tail: spawn the
/// weapon on the kill site (exempt from auto-destroy), red-sky + earthquake to
/// everyone, arm the full-`duration` life task, and announce the drop.
fn drop_weapon(world: &mut World, idx: usize, killer: i32, x: i32, y: i32, z: i32) {
    let (item_id, duration) = {
        let cw = &world.cursed_weapons[idx];
        (cw.item_id, cw.duration)
    };
    let oid = spawn_ground_item(world, item_id, 1, 0, x, y, z, 0, DropSource::CursedWeapon);

    // RedSky + Earthquake at the drop site (Java `dropIt`, fromMonster branch).
    world.broadcast_to_all_online(&server_packets::ex_red_sky(10));
    let quake = {
        let p = position(world, killer).unwrap_or(Position {
            x,
            y,
            z,
            heading: 0,
        });
        server_packets::earthquake(p.x, p.y, p.z, 14, 3)
    };
    world.broadcast_to_all_online(&quake);

    // Java's `checkDrop` arms the life task for the FULL duration (not
    // durationLost) — the ground weapon lives just as long as a wielded one.
    let deadline = now_millis() + (duration as i64) * MILLIS_PER_MINUTE;
    {
        let cw = &mut world.cursed_weapons[idx];
        cw.is_activated = false;
        cw.is_dropped = true;
        cw.dropped_item_oid = oid;
        cw.player_id = 0;
        cw.nb_kills = 0;
        cw.end_time = deadline;
    }
    // "$s2 was dropped in the $s1 region." Java `addZoneName(x, y, z)`: the
    // client resolves the region from the coordinates, so the drop point is
    // all the server sends.
    let announce = server_packets::system_message_with(
        sm_ids::S2_WAS_DROPPED_IN_THE_S1_REGION,
        &[SmParam::ZoneName { x, y, z }, SmParam::ItemName(item_id)],
    );
    world.broadcast_to_all_online(&announce);
    arm_expiry(world, idx);
}

/// Whether `item_id` is a cursed weapon currently lying on the ground — the
/// gate `pickup_ground_item` uses to route into [`try_pickup`].
pub(crate) fn is_dropped_cursed(world: &World, item_id: i32) -> bool {
    idx_by_item(world, item_id).is_some_and(|i| world.cursed_weapons[i].is_dropped)
}

/// `CursedWeaponsManager.activate(player, item)` for a picked-up drop: the
/// pickup animation, despawn, then either curse an un-cursed picker (the common
/// case) or silently consume the weapon if the picker already wields one.
pub(crate) fn try_pickup(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    item_oid: i32,
    region: (i32, i32),
    item_id: i32,
    pos: Position,
) {
    let Some(idx) = idx_by_item(world, item_id) else {
        return;
    };

    // Pickup animation to nearby, then remove the ground item.
    super::helpers::broadcast_near_region(
        world,
        region,
        &server_packets::get_item(player_oid, item_oid, pos.x, pos.y, pos.z),
    );
    despawn_ground_item(world, item_oid, region);
    {
        let cw = &mut world.cursed_weapons[idx];
        cw.is_dropped = false;
        cw.dropped_item_oid = 0;
    }

    let already_cursed = world
        .objects
        .get_component::<Player>(&player_oid)
        .is_some_and(|p| p.cursed_weapon_equipped_id != 0);
    if already_cursed {
        // `CursedWeaponsManager.activate`'s "cannot own 2 cursed swords" branch:
        // the weapon already held gets a **full stage bonus** — Java sets its
        // count to `stageKills - 1` and then calls `increaseKills`, so the very
        // next increment trips the stage boundary and levels the skill — and
        // the newly grabbed one is erased (`setPlayer` + `endOfLife`, which for
        // a non-activated weapon just destroys the item and resets it).
        let held_id = world
            .objects
            .get_component::<Player>(&player_oid)
            .map_or(0, |p| p.cursed_weapon_equipped_id);
        if let Some(held) = idx_by_item(world, held_id) {
            world.cursed_weapons[held].nb_kills = world.cursed_weapons[held].stage_kills - 1;
            increase_kills(world, held);
        }
        world.cursed_weapons[idx].reset();
        let _ = client_id;
        return;
    }

    // Curse the picker (equip + transform + skill + full heal + announce).
    // `activate` leaves `end_time` alone, so the picker inherits what is left of
    // the deadline the drop armed — on-ground plus wielded time is one
    // `duration`, as in Java.
    activate(world, idx, player_oid);
    arm_expiry(world, idx);
}

// ---------------------------------------------------------------------------
// The wielder's own life: kills, and losing the weapon on death
// ---------------------------------------------------------------------------

/// `CursedWeaponsManager.increaseKills` → `CursedWeapon.increaseKills`: the
/// wielder killed a player. Java bumps the kill count (which the client shows
/// in the *PK counter* — `setPkKills(_nbKills)`, deliberately overwriting it),
/// levels the weapon's skill on every `stageKills`-th kill up to the skill max,
/// burns `durationLost` minutes off the remaining life, and persists.
///
/// The time penalty is the reason a cursed weapon punishes an active killer:
/// each kill brings the expiry closer. Because this port arms a **one-shot**
/// task at `end_time` (Java re-polls at a fixed rate), moving `end_time`
/// earlier has to re-arm it — otherwise the shortened life is decorative and
/// the weapon still runs to its original deadline.
pub(crate) fn increase_kills(world: &mut World, idx: usize) {
    let (player_id, stage_kills, skill_max_level, duration_lost) = {
        let cw = &world.cursed_weapons[idx];
        (
            cw.player_id,
            cw.stage_kills.max(1),
            cw.skill_max_level.max(1),
            cw.duration_lost,
        )
    };
    world.cursed_weapons[idx].nb_kills += 1;
    let nb_kills = world.cursed_weapons[idx].nb_kills;

    if world.objects.has_component::<Player>(&player_id) {
        // Java `setPkKills(_nbKills)`: the cursed kill tally *replaces* the PK
        // count while the weapon is held (the pre-curse value was saved at
        // `activate` and is put back at end-of-life).
        if let Some(p) = world.objects.get_component_mut::<Player>(&player_id) {
            p.pk_kills = nb_kills;
        }
        // Stage up: every `stageKills`-th kill, while a higher skill level
        // exists. Java's `_nbKills <= _stageKills * (_skillMaxLevel - 1)` is the
        // clamp — past it `giveSkill` would just re-grant the max level.
        if nb_kills % stage_kills == 0 && nb_kills <= stage_kills * (skill_max_level - 1) {
            give_skill(world, idx, player_id);
        }
        super::party::broadcast_user_info(world, player_id);
    }

    world.cursed_weapons[idx].end_time -= (duration_lost as i64) * MILLIS_PER_MINUTE;
    super::admin::cursed_weapons::save_data(world, idx);
    // `end_time` moved earlier — re-arm so the shortened life is real.
    arm_expiry(world, idx);
}

/// The PvP-kill hook (Java `Player.onPlayerKill`, first branch): a wielder who
/// kills a player scores the weapon and **skips normal PvP/PK reputation
/// entirely** — Java `return`s before the olympiad, duel, siege and PVP-zone
/// legs, so a cursed kill never awards pvp kills or karma and never counts as
/// a PK. Returns `true` when it handled the kill, telling the caller to stop.
pub(crate) fn on_player_kill(world: &mut World, killer_oid: i32, victim_oid: i32) -> bool {
    let equipped = world
        .objects
        .get_component::<Player>(&killer_oid)
        .map_or(0, |p| p.cursed_weapon_equipped_id);
    // Java's `target.isPlayer()` — a slain *summon* does not score.
    if equipped == 0 || !world.objects.has_component::<Player>(&victim_oid) {
        return false;
    }
    let Some(idx) = idx_by_item(world, equipped) else {
        return false;
    };
    increase_kills(world, idx);
    true
}

/// `CursedWeaponsManager.drop` → `CursedWeapon.dropIt(killer)`: the wielder
/// died. Java rolls `Rnd.get(100) <= disapearChance` — note the `<=`, so the
/// configured 50 is really 51-in-100 — and on a hit the weapon leaves the world
/// outright; otherwise it drops at the corpse for the next taker, keeping its
/// remaining life and kill count (only `endOfLife` resets those).
pub(crate) fn on_wielder_death(world: &mut World, victim_oid: i32, killer_oid: i32) {
    let equipped = world
        .objects
        .get_component::<Player>(&victim_oid)
        .map_or(0, |p| p.cursed_weapon_equipped_id);
    if equipped == 0 {
        return;
    }
    let Some(idx) = idx_by_item(world, equipped) else {
        return;
    };
    if world.roll(100) <= world.cursed_weapons[idx].disappear_chance {
        end_of_life(world, idx);
        return;
    }
    drop_from_wielder(world, idx, victim_oid, killer_oid);
}

/// `CursedWeapon.dropIt(null, null, killer, false)` plus the restore tail of
/// `dropIt(Creature)`: unequip and drop the weapon where the wielder fell, put
/// their saved reputation/pk-kills back, lift the curse, and announce.
fn drop_from_wielder(world: &mut World, idx: usize, victim_oid: i32, killer_oid: i32) {
    let (item_id, skill_id, saved_rep, saved_pk) = {
        let cw = &world.cursed_weapons[idx];
        (
            cw.item_id,
            cw.skill_id,
            cw.player_reputation,
            cw.player_pk_kills,
        )
    };
    let pos = position(world, victim_oid)
        .or_else(|| position(world, killer_oid))
        .unwrap_or(Position {
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
        });

    // Take the weapon off the corpse. The cursed weapon is worn, so this goes
    // through the destroy protocol rather than a bare `remove_item`.
    crate::game_loop::items::destroy_item_by_id(world, victim_oid, item_id, 1);
    // Reset the wielder (Java does this in both `dropIt` and its caller).
    if let Some(p) = world.objects.get_component_mut::<Player>(&victim_oid) {
        p.reputation = saved_rep;
        p.pk_kills = saved_pk;
        p.cursed_weapon_equipped_id = 0;
    }
    // `removeSkill()` — drop the weapon skill (with the passive stat pumps it
    // landed: taking only the book entry leaves the wielder with the cursed CP
    // bar) and revert the transform.
    super::skills::remove_player_skill(world, victim_oid, skill_id);
    super::admin::transforms::remove_transform(world, victim_oid);
    super::admin::refresh_skill_list(world, victim_oid);
    if let Some(cid) = super::helpers::client_for_player(world, victim_oid) {
        if let Some(inv) = world.objects.get_component::<Inventory>(&victim_oid) {
            let list = crate::network::enter_world::item_list(inv, &world.data, false);
            send_to_client(world, cid, list);
        }
        // The bag list alone leaves the *model* holding the sword: the client
        // reads its own paperdoll from `ExUserInfoEquipSlot`, which Java emits
        // from `setPaperdollItem` inside the `dropItem` → `removeItem` →
        // unequip chain. See `items::refresh_equip_state`.
        super::items::refresh_equip_state(world, cid, victim_oid);
    }
    super::party::broadcast_user_info(world, victim_oid);

    // On the ground it goes, at the corpse, exempt from auto-destroy.
    let oid = spawn_ground_item(
        world,
        item_id,
        1,
        0,
        pos.x,
        pos.y,
        pos.z,
        0,
        DropSource::CursedWeapon,
    );
    {
        let cw = &mut world.cursed_weapons[idx];
        cw.is_activated = false;
        cw.is_dropped = true;
        cw.dropped_item_oid = oid;
        // `player_id`/`nb_kills`/`end_time` deliberately survive: Java only
        // clears them in `endOfLife`, so the next taker inherits the tally.
    }
    super::admin::cursed_weapons::save_data(world, idx);

    // Java's `dropIt` announces the region the *wielder* fell in.
    let (x, y, z) = world
        .objects
        .get_component::<Position>(&victim_oid)
        .map_or((0, 0, 0), |p| (p.x, p.y, p.z));
    let announce = server_packets::system_message_with(
        sm_ids::S2_WAS_DROPPED_IN_THE_S1_REGION,
        &[SmParam::ZoneName { x, y, z }, SmParam::ItemName(item_id)],
    );
    world.broadcast_to_all_online(&announce);
}

// ---------------------------------------------------------------------------
// Login restore — `CursedWeaponsManager.checkPlayer` + `CursedWeapon.cursedOnLogin`
// ---------------------------------------------------------------------------

/// The curse survives a relog. Java splits this across two call sites:
/// `Player.restore` → `CursedWeaponsManager.checkPlayer` re-binds the weapon to
/// the freshly loaded character (`cursedWeaponEquippedId` + `giveSkill` + the
/// time-left notice), and `EnterWorld.runImpl` → `CursedWeapon.cursedOnLogin`
/// then re-applies the transform, re-grants the skill and announces the login.
/// Both halves land here, at Java's `EnterWorld` position — right after
/// `spawnMe`.
///
/// Without it a relog quietly *lifted* the curse: the character came back
/// holding an ordinary-looking sword, un-transformed and without the cursed
/// skill, and every `isCursedWeaponEquipped()` gate downstream (weapon swap,
/// party join, Olympiad, mounts, support magic) read `false`.
///
/// The `None` tail is `EnterWorld`'s "Remove demonic weapon if character is not
/// cursed weapon equipped" sweep — the safety net for a weapon whose life ran
/// out while its owner was offline, which leaves the item behind in their bag.
pub(crate) fn on_enter_world(world: &mut World, client_id: u32, object_id: i32) {
    let Some(idx) = world
        .cursed_weapons
        .iter()
        .position(|cw| cw.is_activated && cw.player_id == object_id)
    else {
        destroy_stray_cursed_items(world, client_id, object_id);
        return;
    };
    let item_id = world.cursed_weapons[idx].item_id;

    // `checkPlayer`: re-bind the weapon to this character. Everything that
    // gates on the curse reads this field, so it goes first.
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.cursed_weapon_equipped_id = item_id;
    }
    // `cursedOnLogin`: doTransform + giveSkill.
    do_transform(world, object_id, item_id);
    give_skill(world, idx, object_id);

    // "$s2's owner has logged into the $s1 region." to everyone — the region
    // the owner logged in at (Java `cursedOnLogin`'s `addZoneName`).
    let (x, y, z) = world
        .objects
        .get_component::<Position>(&object_id)
        .map_or((0, 0, 0), |p| (p.x, p.y, p.z));
    let announce = server_packets::system_message_with(
        sm_ids::S2_S_OWNER_HAS_LOGGED_INTO_THE_S1_REGION,
        &[SmParam::ZoneName { x, y, z }, SmParam::ItemName(item_id)],
    );
    world.broadcast_to_all_online(&announce);

    // "$s1 has $s2 minute(s) of usage time remaining." to the wielder alone.
    let minutes = (world.cursed_weapons[idx].time_left(now_millis()) / MILLIS_PER_MINUTE) as i32;
    send_to_client(
        world,
        client_id,
        server_packets::system_message_with(
            sm_ids::S1_HAS_S2_MINUTE_S_OF_USAGE_TIME_REMAINING,
            &[SmParam::ItemName(item_id), SmParam::Int(minutes)],
        ),
    );
}

/// `CursedWeapon.doTransform` — Zariche (8190) becomes transform 301, Akamanah
/// (8689) transform 302. Java stops an existing transform and re-transforms
/// 500 ms later (the client needs the two model swaps separated); the state
/// swap here is synchronous, so the revert runs inline and the apply's own
/// delayed visual refresh carries the new model.
pub(crate) fn do_transform(world: &mut World, target: i32, item_id: i32) {
    let transform_id = if item_id == 8689 { 302 } else { 301 };
    if world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.transform_id != 0)
    {
        super::admin::transforms::remove_transform(world, target);
    }
    super::admin::transforms::apply_transform(world, target, transform_id);
}

/// `CursedWeapon.giveSkill` — the weapon's own skill at Java's
/// `1 + kills/stageKills` (clamped to the skill's max level), then a refreshed
/// skill list. Java additionally adds Void Burst / Void Flow as *transform*
/// skills; on this dist the 301/302 transform templates already list both
/// (3630/3631), so [`do_transform`] grants them.
///
/// Written against `nb_kills` rather than `CursedWeapon::level()` on purpose:
/// `level()` returns 0 until `is_activated` is set, and `activate` grants the
/// skill before flipping that flag.
pub(crate) fn give_skill(world: &mut World, idx: usize, target: i32) {
    let (skill_id, level) = {
        let cw = &world.cursed_weapons[idx];
        (
            cw.skill_id,
            (1 + cw.nb_kills / cw.stage_kills.max(1)).min(cw.skill_max_level.max(1)),
        )
    };
    if world.data.skill_data.get(skill_id, level).is_some()
        && let Some(book) = world.objects.get_component_mut::<SkillBook>(&target)
    {
        book.0.insert(skill_id, level);
    }
    // Java's `addSkill` runs the skill's effects through the `EffectList` as it
    // is learned. Both cursed skills are passives whose pumps are most of what
    // wearing the curse *is* (Akamanah 3629 L1: `MaxCp` ×11.5 +1300, ±PAtk/
    // MAtk/defence), so without this the wielder gets the model and the sword
    // and none of the power. `activate`'s full heal runs after this call, so
    // the grown CP bar is the one that gets filled.
    super::passive_skills::refresh_conditioned_passives(world, target);
    super::admin::refresh_skill_list(world, target);
}

/// `EnterWorld`'s "Remove demonic weapon if character is not cursed weapon
/// equipped": a Zariche/Akamanah sitting in the bag of someone the manager does
/// *not* consider cursed is a leftover (its life ended while they were offline)
/// and is destroyed on sight. Java names the two ids inline; iterating the
/// config is the same set on this dist and stays right if it ever changes.
///
/// The **skill** half has no Java counterpart, for the same reason
/// `RestoreOfflineCursedOwner` carries a skill list: Java grants the cursed and
/// transform skills with `addSkill(…, false)` / `addTransformSkill`, which never
/// touch the DB, whereas this port persists the whole `SkillBook`. Any row that
/// escaped into `character_skills` — from an older build, or a crash between the
/// removal and the next flush — would otherwise re-arm the curse's passive pumps
/// (Akamanah 3629 is `MaxCp` ×11.5 +1300) on every single login, with no weapon
/// in sight to explain it. Scrub them here, where the manager has just said this
/// character is not cursed.
fn destroy_stray_cursed_items(world: &mut World, client_id: u32, object_id: i32) {
    let stale_skills: Vec<i32> = (0..world.cursed_weapons.len())
        .flat_map(|idx| {
            let item_id = world.cursed_weapons[idx].item_id;
            super::admin::cursed_weapons::curse_granted_skill_ids(world, idx, item_id)
        })
        .filter(|id| {
            world
                .objects
                .get_component::<SkillBook>(&object_id)
                .is_some_and(|b| b.0.contains_key(id))
        })
        .collect();
    if !stale_skills.is_empty() {
        for skill_id in stale_skills {
            super::skills::remove_player_skill(world, object_id, skill_id);
        }
        super::admin::refresh_skill_list(world, object_id);
        super::party::broadcast_user_info(world, object_id);
    }

    let item_ids: Vec<i32> = world.cursed_weapons.iter().map(|cw| cw.item_id).collect();
    let mut removed = false;
    for item_id in item_ids {
        let holds = world
            .objects
            .get_component::<Inventory>(&object_id)
            .is_some_and(|inv| inv.first_of_item(item_id).is_some());
        if !holds {
            continue;
        }
        crate::game_loop::items::destroy_item_by_id(world, object_id, item_id, 1);
        removed = true;
    }
    if !removed {
        return;
    }
    // Java's `destroyItem(…, sendMessage = true)` refreshes the client's bag;
    // the weight/adena footers ride along (see `helpers::send_inventory_update`).
    let max_load = crate::game_loop::weight::max_load(world, object_id);
    if let Some(inv) = world.objects.get_component::<Inventory>(&object_id) {
        let list = crate::network::enter_world::item_list(inv, &world.data, false);
        let adena = crate::network::enter_world::ex_adena_inven_count(inv);
        let weight = crate::network::enter_world::ex_user_info_inven_weight(
            object_id,
            inv,
            &world.data,
            max_load,
        );
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(list);
            cs.send(adena);
            cs.send(weight);
        }
    }
    // A stray weapon can still be *worn* — and `EnterWorld` already sent its
    // `ExUserInfoEquipSlot` before this sweep runs, so the client would render
    // the sword the sweep just deleted until the next equip change.
    super::items::refresh_equip_state(world, client_id, object_id);
    super::party::broadcast_user_info(world, object_id);
}

/// Java `isCursedWeaponEquipped()` — the curse bars trading, augmenting and
/// anything else that would let the wielder launder the weapon or its profits.
pub(crate) fn is_cursed(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.cursed_weapon_equipped_id != 0)
}

/// `CursedWeaponsManager.isCursed(itemId)` — is this item id a cursed weapon
/// at all (live or not)? Java gates item destruction on it.
pub(crate) fn is_cursed_item(world: &World, item_id: i32) -> bool {
    world.cursed_weapons.iter().any(|cw| cw.item_id == item_id)
}

/// `CursedWeaponsManager.saveData()` — persist every weapon that is in the
/// world. Java's shutdown hook; the per-weapon row is otherwise only written
/// when it changes hands or scores a kill.
pub(crate) fn save_all(world: &World) {
    for idx in 0..world.cursed_weapons.len() {
        if world.cursed_weapons[idx].is_active() {
            super::admin::cursed_weapons::save_data(world, idx);
        }
    }
}

/// Arm the expiry timer at the weapon's current `end_time` (the wielder's
/// duration, or an un-grabbed drop's deadline). A later re-arm (a drop that's
/// then picked up) supersedes the earlier one via the `end_time` guard in
/// [`handle_expiry`].
pub(crate) fn arm_expiry(world: &mut World, idx: usize) {
    let (item_id, end_time) = {
        let cw = &world.cursed_weapons[idx];
        (cw.item_id, cw.end_time)
    };
    let delay_ticks = ((end_time - now_millis()).max(0) / 1000) as u64 * TICKS_PER_SECOND;
    world.scheduler.schedule(
        world.tick + delay_ticks,
        ScheduledTask::CursedWeaponExpiry { item_id },
    );
}

/// `CursedWeapon.RemoveTask.run`: the expiry timer fired — end-of-life the
/// weapon if its `end_time` really has passed. A stale timer (a drop that was
/// picked up and re-armed, or an already-gone weapon) no-ops. A dropped weapon
/// vanishes from the ground; an activated one is stripped from its wielder.
pub(crate) fn handle_expiry(world: &mut World, item_id: i32) {
    let Some(idx) = idx_by_item(world, item_id) else {
        return;
    };
    let (active, dropped, end_time, ground_oid) = {
        let cw = &world.cursed_weapons[idx];
        (
            cw.is_active(),
            cw.is_dropped,
            cw.end_time,
            cw.dropped_item_oid,
        )
    };
    if !active || now_millis() < end_time {
        return; // already gone, or a superseded (re-armed) timer
    }
    if dropped {
        // Despawn the un-grabbed ground item; `end_of_life` then announces +
        // clears the DB row + resets state (its non-activated branch).
        if let Some(region) = region_cell_of(world, ground_oid) {
            despawn_ground_item(world, ground_oid, region);
        }
    }
    end_of_life(world, idx);
}

// ---------------------------------------------------------------------------
// The client's cursed-weapon window (`RequestCursedWeaponList` /
// `RequestCursedWeaponLocation`, ex 0x2A / 0x2B — row 10)
// ---------------------------------------------------------------------------

/// `RequestCursedWeaponList` → `ExCursedWeaponList`: every cursed-weapon item
/// id the server knows, live or not (Java sends `getCursedWeaponsIds()`).
pub(crate) fn handle_request_list(world: &World, client_id: u32) {
    let ids: Vec<i32> = world.cursed_weapons.iter().map(|cw| cw.item_id).collect();
    send_to_client(
        world,
        client_id,
        crate::network::server_packets::ex_cursed_weapon_list(&ids),
    );
}

/// `RequestCursedWeaponLocation` → `ExCursedWeaponLocation`: where each *live*
/// weapon is — the wielder's position when it is being carried, the ground
/// item's when it has been dropped. Java skips inactive ones and **sends
/// nothing at all** when none are live; kept.
pub(crate) fn handle_request_location(world: &World, client_id: u32) {
    let entries: Vec<(i32, i32, i32, i32, i32)> = world
        .cursed_weapons
        .iter()
        // Java's explicit `if (!cw.isActive()) continue`. Mirrored for clarity;
        // the position lookup below already excludes a retired weapon, whose
        // holder ids are cleared when it leaves the world.
        .filter(|cw| cw.is_active())
        .filter_map(|cw| {
            // Java `CursedWeapon.getWorldPosition()`: the player's position
            // while wielded, the dropped item's while on the ground.
            let holder = if cw.is_activated {
                cw.player_id
            } else {
                cw.dropped_item_oid
            };
            let pos = world.objects.get_component::<Position>(&holder)?;
            Some((cw.item_id, i32::from(cw.is_activated), pos.x, pos.y, pos.z))
        })
        .collect();
    if entries.is_empty() {
        return;
    }
    send_to_client(
        world,
        client_id,
        crate::network::server_packets::ex_cursed_weapon_location(&entries),
    );
}
