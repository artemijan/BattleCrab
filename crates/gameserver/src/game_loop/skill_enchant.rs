//! Skill enchanting — slice 2 of PLAN_G19_SKILL_ENCHANT.md: the ex-packet
//! flow (`RequestExEnchantSkillInfo` 0x0E / `RequestExEnchantSkill` 0x0F /
//! `RequestExEnchantSkillInfoDetail` 0x43) and the enchant transaction
//! itself, over slice 1's pre-built sub-level variants and cost table.
//!
//! Java quirks ported as written (dist is spec):
//! - The roll is `Rnd.get(100) <= chance` — a 90% row succeeds 91 times in
//!   100.
//! - **Items are consumed before the SP check** (`RequestExEnchantSkill`
//!   consumes, then tests `getSp()`): a codex can be lost to insufficient SP.
//! - From +2 onward the item consume charges **adena in each holder's
//!   count** (`destroyItemByItemId(57, holder.getCount())` for every holder,
//!   codex included) — the codex itself is only ever consumed on the +1 step
//!   (`skill.getSubLevel() <= 1001` gates the has-items check too).
//! - `NORMAL` failure resets the route to `enchantFailLevel` (0 = unenchanted);
//!   `BLESSED` failure keeps the current step; `CHANGE` failure sets the raw
//!   `enchantFailLevel` as the sub-level.
//! - A sub-level change orphans a running cooldown (Java's reuse hash spans
//!   level and sub-level) — mirrored by dropping the reuse entry whenever the
//!   sub-level moves.
//!
//! `UNTRAIN` is refused exactly like Java: the `SkillEnchantType` value
//! exists there, but `RequestExEnchantSkill.runImpl`'s switch has no UNTRAIN
//! case — parity, not a narrowing. The one Java gate without a port
//! equivalent is the subclass lock (see [`busy_for_enchant`]).

use crate::game_loop::helpers::{send_sm_bare_to_client, send_sm_to_client, send_to_client};
use crate::model::Player;
use crate::model::components::{SkillBook, SkillEnchants};
use crate::network::server_packets;
use crate::world::World;
use commons::network::PacketReader;

use super::helpers::send_sm_and_action_failed;

/// `SkillEnchantType` ordinals (Java enum order).
const NORMAL: i32 = 0;
const BLESSED: i32 = 1;
const CHANGE: i32 = 3;
const IMMORTAL: i32 = 4;

fn type_name(t: i32) -> Option<&'static str> {
    match t {
        NORMAL => Some("NORMAL"),
        BLESSED => Some("BLESSED"),
        CHANGE => Some("CHANGE"),
        IMMORTAL => Some("IMMORTAL"),
        _ => None, // UNTRAIN (2) and out-of-range: unhandled
    }
}

/// The gate every enchant packet shares: a 3rd-class player
/// (`CategoryType.FOURTH_CLASS_GROUP`). The busy-state refusals live only on
/// the transaction packet, as in Java — `RequestExEnchantSkillInfo` carries
/// nothing but this class check.
fn may_enchant(world: &World, object_id: i32) -> bool {
    let Some(p) = world.objects.get_component::<Player>(&object_id) else {
        return false;
    };
    world
        .data
        .categories
        .contains("FOURTH_CLASS_GROUP", p.class_id)
}

/// `RequestExEnchantSkill.runImpl`'s busy-state refusals, transaction-only:
/// `isAllowedToEnchantSkills()` (transformed / attack stance / casting /
/// aboard a boat), `isSellingBuffs()`, `isInOlympiadMode()` and
/// `getPrivateStoreType() != NONE`. Java's remaining
/// `isAllowedToEnchantSkills` leg, the subclass lock, has no port
/// equivalent: a subclass change completes within its own tick, so no
/// packet can arrive mid-change.
fn busy_for_enchant(world: &World, object_id: i32) -> bool {
    let transformed_or_selling = world
        .objects
        .get_component::<Player>(&object_id)
        .is_none_or(|p| p.transform_id != 0 || p.selling_buffs);
    transformed_or_selling
        || world
            .objects
            .has_component::<crate::model::components::Casting>(&object_id)
        || world
            .objects
            .has_component::<crate::model::boat::InVehicle>(&object_id)
        || world
            .objects
            .has_component::<crate::model::components::PrivateStore>(&object_id)
        || world
            .objects
            .has_component::<crate::model::components::PrivateBuyStore>(&object_id)
        || super::combat::has_attack_stance(world, object_id)
        || super::olympiad::in_match(world, object_id)
}

/// `RequestExEnchantSkillInfo` (ex 0x0E: `d skillId, h level, h sub`) — the
/// window asking which routes a known skill can take → `ExEnchantSkillInfo`.
pub(crate) fn handle_request_enchant_skill_info(world: &mut World, client_id: u32, ex_body: &[u8]) {
    let mut r = PacketReader::new(ex_body);
    let (Some(skill_id), Some(level), Some(sub)) = (r.read_i32(), r.read_i16(), r.read_i16())
    else {
        return;
    };
    let (level, sub) = (level as i32, sub as i32);
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    if skill_id <= 0 || level <= 0 || sub < 0 || !may_enchant(world, object_id) {
        return;
    }
    // The queried instance must exist and match the player's known skill.
    if world
        .data
        .skill_data
        .get_enchanted(skill_id, level, sub)
        .is_none()
    {
        return;
    }
    let routes = world.data.skill_data.enchant_routes(skill_id, level);
    if routes.is_empty() {
        return;
    }
    let known = known_skill(world, object_id, skill_id);
    if known != Some((level, sub)) {
        return;
    }
    let route_starts: Vec<i32> = routes.iter().map(|&(first, _)| first).collect();
    let max_enchant = world.data.enchant_skill_groups.len() as i32;
    let pkt = crate::network::enter_world::ex_enchant_skill_info(
        skill_id,
        level,
        sub,
        sub,
        &route_starts,
        max_enchant,
    );
    send_to_client(world, client_id, pkt);
}

/// The `d type, d skillId, h level, h sub` body both enchant requests carry,
/// with the two shorts widened to `i32` and the request sanity-checked: an
/// unknown enchant type, a non-positive skill or level, or a negative sub is a
/// malformed packet and yields `None`.
fn read_enchant_request(ex_body: &[u8]) -> Option<(i32, i32, i32, i32)> {
    let mut r = PacketReader::new(ex_body);
    let ty = r.read_i32()?;
    let skill_id = r.read_i32()?;
    let level = r.read_i16()?;
    let sub = r.read_i16()?;
    let (level, sub) = (level as i32, sub as i32);
    if skill_id <= 0 || level <= 0 || sub < 0 || type_name(ty).is_none() {
        return None;
    }
    Some((ty, skill_id, level, sub))
}

/// `RequestExEnchantSkillInfoDetail` (ex 0x43: `d type, d skillId, h level,
/// h sub`) — the cost preview for one step → `ExEnchantSkillInfoDetail`.
pub(crate) fn handle_request_enchant_skill_info_detail(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some((ty, skill_id, level, sub)) = read_enchant_request(ex_body) else {
        return;
    };
    let Some(name) = type_name(ty) else { return };
    let Some(cost) = world.data.enchant_skill_groups.cost_for(sub % 1000) else {
        return;
    };
    let sp = cost.sp.get(name).copied().unwrap_or(0);
    let chance = cost.chance.get(name).copied().unwrap_or(100);
    let items: Vec<(i32, i64)> = cost.items.get(name).cloned().unwrap_or_default();
    let pkt = crate::network::enter_world::ex_enchant_skill_info_detail(
        ty, skill_id, level, sub, sp, chance, &items,
    );
    send_to_client(world, client_id, pkt);
}

/// The player's known (level, sub) for a skill id.
fn known_skill(world: &World, object_id: i32, skill_id: i32) -> Option<(i32, i32)> {
    let level = world
        .objects
        .get_component::<SkillBook>(&object_id)?
        .0
        .get(&skill_id)
        .copied()?;
    let sub = world
        .objects
        .get_component::<SkillEnchants>(&object_id)
        .and_then(|e| e.0.get(&skill_id).copied())
        .unwrap_or(0);
    Some((level, sub))
}

/// `RequestExEnchantSkill` (ex 0x0F: `d type, d skillId, h level, h sub`) —
/// the transaction: validate the step, pay, roll, apply.
pub(crate) fn handle_request_enchant_skill(world: &mut World, client_id: u32, ex_body: &[u8]) {
    use server_packets::{SmParam, sm_ids};

    let Some((ty, skill_id, level, target_sub)) = read_enchant_request(ex_body) else {
        return;
    };
    let Some(object_id) = world.player_oid(client_id) else {
        return;
    };
    if !may_enchant(world, object_id) || busy_for_enchant(world, object_id) {
        return;
    }
    let Some((known_level, cur_sub)) = known_skill(world, object_id, skill_id) else {
        return;
    };
    if known_level != level
        || world
            .data
            .skill_data
            .enchant_routes(skill_id, level)
            .is_empty()
    {
        return;
    }
    // Step validation (`RequestExEnchantSkill.runImpl`): CHANGE must stay on
    // the same step of another route; everything else advances by exactly 1.
    if cur_sub > 0 {
        if ty == CHANGE {
            if (target_sub % 1000) != (cur_sub % 1000) {
                return;
            }
        } else if cur_sub + 1 != target_sub {
            return;
        }
    }
    // The target instance must exist in the data.
    if world
        .data
        .skill_data
        .get_enchanted(skill_id, level, target_sub)
        .is_none()
    {
        return;
    }
    let type_key = type_name(ty).expect("checked");
    let Some(cost) = world
        .data
        .enchant_skill_groups
        .cost_for(target_sub % 1000)
        .cloned()
    else {
        return;
    };
    let required: Vec<(i32, i64)> = cost.items.get(type_key).cloned().unwrap_or_default();

    // Item gate — Java only checks inventory on the first step (`getSubLevel()
    // <= 1001`); later steps go straight to the adena-flavored consume.
    if cur_sub <= 1001 {
        for &(item_id, count) in &required {
            let have = world
                .objects
                .get_component::<crate::model::inventory::Inventory>(&object_id)
                .map(|inv| inv.count_of(item_id))
                .unwrap_or(0);
            if have < count {
                send_sm_and_action_failed(
                    world,
                    client_id,
                    sm_ids::YOU_DO_NOT_HAVE_ALL_OF_THE_ITEMS_NEEDED_TO_ENCHANT_THAT_SKILL,
                    &[],
                );
                return;
            }
        }
    }
    // The consume. Java's `+2`-onward branch charges **adena** with each
    // holder's count instead of the holder's own item — ported as written.
    for &(item_id, count) in &required {
        let charged = if cur_sub + 1 >= 1002 { 57 } else { item_id };
        if !super::quests::take_items(world, client_id, object_id, charged, count) {
            return;
        }
    }
    // SP — checked *after* the items are gone, like Java.
    let sp_cost = cost.sp.get(type_key).copied().unwrap_or(0);
    let enough_sp = world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.sp >= sp_cost);
    if !enough_sp {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::YOU_DO_NOT_HAVE_ENOUGH_SP_TO_ENCHANT_THAT_SKILL,
            &[],
        );
        return;
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.sp -= sp_cost;
    }

    // The roll: `Rnd.get(100) <= chance` (absent chance rows — CHANGE,
    // IMMORTAL — never fail: 100).
    let chance = cost.chance.get(type_key).copied().unwrap_or(100);
    let success = world.roll(100) <= chance;

    let new_sub = if success {
        Some(target_sub)
    } else {
        match ty {
            // NORMAL failure: back to the route's fail step (0 = unenchanted).
            NORMAL => Some(if cur_sub > 0 && cost.enchant_fail_level > 0 {
                cur_sub - (cur_sub % 1000) + cost.enchant_fail_level
            } else {
                0
            }),
            // CHANGE failure: the raw fail level (Java passes it as the sub).
            CHANGE => Some(cost.enchant_fail_level),
            // BLESSED/IMMORTAL failure: unchanged.
            _ => None,
        }
    };
    if let Some(new_sub) = new_sub
        && let Some(ench) = world.objects.get_component_mut::<SkillEnchants>(&object_id)
    {
        if new_sub > 0 {
            ench.0.insert(skill_id, new_sub);
        } else {
            ench.0.remove(&skill_id);
        }
    }
    // Java keys reuse stamps by `SkillData.getSkillHashCode(id|group, level,
    // subLevel)`, so the moment the sub-level moves — success *or* a
    // NORMAL/CHANGE failure reset — a running cooldown is orphaned under the
    // old hash and the skill is castable at once. The port's `Player.reuses`
    // is keyed per skill; dropping the entry on a sub-level change mirrors
    // the observable.
    if new_sub.is_some_and(|s| s != cur_sub) {
        let key = world
            .data
            .skill_data
            .get(skill_id, level)
            .map(|s| s.reuse_key());
        if let Some(key) = key
            && let Some(reuses) = world
                .objects
                .get_component_mut::<crate::model::components::Reuses>(&object_id)
        {
            reuses.0.remove(&key);
        }
    }

    if success {
        let sm = if ty == CHANGE {
            sm_ids::ENCHANT_SKILL_ROUTE_CHANGE_WAS_SUCCESSFUL
        } else {
            sm_ids::SKILL_ENCHANT_WAS_SUCCESSFUL_S1_HAS_BEEN_ENCHANTED
        };
        send_sm_to_client(
            world,
            client_id,
            sm,
            &[SmParam::SkillName {
                id: skill_id,
                level,
            }],
        );
    } else if ty == BLESSED || ty == IMMORTAL {
        send_sm_to_client(
            world,
            client_id,
            sm_ids::SKILL_ENCHANT_FAILED_CURRENT_LEVEL_WILL_REMAIN_UNCHANGED,
            &[SmParam::SkillName {
                id: skill_id,
                level,
            }],
        );
    } else {
        send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::SKILL_ENCHANT_FAILED_THE_SKILL_WILL_BE_INITIALIZED,
        );
    }
    send_to_client(
        world,
        client_id,
        crate::network::enter_world::ex_enchant_skill_result(success),
    );

    // `broadcastUserInfo()` + `sendSkillList()`.
    super::party::broadcast_user_info(world, object_id);
    if let Some(pkt) = super::helpers::skill_list_packet(world, object_id) {
        send_to_client(world, client_id, pkt);
    }
}
