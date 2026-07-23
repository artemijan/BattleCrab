//! Conquest of Alligator Island (354) — `quests/Q00354_ConquestOfAlligatorIsland`.
//! Kluck (30895, level 38–49) buys Alligator Teeth from the lads on Alligator
//! Island: 400 teeth fetch 2000 adena (repeatable turn-in via the `ADENA`
//! bypass). Most lads drop on a per-mob chance; the Nos Lad drops 1–2 at once.
//! `addCondMaxLevel(49)`.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const KLUCK: i32 = 30895;
const ALLIGATOR_TOOTH: i32 = 5863;
const MIN_LEVEL: i32 = 38;
const NOS_LAD: i32 = 20808;

/// `MOB1`: per-mob tooth drop chance (0..1). The Nos Lad (`MOB2`) is handled
/// separately.
fn mob1_chance(npc_id: i32) -> Option<f64> {
    match npc_id {
        20804 => Some(0.84), // crokian_lad
        20805 => Some(0.91), // dailaon_lad
        20806 => Some(0.88), // crokian_lad_warrior
        20807 => Some(0.92), // farhite_lad
        _ => None,
    }
}

pub struct Q00354ConquestOfAlligatorIsland;

impl QuestScript for Q00354ConquestOfAlligatorIsland {
    fn id(&self) -> i32 {
        354
    }
    fn name(&self) -> &'static str {
        "Q00354_ConquestOfAlligatorIsland"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00354_ConquestOfAlligatorIsland"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KLUCK]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[KLUCK]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[20804, 20805, 20806, 20807, NOS_LAD]
    }
    fn quest_items(&self) -> &[i32] {
        &[ALLIGATOR_TOOTH]
    }

    /// `addCondMaxLevel(49, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 49).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30895-04.html" | "30895-05.html" | "30895-09.html" => Some(event.to_string()),
            "30895-02.html" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "ADENA" => {
                if ctx.quest_items_count(ALLIGATOR_TOOTH) >= 400 {
                    ctx.give_adena(2000, true);
                    ctx.take_items(ALLIGATOR_TOOTH, -1);
                    Some("30895-06.html".to_string())
                } else {
                    Some("30895-08.html".to_string())
                }
            }
            "30895-10.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMemberState(player, -1, 3, npc)`: any started member.
        // Port is killer-only (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        if let Some(chance) = mob1_chance(ctx.npc_id) {
            ctx.give_item_randomly(ALLIGATOR_TOOTH, 1, 0, chance, true);
        } else if ctx.npc_id == NOS_LAD {
            let count = if ctx.roll(100) < 14 { 2 } else { 1 };
            ctx.give_item_randomly(ALLIGATOR_TOOTH, count, 0, 1.0, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL { "30895-01.htm" } else { "30895-03.html" }
                    .to_string(),
            );
        }
        if ctx.is_started() {
            return Some("30895-04.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
