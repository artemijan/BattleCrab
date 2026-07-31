use super::*;

/// The generic `Link <file>` bypass: whitelisted pages are served from
/// `data/html/` through a plain `NpcHtmlMessage` anchored at the last
/// clicked NPC; non-whitelisted or path-escaping requests answer an empty
/// html (Java's null content) or drop.
#[test]
fn link_bypass_serves_whitelisted_html_only() {
    let (mut world, ..) = quest_test_world();
    add_test_npc(&mut world, NPC_OID, 30001, "Folk", 5, 100, 0, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    world.objects.add_components(&3001, LastFolkNpc(NPC_OID));

    // Whitelisted page (real dist file).
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Link common/craft_01.htm"));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("html window");
    assert!(html.contains("Dualsword"), "served the real page: {html}");

    // Non-whitelisted page: empty html window, not the file.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Link merchant/30001.htm"));
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| decode_npc_html(p))
        .expect("empty html window");
    assert!(html.is_empty());

    // Path traversal: dropped outright.
    handle_request_bypass_to_server(&mut world, 1, &bypass_body("Link ../config/Server.ini"));
    assert!(drain(&mut rx).is_empty());
}

/// Stored CP survives a relog. Java `Player.restore` reads `curCp` next to
/// `curHp`/`curMp` and replays it through `setCurrentCp` (which clamps to the
/// freshly recomputed max) — the port used to hard-code `cur_cp: 0.0` at spawn,
/// so every login started at 0 CP and visibly regenerated back up.
#[test]
fn stored_cp_is_restored_on_login() {
    let (mut world, ..) = test_world();
    // The stock test template has no CP table at all, so max CP would be 0 and
    // both assertions below would hold vacuously. Give level 1 a real pool.
    let mut t = human_fighter_template();
    t.cp_table = vec![0.0; 90];
    t.cp_table[1] = 100.0;
    world.data.player_templates = crate::data::PlayerTemplateData::from_vec(vec![t]);

    let mut chr = dummy_char(3101, "Restored");
    chr.cur_cp = 42.0;
    let bundle = Player::from_char(&world.data, &chr);
    assert!(
        bundle.player_vitals.max_cp >= 100,
        "template CP pool is live, so the assertion below is not vacuous"
    );
    assert_eq!(
        bundle.player_vitals.cur_cp, 42.0,
        "stored curCp comes back on login"
    );

    // Java's `setCurrentCp` clamps to the max; max CP is recomputed from the
    // template, so a stale-high stored value must not survive it.
    chr.cur_cp = 10_000.0;
    let bundle = Player::from_char(&world.data, &chr);
    assert!(
        (bundle.player_vitals.cur_cp - f64::from(bundle.player_vitals.max_cp)).abs() < 1.0,
        "over-max curCp is clamped, got {} vs max {}",
        bundle.player_vitals.cur_cp,
        bundle.player_vitals.max_cp
    );
}
