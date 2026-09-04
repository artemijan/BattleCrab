use super::arm_charge_decay;
use super::caster_display_name;
use super::creature_level;
use super::creature_name;
use super::player_or_npc_level;
use crate::game_loop::character::inventory;
pub(crate) use crate::game_loop::helpers::{
    send_sm_bare_to_player as send_sm, send_sm_to_player as send_sm_with,
};
use crate::game_loop::net::broadcast;
use crate::game_loop::{helpers, npc};
use crate::model::components::stats::CombatStats;
use crate::model::formulas;
use crate::model::skill::Skill;
use crate::model::skill::effects::RestorationGroup;
use crate::network::server_packets;
use crate::world::World;

/// `handlers/effecthandlers/Restoration.java` — instant single-item grant.
/// Backs item-use skills wrapping a fixed pack/box reward (spiritshot packs,
/// jewelry boxes, …): the item's `<skills>` entry casts a skill with this
/// effect, and *that* is where the actual reward comes from — before this
/// was ported, such skills loaded with an empty effect list, so the item was
/// still consumed (`items::use_item_skills` destroys it once any skill
/// "lands") but granted nothing.
pub(crate) fn give_item(
    world: &mut World,
    target_oid: i32,
    item_id: i32,
    item_count: i64,
    item_enchant_level: i32,
) {
    use server_packets::sm_ids;

    if item_id <= 0 || item_count <= 0 {
        send_sm(world, target_oid, sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE);
        return;
    }
    // Java `Restoration`: `if (_itemEnchantmentLevel > 0) setEnchantLevel(...)`.
    grant_and_notify(
        world,
        target_oid,
        &[(item_id, item_count, item_enchant_level.max(0))],
    );
}

/// `handlers/effecthandlers/RestorationRandom.java` — one weighted roulette
/// pick among reward groups: walk `groups` accumulating `chance` until the
/// roll falls in a slice's `[chance_from, chance_from + chance)` range, then
/// grant every item in that slice's group together (Java: `100 *
/// Rnd.nextDouble()` against the raw 0-100 XML percentages).
pub(crate) fn give_item_random(world: &mut World, target_oid: i32, groups: &[RestorationGroup]) {
    use server_packets::sm_ids;

    let rnd_num = 100.0 * world.roll_f64();
    let mut chance_from = 0.0;
    let mut picked = None;
    for group in groups {
        if rnd_num >= chance_from && rnd_num <= chance_from + group.chance {
            picked = Some(&group.items);
            break;
        }
        chance_from += group.chance;
    }
    let Some(items) = picked else {
        send_sm(world, target_oid, sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE);
        return;
    };
    // Java `RestorationRandom`: roll `Rnd.get(minEnchant, maxEnchant)` (inclusive)
    // per created item when `maxEnchant > 0`, else no enchant.
    let grants: Vec<(i32, i64, i32)> = items
        .iter()
        .filter(|i| i.item_id > 0 && i.count > 0)
        .map(|i| {
            let enchant = if i.max_enchant > 0 {
                i.min_enchant + world.roll(i.max_enchant - i.min_enchant + 1)
            } else {
                0
            };
            (i.item_id, i.count, enchant)
        })
        .collect();
    grant_and_notify(world, target_oid, &grants);
}

/// Shared grant + `InventoryUpdate` + "You have obtained…" messaging tail for
/// `give_item`/`give_item_random` (Java: `Player.addItem` plus the
/// `sendMessage` helper both `Restoration` variants duplicate).
pub(crate) fn grant_and_notify(world: &mut World, target_oid: i32, grants: &[(i32, i64, i32)]) {
    use crate::model::inventory::Inventory;
    use server_packets::{SmParam, sm_ids};

    for &(item_id, amount, enchant) in grants {
        let Some(added) = inventory::add_inventory_item_tracked(world, target_oid, item_id, amount)
        else {
            continue;
        };
        // Stamp the rolled/fixed enchant onto the freshly created item(s). Only
        // non-stackable items carry an enchant; a stackable grant returns an
        // existing stack's oid, which must not be touched.
        if enchant > 0
            && !world
                .data
                .item_data
                .get(item_id)
                .map(|t| t.is_stackable)
                .unwrap_or(false)
            && let Some(inv) = world.objects.get_component_mut::<Inventory>(&target_oid)
        {
            for &(oid, _) in &added {
                inv.set_item_enchant(oid, enchant);
            }
        }
        // Snapshot after the enchant stamp, so the packet carries the `+N`.
        let changes = inventory::added_changes(world, target_oid, &added);
        if let Some(client_id) = helpers::client_for_player(world, target_oid) {
            // Java `RestorationRandom.sendMessage`: count>1 → "obtained S2 S1";
            // single enchanted → "obtained a +S1 S2"; else "obtained S1".
            let sm = if amount <= 1 && enchant > 0 {
                server_packets::system_message_with(
                    sm_ids::YOU_HAVE_OBTAINED_A_S1_S2,
                    &[SmParam::Int(enchant), SmParam::ItemName(item_id)],
                )
            } else {
                server_packets::obtained_item_sm(item_id, amount)
            };
            helpers::send_to_client(world, client_id, sm);
            inventory::send_inventory_update(world, target_oid, changes);
        }
    }
}

/// `Creature.broadcastSocialAction` — a playable's emote goes to everyone in
/// range *including* itself (`broadcastPacket`), unlike the quest engine's
/// self-only `sendPacket` variant.
pub(crate) fn broadcast_social_action(world: &mut World, oid: i32, action_id: i32) {
    let pkt = server_packets::social_action(oid, action_id);
    broadcast::broadcast_from(world, oid, &pkt);
}

/// Resolve `Formulas.calcMagicSuccess`' inputs for a cast. `penalty` is the
/// caller-owned backing store for the config penalty table (the struct borrows
/// it), since `world` is re-borrowed mutably for the roll.
pub(crate) fn magic_success_input<'a>(
    world: &World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    penalty: &'a [f64],
) -> formulas::magic::MagicSuccess<'a> {
    // Java `attacker.isAttackable() || target.isAttackable()`. `isAttackable()`
    // is the `Attackable` class test (monsters, guards, defenders), not
    // `isAutoAttackable` — a peaceful Folk on either side takes the PvP branch.
    let is_attackable = |oid: i32| {
        crate::game_loop::combat::is_npc_oid(oid)
            && npc::npc_template(world, oid).is_some_and(|t| t.is_attackable_class())
    };

    let caster_player_level = world
        .objects
        .get_component::<crate::model::Player>(&caster_oid)
        .map(|p| p.level);

    // `target.isRaid() || target.isRaidMinion()` — a minion counts as a raid
    // only when its leader is one (Java sets `_isRaidMinion` from the spawning
    // raid boss, not from the minion's own template).
    let target_is_raid = npc::is_raid_npc(world, target_oid)
        || world
            .objects
            .get_component::<npc::minions::MinionOf>(&target_oid)
            .is_some_and(|leader| npc::is_raid_npc(world, leader.0));

    formulas::magic::MagicSuccess {
        pve: is_attackable(caster_oid) || is_attackable(target_oid),
        target_level: creature_level(world, target_oid),
        effective_level: if world
            .cfg
            .character
            .calculate_magic_success_by_skill_magic_level
            && skill.magic_level > 0
        {
            skill.magic_level
        } else {
            player_or_npc_level(world, caster_oid)
        },
        caster_player_level,
        target_is_raid,
        min_npc_level_for_magic_penalty: world.cfg.npc.min_npc_level_for_magic_penalty,
        skill_chance_penalty: penalty,
        // `target.getStat().getMul(MAGIC_SUCCESS_RES, 1)` — read off the
        // *target*, and 1.0 for anyone without Anti Magic / M. Def.
        res_modifier: helpers::stat_mul(
            world,
            target_oid,
            crate::model::stats::Stat::MagicSuccessRes,
        ),
        magic_accuracy: world
            .objects
            .get_component::<CombatStats>(&caster_oid)
            .map(|c| c.magic_accuracy)
            .unwrap_or(0),
        magic_evasion: world
            .objects
            .get_component::<CombatStats>(&target_oid)
            .map(|c| c.magic_evasion)
            .unwrap_or(0),
    }
}

/// `Formulas.calcMagicDam`'s `ALT_GAME_MAGICFAILURES` block: roll
/// `calcMagicSuccess`, and on a miss roll it a *second* time to pick between
/// half damage and a flat 1, messaging both sides the way Java does.
///
/// Two Java quirks are load-bearing here and deliberately preserved:
/// 1. The second roll — and therefore the damage reduction — only happens when
///    the attacker is a player. An NPC caster that fails the first roll deals
///    **full** damage; only the player target's "You resisted" line is sent.
/// 2. Both the attacker-side and target-side messages fire on the same failure,
///    so a resisted PvP nuke messages caster and victim.
pub(crate) fn roll_magic_failure(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    is_drain: bool,
) -> formulas::magic::MagicFailure {
    use server_packets::{SmParam, sm_ids};

    if !world.cfg.character.magic_failures {
        return formulas::magic::MagicFailure::None;
    }

    let penalty = world
        .cfg
        .npc
        .skill_chance_penalty_for_lvl_differences
        .clone();
    let input = magic_success_input(world, caster_oid, target_oid, skill, &penalty);
    if formulas::magic::calc_magic_success(&input, world.roll(100)) {
        return formulas::magic::MagicFailure::None;
    }

    let caster_is_player = world
        .objects
        .get_component::<crate::model::Player>(&caster_oid)
        .is_some();
    let target_is_player = world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
        .is_some();

    let outcome = if caster_is_player {
        // Java re-runs `calcMagicSuccess` here — an independent second roll,
        // not a reuse of the first one's result.
        let input = magic_success_input(world, caster_oid, target_oid, skill, &penalty);
        if formulas::magic::calc_magic_success(&input, world.roll(100)) {
            if is_drain {
                // A drain keeps its own retail line, which says the same thing
                // in the terms of the skill that caused it.
                send_sm(world, caster_oid, sm_ids::DRAIN_WAS_ONLY_50_SUCCESSFUL);
            } else {
                // Java `Formulas.calcMagicDam`: the caster is told whose
                // resistance halved it, target first then attacker, both as
                // plain names.
                use commons::system_messages::generated::DAMAGE_IS_DECREASED_BECAUSE_C1_RESISTED_C2_S_MAGIC;
                let message = DAMAGE_IS_DECREASED_BECAUSE_C1_RESISTED_C2_S_MAGIC::new(
                    creature_name(world, target_oid),
                    creature_name(world, caster_oid),
                );
                helpers::send_to_player(
                    world,
                    caster_oid,
                    server_packets::system_message(&message),
                );
            }
            formulas::magic::MagicFailure::Half
        } else {
            let target_name = creature_name(world, target_oid);
            send_sm_with(
                world,
                caster_oid,
                sm_ids::C1_HAS_RESISTED_YOUR_S2,
                &[
                    SmParam::Text(target_name),
                    SmParam::SkillName {
                        id: skill.id,
                        level: skill.level,
                    },
                ],
            );
            formulas::magic::MagicFailure::Resisted
        }
    } else {
        // NPC caster: Java leaves `damage` untouched.
        formulas::magic::MagicFailure::None
    };

    if target_is_player {
        let caster_name = caster_display_name(world, caster_oid);
        send_sm_with(
            world,
            target_oid,
            if is_drain {
                sm_ids::YOU_RESISTED_C1_S_DRAIN
            } else {
                sm_ids::YOU_RESISTED_C1_S_MAGIC
            },
            &[SmParam::Text(caster_name)],
        );
    }

    outcome
}

// --- arms extracted from the `apply_skill_effects` match -------------------

/// `GiveSp.instant` — SP Scrolls and the Primeval Isle crystals.
///
/// Java credits the **effector**, guards on both ends being players and on
/// the effected not being alike-dead, and calls the plain two-arg
/// `addExpAndSp` — which is `useBonuses = false`, so no vitality or rate
/// multiplier applies to a scroll.
pub(crate) fn give_sp(world: &mut World, caster_oid: i32, target_oid: i32, sp: i64) {
    let both_players = world
        .objects
        .has_component::<crate::model::Player>(&caster_oid)
        && world
            .objects
            .has_component::<crate::model::Player>(&target_oid);
    let dead = helpers::is_dead(world, target_oid);
    if both_players && !dead {
        crate::game_loop::death::add_exp_and_sp(world, caster_oid, 0.0, sp as f64, false);
    }
}

/// `OpenCommonRecipeBook`/`OpenDwarfRecipeBook.instant`: players only,
/// refused while a private store (incl. manufacture) is up, then
/// `RecipeManager.requestBookOpen`.
pub(crate) fn open_recipe_book(world: &mut World, caster_oid: i32, dwarven: bool) {
    use server_packets::sm_ids;
    if world
        .objects
        .get_component::<crate::model::Player>(&caster_oid)
        .is_some()
    {
        let store_type = world
            .objects
            .get_component::<crate::model::Player>(&caster_oid)
            .map(|p| p.store_type)
            .unwrap_or(0);
        if store_type != 0 {
            send_sm(
                world,
                caster_oid,
                sm_ids::ITEM_CREATION_IS_NOT_POSSIBLE_WHILE_ENGAGED_IN_A_TRADE,
            );
        } else if let Some(cid) = helpers::client_for_player(world, caster_oid) {
            crate::game_loop::commerce::crafting::request_book_open(world, cid, dwarven);
        }
    }
}

/// `FocusMomentum.instant` — the "Force" charge gain (Force Meditation and
/// friends), capped at Java's hardcoded fallback for the never-set
/// `MAX_MOMENTUM` stat.
pub(crate) fn focus_momentum(world: &mut World, target_oid: i32, amount: i32, max_charges: i32) {
    use server_packets::{SmParam, sm_ids};
    let max = max_charges.min(8);
    let current = world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
        .map(|p| p.charges)
        .unwrap_or(0);
    let Some(client_id) = helpers::client_for_player(world, target_oid) else {
        return;
    };
    if current >= max {
        helpers::send_to_client(
            world,
            client_id,
            server_packets::system_message_with(
                sm_ids::YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY,
                &[],
            ),
        );
        return;
    }
    let new_charge = (current + amount).min(max);
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&target_oid)
    {
        p.charges = new_charge;
    }
    // `setCharges` restarts the decay clock.
    arm_charge_decay(world, target_oid);
    if new_charge == max {
        helpers::send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY,
        );
    } else {
        helpers::send_sm_to_client(
            world,
            client_id,
            sm_ids::YOUR_FORCE_HAS_INCREASED_TO_LEVEL_S1,
            &[SmParam::Int(new_charge)],
        );
    }
    crate::game_loop::helpers::send_etc_status_update(world, client_id, target_oid);
}

/// `ChangeFace` / `ChangeHairStyle` / `ChangeHairColor` — the appearance
/// potions. Players only (`effected.isPlayer()`), and the re-broadcast is what
/// makes the change visible: the value lives in `UserInfo`/`CharInfo`, so
/// without it the client would keep drawing the old head until relog.
pub(crate) fn change_appearance(
    world: &mut World,
    target_oid: i32,
    part: crate::model::skill::effects::AppearancePart,
    value: i32,
) {
    use crate::model::skill::effects::AppearancePart;
    let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&target_oid)
    else {
        return;
    };
    match part {
        AppearancePart::Face => p.face = value,
        AppearancePart::HairStyle => p.hair_style = value,
        AppearancePart::HairColor => p.hair_color = value,
    }
    crate::game_loop::character::player_info::broadcast_user_info(world, target_oid);
}

/// `SendSystemMessageToClan.instant` — `clan.broadcastToOnlineMembers(msg)`.
/// A clanless caster is a no-op, as in Java.
pub(crate) fn send_system_message_to_clan(world: &mut World, target_oid: i32, message_id: i16) {
    let clan_id = world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
        .map(|p| p.clan_id)
        .filter(|id| *id != 0);
    let Some(clan_id) = clan_id else {
        return;
    };
    for member in crate::game_loop::clans::online_members(world, clan_id) {
        helpers::send_sm_bare_to_player(world, member, message_id);
    }
}

/// `Creature.getRandomDamageMultiplier()` — `1 + Rnd.get(-random, random)/100`
/// over the caster's own `RANDOM_DAMAGE`.
///
/// The auto-attack path rolls this inline because it already holds the
/// attacker's `CombatStats`; the skill paths need it by object id, and Java
/// reads it in `calcMagicDam` and `PhysicalAttack` alike.
pub(crate) fn random_damage_multiplier_of(world: &mut World, oid: i32) -> f64 {
    let r = world
        .objects
        .get_component::<crate::model::components::stats::CombatStats>(&oid)
        .map_or(0, |c| c.random_dmg);
    if r <= 0 {
        return 1.0;
    }
    let roll = world.roll(2 * r + 1) - r;
    crate::model::formulas::physical::random_damage_multiplier(roll)
}
