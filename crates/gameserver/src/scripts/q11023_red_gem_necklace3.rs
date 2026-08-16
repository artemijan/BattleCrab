//! Red Gem Necklace3 (11023) — `quests/Q11023_RedGemNecklace3`.
//!
//! Newbie chain, Orc line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.
//!
//! Gated on Q11022_RedGemNecklace2 — `addCondCompletedQuest`, checked after the
//! level and race pair.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11023,
    name: "Q11023_RedGemNecklace3",
    html_dir: "quests/Q11023_RedGemNecklace3",
    start_npcs: &[30560],
    talk_npcs: &[30560],
    kill_npcs: &[21257, 21117],
    quest_items: &[90280, 90282, 90281],
    levels: (15, 20),
    race: super::newbie_chain::ORC,
    requires: Some(("Q11022_RedGemNecklace2", "30560-06.html")),
    start_event: "30560-02.htm",
    start_brief: Some((2, 90280, 1803565)),
    plain_events: &["abort.html"],
    brief: None,
    created_html: "30560-01.html",
    started_html: &[(30560, 2, "30560-02a.html"), (30560, 4, "30560-03.html")],
    stages: &[
        Stage {
            monsters: &[21257],
            cond: 2,
            item: 90282,
            need: 20,
            chance: 92,
            capped: true,
            next_cond: 3,
            msg: 1803566,
            advance_when: &[(90282, 20)],
        },
        Stage {
            monsters: &[21117],
            cond: 3,
            item: 90281,
            need: 20,
            chance: 91,
            capped: true,
            next_cond: 4,
            msg: 1803567,
            advance_when: &[(90281, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 4,
            take: &[(90280, 1), (90282, 20), (90281, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30560-04.html",
        },
        Reward {
            event: "reward2",
            cond: 4,
            take: &[(90280, 1), (90282, 20), (90281, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30560-05.html",
        },
    ],
};
