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
//! TODO(G17), the module's one remaining deferral bundle: per-subclass hennas
//! and shortcuts (both still load with `class_index = 0`), certification
//! skills, the village-master cancel/replace verbs (cases 3/6/7 — the
//! slot-wipe), the UI flow (G22's occupation quests), and the subclass-change
//! lock Java holds across the swap.

use crate::config::flood_protector::FloodAction;
use crate::model::components::{Position, RegionCell, SkillBook};
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

    let exp = world.data.experience.exp_for_level(BASE_SUBCLASS_LEVEL);
    let slot = SubClass {
        class_id,
        class_index: index,
        level: BASE_SUBCLASS_LEVEL,
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
    super::skills::cast::stop_casting(world, player_oid);

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
        .get_component::<crate::model::components::SkillEnchants>(&player_oid)
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
        .get_component_mut::<crate::model::components::SkillEnchants>(&player_oid)
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
        .get_component::<crate::model::components::HennaSlots>(&player_oid)
        .map(|h| {
            h.0.iter()
                .enumerate()
                .filter_map(|(i, d)| d.map(|dye| (i as i32 + 1, dye)))
                .collect()
        })
        .unwrap_or_default();
    let outgoing_shortcuts: Vec<crate::model::shortcut::Shortcut> = world
        .objects
        .get_component::<crate::model::components::Shortcuts>(&player_oid)
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
        .get_component_mut::<crate::model::components::HennaSlots>(&player_oid)
    {
        h.0 = slots;
    }
    if let Some(sc) = world
        .objects
        .get_component_mut::<crate::model::components::Shortcuts>(&player_oid)
    {
        sc.0.clear();
        for s in incoming_shortcuts {
            sc.0.insert(s.slot + s.page * 12, s);
        }
    }
    // Henna dyes fold into `BaseStats`, so the swap has to re-fold them.
    // `apply_henna_change` also pushes `HennaInfo`, which is exactly what
    // Java's `setActiveClass` does (`restoreHenna(); sendPacket(HennaInfo)`).
    if let Some(cid) = super::helpers::client_for_player(world, player_oid) {
        super::henna::apply_henna_change(world, cid, player_oid);
    }

    // 3c. `resetTimeStamps()` — Java *clears* skill cooldowns on a class
    //     switch rather than banking them per slot, so a subclass can't be
    //     used to sit out a long reuse on the class that started it.
    if let Some(reuses) = world
        .objects
        .get_component_mut::<crate::model::components::Reuses>(&player_oid)
    {
        reuses.0.clear();
    }

    // 4. Rebuild stats and top up the auto-granted tree for the new class.
    //    `set_level` is the same path `//setclass` uses: recompute HP/MP/stats,
    //    grant the class's reachable skills, and push the status/UserInfo/
    //    SkillList refresh.
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

// ---------------------------------------------------------------------------
// The village-master flow (`VillageMaster.onBypassFeedback`'s `Subclass` verb).

/// Java's hard-coded minimum for taking a subclass.
pub(crate) const SUBCLASS_MIN_LEVEL: i32 = 75;

/// Java `neverSubclassed` — Overlord and Warsmith are never offered.
const NEVER_SUBCLASSED: [i32; 2] = [
    91, // Overlord
    99, // Warsmith
];

/// Race ids as the datapack numbers them; only the Elf/Dark Elf pair has a
/// cross-subclass rule on this chronicle.
const RACE_ELF: i32 = 1;
const RACE_DARK_ELF: i32 = 2;

/// `VillageMaster.getAvailableSubClasses`, narrowed to Interlude's rules:
/// every third-class group entry, minus
/// - the player's own base lineage (a "similar class"),
/// - anything already held (or a child of it),
/// - Overlord / Warsmith,
/// - the other side of the Elf ↔ Dark Elf pair.
///
/// Kamael rules are omitted: the race doesn't exist on this chronicle.
pub(crate) fn available_subclasses(world: &World, player_oid: i32) -> Vec<i32> {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return Vec::new();
    };
    let (base_class, race) = (p.base_class_id, p.race);
    let held: Vec<i32> = p.subclasses.iter().map(|s| s.class_id).collect();

    // The base's whole lineage is off-limits, not just the exact class.
    let base_lineage = world.data.skill_trees.class_lineage(base_class);
    // Likewise every held subclass's lineage.
    let held_lineages: Vec<i32> = held
        .iter()
        .flat_map(|c| world.data.skill_trees.class_lineage(*c))
        .collect();

    let mut out: Vec<i32> = Vec::new();
    for class_id in world.data.player_templates.class_ids() {
        if !world
            .data
            .categories
            .contains("THIRD_CLASS_GROUP", class_id)
        {
            continue;
        }
        if NEVER_SUBCLASSED.contains(&class_id) {
            continue;
        }
        let lineage = world.data.skill_trees.class_lineage(class_id);
        if lineage.iter().any(|c| base_lineage.contains(c)) {
            continue;
        }
        if lineage.iter().any(|c| held_lineages.contains(c)) {
            continue;
        }
        // Elves and Dark Elves may not subclass into each other.
        if let Some(other_race) = class_race(world, class_id) {
            let cross_elf = (race == RACE_ELF && other_race == RACE_DARK_ELF)
                || (race == RACE_DARK_ELF && other_race == RACE_ELF);
            if cross_elf {
                continue;
            }
        }
        out.push(class_id);
    }
    out.sort_unstable();
    out
}

/// A class's race. `PlayerTemplate::race` only answers for *creatable*
/// (1st-occupation) classes, so an advanced class is resolved by walking its
/// lineage to the root and asking there.
fn class_race(world: &World, class_id: i32) -> Option<i32> {
    world
        .data
        .skill_trees
        .class_lineage(class_id)
        .into_iter()
        .find_map(|c| world.data.player_templates.get(c).and_then(|t| t.race()))
        .map(|r| r as i32)
}

/// Java's `case 4` gate: level 75 on the *current* class and a free slot.
pub(crate) fn can_add_subclass(world: &World, player_oid: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&player_oid)
        .is_some_and(|p| {
            p.level >= SUBCLASS_MIN_LEVEL
                && (p.subclasses.len() as i32) < world.cfg.character.max_subclass
        })
}

/// `VillageMaster.onBypassFeedback`'s `Subclass <cmd> [arg]` verb.
///
/// Ported cases: **0** the menu, **1** the add list, **2** the change list,
/// **4** add-action, **5** change-action. Java's 3/6/7 (cancel/change an
/// existing subclass, which *replaces* a slot) belong to the module header's
/// remaining deferral bundle — they need the same slot-wipe Java does and
/// have no caller until the UI offers them.
///
/// The HTML is built inline rather than from `data/html/villagemaster/*.htm`
/// because those files carry `%list%` placeholders the port's html cache
/// doesn't template yet; the link targets and bypasses match Java's.
/// Java `VillageMaster`'s `canChangeSubclass()` guard: refuse and log, exactly
/// as Java does before its `return`.
fn subclass_flood_ok(world: &mut World, client_id: u32, player_oid: i32) -> bool {
    if super::flood::gate(world, client_id, FloodAction::Subclass) {
        return true;
    }
    tracing::warn!("VillageMaster: player {player_oid} has performed a subclass change too fast");
    false
}

pub(crate) fn handle_village_master_bypass(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    npc_oid: i32,
    args: &str,
) {
    let mut it = args.split_whitespace();
    let cmd: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let param: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);

    match cmd {
        0 => show_menu(world, client_id, npc_oid),
        1 => show_add_list(world, client_id, player_oid, npc_oid),
        2 => show_change_list(world, client_id, player_oid, npc_oid),
        4 => {
            // `FloodProtectorSubclass` — the one Java call site that is not a
            // packet: it hangs off this bypass, so the dispatch table cannot
            // reach it. Java guards cases 4, 5 and 7 (7 is unported here).
            if !subclass_flood_ok(world, client_id, player_oid) {
                return;
            }
            // Java gates the *action* on level 75 and a free slot, not just
            // the list — a stale link must not slip past.
            if !can_add_subclass(world, player_oid) {
                return html(
                    world,
                    client_id,
                    npc_oid,
                    "You cannot add a subclass right now.",
                );
            }
            if !available_subclasses(world, player_oid).contains(&param) {
                return html(
                    world,
                    client_id,
                    npc_oid,
                    "That class is not available to you.",
                );
            }
            match add_subclass(world, player_oid, param) {
                Ok(_) => html(world, client_id, npc_oid, "Your subclass has been added."),
                Err(_) => html(world, client_id, npc_oid, "You cannot add that subclass."),
            }
        }
        5 => {
            if !subclass_flood_ok(world, client_id, player_oid) {
                return;
            }
            if world
                .objects
                .get_component::<Player>(&player_oid)
                .is_some_and(|p| p.class_index == param)
            {
                return html(
                    world,
                    client_id,
                    npc_oid,
                    "You are already using that class.",
                );
            }
            if set_active_class(world, player_oid, param) {
                html(world, client_id, npc_oid, "Your class has been changed.");
            } else {
                html(
                    world,
                    client_id,
                    npc_oid,
                    "You cannot change to that class.",
                );
            }
        }
        _ => show_menu(world, client_id, npc_oid),
    }
}

fn html(world: &World, client_id: u32, npc_oid: i32, body: &str) {
    let page = format!(
        "<html><body>Subclass<br><br>{body}<br><br>{}</body></html>",
        back_link(npc_oid)
    );
    send_html(world, client_id, npc_oid, &page);
}

fn back_link(npc_oid: i32) -> String {
    format!("<a action=\"bypass -h npc_{npc_oid}_Subclass 0\">Back</a>")
}

fn show_menu(world: &World, client_id: u32, npc_oid: i32) {
    let page = format!(
        "<html><body>Subclass<br><br>\
         <a action=\"bypass -h npc_{npc_oid}_Subclass 1\">Add a subclass</a><br>\
         <a action=\"bypass -h npc_{npc_oid}_Subclass 2\">Change to a subclass</a><br>\
         </body></html>"
    );
    send_html(world, client_id, npc_oid, &page);
}

fn show_add_list(world: &World, client_id: u32, player_oid: i32, npc_oid: i32) {
    if !can_add_subclass(world, player_oid) {
        return html(
            world,
            client_id,
            npc_oid,
            "You must be at least level 75, with a free subclass slot.",
        );
    }
    let available = available_subclasses(world, player_oid);
    if available.is_empty() {
        return html(
            world,
            client_id,
            npc_oid,
            "There are no sub classes available at this time.",
        );
    }
    let mut body = String::new();
    for class_id in available {
        body.push_str(&format!(
            "<a action=\"bypass -h npc_{npc_oid}_Subclass 4 {class_id}\">Class {class_id}</a><br>"
        ));
    }
    let page = format!(
        "<html><body>Add a subclass<br><br>{body}<br>{}</body></html>",
        back_link(npc_oid)
    );
    send_html(world, client_id, npc_oid, &page);
}

fn show_change_list(world: &World, client_id: u32, player_oid: i32, npc_oid: i32) {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    if p.subclasses.is_empty() {
        return html(world, client_id, npc_oid, "You do not have any subclasses.");
    }
    let mut body = format!(
        "<a action=\"bypass -h npc_{npc_oid}_Subclass 5 0\">Class {} (base)</a><br>",
        p.base_class_id
    );
    for s in &p.subclasses {
        body.push_str(&format!(
            "<a action=\"bypass -h npc_{npc_oid}_Subclass 5 {}\">Class {} (level {})</a><br>",
            s.class_index, s.class_id, s.level
        ));
    }
    let page = format!(
        "<html><body>Change class<br><br>{body}<br>{}</body></html>",
        back_link(npc_oid)
    );
    send_html(world, client_id, npc_oid, &page);
}

fn send_html(world: &World, client_id: u32, npc_oid: i32, page: &str) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::server_packets::npc_html_message(
            npc_oid, page,
        ));
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
        super::academy::graduate(world, player_oid);
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

    // `rewardSkills()` + the stat recompute + status/UserInfo/SkillList refresh.
    super::death::set_level(world, player_oid, level);
    super::party::broadcast_user_info(world, player_oid);

    // The class-change flash, to everyone nearby including the player.
    if let Some(pos) = world
        .objects
        .get_component::<Position>(&player_oid)
        .copied()
        && let Some(region) = world
            .objects
            .get_component::<RegionCell>(&player_oid)
            .map(|r| r.0)
    {
        super::helpers::broadcast_near_region(
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
