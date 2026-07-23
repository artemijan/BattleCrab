//! In Search of Fragments of Dimension (634) — `quests/Q00634_InSearchOfFragmentsOfDimension`.
//! Every Dimensional Gate Keeper (the 31095–31194 range, minus the ids that
//! don't exist) starts this level-20+ hunt: killing aggressive mobs in the
//! Dimensional Rift approaches drops Dimension Fragments (80% chance, a
//! level-scaled amount), the currency spent at the Dimensional Merchant. There
//! is no turn-in here — the quest just accrues fragments; `05.htm` exits.
use std::sync::OnceLock;

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const DIMENSION_FRAGMENT: i32 = 7079;

/// Every Dimensional Gate Keeper: `31095..=31194` minus the non-existing ids
/// (31147, 31150, and the contiguous 31151..=31167).
fn gate_npcs() -> &'static [i32] {
    static IDS: OnceLock<Vec<i32>> = OnceLock::new();
    IDS.get_or_init(|| {
        (31095..31195)
            .filter(|&i| i != 31147 && i != 31150 && !(31151..=31167).contains(&i))
            .collect()
    })
}

/// The aggressive mobs of the Rift approaches: `21139..=21165` + `21208..=21255`.
fn kill_ids() -> &'static [i32] {
    static IDS: OnceLock<Vec<i32>> = OnceLock::new();
    IDS.get_or_init(|| (21139..=21165).chain(21208..=21255).collect())
}

pub struct Q00634InSearchOfFragmentsOfDimension;

impl QuestScript for Q00634InSearchOfFragmentsOfDimension {
    fn id(&self) -> i32 {
        634
    }
    fn name(&self) -> &'static str {
        "Q00634_InSearchOfFragmentsOfDimension"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00634_InSearchOfFragmentsOfDimension"
    }
    fn start_npcs(&self) -> &[i32] {
        gate_npcs()
    }
    fn talk_npcs(&self) -> &[i32] {
        gate_npcs()
    }
    fn kill_npcs(&self) -> &[i32] {
        kill_ids()
    }
    fn quest_items(&self) -> &[i32] {
        &[DIMENSION_FRAGMENT]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        // Java returns the event html unconditionally (even when qs is null); the
        // side effects only fire when a quest state exists.
        if ctx.has_qs() {
            match event {
                "02.htm" => ctx.start_quest(),
                "05.htm" => ctx.exit_quest(true, true),
                _ => {}
            }
        }
        Some(event.to_string())
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMemberState(player, -1, 3, npc)`. Port is killer-only
        // (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        if ctx.roll(100) < 80 {
            // `(int) ((npc.getLevel() * 0.15) + 2.6)` — truncated.
            let amount = ((ctx.npc_level() as f64 * 0.15) + 2.6) as i64;
            ctx.give_items(DIMENSION_FRAGMENT, amount);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(if ctx.player_level() < 20 { "01a.htm" } else { "01.htm" }.to_string());
        }
        if ctx.is_started() {
            return Some("03.htm".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
