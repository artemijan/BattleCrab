//! Baium (`ai/bosses/Baium`) — archangels and the strider debuff.
//!
//! Baium is the only one of the three great dragons' scripts with **no
//! cinematics at all** (`SpecialCamera` appears 19 times in Valakas and 7 in
//! Antharas, and zero here), which is why it is portable before the camera
//! packet is.
//!
//! # What is deliberately not here
//!
//! Baium's targeting is a **top-3 threat table** kept on NPC variables
//! (`c_quest0..2` / `i_quest0..2` in `refreshAiParams`), fed by a damage
//! weighting that shifts as he is worn down:
//!
//! | condition | weight |
//! |---|---|
//! | melee (`skill == null`) | `damage × 1000` |
//! | below 25% HP | `(damage / 3) × 100` |
//! | below 50% HP | `damage × 20` |
//! | below 75% HP | `damage × 10` |
//! | otherwise | `(damage / 3) × 20` |
//!
//! Melee threat is worth **fifty times** a caster's at full health, and the
//! caster weighting swings by a factor of ten across the HP bands. Folding that
//! into the port's ordinary aggro list would look like it worked and would not
//! be Baium — so it is left for its own slice rather than approximated.

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
