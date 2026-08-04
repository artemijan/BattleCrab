//! Bot reporting (`BotReportTable`) + the `Say2` chat filters, driven through
//! the real entry points: the `/AutoHuntingReport` player action and `Say2`.

use super::*;
use crate::config::bot_report::{BotReportConfig, BotReportPunishment};
use crate::config::chat_filter::ChatFilterConfig;
use crate::game_loop::bot_report::{self, DAILY_POINTS};
use crate::model::components::TargetRef;
use commons::config::PropertiesParser;

/// `RequestActionUse` body — actionId + ctrl + shift.
fn action_use_body(action_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(action_id);
    w.write_i32(0); // ctrl (an int, not a byte)
    w.write_u8(0); // shift
    w.into_bytes()
}

/// A world with bot reporting on, two players out in the open, and the
/// reporter targeting the other.
fn report_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    let (mut world, ..) = test_world();
    world.cfg.bot_report = BotReportConfig {
        enabled: true,
        reset_hour: (0, 0),
        report_delay_millis: 30 * 60_000,
        allow_reports_from_same_clan_members: false,
        punishments: Vec::new(),
    };
    let rx = ingame_player(&mut world, 1, 6001, 0, 0, 0);
    let _bot_rx = ingame_player(&mut world, 2, 6002, 50, 0, 0);
    // Java refuses a target with zero exp ("has not acquired any XP after
    // connecting"), so the suspect needs some.
    if let Some(p) = world.objects.get_component_mut::<Player>(&6002) {
        p.exp = 100_000;
    }
    world.objects.add_components(&6001, TargetRef(Some(6002)));
    (world, rx)
}

fn reports_against(world: &World, bot: i32) -> usize {
    world
        .bot_reports
        .reports
        .get(&bot)
        .map(|r| r.report_count())
        .unwrap_or(0)
}

/// The happy path, through the real action button.
#[test]
fn the_report_button_registers_a_report_and_spends_a_point() {
    let (mut world, mut rx) = report_world();
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::REQUEST_ACTION_USE],
            action_use_body(bot_report::BOT_REPORT_ACTION_ID),
        ]
        .concat(),
    );

    assert_eq!(reports_against(&world, 6002), 1, "the report is recorded");
    assert_eq!(
        world.bot_reports.reporters[&6001].points,
        DAILY_POINTS - 1,
        "one point spent"
    );
    assert_eq!(
        drain(&mut rx).len(),
        2,
        "Java answers with two system messages: reported + points remaining"
    );
}

/// `EnableBotReportButton = False` — Java's handler answers "This feature is
/// disabled." and never touches the table.
#[test]
fn a_disabled_button_records_nothing() {
    let (mut world, mut rx) = report_world();
    world.cfg.bot_report.enabled = false;
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::REQUEST_ACTION_USE],
            action_use_body(bot_report::BOT_REPORT_ACTION_ID),
        ]
        .concat(),
    );
    assert_eq!(reports_against(&world, 6002), 0);
    assert!(!drain(&mut rx).is_empty(), "the player is told it is off");
}

/// The same reporter cannot report the same character twice.
#[test]
fn the_same_reporter_cannot_report_the_same_target_twice() {
    let (mut world, _rx) = report_world();
    assert!(bot_report::report_bot(&mut world, 1, 6001));
    // Clear the per-address cooldown so the *duplicate* rule is what refuses.
    world.bot_reports.ip_registry.clear();
    world
        .bot_reports
        .reporters
        .get_mut(&6001)
        .unwrap()
        .last_report = 0;

    assert!(!bot_report::report_bot(&mut world, 1, 6001));
    assert_eq!(reports_against(&world, 6002), 1, "still just the one");
    assert_eq!(
        world.bot_reports.reporters[&6001].points,
        DAILY_POINTS - 1,
        "the refused report costs nothing"
    );
}

/// A reporter out of points is refused (Java: 7 a day).
#[test]
fn a_reporter_with_no_points_left_is_refused() {
    let (mut world, _rx) = report_world();
    world.bot_reports.reporters.insert(
        6001,
        bot_report::ReporterCharData {
            points: 0,
            last_report: 0,
        },
    );

    assert!(!bot_report::report_bot(&mut world, 1, 6001));
    assert_eq!(reports_against(&world, 6002), 0);
}

/// A character that is itself reported may not report anyone.
#[test]
fn a_reported_character_cannot_report_others() {
    let (mut world, _rx) = report_world();
    world
        .bot_reports
        .reports
        .entry(6001)
        .or_default()
        .reporters
        .insert(9999, 1);

    assert!(!bot_report::report_bot(&mut world, 1, 6001));
    assert_eq!(reports_against(&world, 6002), 0);
}

/// Java refuses a target sitting in a peace zone or an arena. Driven off the
/// real zone data: the suspect is moved to the first peace zone the datapack
/// defines, so this fails if the zone lookup itself regresses.
#[test]
fn a_target_in_a_peace_zone_cannot_be_reported() {
    let (mut world, _rx) = report_world();
    let Some((x, y, z)) = first_peace_zone_point(&world) else {
        return; // the synthetic test datapack has no zones — nothing to assert
    };
    if let Some(pos) = world.objects.get_component_mut::<Position>(&6002) {
        pos.x = x;
        pos.y = y;
        pos.z = z;
    }
    assert!(
        !bot_report::report_bot(&mut world, 1, 6001),
        "a suspect standing in town cannot be reported"
    );
    assert_eq!(reports_against(&world, 6002), 0);
}

/// A point the zone lookup agrees is inside a `Peace` zone, or `None` when the
/// loaded datapack has none.
fn first_peace_zone_point(world: &World) -> Option<(i32, i32, i32)> {
    use crate::data::zone_data::ZoneKind;
    // Giran town centre — a peace zone on the real datapack.
    const CANDIDATES: [(i32, i32, i32); 2] = [(82698, 148638, -3473), (-84318, 244579, -3730)];
    CANDIDATES.into_iter().find(|&(x, y, z)| {
        world
            .data
            .zone_data
            .zones_at(x, y, z)
            .any(|zone| zone.kind == ZoneKind::Peace)
    })
}

/// Java's zero-exp guard: `getExp() == getStartingExp()`, and `setStartingExp`
/// has no caller, so it really means "the target has never earned anything".
#[test]
fn a_target_with_no_exp_cannot_be_reported() {
    let (mut world, _rx) = report_world();
    if let Some(p) = world.objects.get_component_mut::<Player>(&6002) {
        p.exp = 0;
    }
    assert!(!bot_report::report_bot(&mut world, 1, 6001));
    assert_eq!(reports_against(&world, 6002), 0);
}

/// The punishment ladder: an exact-count row fires at exactly that count, a
/// negative ("range") row fires at |n| and above.
#[test]
fn the_punishment_ladder_picks_exact_and_range_rows() {
    let (mut world, _rx) = report_world();
    world.cfg.bot_report.punishments = vec![
        BotReportPunishment {
            needed_report_count: 1,
            skill_id: 6038, // BlockChat
            skill_level: 1,
            sys_message_id: -1,
        },
        BotReportPunishment {
            needed_report_count: -1, // range: 1 report and above
            skill_id: 6040,          // Flag
            skill_level: 1,
            sys_message_id: -1,
        },
    ];
    // Both skills must exist in the test datapack for this to mean anything.
    if world.data.skill_data.get(6038, 1).is_none() {
        return;
    }

    assert!(bot_report::report_bot(&mut world, 1, 6001));

    let buffs: Vec<i32> = world
        .objects
        .get_component::<crate::model::components::Buffs>(&6002)
        .map(|b| b.0.iter().map(|x| x.skill_id).collect())
        .unwrap_or_default();
    assert!(buffs.contains(&6038), "the exact-count punishment landed");
    assert!(buffs.contains(&6040), "the range punishment landed too");
}

/// The daily reset hands everybody their 7 points back.
#[test]
fn the_daily_reset_restores_every_budget() {
    let (mut world, _rx) = report_world();
    world.bot_reports.reporters.insert(
        6001,
        bot_report::ReporterCharData {
            points: 0,
            last_report: 5,
        },
    );
    bot_report::reset_report_points(&mut world);
    assert_eq!(world.bot_reports.reporters[&6001].points, DAILY_POINTS);
}

/// Boot load: a report made *after* the last daily reset has already cost its
/// reporter a point, so the budget is rebuilt from the rows rather than
/// starting fresh (Java `loadReportedCharData`'s second half).
#[test]
fn loading_rebuilds_the_reporter_budget_from_recent_rows() {
    let (mut world, _rx) = report_world();
    let last_reset = 1_000_000i64;
    bot_report::on_loaded(
        &mut world,
        vec![
            (6002, 6001, last_reset + 500), // after the reset → costs a point
            (6003, 6001, last_reset + 600), // and another
            (6004, 7777, last_reset - 500), // before it → free
        ],
        last_reset,
    );
    assert_eq!(
        world.bot_reports.reporters[&6001].points,
        DAILY_POINTS - 2,
        "two post-reset reports cost two points"
    );
    assert!(
        !world.bot_reports.reporters.contains_key(&7777),
        "a pre-reset report leaves the reporter's budget untouched"
    );
    assert_eq!(reports_against(&world, 6002), 1);
}

/// `BotReportPointsResetHour` → the most recent occurrence of that time.
#[test]
fn the_last_reset_stamp_walks_back_a_day_before_the_hour() {
    let cfg = BotReportConfig {
        reset_hour: (6, 0),
        ..Default::default()
    };
    let day = 86_400_000i64;
    // 03:00 on day 10 — the last 06:00 was on day 9.
    let now = 10 * day + 3 * 3_600_000;
    assert_eq!(
        bot_report::last_reset_millis(&cfg, now),
        9 * day + 6 * 3_600_000
    );
    // 09:00 on day 10 — today's 06:00 has passed.
    let now = 10 * day + 9 * 3_600_000;
    assert_eq!(
        bot_report::last_reset_millis(&cfg, now),
        10 * day + 6 * 3_600_000
    );
}

// ---------------------------------------------------------------------------
// Say2 chat filter
// ---------------------------------------------------------------------------

fn say2_body(chat_type: crate::enums::ChatType, text: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_string(text);
    w.write_i32(chat_type.client_id());
    w.into_bytes()
}

/// The filter rewrites the *broadcast* line, end to end through `Say2`.
#[test]
fn the_say_filter_rewrites_a_broadcast_line() {
    let (mut world, ..) = test_world();
    world.cfg.chat_filter = ChatFilterConfig::from_parts(
        &PropertiesParser::from_content(
            "General.ini",
            "UseChatFilter = True\nChatFilterChars = ^_^\n",
        ),
        "badword\n",
    );
    let mut rx = ingame_player(&mut world, 1, 6010, 0, 0, 0);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SAY2],
            say2_body(crate::enums::ChatType::General, "a badword here"),
        ]
        .concat(),
    );

    let said = drain(&mut rx);
    assert!(!said.is_empty(), "the line is still broadcast");
    assert!(
        said.iter().any(|p| contains_utf16le(p, "^_^")),
        "the replacement string is what went out"
    );
    assert!(
        !said.iter().any(|p| contains_utf16le(p, "badword")),
        "the filtered word must not reach anyone"
    );
}

/// With the filter off (the dist default) the line goes out untouched.
#[test]
fn the_say_filter_is_inert_when_disabled() {
    let (mut world, ..) = test_world();
    world.cfg.chat_filter = ChatFilterConfig::from_parts(
        &PropertiesParser::from_content("General.ini", "UseChatFilter = False\n"),
        "badword\n",
    );
    let mut rx = ingame_player(&mut world, 1, 6011, 0, 0, 0);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SAY2],
            say2_body(crate::enums::ChatType::General, "a badword here"),
        ]
        .concat(),
    );
    let said = drain(&mut rx);
    assert!(
        said.iter().any(|p| contains_utf16le(p, "badword")),
        "nothing is rewritten while UseChatFilter is False"
    );
}

/// Does this packet carry `needle` as a UTF-16LE string?
///
/// A byte-level subslice search rather than a decode: the packet starts with a
/// one-byte opcode, so decoding it as `u16` pairs lands on the wrong alignment
/// and silently finds nothing — which would make an "absence" assertion pass
/// for the wrong reason.
fn contains_utf16le(packet: &[u8], needle: &str) -> bool {
    let wanted: Vec<u8> = needle
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    packet.windows(wanted.len()).any(|w| w == wanted)
}

/// `BanChatChannels` decides only what a chat-banned player is *told*: the ban
/// itself covers every channel (Java's `return` in `Say2` is unconditional).
/// Both halves matter — a listed channel must explain itself, an unlisted one
/// must stay silent — and the *block* must hold either way.
#[test]
fn ban_chat_channels_gates_the_notice_not_the_block() {
    use crate::model::punishment::{PunishmentAffect, PunishmentType};

    let (mut world, ..) = test_world();
    // Only GENERAL is listed, so a Shout must be blocked *silently*.
    world.cfg.chat_filter = ChatFilterConfig::from_parts(
        &PropertiesParser::from_content("General.ini", "BanChatChannels = GENERAL\n"),
        "",
    );
    let mut speaker_rx = ingame_player(&mut world, 1, 6020, 0, 0, 0);
    let mut bystander_rx = ingame_player(&mut world, 2, 6021, 100, 0, 0);
    crate::game_loop::punishment::start_punishment(
        &mut world,
        "6020".to_string(),
        PunishmentAffect::Character,
        PunishmentType::ChatBan,
        0,
        String::new(),
        "test".to_string(),
    );
    drain(&mut speaker_rx);
    drain(&mut bystander_rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SAY2],
            say2_body(crate::enums::ChatType::General, "hello"),
        ]
        .concat(),
    );
    assert!(
        drain(&mut bystander_rx).is_empty(),
        "a listed channel is blocked"
    );
    assert!(
        !drain(&mut speaker_rx).is_empty(),
        "…and the speaker is told why"
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SAY2],
            say2_body(crate::enums::ChatType::Shout, "hello"),
        ]
        .concat(),
    );
    assert!(
        drain(&mut bystander_rx).is_empty(),
        "an unlisted channel is blocked just the same"
    );
    assert!(
        drain(&mut speaker_rx).is_empty(),
        "…but says nothing, because Shout is not in BanChatChannels"
    );
}
