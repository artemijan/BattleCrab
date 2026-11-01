//! Subclasses — `Player.addSubClass` / `Player.setActiveClass`.
//!
//! A character keeps up to `MaxSubclass` (5 here) extra classes, each with its
//! own level, exp, sp and learned skills. Slot 0 is the base class. Everything
//! the character *is* right now belongs to the active slot; switching writes
//! the current progress back into its slot and loads the target's.
//!
//! Each slot keeps its own learned skills (`character_skills.class_index`), so
//! a hand-learned skill survives a switch away and back.
//!
//! Hennas and shortcuts are per class index end to end (banked in
//! `hennas_by_index`/`shortcuts_by_index` on the switch, loaded and stored
//! per index). [`modify_subclass`] is the slot wipe and replace. Not
//! modeled, with reasons: certification skills (no `subClassSkillTree`
//! exists anywhere in this dist's data — nothing to learn or wipe), and
//! Java's subclass-change lock (`_subclassLock`), which guards against two
//! threads swapping at once —
//! the port's single-threaded game loop cannot interleave two swaps.

use crate::game_loop::clans::clan_skills;
use crate::game_loop::net::broadcast;
use crate::game_loop::space::position::maybe_position;
use crate::game_loop::space::position::region_cell_of;
use crate::model::components::skills::SkillBook;
use crate::model::{Player, SubClass};
use crate::world::World;

/// Why an add was refused, so callers can report it (Java returns a bare
/// `false` and the caller prints its own message).
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
/// Java takes an explicit `classIndex` from its caller and
/// refuses index 0; picking the lowest free slot here is the same outcome for
/// every caller that exists, and keeps slot ids dense.
pub(crate) fn add_subclass(
    world: &mut World,
    player_oid: i32,
    class_id: i32,
) -> Result<i32, AddError> {
    if world.data.player_templates.get(class_id).is_none() {
        return Err(AddError::UnknownClass);
    }
    let max = world.cfg.character.max_subclass;

    let (base_class, held): (i32, Vec<i32>) = {
        let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
            return Err(AddError::UnknownClass);
        };
        (
            p.base_class_id,
            p.subclasses.iter().map(|s| s.class_id).collect(),
        )
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

    let exp = world
        .data
        .experience
        .exp_for_level(world.cfg.character.base_subclass_level);
    let slot = SubClass {
        class_id,
        class_index: index,
        level: world.cfg.character.base_subclass_level,
        exp,
        sp: 0,
    };
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
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return false;
    };
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
    let (base_class, cur_index, cur_class, cur_level, cur_exp, cur_sp) = (
        p.base_class_id,
        p.class_index,
        p.class_id,
        p.level,
        p.exp,
        p.sp,
    );

    // A cast in flight would land against the old class's stats.
    crate::game_loop::skills::cast::stop_casting(world, player_oid);

    // 1. Bank the outgoing slot.
    if cur_index == 0 {
        // The base class's progress lives in the `characters` row, which the
        // ordinary player save already writes.
    } else {
        let banked = SubClass {
            class_id: cur_class,
            class_index: cur_index,
            level: cur_level,
            exp: cur_exp,
            sp: cur_sp,
        };
        if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid)
            && let Some(slot) = p.subclasses.iter_mut().find(|s| s.class_index == cur_index)
        {
            *slot = banked;
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

    // 3. Swap the skill books. Java `removeSkill`s everything, then
    //    `restoreSkills()` (the DB rows for the new index) + `rewardSkills()`
    //    (the class's auto-granted tree on top). The port mirrors that: bank
    //    the outgoing book, restore the incoming one if the slot has been
    //    played before, and let `set_level` below add the auto-granted tree.
    let outgoing_enchants = world
        .objects
        .get_component::<crate::model::components::skills::SkillEnchants>(&player_oid)
        .map(|e| e.0.clone())
        .unwrap_or_default();
    let outgoing: Vec<(i32, i32, i32)> = world
        .objects
        .get_component::<SkillBook>(&player_oid)
        .map(|b| {
            b.0.iter()
                .map(|(id, lvl)| (*id, *lvl, outgoing_enchants.get(id).copied().unwrap_or(0)))
                .collect()
        })
        .unwrap_or_default();
    let incoming = {
        let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) else {
            return false;
        };
        p.skills_by_index.insert(cur_index, outgoing);
        p.skills_by_index.get(&class_index).cloned()
    };
    let incoming = incoming.unwrap_or_default();
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&player_oid) {
        book.0.clear();
        // A slot played before restores exactly what it knew — including
        // *manually learned* skills, which re-deriving the tree would lose.
        for &(id, lvl, _) in &incoming {
            book.0.insert(id, lvl);
        }
    }
    // Enchant sub-levels ride the same banked rows.
    if let Some(ench) = world
        .objects
        .get_component_mut::<crate::model::components::skills::SkillEnchants>(&player_oid)
    {
        ench.0.clear();
        for &(id, _, sub) in &incoming {
            if sub > 0 {
                ench.0.insert(id, sub);
            }
        }
    }

    // 3b. Hennas and shortcuts are per class index too (Java clears `_henna`
    //     and calls `restoreHenna()` + `restoreShortcuts()` inside the same
    //     switch). Bank the outgoing set, take the incoming one.
    let outgoing_henna: Vec<(i32, i32)> = world
        .objects
        .get_component::<crate::model::components::skills::HennaSlots>(&player_oid)
        .map(|h| {
            h.0.iter()
                .enumerate()
                .filter_map(|(i, d)| d.map(|dye| (i as i32 + 1, dye)))
                .collect()
        })
        .unwrap_or_default();
    let outgoing_shortcuts: Vec<crate::model::shortcut::Shortcut> = world
        .objects
        .get_component::<crate::model::components::player::Shortcuts>(&player_oid)
        .map(|s| s.0.values().cloned().collect())
        .unwrap_or_default();
    let (incoming_henna, incoming_shortcuts) = {
        let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) else {
            return false;
        };
        p.hennas_by_index.insert(cur_index, outgoing_henna);
        p.shortcuts_by_index.insert(cur_index, outgoing_shortcuts);
        (
            p.hennas_by_index
                .get(&class_index)
                .cloned()
                .unwrap_or_default(),
            p.shortcuts_by_index
                .get(&class_index)
                .cloned()
                .unwrap_or_default(),
        )
    };
    let mut slots = [None; 3];
    for (slot, dye) in incoming_henna {
        if (1..=3).contains(&slot) {
            slots[(slot - 1) as usize] = Some(dye);
        }
    }
    if let Some(h) = world
        .objects
        .get_component_mut::<crate::model::components::skills::HennaSlots>(&player_oid)
    {
        h.0 = slots;
    }
    if let Some(sc) = world
        .objects
        .get_component_mut::<crate::model::components::player::Shortcuts>(&player_oid)
    {
        sc.0.clear();
        for s in incoming_shortcuts {
            sc.0.insert(s.slot + s.page * 12, s);
        }
    }
    // Henna dyes fold into `BaseStats`, so the swap has to re-fold them.
    // `apply_henna_change` also pushes `HennaInfo`, which is exactly what
    // Java's `setActiveClass` does (`restoreHenna(); sendPacket(HennaInfo)`).
    if let Some(cid) = crate::game_loop::helpers::client_for_player(world, player_oid) {
        super::henna::apply_henna_change(world, cid, player_oid);
    }

    // 3c. `resetTimeStamps()` — Java *clears* skill cooldowns on a class
    //     switch rather than banking them per slot, so a subclass can't be
    //     used to sit out a long reuse on the class that started it.
    if let Some(reuses) = world
        .objects
        .get_component_mut::<crate::model::components::skills::Reuses>(&player_oid)
    {
        reuses.0.clear();
    }

    // 4. Rebuild stats and top up the auto-granted tree for the new class.
    //    `set_level` is the same path `//setclass` uses: recompute HP/MP/stats,
    //    grant the class's reachable skills, and push the status/UserInfo/
    //    SkillList refresh.
    crate::game_loop::death::set_level(world, player_oid, new_level);
    // `set_level` normalises exp to the level floor; restore the slot's own.
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.exp = new_exp;
        p.sp = new_sp;
    }
    super::player_info::broadcast_user_info(world, player_oid);
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

/// `Player.modifySubClass`: wipe the slot — its
/// subclass row, banked skills, hennas and shortcuts, in memory and in the DB
/// — then add the new class into the freed index and switch to it. Returns
/// `false` (after reverting to the base class, as Java does) when the
/// replacement could not be added; **the old subclass is gone either way**,
/// which is Java's own documented behaviour ("the information about this
/// subclass will be removed from the subclass list even if false!").
#[allow(dead_code)] // the village-master menu is gone; Raina's flow (S8 dual class) calls it
pub(crate) fn modify_subclass(
    world: &mut World,
    player_oid: i32,
    class_index: i32,
    new_class_id: i32,
) -> bool {
    let Some(old_class_id) = world
        .objects
        .get_component::<Player>(&player_oid)
        .and_then(|p| {
            p.subclasses
                .iter()
                .find(|s| s.class_index == class_index)
                .map(|s| s.class_id)
        })
    else {
        return false;
    };
    // Modifying the slot you stand on: step back to base first (Java relies
    // on the post-wipe `setActiveClass` for this; the port keeps the active
    // state coherent throughout instead).
    if world
        .objects
        .get_component::<Player>(&player_oid)
        .is_some_and(|p| p.class_index == class_index)
        && !set_active_class(world, player_oid, 0)
    {
        return false;
    }
    // Java case 7 stops every effect of the old class before the switch.
    crate::game_loop::olympiad::strip_buffs(world, player_oid);

    // The wipe: memory first, then the DB rows.
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.subclasses.retain(|s| s.class_index != class_index);
        p.skills_by_index.remove(&class_index);
        p.hennas_by_index.remove(&class_index);
        p.shortcuts_by_index.remove(&class_index);
    }
    let _ = world.db.send(crate::db::DbCommand::WipeSubclassSlot {
        char_id: player_oid,
        class_index,
        old_class_id,
    });

    // The replacement lands in the freed index (`add_subclass` picks the
    // lowest free slot). Failure reverts to base, per Java.
    match add_subclass(world, player_oid, new_class_id) {
        Ok(index) => set_active_class(world, player_oid, index),
        Err(_) => {
            set_active_class(world, player_oid, 0);
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Occupation change (`Player.setClassId`).

/// Java's class-change visual (`broadcastPacket(new MagicSkillUse(this, 5103,
/// 1, 0, 0))`) — the flash everyone nearby sees when a character advances.
const CLASS_CHANGE_EFFECT_SKILL: i32 = 5103;

/// `Player.setClassId` — advance (or set) the character's occupation.
///
/// **Which class id moves depends on the active slot**, and getting this wrong
/// corrupts the character: on a subclass, Java updates *that slot's* class id
/// (`getSubClasses().get(_classIndex).setClassId(id)`) and leaves the base
/// class alone. Only on the base slot does `_baseClass` follow.
///
/// Returns `false` for an unknown class id.
pub(crate) fn set_class_id(world: &mut World, player_oid: i32, class_id: i32) -> bool {
    if world.data.player_templates.get(class_id).is_none() {
        return false;
    }
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return false;
    };
    let (index, level) = (p.class_index, p.level);

    // Java `Player.setClassId` opens with the academy block — **before** the
    // class changes, so the graduate is still an academy member when the clan
    // is paid. `THIRD_CLASS_GROUP` is CategoryData's name for the *2nd*-transfer
    // classes (the base class is the first group).
    if world
        .data
        .categories
        .contains("THIRD_CLASS_GROUP", class_id)
    {
        crate::game_loop::clans::academy::graduate(world, player_oid);
    }

    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.class_id = class_id;
        if index == 0 {
            // Base slot: the character's identity really does change.
            p.base_class_id = class_id;
        } else if let Some(slot) = p.subclasses.iter_mut().find(|s| s.class_index == index) {
            // Subclass slot: only that slot advances — the base class is
            // untouched. Overwriting it here would silently rewrite what the
            // character *is*.
            slot.class_id = class_id;
        }
    }
    // Persist the slot's new class so the change survives a restart.
    if index != 0
        && let Some(slot) = world
            .objects
            .get_component::<Player>(&player_oid)
            .and_then(|p| {
                p.subclasses
                    .iter()
                    .find(|s| s.class_index == index)
                    .copied()
            })
    {
        persist_slot(world, player_oid, slot);
    }

    // Java `setClassId`'s "Remove class permitted hennas" — a dye the new class
    // may not wear comes off, before the recompute below folds the remaining
    // dyes' bonuses back into `BaseStats`.
    super::henna::drop_hennas_the_class_cannot_wear(world, player_oid);

    // `rewardSkills()` + the stat recompute + status/UserInfo/SkillList refresh.
    // `set_level` also runs `check_player_skills`, which is Java's *only* skill
    // pruning here: `checkPlayerSkills` downgrades a skill whose level now
    // outranks the character, and does nothing at all to a skill the new class
    // cannot learn. (An admin marker used to claim Java removed those — it
    // does not; `rewardSkills` only ever adds.)
    crate::game_loop::death::set_level(world, player_oid, level);
    super::player_info::broadcast_user_info(world, player_oid);

    // `ClanMaster`'s `ON_PLAYER_PROFESSION_CHANGE` listener, the last of that
    // script's four. **Redundant on this port today, and ported anyway**: the
    // buff it re-lights is not currently stripped by anything on this path
    // (only the subclass-*cancel* flow calls `strip_buffs`), so it refreshes a
    // buff that is already there. It is here so that the day something on the
    // class-change path does clear effects — as Java's does — the aura comes
    // back rather than silently vanishing for every clan member who changes
    // profession.
    clan_skills::reapply_clan_advent_on_profession_change(world, player_oid);

    // The class-change flash, to everyone nearby including the player.
    if let Some(pos) = maybe_position(world, player_oid)
        && let Some(region) = region_cell_of(world, player_oid)
    {
        broadcast::broadcast_near_region(
            world,
            region,
            &crate::network::server_packets::magic_skill_use_raw(
                (player_oid, pos.x, pos.y, pos.z),
                (player_oid, pos.x, pos.y, pos.z),
                CLASS_CHANGE_EFFECT_SKILL,
                1,
                0,
            ),
        );
    }
    true
}
