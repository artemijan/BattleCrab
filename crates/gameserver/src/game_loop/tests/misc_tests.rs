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
