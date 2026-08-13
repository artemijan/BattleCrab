//! EtcItem "use" handlers: the ItemHandler dispatch, seeds, item skills,
//! consume checks and extractables.

use super::*;
/// The `EtcItem` branch of `UseItem.runImpl` (Java:
/// `ItemHandler.getInstance().getHandler(etcItem)`). Dispatches on
/// `ItemTemplate.handler`; only `ExtractableItems` (pack/box items) is
/// implemented so far. Anything else is consumed as a no-op, matching Java's
/// "Unmanaged Item handler" branch (logged, no visible effect to the player).
pub(super) fn use_etc_item(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    let handler = {
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        let Some(item) = inventory.by_object_id(item_object_id) else {
            return;
        };
        world
            .data
            .item_data
            .get(item.item_id)
            .map(|t| t.handler)
            .unwrap_or_default()
    };
    match handler {
        ItemHandler::ExtractableItems => extract_item(world, client_id, object_id, item_object_id),
        ItemHandler::ItemSkills => use_item_skills(world, client_id, object_id, item_object_id),
        ItemHandler::Seed => use_seed_item(world, client_id, object_id, item_object_id),
        ItemHandler::SoulShots | ItemHandler::SpiritShot | ItemHandler::BlessedSpiritShot => {
            let item_id = item_id_of(world, object_id, item_object_id);
            if let Some(item_id) = item_id {
                charge_shot(world, object_id, item_id, handler, false);
            }
        }
        // A Beast shot used by hand does nothing: it is spent by the summon's
        // swing (`Summon.rechargeShots`), not by the owner clicking it. Java's
        // `BeastSoulShot` handler likewise only ever runs *from* that path.
        ItemHandler::BeastSoulShot | ItemHandler::BeastSpiritShot => {}
        ItemHandler::EnchantScrolls => {
            crate::game_loop::enchant::open(world, client_id, object_id, item_object_id)
        }
        ItemHandler::Recipes => {
            crate::game_loop::crafting::learn_recipe(world, client_id, object_id, item_object_id)
        }
        // A fishing shot used by hand charges immediately (the fishing engine
        // otherwise charges it on cast via `rechargeShots(fish=true)`).
        ItemHandler::FishShots => {
            let item_id = item_id_of(world, object_id, item_object_id);
            if let Some(item_id) = item_id {
                charge_fish_shot(world, object_id, item_id);
            }
        }
        ItemHandler::None => {}
    }
}

/// Port of `handlers/itemhandlers/ItemSkillsTemplate.useItem` (potions, buff
/// scrolls, escape scrolls, …). Each of the item's `<skills>` entries takes
/// one of Java's two branches:
///
/// * **instant** (`SkillCaster.triggerCast`) when the skill is
///   `withoutAction` or the item carries `immediate_effect`/
///   `ex_immediate_effect` — the effects land at once, no cast bar. This is
///   the potion/herb/capsule path.
/// * **cast** (`playable.useMagic(itemSkill, item, …)`) otherwise — a real
///   cast bar of the skill's own `hitTime`, interruptible by damage. This is
///   the scroll path: a Scroll of Escape (736 → 2013) casts for 20 s, a
///   Scroll: Might (3933 → 2057) for 4 s.
///
/// Consumption follows `checkConsume` (see [`check_consume`]) and, as in
/// Java, happens as soon as the branch *starts* — a scroll is spent even if
/// the cast is interrupted, since `useMagic` returning true is what sets
/// `successfulUse`.
///
/// Narrowing: no `<cond>` gating (not parsed for items — see `item_data`'s
/// header comment). Java's pet and Olympiad legs are not narrowings any more —
/// both subsystems landed (G29, G25) — but this path has never routed to
/// them; wiring them is the open half, not their absence.
/// A timed item skill that loses the race against a running cast is queued as
/// `QueuedAction::UseItem` and replayed when the cast ends — the port's
/// equivalent of Java's `_queuedSkill` (an immediate-effect item, a potion,
/// never raced: its branch bypasses `Casting` entirely).
/// Port of `handlers/itemhandlers/Seed.useItem` — sow a manor seed on the
/// player's targeted monster: validate the target, flag the mob with the seed
/// (`Attackable.setSeeded(seed, player)`), then cast the item's Sow skill (which
/// runs [`crate::game_loop::skills::effects`]'s `Sow`). The item is consumed by
/// the skill cast, as with any `<skills>` item.
///
/// The sow-location gate (`seed.getCastleId() == target.getTaxCastle()`) is
/// honored, `THIS_SEED_MAY_NOT_BE_SOWN_HERE` included.
fn use_seed_item(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    use crate::model::components::TargetRef;
    use crate::model::npc::Npc;
    use crate::network::server_packets::sm_ids;

    if !world.cfg.general.allow_manor {
        return;
    }
    let item_id = item_id_of(world, object_id, item_object_id);
    let Some(item_id) = item_id else {
        return;
    };
    let send = |world: &World, sm: i16| {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(sm, &[]),
        );
    };

    // The seeded target is the player's current target.
    let Some(target_oid) = world
        .objects
        .get_component::<TargetRef>(&object_id)
        .and_then(|t| t.0)
        .filter(|oid| crate::game_loop::combat::is_npc_oid(*oid))
    else {
        send(world, sm_ids::INVALID_TARGET);
        return;
    };
    // Must be a live, `canBeSown` monster that isn't already seeded.
    let can_be_sown = npc_template(world, target_oid).is_some_and(|t| t.can_be_sown);
    let dead = world
        .objects
        .get_component::<crate::model::components::Vitals>(&target_oid)
        .map(|v| v.dead)
        .unwrap_or(true);
    let already_seeded = world
        .objects
        .get_component::<Npc>(&target_oid)
        .map(|n| n.seeded)
        .unwrap_or(false);
    if !can_be_sown || dead {
        // Java: THE_TARGET_IS_UNAVAILABLE_FOR_SEEDING / INVALID_TARGET.
        send(world, sm_ids::INVALID_TARGET);
        return;
    }
    if already_seeded {
        send_action_failed(world, client_id);
        return;
    }
    // The seed must be in the catalogue (Java `getSeed(itemId)`)…
    let Some(seed_castle) = world.data.manor.seed_by_id(item_id).map(|s| s.castle_id) else {
        return;
    };
    // …and it may only be sown inside its own castle's territory (Java
    // `(taxCastle == null) || (seed.getCastleId() != taxCastle.getResidenceId())`).
    if crate::game_loop::castle::npc_tax_castle(world, target_oid) != Some(seed_castle) {
        send(world, sm_ids::THIS_SEED_MAY_NOT_BE_SOWN_HERE);
        return;
    }

    // Flag the mob (Java `setSeeded(seed, player)` — sets seed + seeder, not the
    // seeded state; the Sow effect sets that on success).
    if let Some(npc) = world.objects.get_component_mut::<Npc>(&target_oid) {
        npc.seed_id = item_id;
        npc.seeder_object_id = object_id;
    }
    // Cast the item's Sow skill (consumes the seed, applies the `Sow` effect).
    use_item_skills(world, client_id, object_id, item_object_id);
}

/// Drink/consume one carried item by object id, on the player's own behalf —
/// the auto-potion loop's entry into the ordinary item-skill path, so the cast,
/// the cooldown and the consumption are identical to using it by hand.
pub(crate) fn use_item_by_object_id(world: &mut World, player_oid: i32, item_object_id: i32) {
    let Some(client_id) = crate::game_loop::helpers::client_for_player(world, player_oid) else {
        return;
    };
    use_item_skills(world, client_id, player_oid, item_object_id);
}

fn use_item_skills(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    use crate::game_loop::skills::cast::{
        check_skill_reuse, resolve_cast_target, set_skill_reuse, start_casting,
    };
    use crate::game_loop::skills::effects::apply_skill_effects;
    use crate::model::Player;
    use crate::model::components::{Casting, TargetRef};
    use crate::model::skill::TargetType;

    let (item_skills, immediate_effect, ex_immediate_effect, default_action) = {
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        let Some(item) = inventory.by_object_id(item_object_id) else {
            return;
        };
        let Some(template) = world.data.item_data.get(item.item_id) else {
            return;
        };
        (
            template.item_skills.clone(),
            template.immediate_effect,
            template.ex_immediate_effect,
            template.default_action,
        )
    };
    if item_skills.is_empty() {
        return;
    }
    // Java's `SummonItems` handler attaches a `PetItemHolder` to the player
    // before casting, because the `SummonPet` effect never receives the item.
    // Park the collar's object id the same way; the effect *takes* it, so an
    // unused one cannot linger into an unrelated cast.
    {
        let is_collar = item_id_of(world, object_id, item_object_id)
            .is_some_and(|item_id| world.data.pet_data.is_pet_collar(item_id));
        if let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&object_id)
        {
            p.pending_pet_collar = if is_collar {
                Some(item_object_id)
            } else {
                None
            };
        }
    }

    let mut used = false;
    // `hasConsumeSkill` — Java sets it for every listed skill, *before* any of
    // the per-skill `continue`s, so a skill that never fires still counts.
    let mut has_consume_skill = false;
    for (skill_id, skill_level) in item_skills {
        let Some(skill) = skill_by_id(world, skill_id, skill_level) else {
            continue;
        };
        if skill.item_consume_id > 0 {
            has_consume_skill = true;
        }
        if !check_skill_reuse(world, client_id, object_id, &skill) {
            continue;
        }
        let target_oid = match skill.target_type {
            TargetType::Self_ => object_id,
            _ => {
                let Some(player) = world.objects.get_component::<Player>(&object_id) else {
                    continue;
                };
                let Some(pos) = maybe_position(world, object_id) else {
                    continue;
                };
                let target_ref = world
                    .objects
                    .get_component::<TargetRef>(&object_id)
                    .copied()
                    .unwrap_or_default()
                    .0;
                match resolve_cast_target(world, player, &pos, target_ref, &skill, true, false) {
                    Ok(oid) => oid,
                    Err(_) => continue,
                }
            }
        };
        if skill.without_action || immediate_effect || ex_immediate_effect {
            apply_skill_effects(world, object_id, target_oid, &skill);
            set_skill_reuse(world, object_id, &skill);
        } else {
            if world.objects.has_component::<Casting>(&object_id) {
                // Java's `useMagic` queues the skill that loses this race
                // (`Player._queuedSkill`); the port queues the *item use* and
                // replays it when the running cast ends — same observable,
                // and the consume happens on the replay's own branch.
                world.objects.add_components(
                    &object_id,
                    crate::model::components::QueuedAction::UseItem { item_object_id },
                );
                continue;
            }
            // `start_casting` registers the reuse itself.
            start_casting(world, client_id, object_id, &skill, target_oid);
            // Java `SkillCaster(caster, target, skill, item, …)`: a
            // `SKILL_REDUCE_ON_SKILL_SUCCESS` item rides the cast and is spent
            // by `finishSkill` only if it lands.
            if default_action == crate::data::item_data::ActionType::SkillReduceOnSkillSuccess {
                crate::game_loop::skills::cast::set_cast_trigger_item(
                    world,
                    object_id,
                    item_object_id,
                );
            }
        }
        used = true;
    }

    if used && check_consume(default_action, has_consume_skill, immediate_effect) {
        destroy_used_item(world, object_id, item_object_id);
    }
}

/// Port of `ItemSkillsTemplate.checkConsume`: whether the *item handler* is
/// the one that destroys the item.
fn check_consume(
    default_action: crate::data::item_data::ActionType,
    has_consume_skill: bool,
    immediate_effect: bool,
) -> bool {
    use crate::data::item_data::ActionType;
    match default_action {
        // Java: `if (!hasConsumeSkill && hasImmediateEffect()) return true;`
        // then falls out of the switch to `return hasConsumeSkill`.
        ActionType::Capsule | ActionType::SkillReduce => has_consume_skill || immediate_effect,
        // Java returns false: these are destroyed by `SkillCaster.finishSkill`
        // when the cast actually *lands* — the cast carries the item
        // (`CastState.trigger_item_object_id`) and the finish phase spends
        // `itemConsumeCount` of it, so an interrupted cast costs nothing.
        ActionType::SkillReduceOnSkillSuccess => false,
        // Summon shots are never consumed by a direct item-use: they are spent
        // by `servitor::recharge_shots` when the summon swings, in the count
        // the pet's level demands. Using one by hand does nothing.
        ActionType::SummonSoulshot | ActionType::SummonSpiritshot => false,
        ActionType::Other => has_consume_skill,
    }
}

/// Destroys one unit of a used etc item and notifies the client — the
/// consume tail shared by `ExtractableItems` and `ItemSkills`.
fn destroy_used_item(world: &mut World, object_id: i32, item_object_id: i32) {
    let Some(destroyed) = ({
        let Some(inventory) = world.objects.get_component_mut::<Inventory>(&object_id) else {
            return;
        };
        inventory.remove_by_object_id(item_object_id, 1)
    }) else {
        return;
    };
    // Memory-first: the count decrement / removal already applied to the
    // `Inventory` component; it persists on the next flush.
    crate::game_loop::helpers::send_inventory_update(world, object_id, vec![destroyed]);
}

/// Port of `handlers/itemhandlers/ExtractableItems.useItem`: destroys the
/// used item, then rolls its `<capsuled_items>` list and grants what hits.
/// `extractableCountMin == 0` (every currently-loaded pack/box item) takes a
/// single pass over the list; `> 0` re-rolls the whole list until at least
/// that many entries have been granted, mirroring Java's `while` loop (used
/// by "pick one of N" reward boxes) — capped at a generous iteration count
/// so a misconfigured item (chances that can never sum to the minimum)
/// can't hang the single-threaded game loop the way it could a Java
/// per-client thread. Per-entry enchant rolls are skipped (later milestone;
/// nothing currently loaded needs them).
fn extract_item(world: &mut World, client_id: u32, object_id: i32, item_object_id: i32) {
    let (capsules, count_min, count_max) = {
        let Some(inventory) = world.objects.get_component::<Inventory>(&object_id) else {
            return;
        };
        let Some(item) = inventory.by_object_id(item_object_id) else {
            return;
        };
        let Some(template) = world.data.item_data.get(item.item_id) else {
            return;
        };
        (
            template.capsuled_items.clone(),
            template.extractable_count_min.max(0),
            template.extractable_count_max,
        )
    };
    if capsules.is_empty() {
        return;
    }

    // Port of `Player.isInventoryUnder80(false)`, the gate
    // `ExtractableItems.useItem` checks before touching the item: refuse
    // (leaving the box and inventory untouched) if the bag is already too
    // full for the reward roll to have anywhere to go.
    let race = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map(|p| p.race)
        .unwrap_or(0);
    let normal_limit = world.cfg.character.inventory_limit(race);
    let under_80 = world
        .objects
        .get_component::<Inventory>(&object_id)
        .is_some_and(|inv| inv.is_under_80_percent(&world.data.item_data, normal_limit));
    if !under_80 {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(sm_ids::YOUR_INVENTORY_IS_FULL, &[]),
        );
        return;
    }

    destroy_used_item(world, object_id, item_object_id);

    let mut granted: Vec<(i32, i64)> = Vec::new();
    for _ in 0..1000 {
        for product in &capsules {
            if count_max > 0 && granted.len() as i32 >= count_max {
                break;
            }
            if world.roll(100_000) > product.chance {
                continue;
            }
            let span = (product.max - product.min + 1).max(1) as i32;
            let amount = if product.max == product.min {
                product.min
            } else {
                product.min + world.roll(span) as i64
            };
            if amount != 0 {
                granted.push((product.item_id, amount));
            }
        }
        if granted.len() as i32 >= count_min {
            break;
        }
    }

    if granted.is_empty() {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(sm_ids::THERE_WAS_NOTHING_FOUND_INSIDE, &[]),
        );
        return;
    }

    for (item_id, amount) in granted {
        let Some(changes) = crate::game_loop::helpers::add_inventory_item_changes(
            world, object_id, item_id, amount,
        ) else {
            warn!("ExtractableItems: object-id pool exhausted, dropping {item_id}x{amount}");
            continue;
        };
        send_to_client(
            world,
            client_id,
            server_packets::obtained_item_sm(item_id, amount),
        );
        crate::game_loop::helpers::send_inventory_update(world, object_id, changes);
    }
}
