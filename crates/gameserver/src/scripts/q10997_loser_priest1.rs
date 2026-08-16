//! Loser Priest1 (10997) — `quests/Q10997_LoserPriest1`.
//!
//! Newbie chain, Dwarf line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 10997,
    name: "Q10997_LoserPriest1",
    html_dir: "quests/Q10997_LoserPriest1",
    start_npcs: &[30538],
    talk_npcs: &[30538, 30650],
    kill_npcs: &[20508, 20403],
    quest_items: &[90296, 90297, 90298],
    levels: (15, 20),
    race: super::newbie_chain::DWARF,
    requires: None,
    start_event: "30538-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30650, 1, "30650-01.htm", 2, 90296, 1803579)),
    created_html: "30538-01.html",
    started_html: &[
        (30538, 1, "30538-02a.html"),
        (30650, 2, "30650-01a.html"),
        (30650, 4, "30650-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20403],
            cond: 2,
            item: 90297,
            need: 20,
            chance: 94,
            capped: true,
            next_cond: 3,
            msg: 1803580,
            advance_when: &[(90297, 20)],
        },
        Stage {
            monsters: &[20508],
            cond: 3,
            item: 90298,
            need: 20,
            chance: 94,
            capped: true,
            next_cond: 4,
            msg: 1803581,
            advance_when: &[(90298, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 4,
            take: &[(90296, 1), (90297, 20), (90298, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30650-03.html",
        },
        Reward {
            event: "reward2",
            cond: 4,
            take: &[(90296, 1), (90297, 20), (90298, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30650-04.html",
        },
    ],
};
