//! The alliance master menu every village master carries
//! (`village_master/AllianceMaster`).

use super::*;

/// AllianceMaster's dist page, run through the same strip the cache applies.
fn alliance_page(name: &str) -> String {
    dist_page(
        &format!("data/scripts/village_master/AllianceMaster/{name}"),
        NPC_OID,
    )
}

/// Talking to any village master opens the alliance menu — with no clan check,
/// so a clanless player does see the two buttons.
#[test]
fn alliance_master_talk_opens_the_menu_without_a_clan() {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 70, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .clan_id,
        0,
        "fixture player is clanless"
    );
    drain(&mut rx);

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Script AllianceMaster")),
    );

    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("a reply");
    assert_eq!(
        html,
        alliance_page("9001-01.htm"),
        "the menu, not the clan refusal"
    );
}

/// Every page *except* the menu needs a clan. The asymmetry is the whole
/// script: the clanless player is refused on click, not on open.
#[test]
fn alliance_master_gates_every_page_but_the_menu_on_having_a_clan() {
    // (clan id, requested page, expected page)
    let cases: [(i32, &str, &str); 6] = [
        (0, "9001-02.htm", "9001-04.htm"), // create, clanless → refused
        (0, "9001-03.htm", "9001-04.htm"), // dissolve, clanless → refused
        (0, "9001-01.htm", "9001-01.htm"), // the menu is NOT gated
        (7, "9001-02.htm", "9001-02.htm"), // in a clan → the real page
        (7, "9001-03.htm", "9001-03.htm"),
        (7, "9001-01.htm", "9001-01.htm"),
    ];
    for (clan_id, requested, expected) in cases {
        let (mut world, _db_rx, _link_rx) = quest_test_world();
        add_test_npc(&mut world, NPC_OID, 30026, "VillageMaster", 70, 100, 0, 0);
        let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
        world
            .objects
            .get_component_mut::<Player>(&3001)
            .unwrap()
            .clan_id = clan_id;
        drain(&mut rx);

        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Script AllianceMaster {requested}")),
        );

        let html = drain(&mut rx)
            .iter()
            .find_map(|p| decode_npc_html(p))
            .expect("a reply");
        assert_eq!(
            html,
            alliance_page(expected),
            "clan {clan_id} asking for {requested} should get {expected}"
        );
    }
}

/// The four pages exist, and they are numbered against the **virtual** NPC id
/// 9001 — no real master ships a page of its own.
#[test]
fn alliance_master_pages_exist_in_dist() {
    const DIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/village_master/AllianceMaster/"
    );
    for n in 1..=4 {
        let p = format!("{DIST}9001-{n:02}.htm");
        assert!(ships(&p), "missing 9001-{n:02}.htm");
    }
    for npc in [30026, 30031, 30913] {
        let p = format!("{DIST}{npc}-01.htm");
        assert!(!ships(&p), "pages are 9001-*, not per-NPC");
    }
}
