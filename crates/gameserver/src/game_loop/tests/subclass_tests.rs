//! Subclasses (G17 slice 2) — `addSubClass` / `setActiveClass`.

use super::*;

use crate::game_loop::character::subclass::{AddError, add_subclass, set_active_class};
use crate::model::Player;

const PLAYER: i32 = 2001;
const CID: u32 = 1;

fn sub_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    // A handful of extra class templates so `add_subclass` has real targets.
    let base = world.data.player_templates.get(0).unwrap().clone();
    let mut all = vec![base.clone()];
    for id in 1..=6 {
        let mut t = base.clone();
        t.class_id = id;
        all.push(t);
    }
    world.data.player_templates = crate::data::PlayerTemplateData::from_vec(all);
    (world, db, l)
}

fn p(world: &World) -> &Player {
    world.objects.get_component::<Player>(&PLAYER).unwrap()
}

// ---------------------------------------------------------------------------

#[test]
fn a_new_character_has_no_subclasses_and_is_on_index_zero() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    assert_eq!(p(&world).class_index, 0);
    assert!(p(&world).subclasses.is_empty());
}

#[test]
fn adding_a_subclass_takes_the_first_slot_at_the_base_level() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    let index = add_subclass(&mut world, PLAYER, 3).expect("added");

    assert_eq!(index, 1, "slots are dense, starting at 1");
    let sub = p(&world).subclasses[0];
    assert_eq!(sub.class_id, 3);
    assert_eq!(
        sub.level, world.cfg.character.base_subclass_level,
        "a subclass starts at 40"
    );
    assert_eq!(p(&world).class_index, 0, "adding does not switch to it");
}

#[test]
fn the_base_class_cannot_be_taken_as_a_subclass() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let base = p(&world).base_class_id;

    assert_eq!(
        add_subclass(&mut world, PLAYER, base),
        Err(AddError::AlreadyHave)
    );
}

#[test]
fn the_same_subclass_cannot_be_taken_twice() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();

    assert_eq!(
        add_subclass(&mut world, PLAYER, 3),
        Err(AddError::AlreadyHave)
    );
}

#[test]
fn slots_are_capped_by_max_subclass() {
    let (mut world, _db, _l) = sub_world();
    world.cfg.character.max_subclass = 2;
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    add_subclass(&mut world, PLAYER, 4).unwrap();

    assert_eq!(
        add_subclass(&mut world, PLAYER, 5),
        Err(AddError::SlotsFull)
    );
}

#[test]
fn an_unknown_class_is_refused() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    assert_eq!(
        add_subclass(&mut world, PLAYER, 9999),
        Err(AddError::UnknownClass)
    );
}

#[test]
fn switching_to_a_subclass_swaps_class_and_level() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();

    assert!(set_active_class(&mut world, PLAYER, 1));

    assert_eq!(p(&world).class_index, 1);
    assert_eq!(p(&world).class_id, 3, "now playing the subclass");
    assert_eq!(p(&world).level, world.cfg.character.base_subclass_level);
}

#[test]
fn switching_back_restores_the_base_class_progress() {
    // The point of banking: the base class's level must survive a round trip
    // through a level-40 subclass.
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let base_class = p(&world).base_class_id;
    // Put the base class on a distinctive level.
    crate::game_loop::death::set_level(&mut world, PLAYER, 7);
    let base_level = p(&world).level;
    add_subclass(&mut world, PLAYER, 3).unwrap();

    set_active_class(&mut world, PLAYER, 1);
    assert_eq!(p(&world).level, world.cfg.character.base_subclass_level);
    set_active_class(&mut world, PLAYER, 0);

    assert_eq!(p(&world).class_index, 0);
    assert_eq!(p(&world).class_id, base_class);
    assert_eq!(p(&world).level, base_level, "the base level came back");
}

#[test]
fn subclass_progress_is_banked_across_a_switch() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    add_subclass(&mut world, PLAYER, 4).unwrap();
    set_active_class(&mut world, PLAYER, 1);
    // Level the subclass up while it is active.
    crate::game_loop::death::set_level(&mut world, PLAYER, 45);

    set_active_class(&mut world, PLAYER, 2); // away
    set_active_class(&mut world, PLAYER, 1); // and back

    assert_eq!(p(&world).level, 45, "slot 1 kept its own level");
}

#[test]
fn switching_to_a_slot_that_does_not_exist_fails() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    assert!(!set_active_class(&mut world, PLAYER, 3), "no such slot");
    assert_eq!(
        p(&world).class_index,
        0,
        "and the active class is untouched"
    );
}

#[test]
fn switching_to_the_already_active_class_is_a_no_op() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    assert!(!set_active_class(&mut world, PLAYER, 0));
}

#[test]
fn adding_a_subclass_is_persisted() {
    let (mut world, mut db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let _ = drain_db(&mut db);

    add_subclass(&mut world, PLAYER, 3).unwrap();

    let cmds = drain_db(&mut db);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            db::DbCommand::StoreSubClass { class_id: 3, class_index: 1, level, .. }
                if *level == world.cfg.character.base_subclass_level
        )),
        "the new slot must reach the DB"
    );
}

// ---------------------------------------------------------------------------
// Per-class-index skill books (slice 3).

use crate::model::components::SkillBook;

/// Put a skill in the book that the class tree would never grant, standing in
/// for one learned by hand from a trainer.
fn learn_by_hand(world: &mut World, skill_id: i32) {
    world
        .objects
        .get_component_mut::<SkillBook>(&PLAYER)
        .unwrap()
        .0
        .insert(skill_id, 3);
}

#[test]
fn a_hand_learned_skill_survives_a_switch_away_and_back() {
    // The gap this slice closes: before per-index books, a switch re-derived
    // only the auto-granted tree, so anything learned by hand vanished.
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    learn_by_hand(&mut world, 7777);

    set_active_class(&mut world, PLAYER, 1);
    assert!(
        !knows(&world, 7777, PLAYER),
        "the subclass has its own, empty book"
    );
    set_active_class(&mut world, PLAYER, 0);

    assert!(
        knows(&world, 7777, PLAYER),
        "the base class got its hand-learned skill back"
    );
}

#[test]
fn each_slot_keeps_its_own_skills() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    add_subclass(&mut world, PLAYER, 4).unwrap();

    set_active_class(&mut world, PLAYER, 1);
    learn_by_hand(&mut world, 8001);
    set_active_class(&mut world, PLAYER, 2);
    learn_by_hand(&mut world, 8002);

    assert!(knows(&world, 8002, PLAYER), "slot 2's own skill");
    assert!(!knows(&world, 8001, PLAYER), "and not slot 1's");

    set_active_class(&mut world, PLAYER, 1);
    assert!(knows(&world, 8001, PLAYER));
    assert!(!knows(&world, 8002, PLAYER));
}

#[test]
fn the_save_carries_every_slots_book() {
    let (mut world, mut db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    set_active_class(&mut world, PLAYER, 1);
    learn_by_hand(&mut world, 8001);
    let _ = drain_db(&mut db);

    save_all_players(&mut world);

    let cmds = drain_db(&mut db);
    let save = cmds.iter().find_map(|c| match c {
        db::DbCommand::StorePlayer { save } => Some(save),
        _ => None,
    });
    let save = save.expect("a save went out");
    assert_eq!(save.class_index, 1, "the active index is recorded");
    assert!(
        save.skills.iter().any(|(id, _, _)| *id == 8001),
        "the active book carries the new skill"
    );
    assert!(
        save.skills_by_index.contains_key(&0),
        "and the base slot's book is banked alongside"
    );
}

// ---------------------------------------------------------------------------
// Per-class-index hennas and shortcuts (slice 4).

use crate::model::components::{HennaSlots, Shortcuts};

fn set_henna(world: &mut World, slot: usize, dye: i32) {
    world
        .objects
        .get_component_mut::<HennaSlots>(&PLAYER)
        .unwrap()
        .0[slot] = Some(dye);
}

fn henna(world: &World, slot: usize) -> Option<i32> {
    world
        .objects
        .get_component::<HennaSlots>(&PLAYER)
        .unwrap()
        .0[slot]
}

fn add_shortcut(world: &mut World, slot: i32, id: i32) {
    let sc = Shortcut {
        slot,
        page: 0,
        kind: ShortcutType::Skill,
        id,
        level: 1,
        character_type: 1,
        shared_reuse_group: -1,
    };
    world
        .objects
        .get_component_mut::<Shortcuts>(&PLAYER)
        .unwrap()
        .0
        .insert(slot, sc);
}

fn shortcut_ids(world: &World) -> Vec<i32> {
    world
        .objects
        .get_component::<Shortcuts>(&PLAYER)
        .map(|s| s.0.values().map(|sc| sc.id).collect())
        .unwrap_or_default()
}

#[test]
fn hennas_are_per_subclass() {
    // Java clears `_henna` and calls `restoreHenna()` inside setActiveClass —
    // dyes belong to the class you painted them on.
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    set_henna(&mut world, 0, 111);

    set_active_class(&mut world, PLAYER, 1);
    assert_eq!(
        henna(&world, 0),
        None,
        "the subclass starts with bare slots"
    );
    set_active_class(&mut world, PLAYER, 0);

    assert_eq!(
        henna(&world, 0),
        Some(111),
        "the base class's dye came back"
    );
}

#[test]
fn shortcuts_are_per_subclass() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    add_shortcut(&mut world, 1, 1001);

    set_active_class(&mut world, PLAYER, 1);
    assert!(
        !shortcut_ids(&world).contains(&1001),
        "the subclass has its own bar"
    );
    add_shortcut(&mut world, 1, 2002);
    set_active_class(&mut world, PLAYER, 0);

    let ids = shortcut_ids(&world);
    assert!(ids.contains(&1001), "the base bar came back");
    assert!(!ids.contains(&2002), "and did not inherit the subclass's");
}

#[test]
fn the_save_carries_hennas_and_shortcuts_for_every_slot() {
    let (mut world, mut db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    set_henna(&mut world, 0, 111);
    add_shortcut(&mut world, 1, 1001);
    set_active_class(&mut world, PLAYER, 1);
    let _ = drain_db(&mut db);

    save_all_players(&mut world);

    let cmds = drain_db(&mut db);
    let save = cmds
        .iter()
        .find_map(|c| match c {
            db::DbCommand::StorePlayer { save } => Some(save),
            _ => None,
        })
        .expect("a save went out");
    assert!(
        save.hennas_by_index.contains_key(&0),
        "the base slot's dyes are banked"
    );
    assert!(
        save.shortcuts_by_index.contains_key(&0),
        "and its shortcut bar"
    );
}

// ---------------------------------------------------------------------------
// The village-master flow (slice 5).

use crate::game_loop::character::subclass::{
    SUBCLASS_MIN_LEVEL, available_subclasses, can_add_subclass,
};

#[test]
fn adding_a_subclass_needs_level_75() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    crate::game_loop::death::set_level(&mut world, PLAYER, SUBCLASS_MIN_LEVEL - 1);
    assert!(!can_add_subclass(&world, PLAYER), "74 is not enough");

    crate::game_loop::death::set_level(&mut world, PLAYER, SUBCLASS_MIN_LEVEL);
    assert!(can_add_subclass(&world, PLAYER), "75 is the gate");
}

#[test]
fn a_full_slot_list_blocks_adding_even_at_level_75() {
    let (mut world, _db, _l) = sub_world();
    world.cfg.character.max_subclass = 1;
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    crate::game_loop::death::set_level(&mut world, PLAYER, SUBCLASS_MIN_LEVEL);
    add_subclass(&mut world, PLAYER, 3).unwrap();

    assert!(!can_add_subclass(&world, PLAYER));
}

#[test]
fn the_available_list_excludes_the_base_lineage_and_held_classes() {
    // Against the real datapack, so the class hierarchy and category groups
    // are the shipped ones rather than a fixture's guess.
    let (mut world, _db, _l) = combat_test_world();
    world.data = dist::game_data_owned();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    // Human Fighter line: base class 0.
    let avail = available_subclasses(&world, PLAYER);
    assert!(
        !avail.is_empty(),
        "a human fighter has subclasses available"
    );

    // Every offering is a third-class group entry...
    for c in &avail {
        assert!(
            world.data.categories.contains("THIRD_CLASS_GROUP", *c),
            "class {c} is not a 3rd-class group entry"
        );
    }
    // ...and none is Overlord or Warsmith.
    assert!(!avail.contains(&91), "Overlord is never subclassable");
    assert!(!avail.contains(&99), "Warsmith is never subclassable");

    // Taking one removes it (and its lineage) from the next offering.
    let taken = avail[0];
    add_subclass(&mut world, PLAYER, taken).unwrap();
    assert!(
        !available_subclasses(&world, PLAYER).contains(&taken),
        "a held class is not offered again"
    );
}

// ---------------------------------------------------------------------------
// Occupation change (slice 6).

use crate::game_loop::character::subclass::set_class_id;

#[test]
fn a_class_change_on_the_base_slot_moves_the_base_class() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    assert!(set_class_id(&mut world, PLAYER, 5));

    assert_eq!(p(&world).class_id, 5);
    assert_eq!(
        p(&world).base_class_id,
        5,
        "advancing on the base slot changes what you are"
    );
}

#[test]
fn a_class_change_on_a_subclass_leaves_the_base_class_alone() {
    // The bug this slice fixes: `//setclass` set base_class_id
    // unconditionally, so advancing while on a subclass rewrote the
    // character's base class. Java updates only that slot.
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let base = p(&world).base_class_id;
    add_subclass(&mut world, PLAYER, 3).unwrap();
    set_active_class(&mut world, PLAYER, 1);

    assert!(set_class_id(&mut world, PLAYER, 5));

    assert_eq!(p(&world).class_id, 5, "the active class advanced");
    assert_eq!(
        p(&world).base_class_id,
        base,
        "the BASE class must not move"
    );
    assert_eq!(
        p(&world)
            .subclasses
            .iter()
            .find(|s| s.class_index == 1)
            .unwrap()
            .class_id,
        5,
        "the slot itself records the new class"
    );
}

#[test]
fn the_subclass_slots_new_class_survives_a_switch_away_and_back() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    set_active_class(&mut world, PLAYER, 1);
    set_class_id(&mut world, PLAYER, 5);

    set_active_class(&mut world, PLAYER, 0);
    set_active_class(&mut world, PLAYER, 1);

    assert_eq!(p(&world).class_id, 5, "the advanced slot kept its class");
}

#[test]
fn an_unknown_class_is_rejected() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let before = p(&world).class_id;

    assert!(!set_class_id(&mut world, PLAYER, 9999));

    assert_eq!(p(&world).class_id, before, "nothing moved");
}

#[test]
fn a_class_change_broadcasts_the_visual_effect() {
    // Java: broadcastPacket(new MagicSkillUse(this, 5103, 1, 0, 0)).
    let (mut world, _db, _l) = sub_world();
    let mut out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let _ = drain(&mut out);

    set_class_id(&mut world, PLAYER, 5);

    let packets = drain(&mut out);
    assert!(
        packets
            .iter()
            .any(|p| p.first() == Some(&server_packets::opcodes::MAGIC_SKILL_USE)),
        "onlookers (and the player) should see the class-change flash"
    );
}

// ---------------------------------------------------------------------------
// Skill cooldowns across a class switch (slice 7).

#[test]
fn a_class_switch_clears_skill_cooldowns() {
    // Java `setActiveClass` calls `resetTimeStamps()`. Without it a player
    // could park a long reuse on one class and sit it out on another.
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    let until_tick = world.tick + 100_000;
    arm_reuse(
        &mut world,
        PLAYER,
        1234,
        model::SkillReuse {
            skill_level: 1,
            until_tick,
            total_ms: 600_000,
        },
    );

    set_active_class(&mut world, PLAYER, 1);

    assert!(
        world
            .objects
            .get_component::<Reuses>(&PLAYER)
            .unwrap()
            .0
            .is_empty(),
        "cooldowns are wiped on a class switch, not carried or banked"
    );
}

#[test]
fn cooldowns_are_saved_under_the_active_class_index() {
    let (mut world, mut db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    set_active_class(&mut world, PLAYER, 1);
    let until_tick = world.tick + 100_000;
    arm_reuse(
        &mut world,
        PLAYER,
        1234,
        model::SkillReuse {
            skill_level: 1,
            until_tick,
            total_ms: 600_000,
        },
    );
    let _ = drain_db(&mut db);

    save_all_players(&mut world);

    let cmds = drain_db(&mut db);
    let save = cmds
        .iter()
        .find_map(|c| match c {
            db::DbCommand::StorePlayer { save } => Some(save),
            _ => None,
        })
        .expect("a save went out");
    assert_eq!(
        save.class_index, 1,
        "the reuse rows go under the active slot, not index 0"
    );
    assert!(!save.skill_reuses.is_empty(), "and the cooldown is in them");
}

/// `ExSubjobInfo` carries the character's class list: the **base class first**,
/// then one row per subclass. The count was hard-coded to 0 — which predates
/// G17 landing subclasses — so the client's class list was always empty, even
/// for a character with no subclasses at all (the base row is never optional).
#[test]
fn ex_subjob_info_lists_the_base_class_and_subclasses() {
    use crate::model::SubClass;

    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    // Row size: index + classId + level (3 ints) + type (1 byte) = 13 bytes.
    const ROW: usize = 13;
    let base_only = {
        let p = world.objects.get_component::<Player>(&PLAYER).unwrap();
        crate::network::enter_world::ex_subjob_info(p)
    };

    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .subclasses = vec![
        SubClass {
            class_id: 20,
            class_index: 1,
            level: 55,
            exp: 0,
            sp: 0,
        },
        SubClass {
            class_id: 30,
            class_index: 2,
            level: 60,
            exp: 0,
            sp: 0,
        },
    ];
    let with_subs = {
        let p = world.objects.get_component::<Player>(&PLAYER).unwrap();
        crate::network::enter_world::ex_subjob_info(p)
    };

    assert_eq!(
        with_subs.len(),
        base_only.len() + 2 * ROW,
        "two subclasses add exactly two rows"
    );

    // The count field sits after: ex-opcode (3) + type (1) + classId (4) + race (4).
    let count_at = 12;
    let count = i32::from_le_bytes(base_only[count_at..count_at + 4].try_into().unwrap());
    assert_eq!(
        count, 1,
        "a character with no subclasses still reports its base class"
    );
}

/// `modifySubClass` (village-master cases 3/6/7): the slot is wiped — its
/// banked skills/hennas/shortcuts included, plus the DB rows — the new class
/// lands in the freed index, and the player is switched onto it. Java's
/// documented sharp edge holds: the old subclass is gone even when the
/// replacement fails, and the player reverts to base.
#[test]
fn modify_subclass_wipes_the_slot_and_replaces_it() {
    use crate::game_loop::character::subclass::modify_subclass;
    let (mut world, mut db_rx, _l) = sub_world();
    let _rx = ingame_player(&mut world, 1, PLAYER, 0, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    // Bank something under index 1 so the wipe is observable.
    {
        let p = world.objects.get_component_mut::<Player>(&PLAYER).unwrap();
        p.hennas_by_index.insert(1, vec![(1, 42)]);
        p.skills_by_index.insert(1, vec![(56, 1, 0)]);
    }
    drain_db(&mut db_rx);

    assert!(modify_subclass(&mut world, PLAYER, 1, 4), "replaced");
    let pl = p(&world);
    assert_eq!(pl.subclasses.len(), 1, "still one slot");
    assert_eq!(pl.subclasses[0].class_id, 4, "the new class holds it");
    assert_eq!(pl.subclasses[0].class_index, 1, "in the freed index");
    assert_eq!(pl.class_index, 1, "and the player switched onto it");
    assert!(
        !pl.hennas_by_index.contains_key(&1) || pl.hennas_by_index[&1].is_empty(),
        "the old slot's banked hennas are gone"
    );
    assert!(
        pl.skills_by_index.get(&1).is_none_or(|v| v.is_empty()),
        "…and its banked skills"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::WipeSubclassSlot { class_index: 1, .. })),
        "the slot's DB rows are wiped"
    );

    // Replacing with an unknown class still costs the slot (Java's warning).
    assert!(!modify_subclass(&mut world, PLAYER, 1, 9999), "refused");
    let pl = p(&world);
    assert!(pl.subclasses.is_empty(), "the slot is gone even on failure");
    assert_eq!(pl.class_index, 0, "reverted to the base class");
}
