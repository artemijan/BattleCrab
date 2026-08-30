//! Life-state commands — `AdminHeal`/`AdminRes`, the `//set_hp`/`//set_mp`/
//! `//set_cp` EditChar vitals, and `AdminKill`. All operate on the current
//! target (or the GM) and push the resulting `StatusUpdate`.

use crate::game_loop::admin::{find_online_player, target_player};
use crate::game_loop::combat::target;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::helpers::{nth_arg, send_message, send_sm_bare_to_client};
use crate::model::Player;
use crate::model::components::{PlayerVitals, Vitals};
use crate::model::npc::Npc;
use crate::network::server_packets::{self, status_update_type as sut};
use crate::world::World;

/// `AdminHeal`'s `//heal [name|radius]` — port of `AdminHeal.handleHeal`. With no
/// argument, heal the current target (or the GM if nothing is selected). A
/// `<name>` heals that online player; a numeric `<radius>` heals every visible
/// creature (players *and* NPCs) within it. Healing sets HP/MP to max (and CP to
/// max for players) but does **not** revive — Java `setCurrentHpMp` leaves the
/// death state untouched. A non-creature target replies `INVALID_TARGET`.
pub(super) fn admin_heal(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let mut obj = target::current(world, object_id);
    if let Some(&arg) = args.first() {
        if let Some(named) = find_online_player(world, arg) {
            obj = Some(named);
        } else if let Ok(radius) = arg.parse::<i32>() {
            for oid in super::creatures_in_range(world, object_id, radius, true, true) {
                heal_creature(world, oid);
            }
            send_message(
                world,
                client_id,
                &format!("Healed within {radius} unit radius."),
            );
            return;
        }
        // A non-name, non-numeric argument falls through to the target/self.
    }
    let target = obj.unwrap_or(object_id);
    if world.objects.has_component::<Vitals>(&target) {
        heal_creature(world, target);
    } else {
        send_sm_bare_to_client(world, client_id, server_packets::sm_ids::INVALID_TARGET);
    }
}

/// Java `//heal` on one creature: HP/MP → max (CP → max for players), no revive.
/// StatusUpdate goes to the player (+ party), or is broadcast near an NPC.
/// Also the `_bbsheal` community-board action.
pub(crate) fn heal_creature(world: &mut World, target: i32) {
    let is_player = world.objects.has_component::<Player>(&target);
    let (max_hp, max_mp) = {
        let Some(v) = world.objects.get_component_mut::<Vitals>(&target) else {
            return;
        };
        v.cur_hp = v.max_hp as f64;
        v.cur_mp = v.max_mp as f64;
        (v.max_hp, v.max_mp)
    };
    let mut updates = vec![(sut::CUR_HP, max_hp), (sut::CUR_MP, max_mp)];
    if is_player && let Some(pv) = world.objects.get_component_mut::<PlayerVitals>(&target) {
        pv.cur_cp = pv.max_cp as f64;
        updates.push((sut::CUR_CP, pv.max_cp));
    }
    let packet = server_packets::status_update(target, &updates);
    if is_player {
        super::helpers::send_to_player(world, target, packet);
        super::party::notify_party_vitals(world, target);
    } else {
        super::helpers::broadcast_from(world, target, &packet);
    }
}

/// `AdminRes`'s `//res [name|radius]` — revive the targeted player (or self);
/// with a `<name>` argument the named online player; with a numeric argument
/// every player within that radius.
pub(super) fn admin_res(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    if let Some(arg) = args.first() {
        if let Some(named) = find_online_player(world, arg) {
            res_creature(world, named);
            return;
        }
        let Some(radius) = arg.parse::<i32>().ok() else {
            send_message(world, client_id, "Enter a valid player name or radius.");
            return;
        };
        for oid in super::creatures_in_range(world, object_id, radius, true, false) {
            res_creature(world, oid);
        }
        send_message(
            world,
            client_id,
            &format!("Resurrected all players within a {radius} unit radius."),
        );
        return;
    }
    let target = target::current_player(world, object_id).unwrap_or(object_id);
    res_creature(world, target);
}

/// `AdminRes`'s `//res_monster [radius]` — revive the targeted NPC, or every
/// non-player creature within `radius`.
pub(super) fn admin_res_monster(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    if let Some(radius) = nth_arg::<i32>(args, 0) {
        for oid in super::creatures_in_range(world, object_id, radius, false, true) {
            res_creature(world, oid);
        }
        send_message(
            world,
            client_id,
            &format!("Resurrected all non-players within a {radius} unit radius."),
        );
        return;
    }
    let Some(target) =
        target::current(world, object_id).filter(|oid| world.objects.has_component::<Npc>(oid))
    else {
        send_sm_bare_to_client(world, client_id, server_packets::sm_ids::INVALID_TARGET);
        return;
    };
    res_creature(world, target);
}

/// Java `AdminRes.doResurrect` — revive one dead creature. For a player: revive
/// + restore vitals (Java restores 100% of lost death-exp). For an NPC corpse:
///   cancel its pending decay (via the `!dead` guard) and revive it in place with
///   a `Revive` broadcast and refilled HP.
fn res_creature(world: &mut World, target: i32) {
    if !world
        .objects
        .get_component::<Vitals>(&target)
        .is_some_and(|v| v.dead)
    {
        return;
    }
    if world.objects.has_component::<Player>(&target) {
        super::death::do_revive(world, target);
        full_restore(world, target);
    } else if world.objects.has_component::<Npc>(&target)
        && let Some(region) = region_cell_of(world, target)
    {
        let max_hp = {
            let Some(v) = world.objects.get_component_mut::<Vitals>(&target) else {
                return;
            };
            v.dead = false;
            v.cur_hp = v.max_hp as f64;
            v.max_hp
        };
        super::helpers::broadcast_near_region(world, region, &server_packets::revive(target));
        super::helpers::broadcast_near_region(
            world,
            region,
            &server_packets::status_update(target, &[(sut::MAX_HP, max_hp), (sut::CUR_HP, max_hp)]),
        );
    }
}

/// Set a player's HP/MP/CP to full (clearing death) and push the resulting
/// `StatusUpdate` to that player + their party. Shared by `//heal` and `//res`.
fn full_restore(world: &mut World, target: i32) {
    let updates = {
        let Some((mut vitals, mut pvitals)) = world
            .objects
            .get_many_mut::<(&mut Vitals, &mut PlayerVitals)>(&target)
        else {
            return;
        };
        vitals.cur_hp = vitals.max_hp as f64;
        vitals.cur_mp = vitals.max_mp as f64;
        vitals.dead = false;
        pvitals.cur_cp = pvitals.max_cp as f64;
        [
            (sut::CUR_HP, vitals.max_hp),
            (sut::CUR_MP, vitals.max_mp),
            (sut::CUR_CP, pvitals.max_cp),
        ]
    };
    let packet = server_packets::status_update(target, &updates);
    super::helpers::send_to_player(world, target, packet);
    super::party::notify_party_vitals(world, target);
}

/// The vitals `//editchar` can set directly.
#[derive(Clone, Copy)]
pub(super) enum Vital {
    Hp,
    Mp,
    Cp,
}

/// `//set_hp`/`//set_mp`/`//set_cp <n>` — set a vital on the target player (or
/// self), clamped to its max, with a `StatusUpdate`.
pub(super) fn set_vital(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    vital: Vital,
    args: &[&str],
) {
    let Some(value) = nth_arg::<f64>(args, 0) else {
        send_message(world, client_id, "Usage: //set_hp <value>");
        return;
    };
    let target = target_player(world, object_id);
    let update = {
        match vital {
            Vital::Hp | Vital::Mp => {
                let Some(v) = world.objects.get_component_mut::<Vitals>(&target) else {
                    return;
                };
                match vital {
                    Vital::Hp => {
                        v.cur_hp = value.clamp(0.0, v.max_hp as f64);
                        v.dead = v.cur_hp < 0.5;
                        (sut::CUR_HP, v.cur_hp as i32)
                    }
                    _ => {
                        v.cur_mp = value.clamp(0.0, v.max_mp as f64);
                        (sut::CUR_MP, v.cur_mp as i32)
                    }
                }
            }
            Vital::Cp => {
                let Some(pv) = world.objects.get_component_mut::<PlayerVitals>(&target) else {
                    return;
                };
                pv.cur_cp = value.clamp(0.0, pv.max_cp as f64);
                (sut::CUR_CP, pv.cur_cp as i32)
            }
        }
    };
    let packet = server_packets::status_update(target, &[update]);
    super::helpers::send_to_player(world, target, packet);
    super::party::notify_party_vitals(world, target);
}

/// `AdminKill`'s `//kill [name|radius]` — kill the current target (player or
/// NPC), the named online player, or (numeric arg) every creature in radius. The
/// `monster` flavour (`//kill_monster`) restricts the radius/target to
/// non-players.
pub(super) fn admin_kill(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    args: &[&str],
    monster: bool,
) {
    if let Some(arg) = args.first() {
        // `//kill <name>` — a named online player (not for `//kill_monster`).
        if !monster && let Some(named) = find_online_player(world, arg) {
            // `//kill <name> <radius>` kills players around that player.
            if let Some(radius) = nth_arg::<i32>(args, 1) {
                for oid in super::creatures_in_range(world, named, radius, true, false) {
                    kill_creature(world, oid, object_id);
                }
                send_message(
                    world,
                    client_id,
                    &format!("Killed all characters within a {radius} unit radius."),
                );
                return;
            }
            kill_creature(world, named, object_id);
            return;
        }
        let Some(radius) = arg.parse::<i32>().ok() else {
            send_message(
                world,
                client_id,
                if monster {
                    "Usage: //kill_monster <radius>"
                } else {
                    "Usage: //kill <player_name | radius>"
                },
            );
            return;
        };
        for oid in super::creatures_in_range(world, object_id, radius, !monster, true) {
            kill_creature(world, oid, object_id);
        }
        send_message(
            world,
            client_id,
            &format!("Killed all characters within a {radius} unit radius."),
        );
        return;
    }
    let Some(target) = target::current(world, object_id) else {
        send_sm_bare_to_client(world, client_id, server_packets::sm_ids::INVALID_TARGET);
        return;
    };
    if monster && world.objects.has_component::<Player>(&target) {
        send_sm_bare_to_client(world, client_id, server_packets::sm_ids::INVALID_TARGET);
        return;
    }
    kill_creature(world, target, object_id);
}

/// Java `AdminKill.kill` — **deal lethal damage**, rather than call the death
/// path directly:
///
/// ```java
/// if (target.isPlayer())
/// {
///     if (!target.isGM()) target.stopAllEffects(); // e.g. invincibility effect
///     target.reduceCurrentHp(target.getMaxHp() + target.getMaxCp() + 1, activeChar, null);
/// }
/// else if (Config.CHAMPION_ENABLE && target.isChampion())
///     target.reduceCurrentHp((target.getMaxHp() * Config.CHAMPION_HP) + 1, activeChar, null);
/// else
/// {
///     // (invul is cleared around the blow and restored after)
///     target.reduceCurrentHp(target.getMaxHp() + 1, activeChar, null);
/// }
/// ```
///
/// **Going through the damage path is the whole point.** The reward split reads
/// the victim's `AggroList`, which only the damage path writes, so a kill that
/// registers no damage has no damage dealer: this used to call `npc_do_die`
/// straight and a GM got neither exp nor drops off it — the corpse of a mob
/// with a full drop table came up empty, which is exactly the case a GM runs
/// `//kill` to check. Found by driving the real server (`e2e_create::drop_check`).
///
/// Two of Java's clauses have no counterpart here and are left out rather than
/// faked: NPCs carry no invulnerability flag in this port (`AdminFlags` is a
/// player component), so there is nothing to clear and restore around the blow;
/// and a player's own `//invul` **is** honoured by the damage path, which is
/// what Java does too — its `reduceCurrentHp` refuses on `isInvul` and only the
/// non-player branch clears the flag first.
fn kill_creature(world: &mut World, target: i32, killer_oid: i32) {
    use crate::game_loop::combat::apply_physical_damage;

    if world.objects.has_component::<Player>(&target) {
        // `if (!target.isGM()) target.stopAllEffects();` — the invincibility
        // *buff* (not the GM flag) is the one Java names, and stripping it is
        // why the sweep comes before the blow.
        let target_is_gm = world
            .objects
            .get_component::<Player>(&target)
            .is_some_and(|p| p.is_gm(&world.data));
        if !target_is_gm {
            crate::game_loop::skills::effects::expire_buffs_where(world, target, |_, _| true);
        }
        let Some(max_hp) = world
            .objects
            .get_component::<Vitals>(&target)
            .map(|v| v.max_hp)
        else {
            return;
        };
        let max_cp = world
            .objects
            .get_component::<PlayerVitals>(&target)
            .map_or(0, |v| v.max_cp);
        let lethal = f64::from(max_hp) + f64::from(max_cp) + 1.0;
        apply_physical_damage(world, killer_oid, target, lethal, false, false);
    } else if world.objects.has_component::<Npc>(&target) {
        let Some(max_hp) = world
            .objects
            .get_component::<Vitals>(&target)
            .map(|v| v.max_hp)
        else {
            return;
        };
        let champion = world.cfg.champion.enable
            && world
                .objects
                .get_component::<Npc>(&target)
                .is_some_and(|n| n.champion);
        let lethal = if champion {
            f64::from(max_hp) * f64::from(world.cfg.champion.hp) + 1.0
        } else {
            f64::from(max_hp) + 1.0
        };
        apply_physical_damage(world, killer_oid, target, lethal, false, false);
    }
}
