//! Mysterious Wizard — port of
//! `dist/game/data/scripts/ai/others/MysteriousWizard/`: the chat window of
//! the wizard (33980) the Winds of Fate quests spawn.
//!
//! In Fortress of the Dead ([`super::fortress_of_the_dead`]) it opens the
//! page that ends Q10752's cinematic; in the open world it serves Q10751's
//! cond 6 and 7 pages. [`super::chamber_of_prophecies`] also listens to the
//! wizard's first talk, for its own room.

use super::q10751_winds_of_fate_encounters;
use crate::game_loop::quests::{QuestCtx, QuestScript};

const MYSTERIOUS_WIZARD: i32 = 33980;
/// The `FortressOfTheDead` instance template.
const FORTRESS_OF_THE_DEAD: i32 = 254;

pub struct MysteriousWizard;

impl QuestScript for MysteriousWizard {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "MysteriousWizard"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/MysteriousWizard"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[MYSTERIOUS_WIZARD]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, _ctx: &mut QuestCtx, event: &str) -> Option<String> {
        (event == "33980-01.html").then(|| event.to_string())
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.instance_template(ctx.npc_instance()) == FORTRESS_OF_THE_DEAD {
            return Some("33980.html".to_string());
        }
        let quest = q10751_winds_of_fate_encounters::QUEST_NAME;
        if !ctx.has_other_quest_state(quest) {
            return None;
        }
        match ctx.other_quest_cond(quest) {
            6 => Some("33980-05.html".to_string()),
            7 => Some("33980-04.html".to_string()),
            _ => None,
        }
    }
}
