//! Nobless (G17 slice 1) — `//setnoble`, the noble skill tree, and the
//! noblesse-gated content it unlocks.

use super::*;

use crate::model::Player;

const PLAYER: i32 = 2001;
const CID: u32 = 1;

fn is_noble(world: &World) -> bool {
    world
        .objects
        .get_component::<Player>(&PLAYER)
        .unwrap()
        .is_noble
}

/// Register a two-skill noble tree so the grant/remove is observable without
/// depending on the datapack.
fn noble_world() -> (World, db::CmdRx, UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    world
        .data
        .skill_trees
        .set_noble_skills_for_test(vec![(1323, 1), (326, 1)]);
    (world, db, l)
}

// ---------------------------------------------------------------------------

#[test]
fn a_new_character_is_not_noble() {
    let (mut world, _db, _l) = noble_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    assert!(
        !is_noble(&world),
        "nobless is earned, not granted at creation"
    );
}

#[test]
fn setnoble_grants_the_noble_skill_tree() {
    let (mut world, _db, _l) = noble_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    crate::game_loop::admin::hero::set_noble(&mut world, PLAYER, true);

    assert!(is_noble(&world));
    assert!(knows(&world, 1323, PLAYER), "Noblesse Blessing granted");
    assert!(
        knows(&world, 326, PLAYER),
        "Build Advanced Headquarters granted"
    );
}

#[test]
fn removing_nobless_takes_the_skills_back() {
    let (mut world, _db, _l) = noble_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    crate::game_loop::admin::hero::set_noble(&mut world, PLAYER, true);

    crate::game_loop::admin::hero::set_noble(&mut world, PLAYER, false);

    assert!(!is_noble(&world));
    assert!(
        !knows(&world, 1323, PLAYER),
        "the noble tree is removed again"
    );
    assert!(!knows(&world, 326, PLAYER));
}

#[test]
fn nobless_survives_a_class_that_is_not_the_base_class() {
    // Unlike hero status, Java does NOT gate `setNoble` on being on the base
    // class — nobless belongs to the character, so a subclass keeps it.
    let (mut world, _db, _l) = noble_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&PLAYER).unwrap();
        p.class_id = 10; // away from base_class_id
    }

    crate::game_loop::admin::hero::set_noble(&mut world, PLAYER, true);

    assert!(is_noble(&world));
    assert!(
        knows(&world, 1323, PLAYER),
        "granted regardless of the active class"
    );
}

#[test]
fn nobless_is_written_to_the_save() {
    // Without this the flag is lost on restart — `characters.nobless` is read
    // at load but was never part of the UPDATE.
    let (mut world, mut db, _l) = noble_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    crate::game_loop::admin::hero::set_noble(&mut world, PLAYER, true);
    let _ = drain_db(&mut db);

    save_all_players(&mut world);

    let cmds = drain_db(&mut db);
    let saved = cmds.iter().find_map(|c| match c {
        db::DbCommand::StorePlayer { save } => Some(save.base.noble),
        _ => None,
    });
    assert_eq!(saved, Some(true), "the nobless flag must reach the DB");
}

/// The real datapack's noble tree.
#[test]
fn real_dist_noble_tree_has_the_expected_skills() {
    let trees = dist::skill_trees();
    let noble = trees.noble_skills();
    assert_eq!(noble.len(), 8, "nobleSkillTree.xml ships 8 skills");
    // Noblesse Blessing (1323) and Build Advanced Headquarters (326) are the
    // two that other systems gate on.
    assert!(noble.iter().any(|(id, _)| *id == 1323), "Noblesse Blessing");
    assert!(
        noble.iter().any(|(id, _)| *id == 326),
        "Build Advanced Headquarters"
    );
}
