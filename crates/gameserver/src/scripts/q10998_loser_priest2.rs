//! Loser Priest2 (10998) — `quests/Q10998_LoserPriest2`.
//!
//! Newbie chain, Dwarf line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.
//!
//! Gated on Q10997_LoserPriest1 — `addCondCompletedQuest`, checked after the
//! level and race pair.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 10998,
    name: "Q10998_LoserPriest2",
    html_dir: "quests/Q10998_LoserPriest2",
    start_npcs: &[30650],
    talk_npcs: &[30650],
    kill_npcs: &[20508, 20403],
    quest_items: &[90299, 90300, 90301],
    levels: (15, 20),
    race: super::newbie_chain::DWARF,
    requires: Some(("Q10997_LoserPriest1", "30650-06.html")),
    start_event: "30650-02.htm",
    start_brief: Some((1, 90299, 1803579)),
    plain_events: &["abort.html"],
    brief: None,
    created_html: "30650-01.html",
    started_html: &[(30650, 2, "30650-02a.html"), (30650, 4, "30650-03.html")],
    stages: &[
        Stage {
            monsters: &[20403],
            cond: 2,
            item: 90300,
            need: 20,
            chance: 94,
            capped: true,
            next_cond: 3,
            msg: 1803580,
            advance_when: &[(90300, 20)],
        },
        Stage {
            monsters: &[20508],
            cond: 3,
            item: 90301,
            need: 20,
            chance: 94,
            capped: true,
            next_cond: 4,
            msg: 1803581,
            advance_when: &[(90301, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 4,
            take: &[(90299, 1), (90300, 20), (90301, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30650-04.html",
        },
        Reward {
            event: "reward2",
            cond: 4,
            take: &[(90299, 1), (90300, 20), (90301, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30650-05.html",
        },
    ],
};
