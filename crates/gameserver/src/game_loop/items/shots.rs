//! Soulshot/spiritshot/fishshot charging, the auto-shot toggle and the
//! shot visual broadcast.

use crate::data::item_data::ItemHandler;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::send_to_client;
use crate::model::inventory::Inventory;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::world::World;
/// Java `Player.addAutoSoulShot(itemId)` — arm auto-use for one shot item.
///
/// The list is a set: Java's `_activeSoulShots` is a `Set<Integer>`, and the
/// toggle packet can arrive twice for the same item (the client re-sends on a
/// re-login burst), so a second arm must not stack a duplicate that
/// `recharge_shots` would then charge twice.
pub(crate) fn add_auto_shot(world: &mut World, object_id: i32, item_id: i32) {
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&object_id)
        && !p.auto_shots.contains(&item_id)
    {
        p.auto_shots.push(item_id);
    }
}

/// Java `Player.removeAutoSoulShot(itemId)` — disarm auto-use for one shot
/// item. `true` when it *was* armed.
///
/// Most callers drop the answer: running out of shots, or the item leaving the
/// inventory, disarms silently. The summon path
/// ([`servitor::shots`](crate::game_loop::servitor::shots)) needs it, because
/// it only echoes `ExAutoSoulShot` + the deactivation message when the toggle
/// actually went dark.
pub(crate) fn remove_auto_shot(world: &mut World, object_id: i32, item_id: i32) -> bool {
    world
        .objects
        .get_component_mut::<crate::model::Player>(&object_id)
        .is_some_and(|p| {
            let before = p.auto_shots.len();
            p.auto_shots.retain(|&id| id != item_id);
            p.auto_shots.len() != before
        })
}

/// The items armed for auto-use, cloned — every caller iterates the list while
/// mutating the world (charging shots, dropping entries), which a borrow of the
/// component cannot survive.
pub(crate) fn auto_shots(world: &World, object_id: i32) -> Vec<i32> {
    world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map(|p| p.auto_shots.clone())
        .unwrap_or_default()
}

/// Port of `handlers/itemhandlers/{SoulShots,SpiritShot,BlessedSpiritShot}.useItem`:
/// charge the matching shot on the equipped weapon. `auto` = true is the
/// `rechargeShots` re-entry (an item toggled for auto-use): it suppresses the
/// enable/error chat and the not-enough message, exactly like Java gating those
/// on `!getAutoSoulShot().contains(itemId)`. Returns whether a shot was charged.
///
/// Narrowing vs. Java: the `reducedSoulshot`/`reducedSoulshotChance` weapon
/// perk (a chance to spend fewer shots) isn't modelled — no Interlude weapon in
/// the dist declares it — and the ruby/sapphire brooch visual swap doesn't
/// exist (no jewels), so the shot's own `<skills>` visual always plays.
pub(crate) fn charge_shot(
    world: &mut World,
    object_id: i32,
    shot_item_id: i32,
    handler: ItemHandler,
    auto: bool,
) -> bool {
    use crate::model::{Player, ShotType};

    let physical = handler.is_soulshot();
    let shot_type = match handler {
        ItemHandler::SoulShots => ShotType::Soulshots,
        ItemHandler::SpiritShot => ShotType::Spiritshots,
        ItemHandler::BlessedSpiritShot => ShotType::BlessedSpiritshots,
        _ => return false,
    };
    let client_id = crate::game_loop::helpers::client_for_player(world, object_id);
    let send = |world: &World, msg: i16| {
        if !auto && let Some(cid) = client_id {
            crate::game_loop::helpers::send_sm_bare_to_client(world, cid, msg);
        }
    };

    // Equipped weapon + its per-charge shot count / grade.
    let (weapon_item_id, shot_visual) = {
        let Some(inv) = world.objects.get_component::<Inventory>(&object_id) else {
            return false;
        };
        let weapon = inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand);
        let visual = world
            .data
            .item_data
            .get(shot_item_id)
            .map(|t| t.item_skills.clone())
            .unwrap_or_default();
        (weapon, visual)
    };
    let shot_count = if physical {
        world.data.item_data.soulshot_count(weapon_item_id)
    } else {
        world.data.item_data.spiritshot_count(weapon_item_id)
    };

    // No weapon, or a weapon that can't take this shot kind.
    if weapon_item_id == 0 || shot_count == 0 {
        send(
            world,
            if physical {
                sm_ids::CANNOT_USE_SOULSHOTS
            } else {
                sm_ids::YOU_MAY_NOT_USE_SPIRITSHOTS
            },
        );
        return false;
    }

    // Grade must match (`getCrystalTypePlus`).
    let weapon_grade = world
        .data
        .item_data
        .get(weapon_item_id)
        .map(|t| t.crystal_type.plus());
    let shot_grade = world
        .data
        .item_data
        .get(shot_item_id)
        .map(|t| t.crystal_type.plus());
    if weapon_grade != shot_grade {
        send(
            world,
            if physical {
                sm_ids::THE_SOULSHOT_YOU_ARE_ATTEMPTING_TO_USE_DOES_NOT_MATCH_THE_GRADE_OF_YOUR_EQUIPPED_WEAPON
            } else {
                sm_ids::YOUR_SPIRITSHOT_DOES_NOT_MATCH_THE_WEAPON_S_GRADE
            },
        );
        return false;
    }

    // Already charged → no-op (also how the auto path avoids re-spending).
    if world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.is_charged_shot(shot_type))
    {
        return false;
    }

    // Consume the shots; not enough → drop auto-use for this item.
    let have = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(|inv| inv.count_of(shot_item_id))
        .unwrap_or(0);
    if have < shot_count as i64 {
        remove_auto_shot(world, object_id, shot_item_id);
        send(
            world,
            if physical {
                sm_ids::YOU_DO_NOT_HAVE_ENOUGH_SOULSHOTS_FOR_THAT
            } else {
                sm_ids::YOU_DO_NOT_HAVE_ENOUGH_SPIRITSHOT_FOR_THAT
            },
        );
        return false;
    }
    let changes = world
        .objects
        .get_component_mut::<Inventory>(&object_id)
        .map(|inv| inv.remove_item(shot_item_id, shot_count as i64))
        .unwrap_or_default();

    // Charge, notify, replay the count change, play the visual.
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.charge_shot(shot_type);
    }
    if !changes.is_empty() {
        crate::game_loop::helpers::send_inventory_update(world, object_id, changes);
    }
    send(
        world,
        if physical {
            sm_ids::YOUR_SOULSHOTS_ARE_ENABLED
        } else {
            sm_ids::YOUR_SPIRITSHOT_HAS_BEEN_ENABLED
        },
    );
    broadcast_shot_visual(world, object_id, &shot_visual);
    true
}

/// Port of `clientpackets/RequestAutoSoulShot.runImpl` (player-shot branch —
/// summon shots aren't in scope): toggle a shot item into the auto-use set.
/// Body: `itemId:i32, enable:i32(1/0), type:i32`.
pub(crate) fn handle_request_auto_soul_shot(world: &mut World, client_id: u32, ex_body: &[u8]) {
    if ex_body.len() < 12 {
        return;
    }
    let item_id = i32::from_le_bytes(ex_body[0..4].try_into().unwrap());
    let enable = i32::from_le_bytes(ex_body[4..8].try_into().unwrap()) == 1;
    let shot_type = i32::from_le_bytes(ex_body[8..12].try_into().unwrap());

    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    // `!player.isDead()` — a dead player can't toggle shots.
    if is_dead(world, object_id) {
        return;
    }
    // The item must be in the inventory, and be a player shot we handle.
    let handler = {
        let Some(inv) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        if inv.count_of(item_id) == 0 {
            return;
        }
        world
            .data
            .item_data
            .get(item_id)
            .map(|t| t.handler)
            .unwrap_or_default()
    };
    if !handler.is_soulshot() && !handler.is_spiritshot() && !handler.is_fishshot() {
        return;
    }

    let send = |world: &World, msg: i16, params: &[SmParam]| {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(msg, params),
        );
    };

    // A **summon** shot takes Java's `isSummonShot` branch, which checks that
    // the player *has* a summon and never looks at their weapon — the shots
    // are for the pet's swing, not the owner's.
    let is_summon_shot = matches!(
        handler,
        ItemHandler::BeastSoulShot | ItemHandler::BeastSpiritShot
    );
    if enable && is_summon_shot {
        if crate::game_loop::servitor::pet_of(world, object_id).is_none()
            && crate::game_loop::servitor::servitor_of(world, object_id).is_none()
        {
            send(world, sm_ids::YOU_DO_NOT_HAVE_A_SERVITOR_FOR_AUTO_USE, &[]);
            return;
        }
        add_auto_shot(world, object_id, item_id);
        send(
            world,
            sm_ids::THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_ACTIVATED,
            &[SmParam::ItemName(item_id)],
        );
        // Java charges the summon immediately on activation.
        if let Some(summon) = crate::game_loop::servitor::pet_of(world, object_id)
            .or_else(|| crate::game_loop::servitor::servitor_of(world, object_id))
        {
            crate::game_loop::servitor::recharge_shots(world, summon, true);
        }
        return;
    }

    if enable {
        // Grade check (`item.getCrystalType() != weapon.getCrystalTypePlus()`,
        // or no weapon at all — fists).
        let weapon_item_id = world
            .objects
            .get_component::<Inventory>(&object_id)
            .map(|inv| inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand))
            .unwrap_or(0);
        let weapon_grade = world
            .data
            .item_data
            .get(weapon_item_id)
            .map(|t| t.crystal_type.plus());
        let shot_grade = world.data.item_data.get(item_id).map(|t| t.crystal_type);
        if weapon_item_id == 0 || weapon_grade != shot_grade {
            send(
                world,
                if handler.is_soulshot() {
                    sm_ids::THE_SOULSHOT_YOU_ARE_ATTEMPTING_TO_USE_DOES_NOT_MATCH_THE_GRADE_OF_YOUR_EQUIPPED_WEAPON
                } else {
                    sm_ids::YOUR_SPIRITSHOT_DOES_NOT_MATCH_THE_WEAPON_S_GRADE
                },
                &[],
            );
            return;
        }
        // Activate.
        add_auto_shot(world, object_id, item_id);
        send_to_client(
            world,
            client_id,
            server_packets::ex_auto_soul_shot(item_id, true, shot_type),
        );
        send(
            world,
            sm_ids::THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_ACTIVATED,
            &[SmParam::ItemName(item_id)],
        );
        // Charge immediately (Java `player.rechargeShots(...)`).
        recharge_shots(
            world,
            object_id,
            handler.is_soulshot(),
            handler.is_spiritshot(),
            handler.is_fishshot(),
        );
    } else {
        // Deactivate.
        remove_auto_shot(world, object_id, item_id);
        send_to_client(
            world,
            client_id,
            server_packets::ex_auto_soul_shot(item_id, false, shot_type),
        );
        send(
            world,
            sm_ids::THE_AUTOMATIC_USE_OF_S1_HAS_BEEN_DEACTIVATED,
            &[SmParam::ItemName(item_id)],
        );
    }
}

/// Port of `Player.rechargeShots(physical, magic, fish)`: for each shot item
/// the player toggled for auto-use, if its category matches the requested one,
/// (re)charge it. Java runs this at the start of every attack (`physical`) and
/// cast (`magic`). A toggled item that's no longer in the inventory is dropped
/// from the auto set (Java's `removeAutoSoulShot` on `getItemByItemId == null`).
pub(crate) fn recharge_shots(
    world: &mut World,
    object_id: i32,
    physical: bool,
    magic: bool,
    fish: bool,
) {
    for item_id in auto_shots(world, object_id) {
        if world
            .objects
            .get_component::<Inventory>(&object_id)
            .map(|inv| inv.count_of(item_id))
            .unwrap_or(0)
            == 0
        {
            remove_auto_shot(world, object_id, item_id);
            continue;
        }
        let handler = world
            .data
            .item_data
            .get(item_id)
            .map(|t| t.handler)
            .unwrap_or_default();
        if (magic && handler.is_spiritshot()) || (physical && handler.is_soulshot()) {
            charge_shot(world, object_id, item_id, handler, true);
        } else if fish && handler.is_fishshot() {
            charge_fish_shot(world, object_id, item_id);
        }
    }
}

/// Java `FishShots` item handler: charge `FISH_SOULSHOTS` and spend one fishing
/// shot. Unlike weapon shots it has no grade/weapon check and always consumes
/// exactly one. Returns whether the flag flipped on.
pub(crate) fn charge_fish_shot(world: &mut World, object_id: i32, shot_item_id: i32) -> bool {
    use crate::model::{Player, ShotType};
    let already = world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.is_charged_shot(ShotType::FishSoulshots));
    if already {
        return false;
    }
    let have = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(|inv| inv.count_of(shot_item_id))
        .unwrap_or(0);
    if have < 1 {
        remove_auto_shot(world, object_id, shot_item_id);
        return false;
    }
    let changes = world
        .objects
        .get_component_mut::<Inventory>(&object_id)
        .map(|inv| inv.remove_item(shot_item_id, 1))
        .unwrap_or_default();
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.charge_shot(ShotType::FishSoulshots);
    }
    if !changes.is_empty() {
        crate::game_loop::helpers::send_inventory_update(world, object_id, changes);
    }
    true
}

/// `Broadcast.toSelfAndKnownPlayersInRadius(player, new MagicSkillUse(...))`:
/// the shot's `<skills>` (NORMAL) entries as a self-targeted, zero-time
/// `MagicSkillUse` — the client renders the charge glow off it.
fn broadcast_shot_visual(world: &mut World, object_id: i32, skills: &[(i32, i32)]) {
    let Some((player, pos)) = ({
        let p = world
            .objects
            .get_component::<crate::model::Player>(&object_id)
            .cloned();
        let pos = maybe_position(world, object_id);
        p.zip(pos)
    }) else {
        return;
    };
    for &(skill_id, skill_level) in skills {
        let pkt = server_packets::magic_skill_use(
            &player,
            &pos,
            (object_id, pos.x, pos.y, pos.z),
            skill_id,
            skill_level,
            0,
            0,
            0,
        );
        crate::game_loop::helpers::broadcast_including_self(world, object_id, &pkt);
    }
}
