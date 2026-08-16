//! Red Gem Necklace1 (11021) — `quests/Q11021_RedGemNecklace1`.
//!
//! Newbie chain, Orc line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11021,
    name: "Q11021_RedGemNecklace1",
    html_dir: "quests/Q11021_RedGemNecklace1",
    start_npcs: &[30564],
    talk_npcs: &[30564, 30560],
    kill_npcs: &[20479, 20474, 20476, 20478],
    quest_items: &[90274, 90275, 90276],
    levels: (15, 20),
    race: super::newbie_chain::ORC,
    requires: None,
    start_event: "30564-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30560, 1, "30560-01.htm", 2, 90274, 1803560)),
    created_html: "30564-01.html",
    started_html: &[
        (30564, 1, "30564-02a.html"),
        (30560, 2, "30560-01a.html"),
        (30560, 4, "30560-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20479],
            cond: 2,
            item: 90275,
            need: 20,
            chance: 92,
            capped: true,
            next_cond: 3,
            msg: 1803561,
            advance_when: &[(90275, 20)],
        },
        Stage {
            monsters: &[20474, 20476, 20478],
            cond: 3,
            item: 90276,
            need: 30,
            chance: 89,
            capped: true,
            next_cond: 4,
            msg: 1803562,
            advance_when: &[(90276, 30)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 4,
            take: &[(90274, 1), (90275, 20), (90276, 30)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30560-03.html",
        },
        Reward {
            event: "reward2",
            cond: 4,
            take: &[(90274, 1), (90275, 20), (90276, 30)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30560-04.html",
        },
    ],
};
