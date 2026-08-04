use super::*;

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
        if let Some(client_id) = client_for_player(world, target_oid)
            && let Some(cs) = world.clients.get(&client_id)
        {
            cs.send(server_packets::system_message_with(
                sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE,
                &[],
            ));
        }
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
        if let Some(client_id) = client_for_player(world, target_oid)
            && let Some(cs) = world.clients.get(&client_id)
        {
            cs.send(server_packets::system_message_with(
                sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE,
                &[],
            ));
        }
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
        let Some(changed_oids) =
            crate::game_loop::items::add_inventory_item(world, target_oid, item_id, amount)
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
            for &oid in &changed_oids {
                inv.set_item_enchant(oid, enchant);
            }
        }
        let Some(inventory) = world.objects.get_component::<Inventory>(&target_oid) else {
            continue;
        };
        if let Some(client_id) = client_for_player(world, target_oid) {
            let iu = crate::network::enter_world::inventory_update(
                inventory,
                &world.data,
                &changed_oids,
            );
            if let Some(cs) = world.clients.get(&client_id) {
                // Java `RestorationRandom.sendMessage`: count>1 → "obtained S2 S1";
                // single enchanted → "obtained a +S1 S2"; else "obtained S1".
                let sm = if amount > 1 {
                    server_packets::system_message_with(
                        sm_ids::YOU_HAVE_OBTAINED_S2_S1,
                        &[SmParam::ItemName(item_id), SmParam::Long(amount)],
                    )
                } else if enchant > 0 {
                    server_packets::system_message_with(
                        sm_ids::YOU_HAVE_OBTAINED_A_S1_S2,
                        &[SmParam::Int(enchant), SmParam::ItemName(item_id)],
                    )
                } else {
                    server_packets::system_message_with(
                        sm_ids::YOU_HAVE_OBTAINED_S1,
                        &[SmParam::ItemName(item_id)],
                    )
                };
                cs.send(sm);
            }
            crate::game_loop::helpers::send_inventory_update(world, client_id, target_oid, iu);
        }
    }
}

/// Send a bare (no-argument) system message to `player_oid`, if online.
pub(crate) fn send_sm(world: &World, player_oid: i32, sm_id: i16) {
    crate::game_loop::helpers::send_sm_to_player(world, player_oid, sm_id, &[]);
}

/// `Creature.broadcastSocialAction` — a playable's emote goes to everyone in
/// range *including* itself (`broadcastPacket`), unlike the quest engine's
/// self-only `sendPacket` variant.
pub(crate) fn broadcast_social_action(world: &mut World, oid: i32, action_id: i32) {
    let Some(region) = world.objects.get_component::<RegionCell>(&oid).map(|r| r.0) else {
        return;
    };
    let pkt = server_packets::social_action(oid, action_id);
    crate::game_loop::helpers::broadcast_near_region(world, region, &pkt);
}

/// Send a system message with parameters to `player_oid`, if online.
pub(crate) fn send_sm_with(
    world: &World,
    player_oid: i32,
    sm_id: i16,
    params: &[server_packets::SmParam],
) {
    crate::game_loop::helpers::send_sm_to_player(world, player_oid, sm_id, params);
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
) -> formulas::MagicSuccess<'a> {
    use crate::model::npc::Npc;

    // Java `attacker.isAttackable() || target.isAttackable()`. `isAttackable()`
    // is the `Attackable` class test (monsters, guards, defenders), not
    // `isAutoAttackable` — a peaceful Folk on either side takes the PvP branch.
    let is_attackable = |oid: i32| {
        crate::game_loop::combat::is_npc_oid(oid)
            && world
                .objects
                .get_component::<Npc>(&oid)
                .and_then(|n| n.template(world))
                .is_some_and(|t| t.is_attackable_class())
    };

    let caster_player_level = world
        .objects
        .get_component::<crate::model::Player>(&caster_oid)
        .map(|p| p.level);

    // `target.isRaid() || target.isRaidMinion()` — a minion counts as a raid
    // only when its leader is one (Java sets `_isRaidMinion` from the spawning
    // raid boss, not from the minion's own template).
    let target_is_raid = world
        .objects
        .get_component::<Npc>(&target_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.is_raid())
        || world
            .objects
            .get_component::<crate::game_loop::minions::MinionOf>(&target_oid)
            .and_then(|leader| world.objects.get_component::<Npc>(&leader.0))
            .and_then(|n| n.template(world))
            .is_some_and(|t| t.is_raid());

    formulas::MagicSuccess {
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
            caster_level(world, caster_oid)
        },
        caster_player_level,
        target_is_raid,
        min_npc_level_for_magic_penalty: world.cfg.npc.min_npc_level_for_magic_penalty,
        skill_chance_penalty: penalty,
        // `target.getStat().getMul(MAGIC_SUCCESS_RES, 1)` — read off the
        // *target*, and 1.0 for anyone without Anti Magic / M. Def.
        res_modifier: world
            .objects
            .get_component::<crate::model::components::StatModifiers>(&target_oid)
            .and_then(|m| {
                m.mul
                    .get(&crate::model::stats::Stat::MagicSuccessRes)
                    .copied()
            })
            .unwrap_or(1.0),
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
) -> formulas::MagicFailure {
    use server_packets::{SmParam, sm_ids};

    if !world.cfg.character.magic_failures {
        return formulas::MagicFailure::None;
    }

    let penalty = world
        .cfg
        .npc
        .skill_chance_penalty_for_lvl_differences
        .clone();
    let input = magic_success_input(world, caster_oid, target_oid, skill, &penalty);
    if formulas::calc_magic_success(&input, world.roll(100)) {
        return formulas::MagicFailure::None;
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
        if formulas::calc_magic_success(&input, world.roll(100)) {
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
                if let Some(client_id) = client_for_player(world, caster_oid)
                    && let Some(cs) = world.clients.get(&client_id)
                {
                    cs.send(server_packets::system_message(&message));
                }
            }
            formulas::MagicFailure::Half
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
            formulas::MagicFailure::Resisted
        }
    } else {
        // NPC caster: Java leaves `damage` untouched.
        formulas::MagicFailure::None
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
            &[crate::network::server_packets::SmParam::Text(caster_name)],
        );
    }

    outcome
}
