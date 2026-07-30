//! Grand Olympiad Manager NPC (31688) dialog — port of
//! `dist/game/data/scripts/ai/others/OlyManager/OlyManager.java`. Owns the
//! chat window through `addFirstTalkId`; its buttons drive registration into
//! the 1v1 waiting list (`Quest OlyManager <event>` bypasses).
//!
//! Slice 2 covers the dialog + join/leave. The class leaderboards
//! (`rank_<classId>`), the point→mark exchange (`calculatePoints*`) and the
//! reward multisell (`showEquipmentReward`) need persistence/scoring that lands
//! in later slices; they are stubbed with `TODO(G25)`.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::Player;
use crate::model::olympiad::{CompetitionType, OLYMPIAD_MANAGER_NPC};

pub struct OlyManager;

const NPCS: &[i32] = &[OLYMPIAD_MANAGER_NPC];
/// The Olympiad equipment reward multisell (`OlyManager.EQUIPMENT_MULTISELL`).
const EQUIPMENT_MULTISELL: i32 = 3168801;

impl QuestScript for OlyManager {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "OlyManager"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/OlyManager"
    }
    fn start_npcs(&self) -> &[i32] {
        NPCS
    }
    fn talk_npcs(&self) -> &[i32] {
        NPCS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        NPCS
    }

    /// `onFirstTalk`: cursed-weapon carriers are turned away; otherwise the
    /// eligible get the main menu and everyone else the "not a noble" page.
    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        Some(first_talk_page(ctx).to_string())
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    /// `onEvent`: the menu buttons. A `.htm`/`.html` event is a static
    /// navigation link returned as-is (Java's explicit passthrough cases).
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if event.ends_with(".html") || event.ends_with(".htm") {
            return Some(event.to_string());
        }
        match event {
            "index" => Some(first_talk_page(ctx).to_string()),
            "joinMatch" => Some(join_match_page(ctx)),
            "register1v1" => register_1v1(ctx),
            "unregister" => {
                crate::game_loop::olympiad::unregister(ctx.world, ctx.player);
                None
            }
            _ if event.starts_with("rank_") => Some(empty_rank_detail(ctx)),
            // The point → Mark of Battle exchange.
            "calculatePoints" => Some(if unclaimed_points(ctx) > 0 {
                "OlyManager-calculateEnough.html".to_string()
            } else {
                "OlyManager-calculateNoEnough.html".to_string()
            }),
            "calculatePointsDone" => {
                calculate_points_done(ctx);
                None
            }
            // The Olympiad equipment reward shop (a multisell, `EQUIPMENT_MULTISELL`).
            "showEquipmentReward" => {
                crate::game_loop::multisell::separate_and_send(
                    ctx.world,
                    ctx.client_id,
                    ctx.player,
                    Some(ctx.npc_id),
                    EQUIPMENT_MULTISELL,
                    false,
                );
                None
            }
            _ => None,
        }
    }
}

/// The player's banked, unexchanged Olympiad points.
fn unclaimed_points(ctx: &QuestCtx) -> i32 {
    ctx.player_var_int(crate::game_loop::olympiad::UNCLAIMED_POINTS_VAR, 0)
}

/// `Player.isInventoryUnder80(false)`: the non-quest slot count is within 80 %
/// of the inventory limit. An absent inventory counts as empty.
fn inventory_under_80(ctx: &QuestCtx) -> bool {
    let used = ctx
        .world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&ctx.player)
        .map_or(0, |inv| inv.non_quest_size(&ctx.world.data.item_data));
    let race = ctx
        .world
        .objects
        .get_component::<Player>(&ctx.player)
        .map_or(0, |p| p.race);
    let limit = ctx.world.cfg.character.inventory_limit(race);
    used as f64 <= limit as f64 * 0.8
}

fn send_sm(ctx: &QuestCtx, sm_id: i16) {
    if let Some(cs) = ctx.world.clients.get(&ctx.client_id) {
        cs.send(crate::network::server_packets::system_message_with(
            sm_id,
            &[],
        ));
    }
}

/// `calculatePointsDone`: convert the banked points to Marks of Battle
/// (`AltOlyMarkPerPoint` each) and clear the variable — refused while the
/// inventory is over 80 % full.
fn calculate_points_done(ctx: &mut QuestCtx) {
    use crate::game_loop::olympiad::{MARK_ITEM, MARK_PER_POINT, UNCLAIMED_POINTS_VAR};
    if !inventory_under_80(ctx) {
        send_sm(
            ctx,
            crate::network::server_packets::sm_ids::UNABLE_TO_PROCESS_UNTIL_INVENTORY_UNDER_80_PERCENT,
        );
        return;
    }
    let points = unclaimed_points(ctx);
    if points > 0 {
        ctx.unset_player_var(UNCLAIMED_POINTS_VAR);
        ctx.give_items(MARK_ITEM, points as i64 * MARK_PER_POINT);
    }
}

/// Java `onFirstTalk` page choice.
fn first_talk_page(ctx: &QuestCtx) -> &'static str {
    let cursed = ctx
        .world
        .objects
        .get_component::<Player>(&ctx.player)
        .is_some_and(|p| p.cursed_weapon_equipped_id != 0);
    if cursed {
        "OlyManager-noCursed.html"
    } else if eligible(ctx) {
        "OlyManager-noble.html"
    } else {
        "OlyManager-noNoble.html"
    }
}

/// The join page, or the "already registered" page. Fills the round / week /
/// participant-count placeholders.
fn join_match_page(ctx: &mut QuestCtx) -> String {
    if ctx.world.olympiad.is_registered(ctx.player) {
        return "OlyManager-registred.html".to_string();
    }
    let round = ctx.world.olympiad.period;
    let week = ctx.world.olympiad.current_cycle;
    let participants = ctx.world.olympiad.count_opponents();
    ctx.get_htm("OlyManager-joinMatch.html")
        .replace("%olympiad_round%", &round.to_string())
        .replace("%olympiad_week%", &week.to_string())
        .replace("%olympiad_participant%", &participants.to_string())
}

/// The `register1v1` button: the NPC-side gates (subclass, eligibility, points)
/// then hand off to the manager's `register`.
fn register_1v1(ctx: &mut QuestCtx) -> Option<String> {
    if ctx.is_subclass_active() {
        return Some("OlyManager-subclass.html".to_string());
    }
    if !eligible(ctx) {
        return Some("OlyManager-noNoble.html".to_string());
    }
    let (base_class, name) = base_class_and_name(ctx);
    if ctx
        .world
        .olympiad
        .noble_points_or_create(ctx.player, base_class, &name)
        <= 0
    {
        return Some("OlyManager-noPoints.html".to_string());
    }
    if !inventory_under_80(ctx) {
        send_sm(
            ctx,
            crate::network::server_packets::sm_ids::UNABLE_TO_PROCESS_UNTIL_INVENTORY_UNDER_80_PERCENT,
        );
        return None;
    }
    crate::game_loop::olympiad::register(ctx.world, ctx.player, CompetitionType::NonClassed);
    None
}

/// The Classic eligibility gate the NPC applies (mirrors the manager's own):
/// 3rd/4th class group and level ≥ 55.
fn eligible(ctx: &QuestCtx) -> bool {
    (ctx.is_in_category("THIRD_CLASS_GROUP") || ctx.is_in_category("FOURTH_CLASS_GROUP"))
        && ctx.player_level() >= 55
}

fn base_class_and_name(ctx: &QuestCtx) -> (i32, String) {
    ctx.world
        .objects
        .get_component::<Player>(&ctx.player)
        .map(|p| (p.base_class_id, p.name.clone()))
        .unwrap_or((0, String::new()))
}

/// The class-rank detail page with an empty board.
/// TODO(G25): fill from the persisted `olympiad_nobles` leaderboard once nobles
/// are stored (slice 3/5); until then every rank row is blanked, like Java does
/// for classes with no ranked players.
fn empty_rank_detail(ctx: &mut QuestCtx) -> String {
    let mut html = ctx.get_htm("OlyManager-rankDetail.html");
    for index in 1..=15 {
        html = html
            .replace(&format!("%Rank{index}%"), "")
            .replace(&format!("%Name{index}%"), "");
    }
    html
}
