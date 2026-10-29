//! Java `sendDamageMessage` — the attacker's side of a hit, sent from the end
//! of `Creature.reduceCurrentHp` with the amount it landed (after the damage
//! cap and the damage-taken stats), once the HP cut and any death are done.
//! The `Hp` and `HealPercent` damage branches send it a second time.
//!
//! Each attacker kind has its own override — `Player`, `Summon` (to its owner)
//! and `Cubic` (to its owner); every other creature's is empty.
//!
//! Java's `miss` argument is `false` at every Salvation caller, so the miss
//! branches (`C1_S_ATTACK_WENT_ASTRAY`, `C1_HAS_EVADED_C2_S_ATTACK`, the
//! `EVADED` popup) never run and are not ported. The `DUELIST_FURY` term of
//! the blocked check is always false here: the `DuelistFury` effect is on no
//! skill in this datapack.
//!
//! `OlympiadGameManager.notifyCompetitorDamage` is not ported yet: the port's
//! matches keep no per-competitor damage tally to feed (docs/PLAN_SALVATION.md).

use super::DamageHit;
use crate::game_loop::{helpers, npc, servitor};
use crate::model::components::player::AdminFlags;
use crate::model::components::summons::{CubicOf, PetOf, ServitorOf};
use crate::model::npc::Npc;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::world::World;

/// `attacker.sendDamageMessage(target, skill, (int) amount, critical, false)`.
pub(crate) fn send_damage_message(
    world: &World,
    attacker: i32,
    target: i32,
    damage: i32,
    hit: DamageHit,
) {
    if world
        .objects
        .has_component::<crate::model::Player>(&attacker)
    {
        player_damage_message(world, attacker, target, damage, hit);
    } else if let Some(owner) = servitor::owner_of(world, attacker) {
        summon_damage_message(world, attacker, owner, target, damage, hit);
    } else if let Some(owner) = world
        .objects
        .get_component::<CubicOf>(&attacker)
        .map(|c| c.owner_object_id)
    {
        cubic_damage_message(world, owner, target, damage);
    }
}

/// `Player.sendDamageMessage`.
fn player_damage_message(world: &World, attacker: i32, target: i32, damage: i32, hit: DamageHit) {
    let Some(client_id) = helpers::client_for_player(world, attacker) else {
        return;
    };
    let name = helpers::player_name_or_empty(world, attacker);
    if hit.critical {
        if !hit.skill || !hit.magic {
            helpers::send_sm_to_client(
                world,
                client_id,
                sm_ids::C1_LANDED_A_CRITICAL_HIT,
                &[SmParam::PlayerName(name.clone())],
            );
        } else {
            helpers::send_sm_bare_to_client(world, client_id, sm_ids::M_CRITICAL);
        }
        if hit.skill {
            helpers::send_to_client(
                world,
                client_id,
                server_packets::ex_magic_attack_info(
                    attacker,
                    target,
                    server_packets::magic_attack_info::CRITICAL,
                ),
            );
        }
    }
    // `(target.isHpBlocked() && !target.isNpc()) || target.isInvul()`.
    if (is_hp_blocked(world, target) && !is_npc(world, target)) || is_invul(world, target) {
        helpers::send_sm_bare_to_client(world, client_id, sm_ids::THE_ATTACK_HAS_BEEN_BLOCKED);
    } else if is_door_or_control_tower(world, target) {
        helpers::send_sm_to_client(
            world,
            client_id,
            sm_ids::YOU_HIT_FOR_S1_DAMAGE,
            &[SmParam::Int(damage)],
        );
    } else if attacker != target {
        helpers::send_sm_to_client(
            world,
            client_id,
            sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
            &[
                SmParam::PlayerName(name),
                SmParam::Text(target_name(world, target)),
                SmParam::Int(damage),
                SmParam::Popup {
                    target,
                    attacker,
                    damage: -damage,
                },
            ],
        );
    }
}

/// `Summon.sendDamageMessage` — to the owner, and not at all when the summon
/// hit its own owner ("Prevents the double spam of system messages").
fn summon_damage_message(
    world: &World,
    summon: i32,
    owner: i32,
    target: i32,
    damage: i32,
    hit: DamageHit,
) {
    if target == owner {
        return;
    }
    let Some(client_id) = helpers::client_for_player(world, owner) else {
        return;
    };
    if hit.critical {
        let message = if world.objects.has_component::<PetOf>(&summon) {
            sm_ids::PET_S_CRITICAL_HIT
        } else {
            sm_ids::SUMMONED_MONSTER_S_CRITICAL_HIT
        };
        helpers::send_sm_bare_to_client(world, client_id, message);
    }
    if is_hp_blocked(world, target) && !is_npc(world, target) {
        helpers::send_sm_bare_to_client(world, client_id, sm_ids::THE_ATTACK_HAS_BEEN_BLOCKED);
    } else {
        helpers::send_sm_to_client(
            world,
            client_id,
            sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
            &[
                SmParam::NpcName(npc::npc_id_of(world, summon).unwrap_or(0)),
                SmParam::Text(target_name(world, target)),
                SmParam::Int(damage),
                SmParam::Popup {
                    target,
                    attacker: summon,
                    damage: -damage,
                },
            ],
        );
    }
}

/// `Cubic.sendDamageMessage` — to the owner, under the owner's name
/// (`Cubic.getName()` returns it).
fn cubic_damage_message(world: &World, owner: i32, target: i32, damage: i32) {
    let Some(client_id) = helpers::client_for_player(world, owner) else {
        return;
    };
    if is_hp_blocked(world, target) && !is_npc(world, target) {
        helpers::send_sm_bare_to_client(world, client_id, sm_ids::THE_ATTACK_HAS_BEEN_BLOCKED);
    } else {
        helpers::send_sm_to_client(
            world,
            client_id,
            sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
            &[
                SmParam::Text(helpers::player_name_or_empty(world, owner)),
                SmParam::Text(target_name(world, target)),
                SmParam::Int(damage),
                SmParam::Popup {
                    target,
                    attacker: owner,
                    damage: -damage,
                },
            ],
        );
    }
}

/// Java `Creature.isHpBlocked()`: `isInvul() || isAffected(HP_BLOCK)`.
fn is_hp_blocked(world: &World, oid: i32) -> bool {
    crate::game_loop::abnormal::is_hp_blocked(world, oid) || is_invul(world, oid)
}

/// Java `Creature.isInvul()`: the invul flag or a teleport in progress, and for
/// a summon its owner's spawn protection (`Summon.isInvul`).
fn is_invul(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<AdminFlags>(&oid)
        .is_some_and(|f| f.invul)
        || world
            .objects
            .get_component::<crate::model::Player>(&oid)
            .is_some_and(|p| p.teleporting)
        || servitor::owner_of(world, oid)
            .is_some_and(|owner| super::spawn_protection::is_protected(world, owner))
}

/// Java `isNpc()`: an `Npc` — not a summon (a `Playable`) and not a door.
fn is_npc(world: &World, oid: i32) -> bool {
    world.objects.has_component::<Npc>(&oid) && !world.objects.has_component::<ServitorOf>(&oid)
}

fn is_door_or_control_tower(world: &World, oid: i32) -> bool {
    world
        .objects
        .has_component::<crate::model::door::Door>(&oid)
        || npc::npc_template(world, oid).is_some_and(|t| t.type_name == "ControlTower")
}

/// `target.getName()`, which every override sends as a plain string.
fn target_name(world: &World, oid: i32) -> String {
    crate::game_loop::skills::effects::creature_name(world, oid)
}
