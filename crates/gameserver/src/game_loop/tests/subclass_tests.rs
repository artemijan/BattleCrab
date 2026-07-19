//! Subclasses (G17 slice 2) — `addSubClass` / `setActiveClass`.

use super::*;

use crate::game_loop::subclass::{add_subclass, set_active_class, AddError, BASE_SUBCLASS_LEVEL};
use crate::model::Player;

const PLAYER: i32 = 2001;
const CID: u32 = 1;

fn sub_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
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
    assert_eq!(sub.level, BASE_SUBCLASS_LEVEL, "a subclass starts at 40");
    assert_eq!(p(&world).class_index, 0, "adding does not switch to it");
}

#[test]
fn the_base_class_cannot_be_taken_as_a_subclass() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let base = p(&world).base_class_id;

    assert_eq!(add_subclass(&mut world, PLAYER, base), Err(AddError::AlreadyHave));
}

#[test]
fn the_same_subclass_cannot_be_taken_twice() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();

    assert_eq!(add_subclass(&mut world, PLAYER, 3), Err(AddError::AlreadyHave));
}

#[test]
fn slots_are_capped_by_max_subclass() {
    let (mut world, _db, _l) = sub_world();
    world.cfg.character.max_subclass = 2;
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    add_subclass(&mut world, PLAYER, 4).unwrap();

    assert_eq!(add_subclass(&mut world, PLAYER, 5), Err(AddError::SlotsFull));
}

#[test]
fn an_unknown_class_is_refused() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    assert_eq!(add_subclass(&mut world, PLAYER, 9999), Err(AddError::UnknownClass));
}

#[test]
fn switching_to_a_subclass_swaps_class_and_level() {
    let (mut world, _db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();

    assert!(set_active_class(&mut world, PLAYER, 1));

    assert_eq!(p(&world).class_index, 1);
    assert_eq!(p(&world).class_id, 3, "now playing the subclass");
    assert_eq!(p(&world).level, BASE_SUBCLASS_LEVEL);
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
    assert_eq!(p(&world).level, BASE_SUBCLASS_LEVEL);
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
    assert_eq!(p(&world).class_index, 0, "and the active class is untouched");
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
                if *level == BASE_SUBCLASS_LEVEL
        )),
        "the new slot must reach the DB"
    );
}

// ---------------------------------------------------------------------------
// Per-class-index skill books (slice 3).

use crate::model::components::SkillBook;

fn knows(world: &World, skill_id: i32) -> bool {
    world.objects.get_component::<SkillBook>(&PLAYER).is_some_and(|b| b.0.contains_key(&skill_id))
}

/// Put a skill in the book that the class tree would never grant, standing in
/// for one learned by hand from a trainer.
fn learn_by_hand(world: &mut World, skill_id: i32) {
    world.objects.get_component_mut::<SkillBook>(&PLAYER).unwrap().0.insert(skill_id, 3);
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
    assert!(!knows(&world, 7777), "the subclass has its own, empty book");
    set_active_class(&mut world, PLAYER, 0);

    assert!(knows(&world, 7777), "the base class got its hand-learned skill back");
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

    assert!(knows(&world, 8002), "slot 2's own skill");
    assert!(!knows(&world, 8001), "and not slot 1's");

    set_active_class(&mut world, PLAYER, 1);
    assert!(knows(&world, 8001));
    assert!(!knows(&world, 8002));
}

#[test]
fn the_save_carries_every_slots_book() {
    let (mut world, mut db, _l) = sub_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    add_subclass(&mut world, PLAYER, 3).unwrap();
    set_active_class(&mut world, PLAYER, 1);
    learn_by_hand(&mut world, 8001);
    let _ = drain_db(&mut db);

    crate::game_loop::net::save_all_players(&mut world);

    let cmds = drain_db(&mut db);
    let save = cmds.iter().find_map(|c| match c {
        db::DbCommand::StorePlayer { save } => Some(save),
        _ => None,
    });
    let save = save.expect("a save went out");
    assert_eq!(save.class_index, 1, "the active index is recorded");
    assert!(save.skills.iter().any(|(id, _)| *id == 8001), "the active book carries the new skill");
    assert!(save.skills_by_index.contains_key(&0), "and the base slot's book is banked alongside");
}
