//! Fishing Specialist's Request (127) — `quests/Q00127_FishingSpecialistsRequest`.
//! Pierre (30013, level 20–75) sends a letter to Ferma (30015), whose fish
//! report goes to Baikal (30016), whose sealed bottle returns to Pierre for a
//! Fishing Rod Chest. A one-time courier chain, `cond` 1 → 3, no kills; Pierre
//! also offers a teleport to Ferma. `addCondLevel(20, 75)`.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const PIERRE: i32 = 30013;
const FERMA: i32 = 30015;
const BAIKAL: i32 = 30016;
const PIERRE_LETTER: i32 = 49510;
const FISH_REPORT: i32 = 49504;
const SEALED_BOTTLE: i32 = 49505;
const FISHING_ROD_CHEST: i32 = 49507;
const TELEPORT: (i32, i32, i32) = (105276, 162500, -3600);
const MIN_LEVEL: i32 = 20;
const MAX_LEVEL: i32 = 75;

pub struct Q00127FishingSpecialistsRequest;

impl QuestScript for Q00127FishingSpecialistsRequest {
    fn id(&self) -> i32 {
        127
    }
    fn name(&self) -> &'static str {
        "Q00127_FishingSpecialistsRequest"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00127_FishingSpecialistsRequest"
    }
    fn start_npcs(&self) -> &[i32] {
        &[PIERRE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[PIERRE, FERMA, BAIKAL]
    }
    fn quest_items(&self) -> &[i32] {
        &[PIERRE_LETTER, FISH_REPORT, SEALED_BOTTLE]
    }

    /// `addCondLevel(20, 75, "30013-00.htm")` — a two-sided level gate.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.cond_level(MIN_LEVEL, MAX_LEVEL, "30013-00.htm")
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == PIERRE {
                return Some(
                    if ctx.player_level() < MIN_LEVEL {
                        "30013-00.htm"
                    } else {
                        "30013-01.htm"
                    }
                    .to_string(),
                );
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            PIERRE => match ctx.cond() {
                1 | 2 => Some("30013-03.html".to_string()),
                3 => {
                    ctx.take_items(SEALED_BOTTLE, -1);
                    ctx.give_items(FISHING_ROD_CHEST, 1);
                    ctx.exit_quest(false, true);
                    Some("30013-04.html".to_string())
                }
                _ => Some(ctx.no_quest_html()),
            },
            FERMA => match ctx.cond() {
                1 => {
                    ctx.take_items(PIERRE_LETTER, -1);
                    ctx.give_items(FISH_REPORT, 1);
                    ctx.set_cond(2, true);
                    Some("30015-01.html".to_string())
                }
                2 => Some("30015-02.html".to_string()),
                3 => Some("30015-03.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            BAIKAL => match ctx.cond() {
                2 => {
                    ctx.take_items(FISH_REPORT, -1);
                    ctx.give_items(SEALED_BOTTLE, 1);
                    ctx.set_cond(3, true);
                    Some("30016-01.html".to_string())
                }
                3 => Some("30016-02.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30013-02.html" => {
                ctx.start_quest();
                ctx.give_items(PIERRE_LETTER, 1);
                Some(event.to_string())
            }
            "teleport_to_ferma" => {
                ctx.teleport_to(TELEPORT.0, TELEPORT.1, TELEPORT.2);
                None
            }
            _ => None,
        }
    }
}
