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
