//! ClanMaster — port of
//! `dist/game/data/scripts/village_master/ClanMaster/ClanMaster.java`, the
//! dialog navigation for every clan-management village master. The htmls
//! drive everything: navigation buttons are `Quest ClanMaster <page>.htm`
//! bypasses handled here; the state-changing verbs (`create_clan $name`,
//! `learn_clan_skills`, `multisell`) are `npc_…` bypasses handled by the
//! bypass router (`create_clan` in `game_loop/clans.rs`; the rest are
//! unported and log-drop). The Clan Advent login/logout skill listeners are
//! not ported (no clan skills).

use crate::game_loop::quests::{QuestCtx, QuestScript};

const NPCS: [i32; 60] = [
    30026, 30031, 30037, 30066, 30070, 30109, 30115, 30120, 30154, 30174, //
    30175, 30176, 30187, 30191, 30195, 30288, 30289, 30290, 30297, 30358, //
    30373, 30462, 30474, 30498, 30499, 30500, 30503, 30504, 30505, 30508, //
    30511, 30512, 30513, 30520, 30525, 30565, 30594, 30595, 30676, 30677, //
    30681, 30685, 30687, 30689, 30694, 30699, 30704, 30845, 30847, 30849, //
    30854, 30857, 30862, 30865, 30894, 30897, 30900, 30905, 30910, 30913,
];

/// Pages that require clan leadership, remapped to their `-no` variant for
/// everyone else (the Java `LEADER_REQUIRED` map verbatim).
const LEADER_REQUIRED: [(&str, &str); 11] = [
    ("9000-03.htm", "9000-03-no.htm"),
    ("9000-04.htm", "9000-04-no.htm"),
    ("9000-05.htm", "9000-05-no.htm"),
    ("9000-07.htm", "9000-07-no.htm"),
    ("9000-12a.htm", "9000-07-no.htm"),
    ("9000-12b.htm", "9000-07-no.htm"),
    ("9000-13a.htm", "9000-07-no.htm"),
    ("9000-13b.htm", "9000-07-no.htm"),
    ("9000-14a.htm", "9000-07-no.htm"),
    ("9000-14b.htm", "9000-07-no.htm"),
    ("9000-15.htm", "9000-07-no.htm"),
];

pub struct ClanMaster;

impl QuestScript for ClanMaster {
    fn id(&self) -> i32 {
        -1 // a "script", not a quest — never in QuestList/choosers
    }

    fn name(&self) -> &'static str {
        "ClanMaster"
    }

    fn html_dir(&self) -> &'static str {
        "village_master/ClanMaster"
    }

    fn start_npcs(&self) -> &[i32] {
        &NPCS
    }

    fn talk_npcs(&self) -> &[i32] {
        &NPCS
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        Some("9000-01.htm".to_string())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if let Some((_, no)) = LEADER_REQUIRED.iter().find(|(page, _)| *page == event) {
            if !ctx.is_clan_leader() {
                return Some(no.to_string());
            }
        }
        Some(event.to_string())
    }
}
