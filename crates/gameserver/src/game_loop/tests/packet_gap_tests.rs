//! Row 15's base opcodes — the client packets Java wires that the port had no
//! arm for. Each is a *re-request*: the client asking for state it was already
//! sent, usually because a window was reopened.
//!
//! `SnoopQuit` (0xB4) is covered in `snoop_tests` beside the rest of snooping.

use super::*;
use crate::model::Player;
use crate::network::server_packets::sm_ids;

/// The ids of every `SystemMessage` in a drained batch, in order.
fn sm_ids_of(out: &[Vec<u8>]) -> Vec<i16> {
    out.iter().filter_map(|p| sysmsg_id(p)).collect()
}

/// `SkillList` — Java writes 0x5F.
const SKILL_LIST_OPCODE: u8 = 0x5F;

/// **`RequestMagicSkillList` (0x38) resends the skill list.**
#[test]
fn the_skill_list_can_be_re_requested() {
    let (mut world, _db, _l) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    let mut body = vec![cop::REQUEST_MAGIC_SKILL_LIST];
    body.extend_from_slice(&3001i32.to_le_bytes());
    on_packet(&mut world, 1, body);

    assert!(
        has_opcode(&drain(&mut rx), SKILL_LIST_OPCODE),
        "the client gets its skill list back"
    );
}

/// **A skill-list request naming someone else is refused.**
///
/// This is the entire reason the packet carries an object id it could have
/// derived from the connection, and the check is what stops a hand-built
/// packet reading another character's book.
#[test]
fn a_skill_list_request_for_another_player_is_refused() {
    let (mut world, _db, _l) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _other = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    drain(&mut rx);

    let mut body = vec![cop::REQUEST_MAGIC_SKILL_LIST];
    body.extend_from_slice(&3002i32.to_le_bytes());
    on_packet(&mut world, 1, body);

    assert!(
        !has_opcode(&drain(&mut rx), SKILL_LIST_OPCODE),
        "no list is sent for a mismatched object id"
    );
}

fn member(char_id: i32, name: &str) -> model::clan::ClanMember {
    model::clan::ClanMember {
        char_id,
        name: name.into(),
        level: 1,
        class_id: 0,
        sex: 0,
        race: 0,
        power_grade: 5,
        title: String::new(),
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
    }
}

fn with_clan(world: &mut World, clan_id: i32, members: &[i32]) {
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: "Testers".into(),
            leader_id: members[0],
            level: 0,
            reputation_score: 0,
            castle_id: 0,
            members: members
                .iter()
                .map(|&o| member(o, &format!("P{o}")))
                .collect(),
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        },
    );
    for &oid in members {
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.clan_id = clan_id;
        }
    }
}

/// **`RequestPledgeMemberList` (0x4D) resends the roster.**
#[test]
fn the_clan_roster_can_be_re_requested() {
    let (mut world, _db, _l) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    with_clan(&mut world, 5000, &[3001, 3002]);
    drain(&mut rx);

    on_packet(&mut world, 1, vec![cop::REQUEST_PLEDGE_MEMBER_LIST]);

    assert!(
        has_opcode(
            &drain(&mut rx),
            server_packets::opcodes::PLEDGE_SHOW_MEMBER_LIST_ALL
        ),
        "the roster goes back out"
    );
}

/// **A clanless player is answered with nothing at all** — Java's
/// `if (clan != null)` has no else branch, so there is no empty-roster packet
/// and no error.
///
/// Unlike the rest of this file this one is *not* falsifiable by disabling the
/// handler: with the arm gone nothing is sent either, which is what it
/// asserts. It guards against the opposite regression — someone deciding a
/// clanless roster should be an empty list rather than silence.
#[test]
fn a_clanless_roster_request_sends_nothing() {
    let (mut world, _db, _l) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    on_packet(&mut world, 1, vec![cop::REQUEST_PLEDGE_MEMBER_LIST]);

    assert!(
        drain(&mut rx).is_empty(),
        "no roster, and no complaint either"
    );
}

/// **`/gmlist` from a plain player reports no GMs even with one online.**
///
/// Not a stub: `GMStartupAutoList = False` on this dist, so `AdminData.addGm`
/// flags every GM hidden, and `sendListToPlayer` passes `includeHidden =
/// player.isGM()`. A normal player's list is therefore always empty. The test
/// exists because "GM online but list says none" reads like a bug.
#[test]
fn gmlist_hides_every_gm_from_a_plain_player() {
    let (mut world, ..) = admin_world();
    let _gm = ingame_player_access(&mut world, 1, 3001, 100);
    // `EnterWorld` → `AdminData.addGm(player, hidden)`; the fixture's login
    // shortcut skips it, and the flag is only ever set there.
    assert!(
        !world.data.gm.startup_auto_list,
        "this dist ships GMStartupAutoList = False"
    );
    crate::game_loop::admin::apply_gm_startup(&mut world, 1, 3001);
    let mut rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    drain(&mut rx);

    on_packet(&mut world, 2, vec![cop::REQUEST_GM_LIST]);

    let msgs = sm_ids_of(&drain(&mut rx));
    assert!(
        msgs.contains(&sm_ids::THERE_ARE_NO_GMS_CURRENTLY_VISIBLE),
        "the player is told no GMs are visible, got {msgs:?}"
    );
    assert!(!msgs.contains(&sm_ids::GM_LIST), "and gets no header");
}

/// **A GM asking sees every GM, annotated `(invis)`.** `includeHidden` is
/// `player.isGM()`, which is the whole difference between the two callers.
#[test]
fn gmlist_shows_hidden_gms_to_a_gm() {
    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 3001, 100);
    let _other_gm = ingame_player_access(&mut world, 2, 3002, 100);
    crate::game_loop::admin::apply_gm_startup(&mut world, 1, 3001);
    crate::game_loop::admin::apply_gm_startup(&mut world, 2, 3002);
    drain(&mut rx);

    on_packet(&mut world, 1, vec![cop::REQUEST_GM_LIST]);

    let msgs = sm_ids_of(&drain(&mut rx));
    assert!(msgs.contains(&sm_ids::GM_LIST), "header sent, got {msgs:?}");
    assert_eq!(
        msgs.iter().filter(|&&m| m == sm_ids::GM_C1).count(),
        2,
        "one line per GM"
    );
    assert!(
        !msgs.contains(&sm_ids::THERE_ARE_NO_GMS_CURRENTLY_VISIBLE),
        "and no empty-list message"
    );
}

/// **`FinishRotating` (0x5C) commits the new heading server-side.**
///
/// This is the half that matters: without it a keyboard turn is a client-side
/// animation the server never learns about, and the next `ValidatePosition`
/// snaps the character back to the heading it had before.
#[test]
fn a_keyboard_turn_commits_the_heading_and_is_broadcast() {
    let (mut world, _db, _l) = quest_test_world();
    let _turner = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    // A second player standing in range is who the rotation is *for*.
    let mut watcher_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    drain(&mut watcher_rx);

    let mut body = vec![cop::FINISH_ROTATING];
    body.extend_from_slice(&32_000i32.to_le_bytes());
    body.extend_from_slice(&0i32.to_le_bytes());
    on_packet(&mut world, 1, body);

    assert_eq!(
        world
            .objects
            .get_component::<Position>(&3001)
            .unwrap()
            .heading,
        32_000,
        "the heading is now the server's too"
    );
    assert!(
        has_opcode(&drain(&mut watcher_rx), 0x61),
        "onlookers are told the turn settled (StopRotation)"
    );
}

/// **`StartRotating` (0x5B) is broadcast but changes no state.** Java sends
/// the opening half to onlookers only and leaves the heading alone until
/// `FinishRotating` — mid-turn the server has no committed answer.
#[test]
fn the_opening_half_of_a_turn_moves_no_heading() {
    let (mut world, _db, _l) = quest_test_world();
    let _turner = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut watcher_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let before = world
        .objects
        .get_component::<Position>(&3001)
        .unwrap()
        .heading;
    drain(&mut watcher_rx);

    let mut body = vec![cop::START_ROTATING];
    body.extend_from_slice(&32_000i32.to_le_bytes());
    body.extend_from_slice(&1i32.to_le_bytes());
    on_packet(&mut world, 1, body);

    assert_eq!(
        world
            .objects
            .get_component::<Position>(&3001)
            .unwrap()
            .heading,
        before,
        "still un-turned until the finish arrives"
    );
    assert!(
        has_opcode(&drain(&mut watcher_rx), 0x7A),
        "but onlookers see the spin start (StartRotation)"
    );
}

/// **`KeyboardMovement = False` silences both halves**, which is Java's own
/// early return and the only thing the config does.
#[test]
fn rotation_packets_are_inert_when_keyboard_movement_is_off() {
    let (mut world, _db, _l) = quest_test_world();
    world.cfg.character.keyboard_movement = false;
    let _turner = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let mut watcher_rx = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    drain(&mut watcher_rx);

    let mut body = vec![cop::FINISH_ROTATING];
    body.extend_from_slice(&32_000i32.to_le_bytes());
    body.extend_from_slice(&0i32.to_le_bytes());
    on_packet(&mut world, 1, body);

    assert_ne!(
        world
            .objects
            .get_component::<Position>(&3001)
            .unwrap()
            .heading,
        32_000,
        "the heading is not committed"
    );
    assert!(drain(&mut watcher_rx).is_empty(), "and nothing is sent");
}

/// A world whose datapack root points at the real `dist/game`, so
/// `data/html/...` resolves to actual files.
fn dist_html_world() -> World {
    let (mut world, _db, _l) = quest_test_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    world
}

/// `shop_world` plus an actually-equipable product, since every stock fixture
/// item has `body_part: 0` and so occupies no paperdoll slot at all — which
/// is exactly what `RequestPreviewItem` filters on.
fn preview_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, _db, rx) = shop_world();
    world.cfg.general.wear_price = 10;
    world.cfg.general.wear_delay = 5;
    world
        .data
        .item_data
        .insert_for_test(crate::data::item_data::template::ItemTemplate {
            item_id: PREVIEW_CHEST,
            name: "Preview Tunic".into(),
            kind: crate::data::item_data::kinds::ItemKind::Armor,
            body_part: crate::data::item_data::SLOT_CHEST,
            type2: 1,
            ..crate::data::item_data::template::ItemTemplate::for_test()
        });
    world
        .data
        .buy_lists
        .insert_for_test(crate::data::buy_list_data::BuyList {
            list_id: 3,
            npcs: vec![30001],
            products: vec![
                crate::data::buy_list_data::Product::unlimited(PREVIEW_CHEST, 100, 0),
                // An etc item with no paperdoll slot — Java `continue`s past
                // it, so it must cost nothing and show nothing.
                crate::data::buy_list_data::Product::unlimited(1061, 10, 0),
            ],
        });
    (world, rx)
}

/// The equipable line `preview_world` adds.
const PREVIEW_CHEST: i32 = 7_770_001;

/// UTF-16LE + NUL terminator, the wire form `read_string` expects.
fn utf16(s: &str) -> Vec<u8> {
    let mut out: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    out.extend_from_slice(&[0, 0]);
    out
}

/// **A noble titles themselves with no clan involved.** The self-branch runs
/// before every clan test, which is what makes nobless a personal cosmetic.
#[test]
fn a_noble_can_title_themselves() {
    let (mut world, _db, _l) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.is_noble = true;
        p.clan_id = 0; // no clan at all
    }
    let name = world
        .objects
        .get_component::<Player>(&3001)
        .unwrap()
        .name
        .clone();
    drain(&mut rx);

    let mut body = vec![cop::REQUEST_GIVE_NICK_NAME];
    body.extend_from_slice(&utf16(&name));
    body.extend_from_slice(&utf16("Wanderer"));
    on_packet(&mut world, 1, body);

    assert_eq!(
        world.objects.get_component::<Player>(&3001).unwrap().title,
        "Wanderer"
    );
    let out = drain(&mut rx);
    assert!(
        sm_ids_of(&out).contains(&sm_ids::YOUR_TITLE_HAS_BEEN_CHANGED),
        "the wearer is told"
    );
    assert!(
        has_opcode(&out, server_packets::opcodes::NICK_NAME_CHANGED),
        "and onlookers get NicknameChanged"
    );
}

/// **A non-noble titling themselves falls through to the clan branch** and is
/// refused for want of the privilege — the self-shortcut is nobless-only.
#[test]
fn a_commoner_cannot_title_themselves() {
    let (mut world, _db, _l) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let name = world
        .objects
        .get_component::<Player>(&3001)
        .unwrap()
        .name
        .clone();
    drain(&mut rx);

    let mut body = vec![cop::REQUEST_GIVE_NICK_NAME];
    body.extend_from_slice(&utf16(&name));
    body.extend_from_slice(&utf16("Wanderer"));
    on_packet(&mut world, 1, body);

    assert_eq!(
        world.objects.get_component::<Player>(&3001).unwrap().title,
        "",
        "no title granted"
    );
    assert!(
        sm_ids_of(&drain(&mut rx)).contains(&sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT),
        "refused on the privilege, not the nobless"
    );
}

/// **A clan leader below level 3 cannot grant titles**, even holding the
/// privilege — the level gate sits after the privilege check.
#[test]
fn granting_a_title_needs_clan_level_three() {
    let (mut world, _db, _l) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _b = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    with_clan(&mut world, 5000, &[3001, 3002]);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_leader = true;
    drain(&mut rx);

    let mut body = vec![cop::REQUEST_GIVE_NICK_NAME];
    body.extend_from_slice(&utf16("P3002"));
    body.extend_from_slice(&utf16("Recruit"));
    on_packet(&mut world, 1, body);

    assert_eq!(
        world.objects.get_component::<Player>(&3002).unwrap().title,
        "",
        "no title at clan level 0"
    );
    assert!(
        sm_ids_of(&drain(&mut rx))
            .contains(&sm_ids::A_PLAYER_CAN_ONLY_BE_GRANTED_A_TITLE_IF_CLAN_LEVEL_3),
        "and the reason is the clan level"
    );

    // Raise the clan to 3 and the same grant lands — on the *member*.
    world.clans.get_mut(&5000).unwrap().level = 3;
    let mut body = vec![cop::REQUEST_GIVE_NICK_NAME];
    body.extend_from_slice(&utf16("P3002"));
    body.extend_from_slice(&utf16("Recruit"));
    on_packet(&mut world, 1, body);
    assert_eq!(
        world.objects.get_component::<Player>(&3002).unwrap().title,
        "Recruit"
    );
}

/// **A target outside the clan is a different refusal from an offline one.**
#[test]
fn a_title_target_must_be_a_clan_member() {
    let (mut world, _db, _l) = quest_test_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _stranger = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    with_clan(&mut world, 5000, &[3001]);
    world.clans.get_mut(&5000).unwrap().level = 3;
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_leader = true;
    drain(&mut rx);

    let mut body = vec![cop::REQUEST_GIVE_NICK_NAME];
    body.extend_from_slice(&utf16("P3002"));
    body.extend_from_slice(&utf16("Outsider"));
    on_packet(&mut world, 1, body);

    assert_eq!(
        world.objects.get_component::<Player>(&3002).unwrap().title,
        ""
    );
    assert!(sm_ids_of(&drain(&mut rx)).contains(&sm_ids::THE_TARGET_MUST_BE_A_CLAN_MEMBER));
}

/// **`RequestLinkHtml` (0x22) serves a page out of `data/html/`.**
#[test]
fn a_link_html_serves_the_named_page() {
    let mut world = dist_html_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    let mut body = vec![cop::REQUEST_LINK_HTML];
    body.extend_from_slice(&utf16("villagemaster/30026.htm"));
    on_packet(&mut world, 1, body);

    assert!(
        has_opcode(&drain(&mut rx), server_packets::opcodes::NPC_HTML_MESSAGE),
        "the page is served"
    );
}

/// **A path that climbs out of the html root is refused even when it would
/// otherwise resolve.**
///
/// `data/html/../html/villagemaster/30026.htm` normalises straight back to a
/// real file, so the filesystem would happily serve it — the `..` check is the
/// only thing that says no. Pointing this at a nonexistent path would pass
/// with the guard removed and prove nothing.
#[test]
fn a_link_html_path_cannot_escape_the_html_root() {
    let mut world = dist_html_world();
    let mut rx = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain(&mut rx);

    for link in ["../html/villagemaster/30026.htm", ""] {
        let mut body = vec![cop::REQUEST_LINK_HTML];
        body.extend_from_slice(&utf16(link));
        on_packet(&mut world, 1, body);
        assert!(
            drain(&mut rx).is_empty(),
            "nothing served for link {link:?}"
        );
    }
}

/// **`RequestPreviewItem` (0xC7) charges `WearPrice` per slot and sends the
/// outfit.** Nothing is equipped and nothing enters the bag — only the client's
/// drawing changes.
#[test]
fn trying_items_on_charges_per_slot_and_previews_them() {
    let (mut world, mut rx) = preview_world();
    let before = adena_of(&world, 3001);
    let inv_before = world
        .objects
        .get_component::<Inventory>(&3001)
        .unwrap()
        .items()
        .len();

    // Both products on the fixture list: 41 is a weapon, 1061 an etc item
    // with no paperdoll slot at all.
    let mut body = vec![cop::REQUEST_PREVIEW_ITEM];
    body.extend_from_slice(&0i32.to_le_bytes()); // unknown
    body.extend_from_slice(&3i32.to_le_bytes()); // list id
    body.extend_from_slice(&2i32.to_le_bytes()); // count
    body.extend_from_slice(&PREVIEW_CHEST.to_le_bytes());
    body.extend_from_slice(&1061i32.to_le_bytes());
    on_packet(&mut world, 1, body);

    assert!(
        has_opcode(&drain(&mut rx), server_packets::opcodes::SHOP_PREVIEW_INFO),
        "the outfit is sent"
    );
    // Only the equipable line costs anything — Java `continue`s past the rest.
    assert_eq!(
        before - adena_of(&world, 3001),
        i64::from(world.cfg.general.wear_price),
        "one slot charged, not two"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .items()
            .len(),
        inv_before,
        "nothing was actually given"
    );
}

/// **Two items competing for one paperdoll slot are refused**, and nothing is
/// charged — the check sits inside the pricing loop, before any adena moves.
#[test]
fn two_items_for_one_slot_are_refused() {
    let (mut world, mut rx) = preview_world();
    let before = adena_of(&world, 3001);

    let mut body = vec![cop::REQUEST_PREVIEW_ITEM];
    body.extend_from_slice(&0i32.to_le_bytes());
    body.extend_from_slice(&3i32.to_le_bytes());
    body.extend_from_slice(&2i32.to_le_bytes());
    body.extend_from_slice(&PREVIEW_CHEST.to_le_bytes());
    body.extend_from_slice(&PREVIEW_CHEST.to_le_bytes()); // the same slot twice
    on_packet(&mut world, 1, body);

    let out = drain(&mut rx);
    assert!(
        !has_opcode(&out, server_packets::opcodes::SHOP_PREVIEW_INFO),
        "no outfit sent"
    );
    assert!(
        sm_ids_of(&out).contains(&sm_ids::YOU_CAN_NOT_TRY_THOSE_ITEMS_ON_AT_THE_SAME_TIME),
        "and the reason is given"
    );
    assert_eq!(adena_of(&world, 3001), before, "nothing charged");
}

/// **The try-on wears off after `WearDelay`**, telling the client to redraw
/// the real outfit.
#[test]
fn the_try_on_wears_off_after_the_delay() {
    let (mut world, mut rx) = preview_world();
    let mut body = vec![cop::REQUEST_PREVIEW_ITEM];
    body.extend_from_slice(&0i32.to_le_bytes());
    body.extend_from_slice(&3i32.to_le_bytes());
    body.extend_from_slice(&1i32.to_le_bytes());
    body.extend_from_slice(&PREVIEW_CHEST.to_le_bytes());
    on_packet(&mut world, 1, body);
    drain(&mut rx);

    let ticks = world.cfg.general.wear_delay as u64 * 10;
    advance_ticks(&mut world, ticks - 1);
    assert!(drain(&mut rx).is_empty(), "not yet");
    advance_ticks(&mut world, 1);
    assert!(
        sm_ids_of(&drain(&mut rx)).contains(&sm_ids::YOU_ARE_NO_LONGER_TRYING_ON_EQUIPMENT),
        "the outfit comes off"
    );
}

/// **`RequestGMCommand` (0x7E) answers each pane with its own packet, and
/// refuses a GM whose access level lacks `allowAltg`.**
#[test]
fn the_gm_view_panes_answer_and_honour_allow_alt_g() {
    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 3001, 100);
    let _target = ingame_player(&mut world, 2, 3002, 0, 0, 0);
    let target_name = world
        .objects
        .get_component::<Player>(&3002)
        .unwrap()
        .name
        .clone();
    drain(&mut rx);

    let pane = |world: &mut World, cmd: i32, name: &str| {
        let mut body = vec![cop::REQUEST_GM_COMMAND];
        body.extend_from_slice(&utf16(name));
        body.extend_from_slice(&cmd.to_le_bytes());
        on_packet(world, 1, body);
    };

    pane(&mut world, 1, &target_name);
    let out = drain(&mut rx);
    assert!(
        has_opcode(&out, server_packets::opcodes::GM_VIEW_CHARACTER_INFO),
        "status pane"
    );
    assert!(
        has_opcode(&out, server_packets::opcodes::GM_HENNA_INFO),
        "with the dye panel beside it"
    );

    pane(&mut world, 3, &target_name);
    assert!(
        has_opcode(&drain(&mut rx), server_packets::opcodes::GM_VIEW_SKILL_INFO),
        "skills pane"
    );

    pane(&mut world, 4, &target_name);
    assert!(
        has_opcode(&drain(&mut rx), server_packets::opcodes::GM_VIEW_QUEST_INFO),
        "quests pane"
    );

    pane(&mut world, 6, &target_name);
    assert!(
        has_opcode(
            &drain(&mut rx),
            server_packets::opcodes::GM_VIEW_WAREHOUSE_WITHDRAW_LIST
        ),
        "warehouse pane"
    );

    // A non-GM gets nothing at all. This is the *reachable* half of Java's
    // `isGM() && allowAltG()`: every `isGM` level on this dist also has
    // `allowAltg`, so `isGM` alone is what decides here.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .access_level = 0;
    pane(&mut world, 1, &target_name);
    assert!(drain(&mut rx).is_empty(), "a non-GM is refused silently");
}
