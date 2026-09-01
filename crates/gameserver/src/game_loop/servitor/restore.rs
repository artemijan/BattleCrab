//! Reconnect resummon: restoring the pet or servitor that was out when the
//! owner logged off.

use super::PetInfoKind;
use super::broadcast_summon_info;
use super::send_pet_info;
use super::servitor_of;
use super::summon_pet;
use crate::game_loop::skills::skill_by_id;
use crate::game_loop::time::TICKS_PER_SECOND;
use crate::model::components::ServitorOf;
use crate::model::components::Vitals;
use crate::world::World;
/// Java `CharSummonTable.restorePet` — bring back the pet that was out when the
/// owner logged off.
///
/// `RestorePetOnReconnect` is **True** on this dist, so this is the normal
/// path, not an opt-in. The saved row's `restore` flag is what marks a pet as
/// "was out"; a pet deliberately unsummoned before logout has it cleared, and
/// stays in its collar.
///
/// Called at enter-world, after the inventory exists — the collar has to be
/// found before the pet can be rebuilt from it.
pub(crate) fn restore_pet_on_login(world: &mut World, owner_oid: i32) {
    if !world.cfg.character.restore_pet_on_reconnect {
        return;
    }
    let collar = world
        .objects
        .get_component::<crate::model::components::PlayerPets>(&owner_oid)
        .and_then(|p| p.0.values().find(|r| r.restore).map(|r| r.collar_object_id));
    let Some(collar) = collar else { return };
    // The collar must still be there: it can have been traded or destroyed
    // between sessions, and `summon_pet` re-checks anyway — but setting the
    // holder for a collar that is gone would leave it dangling.
    let have_collar = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&owner_oid)
        .is_some_and(|inv| inv.items().iter().any(|i| i.object_id == collar));
    if !have_collar {
        return;
    }
    // Reuse the normal summon path rather than a parallel one, so a restored
    // pet is identical to a freshly summoned one — same stats, same feed clock,
    // same packets. It reads its state from the saved row exactly as it does
    // after a mid-session re-summon.
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&owner_oid)
    {
        p.pending_pet_collar = Some(collar);
    }
    summon_pet(world, owner_oid);
}

/// Capture the owner's live servitor into `PlayerSummons` (Java's
/// `character_summons` write). The pet counterpart is `sync_pet_row`; this runs
/// in the same place, before the summon leaves the world.
pub(crate) fn sync_summon_row(world: &mut World, owner_oid: i32) {
    if !world.cfg.character.restore_servitor_on_reconnect {
        return;
    }
    let Some(servitor_oid) = servitor_of(world, owner_oid) else {
        // Nothing out: clear any stale row, or a servitor dismissed before
        // logout would come back anyway.
        if let Some(s) = world
            .objects
            .get_component_mut::<crate::model::components::PlayerSummons>(&owner_oid)
        {
            s.0.clear();
        }
        return;
    };
    let Some(link) = world
        .objects
        .get_component::<ServitorOf>(&servitor_oid)
        .copied()
    else {
        return;
    };
    // A servitor summoned with no lifetime (`lifeTime <= 0` → `u64::MAX`) has
    // nothing to count down; store 0 and let the re-cast decide again.
    let remaining_secs = if link.expires_at_tick == u64::MAX {
        0
    } else {
        ((link.expires_at_tick.saturating_sub(world.tick)) / TICKS_PER_SECOND) as i32
    };
    let (cur_hp, cur_mp) = world
        .objects
        .get_component::<Vitals>(&servitor_oid)
        .map(|v| (v.cur_hp as i32, v.cur_mp as i32))
        .unwrap_or((0, 0));
    // The servitor's own buffs go with it — a Summoner's investment in
    // buffing their servitor should survive a relog, which is exactly why Java
    // keeps `character_summon_skills_save`.
    let now = world.tick;
    let buffs = world
        .objects
        .get_component::<crate::model::components::Buffs>(&servitor_oid)
        .map(|b| {
            b.0.iter()
                .filter(|buf| buf.expires_at_tick > now)
                .map(|buf| crate::db::SkillBuffRow {
                    skill_id: buf.skill_id,
                    skill_level: buf.skill_level,
                    remaining_time_secs: ((buf.expires_at_tick - now) / TICKS_PER_SECOND) as i32,
                })
                .collect()
        })
        .unwrap_or_default();
    let row = crate::db::SummonRow {
        summon_skill_id: link.reference_skill,
        cur_hp,
        cur_mp,
        remaining_secs,
        buffs,
    };
    if world
        .objects
        .get_component::<crate::model::components::PlayerSummons>(&owner_oid)
        .is_none()
    {
        world.objects.add_components(
            &owner_oid,
            crate::model::components::PlayerSummons::default(),
        );
    }
    if let Some(s) = world
        .objects
        .get_component_mut::<crate::model::components::PlayerSummons>(&owner_oid)
    {
        s.0.clear();
        s.0.push(row);
    }
}

/// Java `CharSummonTable.restoreServitor` — bring back the servitor that was
/// out when the owner logged off.
///
/// Java restores by **re-casting the summoning skill**
/// (`skill.applyEffects(player, player)`) and then stamping the saved vitals
/// and remaining lifetime onto the result. Doing the same here means a restored
/// servitor is built by the ordinary summon path, so it can never drift from a
/// freshly summoned one — and it comes back at the player's *current* level of
/// the skill, so levelling up between sessions is not punished.
pub(crate) fn restore_servitor_on_login(world: &mut World, owner_oid: i32) {
    if !world.cfg.character.restore_servitor_on_reconnect {
        return;
    }
    let Some(row) = world
        .objects
        .get_component::<crate::model::components::PlayerSummons>(&owner_oid)
        .and_then(|s| s.0.first().cloned())
    else {
        return;
    };
    // The row is consumed either way (Java `removeServitor` before the recast):
    // a skill the player no longer knows must not be retried every login.
    if let Some(s) = world
        .objects
        .get_component_mut::<crate::model::components::PlayerSummons>(&owner_oid)
    {
        s.0.clear();
    }
    let Some(level) = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&owner_oid)
        .and_then(|b| b.0.get(&row.summon_skill_id).copied())
    else {
        return; // unlearned across a subclass change — nothing to restore
    };
    let Some(skill) = skill_by_id(world, row.summon_skill_id, level) else {
        return;
    };
    crate::game_loop::skills::effects::apply_skill_effects(world, owner_oid, owner_oid, &skill);

    // Stamp the saved state back over the fresh summon.
    let Some(servitor_oid) = servitor_of(world, owner_oid) else {
        return;
    };
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&servitor_oid) {
        v.cur_hp = (row.cur_hp as f64).clamp(1.0, v.max_hp as f64);
        v.cur_mp = (row.cur_mp as f64).clamp(0.0, v.max_mp as f64);
    }
    if row.remaining_secs > 0 {
        let expires = world.tick + (row.remaining_secs as u64) * TICKS_PER_SECOND;
        if let Some(s) = world.objects.get_component_mut::<ServitorOf>(&servitor_oid) {
            s.expires_at_tick = expires;
        }
    }
    // Its buffs come back too, through the same path the player's own
    // persisted buffs use — relative remaining time, frozen while offline.
    if !row.buffs.is_empty() {
        crate::game_loop::skills::effects::restore_persisted_buffs(world, servitor_oid, &row.buffs);
    }
    send_pet_info(world, owner_oid, servitor_oid, PetInfoKind::Summoned);
    broadcast_summon_info(world, servitor_oid, true);
}
