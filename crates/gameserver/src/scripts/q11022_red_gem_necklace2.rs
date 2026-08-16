//! Red Gem Necklace2 (11022) — `quests/Q11022_RedGemNecklace2`.
//!
//! Newbie chain, Orc line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.
//!
//! Gated on Q11021_RedGemNecklace1 — `addCondCompletedQuest`, checked after the
//! level and race pair.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11022,
    name: "Q11022_RedGemNecklace2",
    html_dir: "quests/Q11022_RedGemNecklace2",
    start_npcs: &[30560],
    talk_npcs: &[30560],
    kill_npcs: &[20479, 20474, 20476, 20478],
    quest_items: &[90277, 90278, 90279],
    levels: (15, 20),
    race: super::newbie_chain::ORC,
    requires: Some(("Q11021_RedGemNecklace1", "30560-06.html")),
    start_event: "30560-02.htm",
    start_brief: Some((2, 90277, 1803560)),
    plain_events: &["abort.html"],
    brief: None,
    created_html: "30560-01.html",
    started_html: &[(30560, 2, "30560-02a.html"), (30560, 4, "30560-03.html")],
    stages: &[
        Stage {
            monsters: &[20479],
            cond: 2,
            item: 90278,
            need: 20,
            chance: 92,
            capped: true,
            next_cond: 3,
            msg: 1803561,
            advance_when: &[(90278, 20)],
        },
        Stage {
            monsters: &[20474, 20476, 20478],
            cond: 3,
            item: 90279,
            need: 30,
            chance: 89,
            capped: true,
            next_cond: 4,
            msg: 1803562,
            advance_when: &[(90279, 30)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 4,
            take: &[(90277, 1), (90278, 20), (90279, 30)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30560-04.html",
        },
        Reward {
            event: "reward2",
            cond: 4,
            take: &[(90277, 1), (90278, 20), (90279, 30)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30560-05.html",
        },
    ],
};
