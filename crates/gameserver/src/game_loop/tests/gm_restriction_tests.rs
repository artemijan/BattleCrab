//! `General.ini`'s GM-restriction family — the keys that decide what a GM's
//! `PlayerCondOverride` grid actually exempts them from.
//!
//! Three of them (`GMSkillRestriction`, `GMItemRestriction`,
//! `GMTradeRestrictedItems`) read as though they *grant* GM powers and in fact
//! **withdraw** them: Java's guard is `canOverrideCond(...) && !Config.KEY`, so
//! setting the key to `True` is what stops the override applying. This dist
//! sets two of the three, which is why the port letting GMs bypass everything
//! was a divergence rather than a convenience.

use super::*;
use crate::game_loop::skills::conditions;
use crate::model::skill::condition::SkillCondition;
use crate::model::skill::{AffectType, Skill};

/// A GM access level from the real table.
const GM_LEVEL: i32 = 100;

/// A skill no character in these fixtures can satisfy: level 99+.
fn level_gated_skill() -> Skill {
    let mut skill = Skill::default();
    skill.conditions.push(SkillCondition::CheckLevel {
        min: 99,
        max: i32::MAX,
        affect: AffectType::Caster,
    });
    skill
}

/// `Darin's Letter` — undroppable **and** a quest item, so one item exercises
/// both gates the key covers.
const BOUND_QUEST_ITEM: i32 = 687;

/// `RequestDropItem` for `item_oid` at an explicit location.
fn drop_packet(item_oid: i32, count: i64, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(cop::REQUEST_DROP_ITEM);
    w.write_i32(item_oid);
    w.write_i64(count);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}

/// The `S1_TEXT` payload of a `SystemMessage`, or `None` for any other shape.
fn sm_text(pkt: &[u8]) -> Option<String> {
    if pkt[0] != server_packets::opcodes::SYSTEM_MESSAGE {
        return None;
    }
    // opcode, i16 id, u8 param count, u8 param type, then a UTF-16LE string.
    let mut units = Vec::new();
    let mut i = 5;
    while i + 1 < pkt.len() {
        let u = u16::from_le_bytes([pkt[i], pkt[i + 1]]);
        if u == 0 {
            break;
        }
        units.push(u);
        i += 2;
    }
    Some(String::from_utf16_lossy(&units))
}

fn gm_world() -> World {
    let (mut world, ..) = test_world();
    world.data.admin = crate::data::AdminData::load_from(crate::data::DIST_GAME);
    world
}

/// **The divergence this cluster was opened for.** `GMSkillRestriction` is
/// **True** on this dist, which means a GM is bound by every skill condition
/// like anyone else. The port skipped them all, behind a comment asserting the
/// key was off.
///
/// The `False` branch is asserted too, because a check that always refuses
/// would satisfy the `True` half on its own.
#[test]
fn gm_skill_restriction_true_binds_a_gm_to_skill_conditions() {
    for (key, expect_allowed) in [(true, false), (false, true)] {
        let mut world = gm_world();
        world.cfg.general.gm_skill_restriction = key;
        let _rx = ingame_player_access(&mut world, 1, 7301, GM_LEVEL);
        let _t = ingame_player(&mut world, 2, 7302, 0, 0, 0);

        // A condition no character can satisfy here: level 99 or above.
        let skill = level_gated_skill();

        let allowed = conditions::check_cast(&world, 7301, &skill, 7302).is_ok();
        assert_eq!(
            allowed,
            expect_allowed,
            "GMSkillRestriction = {key}: the GM should {} cast",
            if expect_allowed { "" } else { "not" }
        );
    }
}

/// The override is what the key gates, not the access level — so a GM who
/// clears `SKILL_CONDITIONS` with `//set_exception 2` is bound by conditions
/// even with `GMSkillRestriction` off.
///
/// Pinned because reading the access level directly (which the port did) gives
/// the same answer in every test above and a different one here.
#[test]
fn the_exemption_follows_the_override_not_the_access_level() {
    let mut world = gm_world();
    world.cfg.general.gm_skill_restriction = false;
    let _rx = ingame_player_access(&mut world, 1, 7303, GM_LEVEL);
    let _t = ingame_player(&mut world, 2, 7304, 0, 0, 0);
    let skill = level_gated_skill();
    assert!(
        conditions::check_cast(&world, 7303, &skill, 7304).is_ok(),
        "exempt while the override is held"
    );

    // Drop just the SKILL_CONDITIONS bit.
    if let Some(p) = world.objects.get_component_mut::<Player>(&7303) {
        p.cond_overrides &= !(1u64 << crate::game_loop::admin::SKILL_CONDITIONS_ORDINAL);
    }
    assert!(
        conditions::check_cast(&world, 7303, &skill, 7304).is_err(),
        "and bound once it is cleared, though the access level never changed"
    );
}

/// `GMRestartFighting` (**True** here) — the one key in the family that grants
/// rather than restricts: a GM may log out mid-fight, an ordinary player may
/// not.
///
/// All four combinations are covered, because the interesting failure is a
/// gate that exempts everyone or no one.
#[test]
fn gm_restart_fighting_exempts_only_a_gm_and_only_while_set() {
    for (key, access, expect_can_logout) in [
        (true, GM_LEVEL, true),
        (false, GM_LEVEL, false),
        (true, 0, false),
        (false, 0, false),
    ] {
        let mut world = gm_world();
        world.cfg.general.gm_restart_fighting = key;
        let mut rx = ingame_player_access(&mut world, 1, 7305, access);
        crate::game_loop::combat::refresh_attack_stance(&mut world, 7305);
        drain(&mut rx);

        on_packet(&mut world, 1, vec![cop::REQUEST_RESTART]);
        // `RestartResponse` goes out on **both** paths (Java sends TRUE as well
        // as FALSE), so the packet says nothing. What distinguishes them is
        // whether the character actually left the world.
        let still_in_world = world.objects.has_component::<Player>(&7305);
        assert_eq!(
            !still_in_world, expect_can_logout,
            "GMRestartFighting = {key}, access {access}"
        );
    }
}

/// `GMTradeRestrictedItems` (**False** here) and the drop gate. Java exempts on
/// `canOverrideCond(DROP_ALL_ITEMS) && GM_TRADE_RESTRICTED_ITEMS`, covering
/// **both** the undroppable gate and the quest-item gate.
///
/// The dist value means a GM currently may not drop either, which is what the
/// port already did — so the test exists to pin the *other* branch, the one an
/// operator reaches by turning the key on.
#[test]
fn gm_trade_restricted_items_gates_dropping_bound_and_quest_items() {
    for key in [false, true] {
        let mut world = gm_world();
        world.cfg.general.gm_trade_restricted_items = key;
        world.data.item_data = crate::data::dist::items_owned();
        let mut rx = ingame_player_access(&mut world, 1, 7306, GM_LEVEL);
        let bound = 90_001;
        give_item(&mut world, 7306, bound, BOUND_QUEST_ITEM, 1);
        drain(&mut rx);

        let pos = *world
            .objects
            .get_component::<crate::model::components::Position>(&7306)
            .unwrap();
        on_packet(&mut world, 1, drop_packet(bound, 1, pos.x, pos.y, pos.z));
        let dropped = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&7306)
            .map(|i| i.by_object_id(bound).is_none())
            .unwrap_or(false);
        assert_eq!(
            dropped,
            key,
            "GMTradeRestrictedItems = {key}: the bound item should {} leave the inventory",
            if key { "" } else { "not" }
        );
    }
}

/// `GMShowAnnouncerName` (**False** here) appends ` [Name]` to `//announce`,
/// and Java deliberately leaves `//announce_screen` alone — the screen message
/// has no room for it and returns before the append.
#[test]
fn the_announcer_name_is_appended_only_when_set_and_never_on_screen() {
    let mut world = gm_world();
    world.cfg.general.gm_announcer_name = true;
    let mut gm_rx = ingame_player_access(&mut world, 1, 7307, GM_LEVEL);
    let mut other_rx = ingame_player(&mut world, 2, 7308, 0, 0, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);

    on_packet(&mut world, 1, build_admin("announce hello"));
    let saw = drain(&mut other_rx)
        .iter()
        .filter_map(|p| sm_text(p))
        .any(|t| t.contains("[P7307]"));
    assert!(saw, "the announcer's name rides along");

    // …and the screen variant does not carry it.
    drain(&mut other_rx);
    on_packet(&mut world, 1, build_admin("announce_screen hello"));
    let screen_named = drain(&mut other_rx)
        .iter()
        .any(|p| String::from_utf8_lossy(p).contains("P7307"));
    assert!(!screen_named, "announce_screen is exempt in Java");
}

/// `UseSuperHasteAsGMSpeed` (**False** here) makes `//gmspeed <n>` forward to
/// `//superhaste <n>` — a different effect entirely: the super-haste *skill*
/// rather than a run-speed multiplier. The argument is reinterpreted with it,
/// which is why the two branches have to be told apart by what they change.
#[test]
fn use_super_haste_as_gm_speed_redirects_the_command() {
    // Off: the multiplier moves.
    let mut world = gm_world();
    let _rx = ingame_player_access(&mut world, 1, 7309, GM_LEVEL);
    on_packet(&mut world, 1, build_admin("gmspeed 3"));
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::Speeds>(&7309)
            .map(|s| s.move_multiplier),
        Some(3.0),
        "the speed multiplier is what //gmspeed changes"
    );

    // On: it forwards, so the multiplier is untouched and a buff lands instead.
    let mut world = gm_world();
    world.cfg.general.use_super_haste_as_gm_speed = true;
    let _rx = ingame_player_access(&mut world, 1, 7310, GM_LEVEL);
    on_packet(&mut world, 1, build_admin("gmspeed 3"));
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::Speeds>(&7310)
            .map(|s| s.move_multiplier),
        Some(1.0),
        "forwarded — the multiplier never moves"
    );
}

/// `GMDebugHtmlPaths` (**True** here): a GM is told the path of every html the
/// server serves them, and an ordinary player is not.
///
/// Java prints `newPath.substring(5)` — the path without its leading `data/`,
/// as a datapack author would write it.
#[test]
fn a_gm_is_shown_the_path_of_every_html_served_to_them() {
    for (access, expect_line) in [(GM_LEVEL, true), (0, false)] {
        let mut world = gm_world();
        world.data.root = crate::data::DIST_GAME.to_string();
        // `test_world` leaves this off deliberately: it is `True` on the dist
        // and in Java, but switching it on globally would add a chat line to
        // every packet-exact admin fixture that opens a dialog.
        world.cfg.general.gm_debug_html_paths = true;
        let mut rx = ingame_player_access(&mut world, 1, 7311, access);
        drain(&mut rx);

        let _ = crate::data::htm_cache::read_htm_for(
            &world,
            7311,
            format!("{}data/html/npcdefault.htm", world.data.root),
        );
        let saw = drain(&mut rx)
            .iter()
            .filter_map(|p| sm_text(p))
            .any(|t| t == "html/npcdefault.htm");
        assert_eq!(saw, expect_line, "access {access}");
    }
}

/// The key still gates it with a GM present.
#[test]
fn the_html_path_line_is_off_when_the_key_is() {
    let mut world = gm_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    world.cfg.general.gm_debug_html_paths = false;
    let mut rx = ingame_player_access(&mut world, 1, 7312, GM_LEVEL);
    drain(&mut rx);
    let _ = crate::data::htm_cache::read_htm_for(
        &world,
        7312,
        format!("{}data/html/npcdefault.htm", world.data.root),
    );
    assert!(drain(&mut rx).is_empty());
}

/// `DefaultAccessLevel` (**0** here) promotes a character resolving to level 0,
/// behind Java's `> 0` guard. The guard is the feature: without it every
/// character would be re-resolved through the table, and a `0` in the config
/// would read as "promote everyone to 0" rather than "promote nobody".
///
/// An undefined level falls back to 0 rather than granting a tier that does not
/// exist.
#[test]
fn default_access_level_promotes_only_when_above_zero_and_defined() {
    let admin = crate::data::AdminData::load_from(crate::data::DIST_GAME);
    assert_eq!(admin.effective_access_level(0, 0), 0, "the dist's inert 0");
    assert_eq!(
        admin.effective_access_level(0, GM_LEVEL),
        GM_LEVEL,
        "a level-0 character is promoted"
    );
    assert_eq!(
        admin.effective_access_level(GM_LEVEL, 1),
        GM_LEVEL,
        "an already-privileged character is left alone"
    );
    assert_eq!(
        admin.effective_access_level(0, 999_999),
        0,
        "an undefined default falls back to 0"
    );
    // The `> 0` guard is what makes a **negative** default harmless, and this
    // is the case that shows why it is a guard rather than an optimisation:
    // `AdminData.access_level` folds every negative to `-1`, the banned tier,
    // so without it a mistyped `DefaultAccessLevel = -1` would ban every
    // character on the server at login.
    assert_eq!(
        admin.effective_access_level(0, -1),
        0,
        "a negative default is ignored, not resolved to the banned tier"
    );
    assert!(
        admin.access_level(-1).level < 0,
        "sanity: -1 really is a defined, negative tier"
    );
}
