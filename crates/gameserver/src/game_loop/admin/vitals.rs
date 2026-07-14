//! Life-state commands — `AdminHeal`/`AdminRes`, the `//set_hp`/`//set_mp`/
//! `//set_cp` EditChar vitals, and `AdminKill`. All operate on the current
//! target (or the GM) and push the resulting `StatusUpdate`.

use crate::model::components::{PlayerVitals, Vitals};
use crate::model::Player;
use crate::network::server_packets::{self, status_update_type as sut};
use crate::world::World;

use super::{current_target, send_message, target_player};

/// `AdminHeal` (first slice): fully restore the targeted player's HP/MP/CP, or
/// the GM's own if no *player* is targeted. NPC targets and the `<name>` form
/// are TODO (G13.B breadth).
pub(super) fn admin_heal(world: &mut World, object_id: i32) {
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    full_restore(world, target);
}

/// `AdminRes` (first slice): revive the targeted player (or self) and fully
/// restore them. `admin_res_monster` (NPC) is TODO.
pub(super) fn admin_res(world: &mut World, object_id: i32) {
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    if world.objects.get_component::<Vitals>(&target).is_some_and(|v| v.dead) {
        super::death::do_revive(world, target);
    }
    full_restore(world, target);
}

/// Set a player's HP/MP/CP to full (clearing death) and push the resulting
/// `StatusUpdate` to that player + their party. Shared by `//heal` and `//res`.
fn full_restore(world: &mut World, target: i32) {
    let updates = {
        let Some((mut vitals, mut pvitals)) =
            world.objects.get_many_mut::<(&mut Vitals, &mut PlayerVitals)>(&target)
        else {
            return;
        };
        vitals.cur_hp = vitals.max_hp as f64;
        vitals.cur_mp = vitals.max_mp as f64;
        vitals.dead = false;
        pvitals.cur_cp = pvitals.max_cp as f64;
        [(sut::CUR_HP, vitals.max_hp), (sut::CUR_MP, vitals.max_mp), (sut::CUR_CP, pvitals.max_cp)]
    };
    let packet = server_packets::status_update(target, &updates);
    if let Some(cid) = super::helpers::client_for_player(world, target) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(packet);
        }
    }
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
pub(super) fn set_vital(world: &mut World, client_id: u32, object_id: i32, vital: Vital, args: &[&str]) {
    let Some(value) = args.first().and_then(|s| s.parse::<f64>().ok()) else {
        send_message(world, client_id, "Usage: //set_hp <value>");
        return;
    };
    let target = target_player(world, object_id);
    let update = {
        match vital {
            Vital::Hp | Vital::Mp => {
                let Some(v) = world.objects.get_component_mut::<Vitals>(&target) else { return };
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
                let Some(pv) = world.objects.get_component_mut::<PlayerVitals>(&target) else { return };
                pv.cur_cp = value.clamp(0.0, pv.max_cp as f64);
                (sut::CUR_CP, pv.cur_cp as i32)
            }
        }
    };
    let packet = server_packets::status_update(target, &[update]);
    if let Some(cid) = super::helpers::client_for_player(world, target) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(packet);
        }
    }
    super::party::notify_party_vitals(world, target);
}

/// `AdminKill` (first slice): kill the current target (player or NPC) with the
/// GM as the killer. The `<name>` / radius forms are TODO (G13.B breadth).
pub(super) fn admin_kill(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) = current_target(world, object_id) else {
        send_message(world, client_id, "Select a target first.");
        return;
    };
    if world.objects.has_component::<Player>(&target) {
        super::death::player_do_die(world, target, object_id);
    } else if world.objects.has_component::<crate::model::npc::Npc>(&target) {
        super::death::npc_do_die(world, target, object_id);
    }
}
