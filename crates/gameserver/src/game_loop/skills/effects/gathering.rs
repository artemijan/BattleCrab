use super::*;
use crate::game_loop::helpers::npc_template;
use crate::game_loop::helpers::send_sm_bare_to_player;

/// `handlers/effecthandlers/Spoil.java` + its `calcSuccess`
/// (`Formulas.calcMagicSuccess`): mark a live monster spoiled so its `<spoil>`
/// list rolls into sweep loot on death, wake its AI (`EVT_ATTACKED`), and
/// message the caster. Non-monster/dead targets are rejected; an already-
/// spoiled mob reports it; a resisted cast lands silently (no effect).
pub(crate) fn apply_spoil(world: &mut World, caster_oid: i32, target_oid: i32, skill: &Skill) {
    use crate::model::npc::Npc;
    use server_packets::sm_ids;

    // `!effected.isMonster() || effected.isDead()` → INVALID_TARGET.
    let is_monster = crate::game_loop::combat::is_npc_oid(target_oid)
        && npc_template(world, target_oid).is_some_and(|t| t.is_auto_attackable());
    let dead = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.dead)
        .unwrap_or(true);
    if !is_monster || dead {
        send_sm(world, caster_oid, sm_ids::INVALID_TARGET);
        return;
    }
    // `target.isSpoiled()` → already spoiled.
    if world
        .objects
        .get_component::<Npc>(&target_oid)
        .map(|n| n.spoiler_object_id != 0)
        .unwrap_or(false)
    {
        send_sm(world, caster_oid, sm_ids::IT_HAS_ALREADY_BEEN_SPOILED);
        return;
    }
    // `calcSuccess` = `Formulas.calcMagicSuccess`, unconditional here — Spoil's
    // own handler calls it directly, so `MagicFailures` doesn't gate it.
    let penalty = world
        .cfg
        .npc
        .skill_chance_penalty_for_lvl_differences
        .clone();
    let input = magic_success_input(world, caster_oid, target_oid, skill, &penalty);
    if !formulas::calc_magic_success(&input, world.roll(100)) {
        // Magic resisted: `applyEffectScope` skips `instant()` — no effect,
        // and Java sends no message on a failed `calcSuccess`.
        return;
    }
    if let Some(npc) = world.objects.get_component_mut::<Npc>(&target_oid) {
        npc.spoiler_object_id = caster_oid;
    }
    send_sm(
        world,
        caster_oid,
        sm_ids::THE_SPOIL_CONDITION_HAS_BEEN_ACTIVATED,
    );
    // `target.getAI().notifyEvent(EVT_ATTACKED, effector)`.
    crate::game_loop::combat::npc_wake_on_attacked(world, target_oid, caster_oid);
}

/// `handlers/effecthandlers/Sweeper.java`: hand out the spoil loot rolled at
/// death (`Attackable.takeSweep`). The dead/spoiled/owner gate is enforced up
/// front by `resolve_cast_target` (the `OpSweeper` condition), so here we only
/// re-check ownership defensively and distribute the claimed items.
pub(crate) fn apply_sweeper(world: &mut World, caster_oid: i32, target_oid: i32) {
    use crate::model::components::Position;
    use crate::model::npc::Npc;

    if !crate::game_loop::combat::is_npc_oid(target_oid) {
        return;
    }
    // `checkSpoilOwner(player, false)` — silent (the message-carrying check ran
    // at cast start).
    let spoiler = world
        .objects
        .get_component::<Npc>(&target_oid)
        .map(|n| n.spoiler_object_id)
        .unwrap_or(0);
    if spoiler == 0
        || (spoiler != caster_oid
            && !crate::game_loop::party::same_party(world, caster_oid, spoiler))
    {
        return;
    }
    // `checkInventorySlotsAndWeight(spoilLootItems, true, true)` — refuse the
    // whole sweep (loot stays on the corpse) when the sweeper lacks the slots
    // or the carry weight for it, with Java's YOUR_INVENTORY_IS_FULL.
    {
        let pending: Vec<(i32, i64)> = world
            .objects
            .get_component::<Npc>(&target_oid)
            .and_then(|n| n.sweep_items.clone())
            .unwrap_or_default();
        let mut slots = 0i64;
        let mut loot_weight = 0i64;
        for &(item_id, count) in &pending {
            slots += crate::game_loop::weight::slots_needed(world, caster_oid, item_id, count);
            // Java's `getSpoilLootItems` yields one *template* per line, so
            // `lootWeight += item.getWeight()` counts each line once — the
            // stack count is deliberately NOT multiplied in, quirk and all.
            let _ = count;
            loot_weight += world
                .data
                .item_data
                .get(item_id)
                .map_or(0, |t| i64::from(t.weight));
        }
        if !crate::game_loop::weight::validate_capacity(world, caster_oid, slots)
            || !crate::game_loop::weight::validate_weight(world, caster_oid, loot_weight)
        {
            send_sm_bare_to_player(
                world,
                caster_oid,
                server_packets::sm_ids::YOUR_INVENTORY_IS_FULL,
            );
            return;
        }
    }
    // `takeSweep()` — atomically claim the loot (a second sweep gets nothing).
    let Some(items) = world
        .objects
        .get_component_mut::<Npc>(&target_oid)
        .and_then(|n| n.sweep_items.take())
    else {
        return;
    };
    let corpse = world
        .objects
        .get_component::<Position>(&target_oid)
        .map(|p| (p.x, p.y))
        .unwrap_or((0, 0));
    for (item_id, count) in items {
        // Solo → the sweeper; partied `*_INCLUDING_SPOIL` → a party member.
        // Sweep loot always enters the looter's inventory (Java `addItem`),
        // bypassing the auto-loot ground-drop toggle.
        let looter = crate::game_loop::party::spoil_looter(world, caster_oid, corpse);
        grant_and_notify(world, looter, &[(item_id, count, 0)]);
    }
}

/// `handlers/effecthandlers/Sow.java` — the manor sow (skill 2097). The Seed
/// item handler has already flagged the mob (`seed_id`/`seeder_object_id`); on a
/// live `canBeSown` monster the caster sowed and hasn't yet seeded, roll
/// `calcSuccess` and — on success — mark it seeded and stash the crop it yields.
///
/// Java consumes the seed item inside this effect; this port consumes it via the
/// item-skill path that cast the sow skill (the Seed handler), so no consume
/// here — the same one-seed cost.
pub(crate) fn apply_sow(world: &mut World, caster_oid: i32, target_oid: i32) {
    use crate::model::Player;
    use crate::model::npc::Npc;
    use server_packets::sm_ids;

    let Some(player_level) = world
        .objects
        .get_component::<Player>(&caster_oid)
        .map(|p| p.level)
    else {
        return;
    };
    if !crate::game_loop::combat::is_npc_oid(target_oid) {
        return;
    }
    let Some((seed_id, seeder, seeded, can_be_sown, target_level, skill_ids)) = world
        .objects
        .get_component::<Npc>(&target_oid)
        .and_then(|npc| {
            let state = (npc.seed_id, npc.seeder_object_id, npc.seeded);
            npc.template(world).map(|t| {
                (
                    state.0,
                    state.1,
                    state.2,
                    t.can_be_sown,
                    t.level,
                    t.skill_list.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                )
            })
        })
    else {
        return;
    };
    let dead = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.dead)
        .unwrap_or(true);
    // Java: dead / !canBeSown / already seeded / not this player's seed → bail.
    if dead || !can_be_sown || seeded || seed_id == 0 || seeder != caster_oid {
        return;
    }
    let Some((crop_id, seed_level, alternative)) = world
        .data
        .manor
        .seed_by_id(seed_id)
        .map(|s| (s.crop_id, s.level, s.alternative))
    else {
        return;
    };

    let sown = calc_sow_success(
        seed_level,
        alternative,
        player_level,
        target_level,
        world.roll(99),
    );
    if sown {
        // `player.sendPacket(QuestSound.ITEMSOUND_QUEST_ITEMGET.getPacket())`,
        // which Java fires before flagging the mob. Private to the sower even
        // when partied — only the result message below is broadcast.
        send_to_player(
            world,
            caster_oid,
            server_packets::play_sound(server_packets::quest_sounds::ITEMGET),
        );
        // The crop count: a "strong type" mob (skills 4303..=4310) multiplies it
        // ×2..×9, plus a hi-level-mob bonus, all scaled by `RateDropManor`.
        let mut count: i64 = 1;
        for id in &skill_ids {
            if (4303..=4310).contains(id) {
                count *= (*id - 4301) as i64; // 4303→×2 … 4310→×9
            }
        }
        let diff = target_level - seed_level - 5;
        if diff > 0 {
            count += diff as i64;
        }
        let harvest_count = count * world.cfg.rates.rate_drop_manor as i64;
        if let Some(npc) = world.objects.get_component_mut::<Npc>(&target_oid) {
            npc.seeded = true;
            npc.harvest_item = Some((crop_id, harvest_count));
        }
    }

    // Java builds one `SystemMessage` for either outcome and then routes it:
    // `party.broadcastPacket(sm)` when the sower is grouped, else a plain
    // `player.sendPacket(sm)`. The whole party learns the result, not just the
    // caster — a sown mob is shared loot, so this is information the group
    // needs, not flavour.
    let sm_id = if sown {
        sm_ids::THE_SEED_WAS_SUCCESSFULLY_SOWN
    } else {
        sm_ids::THE_SEED_WAS_NOT_SOWN
    };
    match world
        .objects
        .get_component::<crate::model::components::PartyRef>(&caster_oid)
        .map(|p| p.0)
    {
        Some(party_id) => {
            let sm = server_packets::system_message_with(sm_id, &[]);
            crate::game_loop::party::broadcast_to_party(world, party_id, &sm, None);
        }
        None => send_sm(world, caster_oid, sm_id),
    }

    // Java sets the mob's AI to IDLE after a sow attempt.
    crate::game_loop::helpers::set_active_intention(world, target_oid);
}

/// `Sow.calcSuccess`: a level-scaled chance (base 90 %, or 20 % for the
/// alternative seed). **Java quirk kept**: its `Math.max(basicSuccess, 1)` is a
/// discarded statement, so `basic` is never floored — a large level mismatch
/// yields a ≤0 % (always-fail) chance.
pub(crate) fn calc_sow_success(
    seed_level: i32,
    alternative: bool,
    player_level: i32,
    target_level: i32,
    roll: i32,
) -> bool {
    let min = seed_level - 5;
    let max = seed_level + 5;
    let mut basic = if alternative { 20 } else { 90 };
    if target_level < min {
        basic -= 5 * (min - target_level);
    }
    if target_level > max {
        basic -= 5 * (target_level - max);
    }
    let diff = (player_level - target_level).abs();
    if diff > 5 {
        basic -= 5 * (diff - 5);
    }
    roll < basic
}

/// `handlers/effecthandlers/Harvesting.java` — the manor harvest (skill 2098):
/// on a dead, seeded corpse the caster sowed, roll `calcSuccess` and hand over
/// the stashed crop (`Attackable.takeHarvest`).
pub(crate) fn apply_harvesting(world: &mut World, caster_oid: i32, target_oid: i32) {
    use crate::model::Player;
    use crate::model::npc::Npc;
    use server_packets::sm_ids;

    let Some(player_level) = world
        .objects
        .get_component::<Player>(&caster_oid)
        .map(|p| p.level)
    else {
        return;
    };
    if !crate::game_loop::combat::is_npc_oid(target_oid) {
        return;
    }
    let dead = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.dead)
        .unwrap_or(false);
    if !dead {
        return;
    }
    let Some((seeder, seeded, target_level)) = world
        .objects
        .get_component::<Npc>(&target_oid)
        .and_then(|npc| {
            let state = (npc.seeder_object_id, npc.seeded);
            npc.template(world).map(|t| (state.0, state.1, t.level))
        })
    else {
        return;
    };
    if caster_oid != seeder {
        send_sm(world, caster_oid, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_HARVEST);
        return;
    }
    if !seeded {
        return;
    }
    if calc_harvest_success(player_level, target_level, world.roll(99)) {
        // `takeHarvest()` — read and clear the stashed crop.
        let harvest = world
            .objects
            .get_component_mut::<Npc>(&target_oid)
            .and_then(|npc| npc.harvest_item.take());
        if let Some((crop_id, count)) = harvest {
            grant_and_notify(world, caster_oid, &[(crop_id, count, 0)]);
        }
    }
}

/// `Harvesting.calcSuccess`: base 100 %, a 5 % penalty per level of gap beyond
/// 5, floored at 1 % (this one *is* clamped, unlike [`calc_sow_success`]).
pub(crate) fn calc_harvest_success(player_level: i32, target_level: i32, roll: i32) -> bool {
    let diff = (player_level - target_level).abs();
    let mut basic = 100;
    if diff > 5 {
        basic -= (diff - 5) * 5;
    }
    if basic < 1 {
        basic = 1;
    }
    roll < basic
}

/// `handlers/effecthandlers/ConsumeBody.java`: decay the swept corpse at once
/// (`Npc.endDecayTask` → `onDecay`). Paired after `Sweeper` on skill 42 so the
/// body vanishes immediately. Only a dead NPC (the resolved corpse target).
pub(crate) fn apply_consume_body(world: &mut World, _caster_oid: i32, target_oid: i32) {
    if !crate::game_loop::combat::is_npc_oid(target_oid) {
        return;
    }
    if world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| !v.dead)
        .unwrap_or(true)
    {
        return;
    }
    // `endDecayTask()` runs `onDecay` now; the corpse's originally-scheduled
    // `NpcDecay` task then becomes a no-op (the entity is already despawned).
    crate::game_loop::death::handle_npc_decay(world, target_oid);
}
