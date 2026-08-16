//! Loser Priest3 (10999) — `quests/Q10999_LoserPriest3`.
//!
//! Newbie chain, Dwarf line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.
//!
//! Gated on Q10998_LoserPriest2 — `addCondCompletedQuest`, checked after the
//! level and race pair.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 10999,
    name: "Q10999_LoserPriest3",
    html_dir: "quests/Q10999_LoserPriest3",
    start_npcs: &[30650],
    talk_npcs: &[30650],
    kill_npcs: &[21125, 21124, 21129, 21126],
    quest_items: &[90302, 90303, 90304, 90305],
    levels: (15, 20),
    race: super::newbie_chain::DWARF,
    requires: Some(("Q10998_LoserPriest2", "30650-05.html")),
    start_event: "30650-02.htm",
    start_brief: Some((2, 90302, 1803585)),
    plain_events: &["abort.html"],
    brief: None,
    created_html: "30650-01.html",
    started_html: &[(30650, 2, "30650-02a.html"), (30650, 5, "30650-03.html")],
    stages: &[
        Stage {
            monsters: &[21124],
            cond: 2,
            item: 90303,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 3,
            msg: 1803586,
            advance_when: &[(90303, 20)],
        },
        Stage {
            monsters: &[21125],
            cond: 3,
            item: 90304,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 4,
            msg: 1803587,
            advance_when: &[(90304, 20)],
        },
        Stage {
            monsters: &[21129, 21126],
            cond: 4,
            item: 90305,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 5,
            msg: 1803588,
            advance_when: &[(90305, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 5,
            take: &[(90302, 1), (90303, 20), (90304, 20), (90305, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30650-04.html",
        },
        Reward {
            event: "reward2",
            cond: 5,
            take: &[(90302, 1), (90303, 20), (90304, 20), (90305, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30650-04.html",
        },
    ],
};
