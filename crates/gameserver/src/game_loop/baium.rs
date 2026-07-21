//! Baium (`ai/bosses/Baium`) — archangels and the strider debuff.
//!
//! Baium is the only one of the three great dragons' scripts with **no
//! cinematics at all** (`SpecialCamera` appears 19 times in Valakas and 7 in
//! Antharas, and zero here), which is why it is portable before the camera
//! packet is.
//!
//! His threat table is the shared `boss_threat` one — Antharas keeps an
//! identical copy in Java. Only the skill ladder below is Baium's own.

use crate::world::World;

pub const BAIUM: i32 = 29020;
/// Archangel — five of them circle Baium.
pub const ARCHANGEL: i32 = 29021;

/// `ANTI_STRIDER` (4258, "Hinder Strider").
const ANTI_STRIDER: i32 = 4258;
/// Java `MountType.STRIDER`.
const MOUNT_STRIDER: u8 = 1;

/// `ARCHANGEL_LOC` — five fixed points, with headings.
const ARCHANGEL_LOC: [(i32, i32, i32, i32); 5] = [
    (115_792, 16_608, 10_136, 0),
    (115_168, 17_200, 10_136, 0),
    (115_780, 15_564, 10_136, 13_620),
    (114_880, 16_236, 10_136, 5_400),
    (114_239, 17_168, 10_136, -1_992),
];

/// Baium spawned: bring out his five archangels.
///
/// Unlike Queen Ant's nurses these are **not** in a minion table — the script
/// places them, so nothing else would.
pub(crate) fn on_baium_spawned(world: &mut World) {
    for (x, y, z, heading) in ARCHANGEL_LOC {
        crate::model::npc::spawn_npc_at(world, ARCHANGEL, x, y, z, heading);
    }
}

/// `Baium.onAttack`'s strider clause: a strider-mounted attacker is hindered,
/// **once** — Java checks both `!isAffectedBySkill(4258)` and that the skill
/// is off cooldown, so it is not recast every swing.
/// `Baium.onAttack`'s threat half: weight the hit, then act on the table.
pub(crate) fn on_baium_damage(world: &mut World, baium_oid: i32, attacker_oid: i32, damage: i32, is_melee: bool) {
    super::boss_threat::on_boss_damage(world, baium_oid, attacker_oid, damage, is_melee);
    manage_and_cast(world, baium_oid);
}

pub(crate) fn on_baium_attacked(world: &mut World, baium_oid: i32, attacker_oid: i32) {
    let on_strider = world
        .objects
        .get_component::<crate::model::Player>(&attacker_oid)
        .is_some_and(|p| p.mount_type == MOUNT_STRIDER);
    if !on_strider {
        return;
    }
    let already = world
        .objects
        .get_component::<crate::model::components::Buffs>(&attacker_oid)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == ANTI_STRIDER));
    if already {
        return;
    }
    let Some(skill) = world.data.skill_data.get(ANTI_STRIDER, 1).cloned() else { return };
    if !crate::game_loop::npc_cast::check_use_conditions_pub(world, baium_oid, &skill) {
        return;
    }
    crate::game_loop::npc_cast::start_cast(world, baium_oid, attacker_oid, &skill);
}

// ---------------------------------------------------------------------------
// Skill selection (`manageSkills`)
// ---------------------------------------------------------------------------

/// Baium's five skills.
const BAIUM_ATTACK: i32 = 4127;
const ENERGY_WAVE: i32 = 4128;
const EARTH_QUAKE: i32 = 4129;
const THUNDERBOLT: i32 = 4130;
const GROUP_HOLD: i32 = 4131;

/// `manageSkills` — prune the table, pick the top threat, decay it, and choose
/// a skill for Baium's current health.
///
/// Returns `(target, skill_id)`, or `None` when there is nobody to act on.
pub(crate) fn manage_skills(world: &mut World, baium_oid: i32) -> Option<(i32, i32)> {
    let target = super::boss_threat::take_top_threat(world, baium_oid)?;
    let skill = choose_skill(world, baium_oid);
    Some((target, skill))
}

/// `onAttack`'s skill half: pick a target and skill, then actually cast.
///
/// **This is the half that was missing.** `manage_skills` has existed since the
/// threat slice and nothing ever called it, so Baium chose skills into the void
/// and only ever swung — the "parsed but unconsumed" shape, one level up: a
/// whole decision procedure with no caller. Casting on the boss himself is not
/// applicable here (all five of Baium's are target-cast).
pub(crate) fn manage_and_cast(world: &mut World, baium_oid: i32) {
    if world.objects.has_component::<crate::model::components::Casting>(&baium_oid) {
        return;
    }
    let Some((target, skill_id)) = manage_skills(world, baium_oid) else { return };
    super::boss_threat::cast_boss_skill(world, baium_oid, target, skill_id, false);
}

/// The skill ladder. Each 10% roll is taken **in order**, and the pool widens
/// as Baium weakens: two options above 75%, three above 50%, four above 25%,
/// and all four below — with `BAIUM_ATTACK` as the fallback throughout.
///
/// So a party sees Baium's repertoire grow as the fight goes on, which is the
/// same shape as his threat weighting.
fn choose_skill(world: &mut World, baium_oid: i32) -> i32 {
    let (cur, max) = match world.objects.get_component::<crate::model::components::Vitals>(&baium_oid) {
        Some(v) => (v.cur_hp, v.max_hp as f64),
        None => return BAIUM_ATTACK,
    };
    // Java's ladders share a tail, so the pool is expressed as the list of
    // skills tried before falling back — top band first.
    let pool: &[i32] = if cur > max * 0.75 {
        &[ENERGY_WAVE, EARTH_QUAKE]
    } else if cur > max * 0.5 {
        &[GROUP_HOLD, ENERGY_WAVE, EARTH_QUAKE]
    } else if cur > max * 0.25 {
        &[THUNDERBOLT, GROUP_HOLD, ENERGY_WAVE, EARTH_QUAKE]
    } else {
        &[THUNDERBOLT, GROUP_HOLD, ENERGY_WAVE, EARTH_QUAKE]
    };
    for skill in pool {
        if world.roll(100) < 10 {
            return *skill;
        }
    }
    BAIUM_ATTACK
}
