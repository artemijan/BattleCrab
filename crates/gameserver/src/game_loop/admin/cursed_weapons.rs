//! `AdminCursedWeapons` — the Game panel's "Cursed Weapons" buttons plus the
//! `//cw_*` bar commands. Drives the [`crate::model::cursed_weapon`] slice: the
//! read-only status views (`//cw_info` / `//cw_info_menu`), the config reload
//! (`//cw_reload`), the GM teleport (`//cw_goto`), and the give/remove pair
//! (`//cw_add` / `//cw_remove`) with the activate / end-of-life lifecycle.
//!
//! The autonomous half of the system — drop-from-monster, pickup, and the
//! expiry `RemoveTask` — now lives in [`crate::game_loop::cursed_weapon`] (G28),
//! which calls back into `activate` / `end_of_life` here, and owns the login
//! restore (`on_enter_world`). Still deferred (TODO(G28)): drop-on-PK-death,
//! the "hungry" HP-drain / decay task, and the "already wields another cursed
//! weapon" stage-kill bonus branch of `activate`.

use crate::db::DbCommand;
use crate::model::Player;
use crate::model::components::Position;
use crate::model::inventory::Inventory;
use crate::network::server_packets::{self, SmParam, sm_ids};
use crate::session::ClientSession;
use crate::world::World;

use super::{current_target, send_message};

/// Wall-clock millis (Java `System.currentTimeMillis()`).
pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `CursedWeapon.saveData` — upsert this weapon's wielder row. Java calls it
/// from `activate`, `increaseKills` and `CursedWeaponsManager.saveData` (the
/// shutdown hook), so it is shared rather than inlined at the one site.
pub(crate) fn save_data(world: &World, idx: usize) {
    let cw = &world.cursed_weapons[idx];
    let _ = world.db.send(DbCommand::StoreCursedWeapon {
        item_id: cw.item_id,
        char_id: cw.player_id,
        reputation: cw.player_reputation,
        pk_kills: cw.player_pk_kills,
        nb_kills: cw.nb_kills,
        end_time: cw.end_time,
    });
}

/// Resolve the `<itemid|name>` argument to a cursed-weapon index in
/// `world.cursed_weapons` (Java: digit → item id, else case-insensitive name
/// substring). `None` when unmatched.
/// The index of the cursed weapon with `item_id`, if this dist has it.
pub(crate) fn idx_by_item(world: &World, item_id: i32) -> Option<usize> {
    world
        .cursed_weapons
        .iter()
        .position(|cw| cw.item_id == item_id)
}

fn resolve(world: &World, arg: &str) -> Option<usize> {
    if arg.chars().all(|c| c.is_ascii_digit()) {
        let id: i32 = arg.parse().ok()?;
        world.cursed_weapons.iter().position(|cw| cw.item_id == id)
    } else {
        let needle = arg.replace('_', " ").to_ascii_lowercase();
        world
            .cursed_weapons
            .iter()
            .position(|cw| cw.name.to_ascii_lowercase().contains(&needle))
    }
}

/// `//cw_info` — the plain-text status dump (Java `AdminCursedWeapons`,
/// non-menu branch).
pub(super) fn admin_cw_info(world: &mut World, client_id: u32) {
    send_message(world, client_id, "====== Cursed Weapons: ======");
    let now = now_millis();
    let lines: Vec<String> = {
        let mut out = Vec::new();
        for cw in &world.cursed_weapons {
            out.push(format!("> {} ({})", cw.name, cw.item_id));
            if cw.is_activated {
                let holder = world
                    .objects
                    .get_component::<Player>(&cw.player_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "null".to_string());
                out.push(format!("  Player holding: {holder}"));
                out.push(format!("    Player Reputation: {}", cw.player_reputation));
                out.push(format!(
                    "    Time Remaining: {} min.",
                    cw.time_left(now) / 60000
                ));
                out.push(format!("    Kills : {}", cw.nb_kills));
            } else if cw.is_dropped {
                out.push("  Lying on the ground.".to_string());
                out.push(format!(
                    "    Time Remaining: {} min.",
                    cw.time_left(now) / 60000
                ));
                out.push(format!("    Kills : {}", cw.nb_kills));
            } else {
                out.push("  Don't exist in the world.".to_string());
            }
            out.push(String::new()); // marks the EMPTY_3 divider slot
        }
        out
    };
    for line in lines {
        if line.is_empty() {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::system_message_with(sm_ids::EMPTY_3, &[]));
            }
        } else {
            send_message(world, client_id, &line);
        }
    }
}

/// `//cw_info_menu` — the `cwinfo.htm` status/action panel (Java builds the
/// per-weapon rows in a `StringBuilder`).
pub(super) fn admin_cw_info_menu(world: &mut World, client_id: u32) {
    let now = now_millis();
    let mut body = String::new();
    for cw in &world.cursed_weapons {
        body.push_str(&format!(
            "<table width=270><tr><td>Name:</td><td>{}</td></tr>",
            cw.name
        ));
        if cw.is_activated {
            let holder = world
                .objects
                .get_component::<Player>(&cw.player_id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "null".to_string());
            body.push_str(&format!("<tr><td>Weilder:</td><td>{holder}</td></tr>"));
            body.push_str(&format!(
                "<tr><td>Karma:</td><td>{}</td></tr>",
                cw.player_reputation
            ));
            body.push_str(&format!(
                "<tr><td>Kills:</td><td>{}/{}</td></tr>",
                cw.player_pk_kills, cw.nb_kills
            ));
            body.push_str(&format!(
                "<tr><td>Time remaining:</td><td>{} min.</td></tr>",
                cw.time_left(now) / 60000
            ));
            body.push_str(&format!(
                "<tr><td><button value=\"Remove\" action=\"bypass -h admin_cw_remove {0}\" width=73 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td><button value=\"Go\" action=\"bypass -h admin_cw_goto {0}\" width=73 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr>",
                cw.item_id
            ));
        } else if cw.is_dropped {
            body.push_str("<tr><td>Position:</td><td>Lying on the ground</td></tr>");
            body.push_str(&format!(
                "<tr><td>Time remaining:</td><td>{} min.</td></tr>",
                cw.time_left(now) / 60000
            ));
            body.push_str(&format!("<tr><td>Kills:</td><td>{}</td></tr>", cw.nb_kills));
            body.push_str(&format!(
                "<tr><td><button value=\"Remove\" action=\"bypass -h admin_cw_remove {0}\" width=73 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td><button value=\"Go\" action=\"bypass -h admin_cw_goto {0}\" width=73 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr>",
                cw.item_id
            ));
        } else {
            body.push_str(&format!(
                "<tr><td>Position:</td><td>Doesn't exist.</td></tr><tr><td><button value=\"Give to Target\" action=\"bypass -h admin_cw_add {}\" width=130 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td></td></tr>",
                cw.item_id
            ));
        }
        body.push_str("</table><br>");
    }
    super::menu::show_admin_html_replace(world, client_id, "cwinfo.htm", &[("cwinfo", body)]);
}

/// `//cw_reload` — re-read `CursedWeapons.xml` config (Java `cwm.load()`),
/// refreshing the config fields on the live weapons and preserving runtime
/// state. (Java also re-restores state from the DB; that async reload is
/// deferred — the boot restore already loaded it.)
pub(super) fn admin_cw_reload(world: &mut World) {
    let fresh = crate::data::CursedWeaponData::load_from(&world.data.root);
    for cfg in fresh.weapons {
        if let Some(cw) = world
            .cursed_weapons
            .iter_mut()
            .find(|c| c.item_id == cfg.item_id)
        {
            cw.name = cfg.name;
            cw.skill_id = cfg.skill_id;
            cw.disappear_chance = cfg.disappear_chance;
            cw.drop_rate = cfg.drop_rate;
            cw.duration = cfg.duration;
            cw.duration_lost = cfg.duration_lost;
            cw.stage_kills = cfg.stage_kills;
        }
    }
    world.data.cursed_weapons = crate::data::CursedWeaponData::load_from(&world.data.root);
}

/// `//cw_goto <id|name>` — teleport the GM to the weapon (Java `cw.goTo`).
/// Only an activated weapon (its wielder) is reachable here; teleporting to a
/// ground drop's position is a TODO(G28) nicety.
pub(super) fn admin_cw_goto(world: &mut World, client_id: u32, gm_object_id: i32, args: &[&str]) {
    let Some(idx) = args.first().and_then(|a| resolve(world, a)) else {
        send_message(
            world,
            client_id,
            "Usage: //cw_remove|//cw_goto|//cw_add <itemid|name>",
        );
        return;
    };
    let cw = &world.cursed_weapons[idx];
    if cw.is_activated {
        let holder = cw.player_id;
        if let Some(pos) = world.objects.get_component::<Position>(&holder).copied() {
            crate::game_loop::death::teleport_player(world, gm_object_id, pos.x, pos.y, pos.z);
            return;
        }
    }
    let name = cw.name.clone();
    send_message(world, client_id, &format!("{name} isn't in the World."));
}

/// `//cw_remove <id|name>` — Java `cw.endOfLife()`, narrowed to the online
/// wielder / not-in-world cases the admin path reaches.
pub(super) fn admin_cw_remove(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(idx) = args.first().and_then(|a| resolve(world, a)) else {
        send_message(
            world,
            client_id,
            "Usage: //cw_remove|//cw_goto|//cw_add <itemid|name>",
        );
        return;
    };
    end_of_life(world, idx);
}

/// `//cw_add <id|name>` — give the weapon to the GM's target (or the GM) and
/// activate it (Java: `addItem` → `CursedWeaponsManager.activate` +
/// `setEndTime` + `reActivate`).
pub(crate) fn admin_cw_add(world: &mut World, client_id: u32, gm_object_id: i32, args: &[&str]) {
    let Some(idx) = args.first().and_then(|a| resolve(world, a)) else {
        send_message(
            world,
            client_id,
            "Usage: //cw_remove|//cw_goto|//cw_add <itemid|name>",
        );
        return;
    };
    if world.cursed_weapons[idx].is_active() {
        send_message(world, client_id, "This cursed weapon is already active.");
        return;
    }
    // Target the selected player, else the GM (Java falls back to activeChar).
    let target = current_target(world, gm_object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(gm_object_id);
    // TODO(G21): Java's `activate` gives a stage bonus to an existing cursed
    // weapon and end-of-lifes the new one when the target already wields one;
    // here we only handle the common (unowned) case.
    if world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.cursed_weapon_equipped_id != 0)
    {
        send_message(world, client_id, "Target already wields a cursed weapon.");
        return;
    }
    activate(world, idx, target);
    // Java `//cw_add`: `cw.setEndTime(now + duration*60000); cw.reActivate();`
    // — a GM-granted weapon starts a full-length life of its own.
    let duration = world.cursed_weapons[idx].duration as i64;
    world.cursed_weapons[idx].end_time = now_millis() + duration * 60_000;
    save_data(world, idx);
    super::super::cursed_weapon::arm_expiry(world, idx);
}

/// Port of `CursedWeapon.activate` (via `addItem`) + the admin `setEndTime`/
/// `reActivate` tail. `target` holds no cursed weapon (checked by the caller).
pub(crate) fn activate(world: &mut World, idx: usize, target: i32) {
    let (item_id, duration) = {
        let cw = &world.cursed_weapons[idx];
        (cw.item_id, cw.duration)
    };
    let target_client = super::helpers::client_for_player(world, target);

    // Save the wielder's current reputation/pk-kills (restored on end-of-life).
    let (saved_rep, saved_pk) = {
        let Some(p) = world.objects.get_component::<Player>(&target) else {
            return;
        };
        (p.reputation, p.pk_kills)
    };

    // addItem — give the weapon.
    let Some(item_oids) = crate::game_loop::items::add_inventory_item(world, target, item_id, 1)
    else {
        return;
    };
    let item_oid = item_oids[0];

    // Change wielder stats (Java: reputation = -9999999, pkKills = 0).
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.reputation = -9_999_999;
        p.pk_kills = 0;
        p.cursed_weapon_equipped_id = item_id;
    }

    // Java `activate`: `if (_player.isInParty()) getParty().removePartyMember(
    // _player, PartyMessageType.EXPELLED)` — the curse is a solo affair, so the
    // new wielder is thrown out of their group the moment they pick it up.
    if let Some(crate::model::components::PartyRef(party_id)) = world
        .objects
        .get_component::<crate::model::components::PartyRef>(&target)
        .copied()
    {
        crate::game_loop::party::remove_party_member(
            world,
            party_id,
            target,
            crate::game_loop::party::LeaveType::Expelled,
        );
    }

    // doTransform + giveSkill — shared with the login restore
    // (`cursed_weapon::on_enter_world`), which runs the same two Java calls.
    // `nb_kills` is still whatever the caller left on the weapon (0 on a fresh
    // grant), so `give_skill` picks Java's level for it either way.
    crate::game_loop::cursed_weapon::do_transform(world, target, item_id);
    crate::game_loop::cursed_weapon::give_skill(world, idx, target);

    // Equip the weapon (recalc + broadcast) — the freshly-added item is
    // unequipped, so `use_equipable_item` equips it.
    if let Some(tc) = target_client {
        crate::game_loop::items::use_equipable_item(world, tc, target, item_oid);
        // "You have equipped your $s1."
        if let Some(cs) = world.clients.get(&tc) {
            cs.send(server_packets::system_message_with(
                sm_ids::YOU_HAVE_EQUIPPED_YOUR_S1,
                &[SmParam::ItemName(item_id)],
            ));
        }
    }

    // Fully heal (Java `setCurrentHpMp/Cp(max)`), refresh UI.
    super::vitals::heal_creature(world, target);

    // SocialAction(17) — the levelup-style pose Java broadcasts on activation.
    let social = server_packets::social_action(target, 17);
    broadcast_to_visible(world, target, &social);

    // Java `activate` deliberately does **not** touch `_endTime`: the clock was
    // started by whoever put the weapon into the world. A mob drop set it in
    // `checkDrop` (now + full duration), so a player who picks the sword up
    // inherits however much of that life is left rather than resetting it —
    // leaving a weapon on the ground does not buy the next owner a fresh 300
    // minutes. `//cw_add` is the one caller that sets it, right after this.
    {
        let cw = &mut world.cursed_weapons[idx];
        cw.is_activated = true;
        cw.is_dropped = false;
        cw.player_id = target;
        cw.player_reputation = saved_rep;
        cw.player_pk_kills = saved_pk;
        cw.nb_kills = 0;
    }
    save_data(world, idx);

    // announce THE_OWNER_OF_S2_HAS_APPEARED_IN_THE_S1_REGION to everyone.
    // TODO(G21): the S1 region SysString (`addZoneName`) isn't modelled — the
    // region name renders blank until MapRegion carries its sysstring id.
    let announce = server_packets::system_message_with(
        sm_ids::THE_OWNER_OF_S2_HAS_APPEARED_IN_THE_S1_REGION,
        &[SmParam::SysString(0), SmParam::ItemName(item_id)],
    );
    broadcast_to_all(world, &announce);
    // Arming the `RemoveTask` is the caller's job (Java `reActivate`), since it
    // must happen *after* `//cw_add` has set the end time — arming here would
    // schedule against a stale (or zero) deadline and fire immediately.
    let _ = duration;
}

/// Port of `CursedWeapon.endOfLife` for an activated (online) or not-in-world
/// weapon: restore the wielder, strip the weapon, announce, clear the DB row,
/// and reset the state.
pub(crate) fn end_of_life(world: &mut World, idx: usize) {
    let (item_id, name, is_activated, player_id, saved_rep, saved_pk) = {
        let cw = &world.cursed_weapons[idx];
        (
            cw.item_id,
            cw.name.clone(),
            cw.is_activated,
            cw.player_id,
            cw.player_reputation,
            cw.player_pk_kills,
        )
    };

    if is_activated && world.objects.has_component::<Player>(&player_id) {
        let target_client = super::helpers::client_for_player(world, player_id);
        // Restore reputation/pk-kills, clear the cursed-weapon flag.
        if let Some(p) = world.objects.get_component_mut::<Player>(&player_id) {
            p.reputation = saved_rep;
            p.pk_kills = saved_pk;
            p.cursed_weapon_equipped_id = 0;
        }
        // removeSkill: drop the cursed-weapon skill + untransform. The skill is
        // a passive, so Java's `removeSkill` unmerges its stat pumps through
        // `EffectList.stopSkillEffects` — take the book entry alone and the
        // freed player keeps the curse's `MaxCp`/`PAtk`/defence bonuses (the
        // reported "still MAX CP 3844 after losing Akamanah").
        let skill_id = world.cursed_weapons[idx].skill_id;
        crate::game_loop::skills::remove_player_skill(world, player_id, skill_id);
        super::transforms::remove_transform(world, player_id);
        super::skills::refresh_skill_list(world, player_id);

        // Unequip (if worn) + destroy the weapon item, refresh the inventory.
        if let Some(inv) = world.objects.get_component_mut::<Inventory>(&player_id) {
            if let Some(item_oid) = inv
                .items()
                .iter()
                .find(|i| i.item_id == item_id)
                .map(|i| i.object_id)
                && inv.paperdoll_slot_of(item_oid).is_some()
            {
                inv.unequip_item(item_oid);
            }
            inv.remove_item(item_id, 1);
        }
        if let Some(tc) = target_client {
            refresh_inventory(world, tc, player_id);
            // The bag list is only half of `unEquipItemInBodySlot` +
            // `destroyItemByItemId`: the client's own paperdoll rides
            // `ExUserInfoEquipSlot`, so without this the freed player keeps
            // rendering the weapon that was just taken away.
            crate::game_loop::items::refresh_equip_state(world, tc, player_id);
        }
        crate::game_loop::party::broadcast_user_info(world, player_id);
    } else if is_activated {
        // Java's offline branch of `endOfLife`: the wielder isn't logged in, so
        // the restore happens straight in the database — otherwise they come
        // back still holding the sword with reputation pinned at -9999999,
        // which nothing later would ever undo. Reachable since the expiry task
        // is armed at boot, so a curse really can run out while its owner is
        // away.
        //
        // The skill list is a Rust-only addition to Java's two statements:
        // Java grants the cursed and transform skills with `addSkill(…, false)`
        // / `addTransformSkill`, which never touch the DB, whereas here the
        // `SkillBook` is persisted wholesale — so they have to be deleted or
        // the freed character keeps the weapon's skill and Void Burst forever.
        let skill_ids = curse_granted_skill_ids(world, idx, item_id);
        let _ = world.db.send(DbCommand::RestoreOfflineCursedOwner {
            char_id: player_id,
            item_id,
            reputation: saved_rep,
            pk_kills: saved_pk,
            skill_ids,
        });
    }

    // Drop the DB row + announce the disappearance to everyone.
    let _ = world.db.send(DbCommand::RemoveCursedWeapon { item_id });
    let announce = server_packets::system_message_with(
        sm_ids::S1_HAS_DISAPPEARED,
        &[SmParam::ItemName(item_id)],
    );
    broadcast_to_all(world, &announce);
    let _ = name;

    world.cursed_weapons[idx].reset();
}

/// Every skill wearing the curse hands out: the weapon's own skill plus the
/// 301/302 transform template's (Void Burst / Void Flow and the demon attacks),
/// both genders since only the wielder's is granted but either may be stored.
fn curse_granted_skill_ids(world: &World, idx: usize, item_id: i32) -> Vec<i32> {
    let mut ids = vec![world.cursed_weapons[idx].skill_id];
    let transform_id = if item_id == 8689 { 302 } else { 301 };
    if let Some(tf) = world.data.transforms.get(transform_id) {
        for female in [false, true] {
            ids.extend(tf.template(female).skills.iter().map(|(id, _)| *id));
        }
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Send the target's item list + adena counter (Java `sendItemList(false)`).
fn refresh_inventory(world: &World, client_id: u32, target: i32) {
    if let Some(inv) = world.objects.get_component::<Inventory>(&target) {
        let list = crate::network::enter_world::item_list(inv, &world.data, false);
        let adena = crate::network::enter_world::ex_adena_inven_count(inv);
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(list);
            cs.send(adena);
        }
    }
}

/// Send `pkt` to the subject's own client plus every player in visibility range
/// (Java `broadcastPacket`).
fn broadcast_to_visible(world: &World, subject: i32, pkt: &[u8]) {
    if let Some(cid) = super::helpers::client_for_player(world, subject)
        && let Some(cs) = world.clients.get(&cid)
    {
        cs.send(pkt.to_vec());
    }
    for oid in super::creatures_in_range(world, subject, 1400, true, false) {
        if let Some(cid) = super::helpers::client_for_player(world, oid)
            && let Some(cs) = world.clients.get(&cid)
        {
            cs.send(pkt.to_vec());
        }
    }
}

/// Broadcast `pkt` to every online player (Java `CursedWeaponsManager.announce`).
fn broadcast_to_all(world: &World, pkt: &[u8]) {
    for cs in world.clients.values() {
        if let ClientSession::InGame(_) = cs {
            cs.send(pkt.to_vec());
        }
    }
}
