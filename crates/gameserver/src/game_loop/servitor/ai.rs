//! Servitor AI and owner commands: the follow tick, attack/stop orders, the
//! action-bar packet, and ordered skill use.

use super::*;
/// How close a servitor trails its owner before it stops — Java's
/// `AI_INTENTION_FOLLOW` keeps roughly this spacing, and the port's own
/// `FOLLOW_RANGE` for GM-controlled mobs uses the same figure.
const FOLLOW_RANGE: f64 = 150.0;

/// Java `SummonAI.onIntentionActive` → `setIntention(AI_INTENTION_FOLLOW,
/// owner)`: an idle servitor trails its owner.
///
/// Run from the NPC AI tick. A servitor with an attack target is left alone —
/// the ordinary NPC attack think drives it from that point, exactly as it does
/// for a mob, because "attack whoever is on the aggro list" is the same
/// behaviour once the owner's order has seeded that list.
pub(crate) fn servitor_follow_tick(world: &mut World, servitor_oid: i32) {
    let Some(link) = world
        .objects
        .get_component::<ServitorOf>(&servitor_oid)
        .copied()
    else {
        return;
    };
    if !link.following {
        return;
    }
    // Busy attacking? Leave it to the attack think.
    if world
        .objects
        .get_component::<crate::model::npc::NpcAi>(&servitor_oid)
        .is_some_and(|ai| ai.intention == crate::model::npc::NpcIntention::Attack)
    {
        return;
    }
    let (Some(owner), Some(me)) = (
        world
            .objects
            .get_component::<Position>(&link.owner_object_id)
            .copied(),
        maybe_position(world, servitor_oid),
    ) else {
        return;
    };
    let dx = (owner.x - me.x) as f64;
    let dy = (owner.y - me.y) as f64;
    if (dx * dx + dy * dy).sqrt() <= FOLLOW_RANGE {
        return;
    }
    crate::game_loop::ai::move_npc_to(world, servitor_oid, owner.x, owner.y, owner.z);
}

/// The owner orders are **summon-scoped, not servitor-scoped.** A pet carries
/// the same [`ServitorOf`] link as a skill-summoned servitor (see its doc:
/// "owned summon is the same relationship whether it came from a skill or a
/// collar"), so `PetAttack`/`PetStop`/`PetHold` and their `Servitor*` twins are
/// the same three primitives pointed at a different id. The pair of thin
/// wrappers under each one is what picks the id; the guards that *do* differ —
/// a pet can starve, a servitor cannot — stay up in `player_actions`.
///
/// `Summon.doAttack` via the owner's target.
///
/// Java bails to `AI_INTENTION_FOLLOW` when the target is more than 3000 units
/// off, so a stray click doesn't send the summon across the map. Otherwise it
/// seeds hate and switches the AI to attack, the same primitive `GetAgro` and
/// `Confuse` use — the ported NPC AI derives its target from the aggro list
/// each think rather than caching one.
///
/// The range is measured from the **owner**, not the summon
/// (`player.calculateDistance3D(target)` in both handlers).
pub(crate) fn summon_attack(world: &mut World, summon_oid: i32, target_oid: i32) -> bool {
    let Some(owner_oid) = world
        .objects
        .get_component::<ServitorOf>(&summon_oid)
        .map(|l| l.owner_object_id)
    else {
        return false;
    };
    let Some(distance) = crate::geo::distance::distance_3d(world, owner_oid, target_oid) else {
        return false;
    };
    if distance > 3000.0 {
        // Too far — Java falls back to following rather than obeying.
        if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&summon_oid) {
            l.following = true;
        }
        return false;
    }
    // An ordered attack stops the follow, or the summon would drift home
    // between swings.
    if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&summon_oid) {
        l.following = false;
    }
    force_attack_target(world, summon_oid, target_oid);
    true
}

/// `Summon.cancelAction()`: drop the target, stop moving, and go back to
/// trailing the owner.
pub(crate) fn summon_stop(world: &mut World, summon_oid: i32) -> bool {
    crate::game_loop::ai::clear_aggro(world, summon_oid);
    world
        .objects
        .remove_component::<crate::model::components::Movement>(&summon_oid);
    crate::game_loop::helpers::set_active_intention(world, summon_oid);
    if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&summon_oid) {
        l.following = true;
    }
    true
}

/// `SummonAI.notifyFollowStatusChange()` — toggle "follow me" / "hold your
/// ground". Returns the new follow state.
pub(crate) fn summon_toggle_follow(world: &mut World, summon_oid: i32) -> Option<bool> {
    let l = world.objects.get_component_mut::<ServitorOf>(&summon_oid)?;
    l.following = !l.following;
    let now = l.following;
    if !now {
        // Holding ground: stop where you are.
        world
            .objects
            .remove_component::<crate::model::components::Movement>(&summon_oid);
    }
    Some(now)
}

/// `PetMove` / `ServitorMove` — walk to where the owner's target is standing.
///
/// Java drops follow first and sets `AI_INTENTION_MOVE_TO` on the *target's
/// location*, not on the target itself: the summon walks to a spot and stops,
/// rather than chasing. Refused when the summon is the target, when it cannot
/// move, and (for a servitor) when it is betrayed — all checked by the caller.
pub(crate) fn summon_move_to_target(world: &mut World, summon_oid: i32, target_oid: i32) -> bool {
    let Some(dest) = world
        .objects
        .get_component::<Position>(&target_oid)
        .copied()
    else {
        return false;
    };
    if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&summon_oid) {
        l.following = false;
    }
    crate::game_loop::ai::move_npc_to(world, summon_oid, dest.x, dest.y, dest.z);
    true
}

/// `ServitorAttack` (action 22) — the servitor half of [`summon_attack`].
pub(crate) fn servitor_attack(world: &mut World, owner_oid: i32, target_oid: i32) -> bool {
    let Some(servitor_oid) = servitor_of(world, owner_oid) else {
        return false;
    };
    summon_attack(world, servitor_oid, target_oid)
}

/// `ServitorStop` (action 23) — the servitor half of [`summon_stop`].
pub(crate) fn servitor_stop(world: &mut World, owner_oid: i32) -> bool {
    let Some(servitor_oid) = servitor_of(world, owner_oid) else {
        return false;
    };
    summon_stop(world, servitor_oid)
}

/// `ServitorHold` (action 21) — the servitor half of [`summon_toggle_follow`].
pub(crate) fn servitor_toggle_follow(world: &mut World, owner_oid: i32) -> Option<bool> {
    let servitor_oid = servitor_of(world, owner_oid)?;
    summon_toggle_follow(world, servitor_oid)
}

/// `ServitorHold` / `ServitorAttack` / `ServitorStop` / `ServitorSkillUse` —
/// the four `handlers/playeractions/*` entries that order a summon.
///
/// Routed here by handler **name** from `game_loop::player_actions`, which owns
/// `RequestActionUse` and its guards; this function is only the servitor half.
/// It used to *be* the packet handler, with an allow-list of ids that silently
/// dropped every other action in the file — see that module's header.
///
/// All four share the same two refusals, which is why they share an entry
/// point: no servitor at all, and a servitor under Betray (1380) that obeys
/// nothing.
pub(crate) fn handle_servitor_action(
    world: &mut World,
    client_id: u32,
    owner_oid: i32,
    handler: &str,
    option: i32,
) {
    use crate::network::server_packets::sm_ids;

    // Every handler opens with the same "do you even have one" check.
    if servitor_of(world, owner_oid).is_none() {
        send_to_client(
            world,
            client_id,
            server_packets::system_message_with(sm_ids::YOU_DO_NOT_HAVE_A_SERVITOR, &[]),
        );
        return;
    }
    // `Summon.canAttack`'s `isBetrayed()` gate — a servitor under Betray
    // (1380) obeys nothing at all, and says so.
    if let Some(servitor) = servitor_of(world, owner_oid)
        && crate::game_loop::abnormal::flags_of(world, servitor)
            & crate::model::skill::effect_flag::BETRAYED
            != 0
    {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::YOUR_SERVITOR_IS_UNRESPONSIVE_AND_WILL_NOT_OBEY_ANY_ORDERS,
            &[],
        );
        return;
    }
    match handler {
        // `option` is the skill id the row binds; the level comes from what
        // this particular servitor knows.
        "ServitorSkillUse" => use_servitor_skill(world, owner_oid, option),
        "ServitorAttack" => {
            // `player.getTarget()` — no target, nothing to order.
            let Some(target_oid) = world
                .objects
                .get_component::<crate::model::components::TargetRef>(&owner_oid)
                .and_then(|t| t.0)
            else {
                return;
            };
            servitor_attack(world, owner_oid, target_oid);
        }
        "ServitorStop" => {
            servitor_stop(world, owner_oid);
        }
        // `ServitorMove` — walk to the owner's target. Java skips a summon
        // that *is* the target and one that cannot move.
        "ServitorMove" => {
            let Some(servitor_oid) = servitor_of(world, owner_oid) else {
                return;
            };
            let Some(target_oid) = target_of(world, owner_oid) else {
                return;
            };
            if target_oid == servitor_oid
                || crate::game_loop::abnormal::is_movement_disabled(world, servitor_oid)
            {
                return;
            }
            summon_move_to_target(world, servitor_oid, target_oid);
        }
        // `ServitorMode` — option 1 passive, option 2 defending
        // (`SummonAI.setDefending`). What the flag then does is in
        // `combat::damage::npc_receive_damage`.
        "ServitorMode" => {
            let Some(servitor_oid) = servitor_of(world, owner_oid) else {
                return;
            };
            let defending = match option {
                1 => false,
                2 => true,
                // Java's switch has no default: an unknown option changes
                // nothing.
                _ => return,
            };
            if let Some(l) = world.objects.get_component_mut::<ServitorOf>(&servitor_oid) {
                l.defending = defending;
            }
        }
        // `UnsummonServitor` — "Removes shackles from the Servitor and sets him
        // free."
        "UnsummonServitor" => {
            let Some(servitor_oid) = servitor_of(world, owner_oid) else {
                return;
            };
            if is_engaged(world, servitor_oid) {
                send_sm_and_action_failed(
                    world,
                    client_id,
                    sm_ids::A_SERVITOR_WHOM_IS_ENGAGED_IN_BATTLE_CANNOT_BE_DE_ACTIVATED,
                    &[],
                );
                return;
            }
            unsummon_servitor(world, owner_oid);
        }
        "ServitorHold" => {
            // `notifyFollowStatusChange()`.
            servitor_toggle_follow(world, owner_oid);
        }
        // Explicit rather than a catch-all: `player_actions` routes by handler
        // name, and a name added there but not here must be as visible as one
        // with no arm at all — silently toggling follow is the one outcome
        // that would look like it worked.
        other => tracing::warn!("servitor action: no arm for handler {other}"),
    }
}

/// `player.getTarget()`.
fn target_of(world: &World, owner_oid: i32) -> Option<i32> {
    world
        .objects
        .get_component::<crate::model::components::TargetRef>(&owner_oid)
        .and_then(|t| t.0)
}

/// Java's shared "busy fighting" test for the two unsummon handlers:
/// `isAttackingNow() || isInCombat() || isMovementDisabled()`.
fn is_engaged(world: &World, summon_oid: i32) -> bool {
    let mid_swing = world
        .objects
        .get_component::<crate::model::components::AttackState>(&summon_oid)
        .is_some_and(|st| st.attack_end_tick > world.tick);
    mid_swing
        || crate::game_loop::user_commands::in_combat(world, summon_oid)
        || crate::game_loop::abnormal::is_movement_disabled(world, summon_oid)
}

/// `PetHold` / `PetAttack` / `PetStop` / `PetMove` / `UnsummonPet` /
/// `PetSkillUse` — the pet window's orders.
///
/// The servitor twin is [`handle_servitor_action`]; the primitives underneath
/// are shared (see [`summon_attack`]) and only the *guards* differ. A pet can
/// starve, which is what `isUncontrollable()` reads — a hunger gauge at 0 makes
/// it ignore its owner — and `UnsummonPet` additionally refuses on death,
/// combat and hunger, because putting a pet back in its collar persists it.
pub(crate) fn handle_pet_action(
    world: &mut World,
    client_id: u32,
    owner_oid: i32,
    handler: &str,
    option: i32,
) {
    use crate::network::server_packets::sm_ids;

    // `player.getPet() == null || !player.getPet().isPet()`. A skill-summoned
    // servitor is not a pet, and these buttons do not reach it.
    let Some(pet_oid) = pet_of(world, owner_oid) else {
        send_sm_and_action_failed(world, client_id, sm_ids::YOU_DO_NOT_HAVE_A_PET, &[]);
        return;
    };
    // `Pet.isUncontrollable()` — the hunger gauge is at 0.
    if is_uncontrollable(world, pet_oid) {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::WHEN_YOUR_PETS_HUNGER_GAUGE_IS_AT_0_YOU_CANNOT_USE_YOUR_PET,
            &[],
        );
        return;
    }
    let betrayed = crate::game_loop::abnormal::flags_of(world, pet_oid)
        & crate::model::skill::effect_flag::BETRAYED
        != 0;
    if betrayed {
        // Java's `UnsummonPet` sends the *hunger* message on this branch — a
        // copy-paste slip in the reference, ported as-is rather than corrected,
        // because a client that sees a different string here has diverged from
        // the server it is meant to match. Every other pet handler sends the
        // unresponsive one.
        let msg = if handler == "UnsummonPet" {
            sm_ids::WHEN_YOUR_PETS_HUNGER_GAUGE_IS_AT_0_YOU_CANNOT_USE_YOUR_PET
        } else {
            sm_ids::YOUR_SERVITOR_IS_UNRESPONSIVE_AND_WILL_NOT_OBEY_ANY_ORDERS
        };
        send_sm_and_action_failed(world, client_id, msg, &[]);
        return;
    }

    match handler {
        "PetAttack" => {
            let Some(target_oid) = target_of(world, owner_oid) else {
                return;
            };
            summon_attack(world, pet_oid, target_oid);
        }
        "PetStop" => {
            summon_stop(world, pet_oid);
        }
        "PetMove" => {
            let Some(target_oid) = target_of(world, owner_oid) else {
                return;
            };
            if target_oid == pet_oid
                || crate::game_loop::abnormal::is_movement_disabled(world, pet_oid)
            {
                return;
            }
            summon_move_to_target(world, pet_oid, target_oid);
        }
        "UnsummonPet" => unsummon_pet(world, client_id, owner_oid, pet_oid),
        "PetSkillUse" => use_pet_skill(world, client_id, owner_oid, pet_oid, option),
        "PetHold" => {
            // `notifyFollowStatusChange()`.
            summon_toggle_follow(world, pet_oid);
        }
        other => tracing::warn!("pet action: no arm for handler {other}"),
    }
}

/// `UnsummonPet.useAction`'s tail — the three refusals past the shared guards,
/// then `Pet.unSummon`.
fn unsummon_pet(world: &mut World, client_id: u32, owner_oid: i32, pet_oid: i32) {
    use crate::network::server_packets::sm_ids;
    if is_dead(world, pet_oid) {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::DEAD_PETS_CANNOT_BE_RETURNED_TO_THEIR_SUMMONING_ITEM,
            &[],
        );
        return;
    }
    if is_engaged(world, pet_oid) {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::A_PET_CANNOT_BE_UNSUMMONED_DURING_BATTLE,
            &[],
        );
        return;
    }
    if is_hungry(world, pet_oid) {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::YOU_MAY_NOT_RESTORE_A_HUNGRY_PET,
            &[],
        );
        return;
    }
    // `Pet.unSummon` stores the pet before the entity goes away — without this
    // the session's hp/exp/fed deltas are lost, the same trap the `/mount` path
    // documents.
    sync_pet_row(world, owner_oid);
    unsummon_servitor(world, owner_oid);
}

/// `handlers/playeractions/PetSkillUse` — the owner presses one of the pet's
/// action-bar buttons and the **pet** casts it.
///
/// Unlike `ServitorSkillUse`, the level is not the one the NPC template
/// declares: a pet levels independently of its species, so `PetData` resolves
/// `(skillId, petLevel) → level` (see
/// [`PetTemplate::available_level`](crate::data::pet_data::PetTemplate::available_level)).
/// Level 0 back means this is not this pet's skill, and Java casts nothing.
fn use_pet_skill(world: &mut World, client_id: u32, owner_oid: i32, pet_oid: i32, skill_id: i32) {
    use crate::network::server_packets::sm_ids;

    // Java checks the target *first*, before it even looks at the pet.
    let Some(owner_target) = target_of(world, owner_oid) else {
        return;
    };
    let (pet_level, owner_level) = (
        world
            .objects
            .get_component::<crate::model::components::PetOf>(&pet_oid)
            .map(|p| p.level)
            .unwrap_or(0),
        world
            .objects
            .get_component::<crate::model::Player>(&owner_oid)
            .map(|p| p.level)
            .unwrap_or(0),
    );
    // "Your pet is too high level to control." — a pet more than 20 levels
    // above its owner stops taking orders.
    if pet_level - owner_level > 20 {
        send_sm_and_action_failed(
            world,
            client_id,
            sm_ids::YOUR_PET_IS_TOO_HIGH_LEVEL_TO_CONTROL,
            &[],
        );
        return;
    }

    let max_skill_level = world.data.skill_data.max_level(skill_id);
    let level = npc_template_id(world, pet_oid)
        .and_then(|id| world.data.pet_data.get(id))
        .map(|t| t.available_level(skill_id, pet_level, max_skill_level))
        .unwrap_or(0);
    if level > 0
        && let Some(skill) = skill_by_id(world, skill_id, level)
    {
        // `pet.setTarget(player.getTarget())` then `useMagic` — a self-targeted
        // skill still resolves onto the pet, exactly as the servitor path does.
        let target_oid = if matches!(
            skill.target_type,
            crate::model::skill::TargetType::Self_ | crate::model::skill::TargetType::None_
        ) {
            pet_oid
        } else if skill.target_type == crate::model::skill::TargetType::OwnerPet {
            owner_oid
        } else {
            owner_target
        };
        crate::game_loop::npc::cast::cast_checked(world, pet_oid, target_oid, &skill);
    }

    // SKIP(census): `if (optionId == PET_SWITCH_STANCE) pet.switchMode()`.
    // Skill 6054 is declared only by npcs 1601–1603 (the "Super … Z" cats),
    // which no skill on this dist summons and no collar produces — the stance
    // toggle has no carrier here.
}

/// `handlers/actionhandlers/ServitorSkillUse` — the owner presses one of the
/// summon's action-bar buttons and the **servitor** casts it.
///
/// The skill must be one the servitor actually knows: the bindings in
/// `ActionData.xml` cover every summon in the game, so most of the 105 rows
/// name a skill this particular servitor has never had. Casting one anyway
/// would let any summon use any other summon's abilities.
///
/// The cast itself goes through `npc_cast::start_cast`, the same path the AI
/// uses, so an ordered skill obeys the same MP cost, mute gates and cooldowns
/// as one the servitor chose itself.
pub(crate) fn use_servitor_skill(world: &mut World, owner_oid: i32, skill_id: i32) {
    use crate::network::server_packets::sm_ids;
    let Some(servitor_oid) = servitor_of(world, owner_oid) else {
        return;
    };

    let known_level = npc_template_id(world, servitor_oid)
        .and_then(|id| world.data.npc_data.get(id))
        .and_then(|t| {
            t.skill_list
                .iter()
                .find(|(sid, _)| *sid == skill_id)
                .map(|(_, lvl)| *lvl)
        });
    let Some(level) = known_level else {
        // Not this summon's skill — Java's handler simply finds nothing to
        // cast. Silent, as it is: the client only shows buttons the summon has.
        return;
    };
    let Some(skill) = skill_by_id(world, skill_id, level) else {
        return;
    };

    // A self/support skill targets the servitor; anything else needs the
    // owner's current target, exactly like the attack command.
    //
    // `OWNER_PET` is the exception Java writes out by hand ahead of target
    // resolution (`Summon.useMagic`: `if (targetType == OWNER_PET) target =
    // _owner`) — the skill aims at the owner whatever they have selected.
    // Master Recharge (4025) is the carrier: without this branch a Baby
    // Kookaburra recharged whatever mob its owner had clicked, and refused
    // with "invalid target" when they had clicked nothing at all.
    let target_oid = if matches!(
        skill.target_type,
        crate::model::skill::TargetType::Self_ | crate::model::skill::TargetType::None_
    ) {
        servitor_oid
    } else if skill.target_type == crate::model::skill::TargetType::OwnerPet {
        owner_oid
    } else {
        match world
            .objects
            .get_component::<crate::model::components::TargetRef>(&owner_oid)
            .and_then(|t| t.0)
        {
            Some(t) => t,
            None => {
                send_sm_bare_to_player(world, owner_oid, sm_ids::INVALID_TARGET);
                return;
            }
        }
    };

    crate::game_loop::npc::cast::cast_checked(world, servitor_oid, target_oid, &skill);
}
