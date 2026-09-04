//! Baby pets' auto-heal — port of
//! `dist/game/data/scripts/ai/areas/BeastFarm/BabyPets.java`.
//!
//! A Baby Buffalo, Kookaburra or Cougar watches its owner's health and heals
//! them without being asked. Every 5 s it rolls twice: a 25 % chance at Heal
//! Trick, which only fires below 80 % HP, and a 75 % chance at Greater Heal
//! Trick, which only fires below 15 %. The second is the emergency heal and is
//! the one players actually notice.
//!
//! Both skills auto-scale with the *pet's* level on the same curve
//! `PetData.getAvailableLevel` uses for a `skillLevel="0"` row — which is
//! exactly what these two are in the pets' XML, and why row 2's pet-skill
//! loader had to land before this could.
//!
//! Java hangs the timer on the **owner** (`startQuestTimer("HEAL", 5000, null,
//! owner, true)`) and cancels it on logout; this port keys it on the pet and
//! lets it lapse when the pet is gone, which needs no logout hook — the pet is
//! despawned before the player leaves in every path that removes either.

use crate::model::components::stats::Vitals;
use crate::model::components::summons::PetOf;
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// `BABY_PETS` — Baby Buffalo, Baby Kookaburra, Baby Cougar.
pub(crate) const BABY_PETS: &[i32] = &[12780, 12781, 12782];

/// `HEAL_1` / `HEAL_2`.
const HEAL_TRICK: i32 = 4717;
const GREATER_HEAL_TRICK: i32 = 4718;

/// `startQuestTimer("HEAL", 5000, …, repeating)`.
const HEAL_PERIOD_SECS: u64 = 5;

/// Java `getHealLv`: the pet's level on the standard auto-scaling curve,
/// clamped to the skills' twelve levels.
fn heal_level(pet_level: i32) -> i32 {
    let lvl = if pet_level < 70 {
        pet_level / 10
    } else {
        7 + ((pet_level - 70) / 5)
    };
    lvl.clamp(1, 12)
}

#[cfg(test)]
pub(crate) fn heal_level_for_test(pet_level: i32) -> i32 {
    heal_level(pet_level)
}

/// Whether this npc is one of the three baby pets — the `addSummonSpawnId` set.
pub(crate) fn is_baby_pet(npc_id: i32) -> bool {
    BABY_PETS.contains(&npc_id)
}

/// `onSummonSpawn` — arm the repeating heal for a baby pet that has just come
/// out. A no-op for every other summon.
pub(crate) fn on_summon_spawn(world: &mut World, pet_oid: i32) {
    let is_baby = crate::game_loop::npc::npc_id_of(world, pet_oid).is_some_and(is_baby_pet);
    if !is_baby {
        return;
    }
    schedule(world, pet_oid);
}

fn schedule(world: &mut World, pet_oid: i32) {
    world.scheduler.schedule(
        world.tick + HEAL_PERIOD_SECS * crate::game_loop::time::TICKS_PER_SECOND,
        ScheduledTask::BabyPetHealTick { pet_oid },
    );
}

/// The `HEAL` timer's body.
///
/// Java re-reads the pet off the owner each tick (`player.getPet()`) and
/// cancels the timer when it is gone; keying on the pet makes that the same
/// check, and letting the chain lapse is the cancellation.
pub(crate) fn handle_heal_tick(world: &mut World, pet_oid: i32) {
    let Some(link) = world.objects.get_component::<PetOf>(&pet_oid).copied() else {
        return; // pet gone — the chain ends here
    };
    let Some(owner_oid) = world
        .objects
        .get_component::<crate::model::components::summons::ServitorOf>(&pet_oid)
        .map(|s| s.owner_object_id)
    else {
        return;
    };
    if crate::game_loop::servitor::pet_of(world, owner_oid) != Some(pet_oid) {
        return; // no longer this owner's pet
    }

    let level = heal_level(link.level);
    // `if (getRandom(100) <= 25)` then `if (getRandom(100) <= 75)` — two
    // independent rolls each tick, and both may fire.
    if world.roll(100) <= 25 {
        cast_heal(world, pet_oid, owner_oid, HEAL_TRICK, level, 80.0);
    }
    if world.roll(100) <= 75 {
        cast_heal(world, pet_oid, owner_oid, GREATER_HEAL_TRICK, level, 15.0);
    }

    schedule(world, pet_oid);
}

/// Java `castHeal(summon, skill, maxHpPer)`.
///
/// The HP gate is a **percentage of the owner's maximum**, which is what makes
/// the two rolls different skills rather than the same one twice: Heal Trick
/// tops up anyone under 80 %, Greater Heal Trick is held back for under 15 %.
///
/// SKIP(census): Java saves and restores `getFollowStatus()` around the cast,
/// because `AI_INTENTION_CAST` clears it. The port's ordered cast goes through
/// `npc_cast` and never touches the follow flag, so there is nothing to
/// restore.
fn cast_heal(
    world: &mut World,
    pet_oid: i32,
    owner_oid: i32,
    skill_id: i32,
    level: i32,
    max_hp_percent: f64,
) {
    // `!owner.isDead()`.
    if crate::game_loop::helpers::is_dead(world, owner_oid) {
        return;
    }
    let Some(vitals) = world.objects.get_component::<Vitals>(&owner_oid) else {
        return;
    };
    if vitals.max_hp <= 0 {
        return;
    }
    let percent = (vitals.cur_hp / f64::from(vitals.max_hp)) * 100.0;
    if percent >= max_hp_percent {
        return;
    }
    // `!summon.isHungry()` — a starving pet does not heal.
    if crate::game_loop::servitor::is_hungry(world, pet_oid) {
        return;
    }
    let Some(skill) = crate::game_loop::skills::skill_by_id(world, skill_id, level) else {
        return;
    };
    // `SkillCaster.checkUseConditions` + the cast itself, through the same
    // path an ordered pet skill takes — so MP cost, mute and cooldowns apply.
    crate::game_loop::npc::cast::cast_checked(world, pet_oid, owner_oid, &skill);
    crate::game_loop::helpers::send_sm_to_player(
        world,
        owner_oid,
        crate::network::server_packets::sm_ids::YOUR_PET_USES_S1,
        &[crate::network::server_packets::SmParam::SkillName {
            id: skill_id,
            level,
        }],
    );
}
