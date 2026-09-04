//! `admin/cursed_weapons.rs` — the cursed-weapon panel, giving and removing
//! one, and the goto that follows it.

use super::*;

/// `//cw_info` lists both cursed weapons; `//cw_add` activates one on the GM
/// (item + karma swap + cursed-weapon flag + DB persist + world announce);
/// `//cw_remove` reverses it. Port of `AdminCursedWeapons`.
#[test]
fn admin_cursed_weapons_info_add_remove() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, _db_tx, mut db_rx, _link) = admin_world();
    world.data.root = ROOT.to_string();
    // Boot-equivalent: load config, then build the runtime list (as net.rs does).
    world.data.cursed_weapons = crate::data::CursedWeaponData::load_from(ROOT);
    world.cursed_weapons = world
        .data
        .cursed_weapons
        .weapons
        .iter()
        .cloned()
        .map(|mut cw| {
            cw.skill_max_level = (1..=100)
                .take_while(|l| world.data.skill_data.get(cw.skill_id, *l).is_some())
                .last()
                .unwrap_or(1);
            cw
        })
        .collect();
    assert_eq!(
        world.cursed_weapons.len(),
        2,
        "Zariche + Akamanah loaded from XML"
    );

    world.id_pool = 0x3000_0000..0x3000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 7001, 100);
    drain(&mut rx);
    let original_rep = world
        .objects
        .get_component::<Player>(&7001)
        .unwrap()
        .reputation;

    // //cw_info — both weapons inactive.
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("cw_info")].concat(),
    );
    let info: Vec<String> = drain(&mut rx)
        .iter()
        .filter_map(|p| sysmsg_text(p))
        .collect();
    assert!(
        info.iter()
            .any(|t| t.contains("Demonic Sword Zariche (8190)")),
        "lists Zariche"
    );
    assert!(
        info.iter().any(|t| t.contains("Don't exist in the world.")),
        "inactive status"
    );

    // //cw_add 8190 — Java marks it confirmDlg, so it prompts first; the "yes"
    // reply then activates it on the GM (no target).
    drain_db(&mut db_rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_add 8190"),
        ]
        .concat(),
    );
    let prompt = drain(&mut rx);
    assert_eq!(
        prompt
            .iter()
            .filter(|p| p[0] == server_packets::opcodes::CONFIRM_DLG)
            .count(),
        1,
        "confirm prompt"
    );
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    let add_pkts = drain(&mut rx);
    let p = world.objects.get_component::<Player>(&7001).unwrap();
    assert_eq!(p.cursed_weapon_equipped_id, 8190, "cursed-weapon flag set");
    assert_eq!(
        p.reputation, -9_999_999,
        "karma slammed to the cursed value"
    );
    let cw = world
        .cursed_weapons
        .iter()
        .find(|c| c.item_id == 8190)
        .unwrap();
    assert!(
        cw.is_activated && cw.player_id == 7001,
        "weapon activated on the wielder"
    );
    assert_eq!(
        cw.player_reputation, original_rep,
        "saved the wielder's real karma"
    );
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            db::DbCommand::StoreCursedWeapon {
                item_id: 8190,
                char_id: 7001,
                ..
            }
        )),
        "persisted"
    );
    assert!(
        ids_after_opcode(&add_pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::THE_OWNER_OF_S2_HAS_APPEARED_IN_THE_S1_REGION),
        "appearance announced"
    );

    // //cw_info now shows the wielder.
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("cw_info")].concat(),
    );
    let info: Vec<String> = drain(&mut rx)
        .iter()
        .filter_map(|p| sysmsg_text(p))
        .collect();
    assert!(
        info.iter().any(|t| t.contains("Player holding: P7001")),
        "shows the holder"
    );

    // //cw_remove 8190 — end of life restores the wielder + resets state.
    drain_db(&mut db_rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_remove 8190"),
        ]
        .concat(),
    );
    let rm_pkts = drain(&mut rx);
    let p = world.objects.get_component::<Player>(&7001).unwrap();
    assert_eq!(p.cursed_weapon_equipped_id, 0, "flag cleared");
    assert_eq!(p.reputation, original_rep, "karma restored");
    let cw = world
        .cursed_weapons
        .iter()
        .find(|c| c.item_id == 8190)
        .unwrap();
    assert!(!cw.is_active(), "weapon reset to not-in-world");
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, db::DbCommand::RemoveCursedWeapon { item_id: 8190 })),
        "db row dropped"
    );
    assert!(
        ids_after_opcode(&rm_pkts, server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::S1_HAS_DISAPPEARED),
        "disappearance announced"
    );
}

/// The `cwinfo.htm` panel draws its buttons from the weapon's live state —
/// "Give to Target" while it is nowhere, "Remove"/"Go" once it is live. Java
/// returns from `//cw_add` without touching the window, so the page kept
/// offering "Give to Target" and could not remove the sword it had just handed
/// out; the GM had to back out and re-enter `//cw_info_menu`. Both commands now
/// redraw the panel from the state they just changed.
#[test]
fn cursed_weapon_panel_redraws_after_give_and_remove() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, _db_tx, _db_rx, _link) = admin_world();
    world.data.root = ROOT.to_string();
    world.data.cursed_weapons = crate::data::CursedWeaponData::load_from(ROOT);
    world.cursed_weapons = world
        .data
        .cursed_weapons
        .weapons
        .iter()
        .cloned()
        .map(|mut cw| {
            cw.skill_max_level = (1..=100)
                .take_while(|l| world.data.skill_data.get(cw.skill_id, *l).is_some())
                .last()
                .unwrap_or(1);
            cw
        })
        .collect();
    world.id_pool = 0x3000_0000..0x3000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 7003, 100);
    drain(&mut rx);

    // The panel as the GM first sees it: nothing in the world, so the only
    // button on offer is "Give to Target".
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_info_menu"),
        ]
        .concat(),
    );
    let first = last_admin_html(&drain(&mut rx)).expect("the panel opens");
    assert!(
        first.contains("admin_cw_add 8190") && !first.contains("admin_cw_remove 8190"),
        "a weapon that is nowhere offers only Give to Target"
    );

    // "Give to Target" → confirm → the weapon is live on the GM.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_add 8190"),
        ]
        .concat(),
    );
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    let after_add = last_admin_html(&drain(&mut rx))
        .expect("the give redraws the panel instead of leaving it stale");
    assert!(
        after_add.contains("admin_cw_remove 8190"),
        "the redrawn page can remove what it just gave out"
    );
    assert!(
        after_add.contains("Weilder:"),
        "and shows the wielder row rather than the not-in-world one"
    );

    // "Remove" → the row goes back to offering the weapon.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_remove 8190"),
        ]
        .concat(),
    );
    let after_remove = last_admin_html(&drain(&mut rx)).expect("the remove redraws the panel too");
    assert!(
        after_remove.contains("admin_cw_add 8190")
            && !after_remove.contains("admin_cw_remove 8190"),
        "back to Give to Target once the weapon is gone"
    );
}

/// `//cw_add 8689` then `//cw_remove 8689` — the Akamanah passive (3629) has to
/// be gone from the live book *and* from every list the next flush writes.
///
/// The live half always worked; the flush half did not. `PlayerSaveData` ships
/// both the live `SkillBook` **and** `Player.skills_by_index`, a login-time
/// snapshot that still carried an entry for the class being played, and
/// `store_player` inserts both. So after a relog while cursed (which is what
/// puts 3629 into `character_skills`, and therefore into the banked map at the
/// next login), removing the weapon dropped the `MaxCp` pump live but the
/// banked row put 3629 straight back on the next flush — the reported "max CP
/// returns to the cursed value after a relog".
#[test]
fn cursed_weapon_skill_not_persisted_after_removal() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, _db_tx, mut db_rx, _link) = admin_world();
    world.data.root = ROOT.to_string();
    world.data.skill_data = dist::skills_owned();
    world.data.transforms = crate::data::TransformData::load_from(ROOT);
    world.data.cursed_weapons = crate::data::CursedWeaponData::load_from(ROOT);
    world.cursed_weapons = world
        .data
        .cursed_weapons
        .weapons
        .iter()
        .cloned()
        .map(|mut cw| {
            cw.skill_max_level = (1..=100)
                .take_while(|l| world.data.skill_data.get(cw.skill_id, *l).is_some())
                .last()
                .unwrap_or(1);
            cw
        })
        .collect();
    world.id_pool = 0x3000_0000..0x3000_0100;
    let mut rx = ingame_player_access(&mut world, 1, 7009, 100);
    drain(&mut rx);
    drain_db(&mut db_rx);

    let max_cp_before = world
        .objects
        .get_component::<PlayerVitals>(&7009)
        .unwrap()
        .max_cp;

    // //cw_add 8689 (+ confirm).
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_add 8689"),
        ]
        .concat(),
    );
    drain(&mut rx);
    on_packet(
        &mut world,
        1,
        [
            vec![cop::DLG_ANSWER],
            dlg_answer_body(server_packets::S1_3_MESSAGE_ID, 1, 0),
        ]
        .concat(),
    );
    drain(&mut rx);

    let book_has = |w: &World| {
        w.objects
            .get_component::<SkillBook>(&7009)
            .unwrap()
            .0
            .contains_key(&3629)
    };
    assert!(book_has(&world), "cursed passive granted");
    let max_cp_cursed = world
        .objects
        .get_component::<PlayerVitals>(&7009)
        .unwrap()
        .max_cp;
    assert!(
        max_cp_cursed > max_cp_before,
        "curse pumps MaxCp ({max_cp_before} -> {max_cp_cursed})"
    );

    // An autosave while cursed writes 3629 to `character_skills`.
    let cursed_save = build_save_data(&world, 7009).expect("snapshot");
    assert!(
        cursed_save.skills.iter().any(|(id, _, _)| *id == 3629),
        "while cursed, the flush carries 3629"
    );

    // A relog *while cursed* is the state that matters: the character loads
    // with 3629 already in `character_skills`, so `Player.skills_by_index`
    // banks it for the active class index. Seed exactly that.
    {
        let live: Vec<(i32, i32, i32)> = world
            .objects
            .get_component::<SkillBook>(&7009)
            .unwrap()
            .0
            .iter()
            .map(|(id, lvl)| (*id, *lvl, 0))
            .collect();
        let p = world.objects.get_component_mut::<Player>(&7009).unwrap();
        let ci = p.class_index;
        p.skills_by_index.insert(ci, live);
    }

    // //cw_remove 8689.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("cw_remove 8689"),
        ]
        .concat(),
    );
    drain(&mut rx);

    assert!(!book_has(&world), "cursed passive dropped from the book");
    let max_cp_after = world
        .objects
        .get_component::<PlayerVitals>(&7009)
        .unwrap()
        .max_cp;
    assert_eq!(max_cp_after, max_cp_before, "MaxCp back to normal live");

    // The flush that follows must not carry it, or the relog reloads it.
    let clean_save = build_save_data(&world, 7009).expect("snapshot");
    let ids: Vec<i32> = clean_save.skills.iter().map(|(id, _, _)| *id).collect();
    let by_index: Vec<(i32, Vec<i32>)> = clean_save
        .skills_by_index
        .iter()
        .map(|(i, v)| (*i, v.iter().map(|(id, _, _)| *id).collect()))
        .collect();
    assert!(
        !ids.contains(&3629),
        "3629 still in the active flush list: {ids:?}"
    );
    assert!(
        !by_index.iter().any(|(_, v)| v.contains(&3629)),
        "3629 still in a banked per-index flush list: {by_index:?}"
    );
}

/// `//cw_goto` tries the holder first, then the dropped item, and only reports
/// "isn't in the World" when neither has a position.
///
/// The fall-through is the whole behaviour: a cursed weapon can be flagged
/// activated while its holder carries no position (offline, mid-teleport), and
/// Java still checks the ground item before giving up. Pinned because the
/// command has no other test and the branch order is easy to flatten.
#[test]
fn cw_goto_falls_through_from_the_holder_to_the_dropped_item() {
    use model::components::space::Position;
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, ..) = admin_world();
    world.data.root = ROOT.to_string();
    world.data.cursed_weapons = crate::data::CursedWeaponData::load_from(ROOT);
    world.cursed_weapons = world.data.cursed_weapons.weapons.clone();
    let item_id = world.cursed_weapons[0].item_id;

    let mut gm_rx = ingame_player_access(&mut world, 1, 9101, 100);
    // Both anchors must be *registered* objects — components attached to an
    // unknown object id are silently dropped by the store.
    const HOLDER: i32 = 9102;
    const GROUND_ITEM: i32 = 9103;
    let _holder_rx = ingame_player_access(&mut world, 2, HOLDER, 0);
    let _item_rx = ingame_player_access(&mut world, 3, GROUND_ITEM, 0);
    // The holder is flagged but has NO position — the case that must fall through.
    world.objects.remove_component::<Position>(&HOLDER);
    set_position(&mut world, GROUND_ITEM, (84_000, 148_000, -3400));
    world.cursed_weapons[0].is_activated = true;
    world.cursed_weapons[0].player_id = HOLDER;
    world.cursed_weapons[0].is_dropped = true;
    world.cursed_weapons[0].dropped_item_oid = GROUND_ITEM;
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin(&format!("cw_goto {item_id}")));

    let at = world
        .objects
        .get_component::<Position>(&9101)
        .copied()
        .expect("gm position");
    assert_eq!(
        (at.x, at.y),
        (84_000, 148_000),
        "the GM landed on the dropped item after the position-less holder"
    );

    // With neither anchor placed, the command reports the weapon as absent.
    world.cursed_weapons[0].is_dropped = false;
    world.cursed_weapons[0].dropped_item_oid = 0;
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin(&format!("cw_goto {item_id}")));
    let after = world
        .objects
        .get_component::<Position>(&9101)
        .copied()
        .expect("gm position");
    assert_eq!((after.x, after.y), (at.x, at.y), "no anchor, no teleport");
    assert!(
        count_system_messages(&drain(&mut gm_rx)) >= 1,
        "the \"isn't in the World\" line is sent"
    );
}
