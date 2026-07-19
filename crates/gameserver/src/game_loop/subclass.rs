//! Subclasses — `Player.addSubClass` / `Player.setActiveClass`.
//!
//! A character keeps up to `MaxSubclass` (5 here) extra classes, each with its
//! own level, exp, sp and learned skills. Slot 0 is the base class. Everything
//! the character *is* right now belongs to the active slot; switching writes
//! the current progress back into its slot and loads the target's.
//!
//! **Deliberately scoped to the gate** ("a subclass can be added and
//! switched"). Not here, each a `TODO(G17)` at the site: per-subclass hennas
//! and shortcuts (both still load with `class_index = 0`), certification
//! skills, the village-master UI flow (G22's occupation quests), and the
//! subclass-change lock Java holds across the swap.

use crate::model::{Player, SubClass};
use crate::world::World;

/// Java `Config.BASE_SUBCLASS_LEVEL` — a new subclass starts here.
pub(crate) const BASE_SUBCLASS_LEVEL: i32 = 40;

/// Why an add was refused, so callers can report it (Java returns a bare
/// `false` and the village master prints its own message).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AddError {
    /// `getTotalSubClasses() == Config.MAX_SUBCLASS`.
    SlotsFull,
    /// The class is already held, as base or as another subclass.
    AlreadyHave,
    /// Not a real class id.
    UnknownClass,
}

/// `Player.addSubClass` — take the lowest free slot.
///
/// Java takes an explicit `classIndex` from the village-master flow and
/// refuses index 0; picking the lowest free slot here is the same outcome for
/// every caller that exists, and keeps slot ids dense.
pub(crate) fn add_subclass(world: &mut World, player_oid: i32, class_id: i32) -> Result<i32, AddError> {
    if world.data.player_templates.get(class_id).is_none() {
        return Err(AddError::UnknownClass);
    }
    let max = world.cfg.character.max_subclass;

    let (base_class, held): (i32, Vec<i32>) = {
        let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
            return Err(AddError::UnknownClass);
        };
        (p.base_class_id, p.subclasses.iter().map(|s| s.class_id).collect())
    };
    if class_id == base_class || held.contains(&class_id) {
        return Err(AddError::AlreadyHave);
    }
    if held.len() as i32 >= max {
        return Err(AddError::SlotsFull);
    }

    // Lowest free index in 1..=max.
    let used: Vec<i32> = world
        .objects
        .get_component::<Player>(&player_oid)
        .map(|p| p.subclasses.iter().map(|s| s.class_index).collect())
        .unwrap_or_default();
    let Some(index) = (1..=max).find(|i| !used.contains(i)) else {
        return Err(AddError::SlotsFull);
    };

    let exp = world.data.experience.exp_for_level(BASE_SUBCLASS_LEVEL);
    let slot = SubClass { class_id, class_index: index, level: BASE_SUBCLASS_LEVEL, exp, sp: 0 };
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.subclasses.push(slot);
    }
    persist_slot(world, player_oid, slot);
    Ok(index)
}

/// `Player.setActiveClass` — swap to `class_index` (0 = base class).
///
/// Order matters and mirrors Java: bank the *current* slot's progress first
/// (Java calls `store()` before touching `_classIndex`, "to avoid skill
/// effects rollover"), then load the target, then rebuild the skill list.
pub(crate) fn set_active_class(world: &mut World, player_oid: i32, class_index: i32) -> bool {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else { return false };
    if p.class_index == class_index {
        return false;
    }
    // The target must exist: 0 is always the base class, anything else must be
    // a held slot.
    let target: Option<SubClass> = if class_index == 0 {
        None
    } else {
        match p.subclasses.iter().find(|s| s.class_index == class_index) {
            Some(s) => Some(*s),
            None => return false,
        }
    };
    let (base_class, cur_index, cur_class, cur_level, cur_exp, cur_sp) =
        (p.base_class_id, p.class_index, p.class_id, p.level, p.exp, p.sp);

    // A cast in flight would land against the old class's stats.
    super::skills::cast::stop_casting(world, player_oid);

    // 1. Bank the outgoing slot.
    if cur_index == 0 {
        // The base class's progress lives in the `characters` row, which the
        // ordinary player save already writes.
    } else {
        let banked =
            SubClass { class_id: cur_class, class_index: cur_index, level: cur_level, exp: cur_exp, sp: cur_sp };
        if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
            if let Some(slot) = p.subclasses.iter_mut().find(|s| s.class_index == cur_index) {
                *slot = banked;
            }
        }
        persist_slot(world, player_oid, banked);
    }

    // 2. Load the incoming slot.
    let (new_class, new_level, new_exp, new_sp) = match target {
        Some(s) => (s.class_id, s.level, s.exp, s.sp),
        None => {
            // Back to base: its progress is whatever the base row holds, which
            // is tracked separately from the subclass slots.
            let base = world
                .objects
                .get_component::<Player>(&player_oid)
                .map(|p| (p.base_level, p.base_exp, p.base_sp))
                .unwrap_or((1, 0, 0));
            (base_class, base.0, base.1, base.2)
        }
    };
    if cur_index == 0 {
        // Leaving the base class — stash its progress so returning restores it.
        if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
            p.base_level = cur_level;
            p.base_exp = cur_exp;
            p.base_sp = cur_sp;
        }
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.class_index = class_index;
        p.class_id = new_class;
        p.level = new_level;
        p.exp = new_exp;
        p.sp = new_sp;
    }

    // 3. Rebuild stats and the skill book for the new class. `set_level` is
    //    the same path `//setclass` uses: recompute HP/MP/stats, grant the
    //    class's reachable skills, and push the status/UserInfo/SkillList
    //    refresh. Java instead removes every skill then `restoreSkills` +
    //    `rewardSkills` from the DB.
    //    TODO(G17): persist `character_skills` per `class_index` so *manually*
    //    learned skills survive a switch — right now only the auto-granted
    //    tree is re-derived, so a hand-learned skill is lost on the round trip.
    super::death::set_level(world, player_oid, new_level);
    // `set_level` normalises exp to the level floor; restore the slot's own.
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.exp = new_exp;
        p.sp = new_sp;
    }
    super::party::broadcast_user_info(world, player_oid);
    true
}

fn persist_slot(world: &World, player_oid: i32, slot: SubClass) {
    let _ = world.db.send(crate::db::DbCommand::StoreSubClass {
        char_id: player_oid,
        class_id: slot.class_id,
        class_index: slot.class_index,
        level: slot.level,
        exp: slot.exp,
        sp: slot.sp,
    });
}
