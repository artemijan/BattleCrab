//! Blacksmiths Request (11020) — `quests/Q11020_BlacksmithsRequest`.
//!
//! Newbie chain, Orc line, levels 11–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11020,
    name: "Q11020_BlacksmithsRequest",
    html_dir: "quests/Q11020_BlacksmithsRequest",
    start_npcs: &[30582],
    talk_npcs: &[30564, 30582],
    kill_npcs: &[20316, 20320, 20333, 20428],
    quest_items: &[90268, 90269, 90270, 90271, 90272],
    levels: (11, 20),
    race: super::newbie_chain::ORC,
    requires: None,
    start_event: "30582-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30564, 1, "30564-01.htm", 2, 90268, 1803555)),
    created_html: "30582-01.html",
    started_html: &[
        (30582, 1, "30582-02a.html"),
        (30564, 2, "30564-01a.html"),
        (30564, 6, "30564-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20316],
            cond: 2,
            item: 90269,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 3,
            msg: 1803556,
            advance_when: &[(90269, 20)],
        },
        Stage {
            monsters: &[20320],
            cond: 3,
            item: 90270,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 4,
            msg: 1803557,
            advance_when: &[(90270, 20)],
        },
        Stage {
            monsters: &[20333],
            cond: 4,
            item: 90271,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 5,
            msg: 1803558,
            advance_when: &[(90271, 20)],
        },
        Stage {
            monsters: &[20428],
            cond: 5,
            item: 90272,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 6,
            msg: 1803559,
            advance_when: &[(90272, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 6,
            take: &[
                (90268, 1),
                (90269, 20),
                (90270, 20),
                (90271, 20),
                (90272, 20),
            ],
            give: &[(90306, 1), (90307, 1), (49040, 2)],
            exp: 80000,
            sp: 0,
            html: "30564-03.html",
        },
        Reward {
            event: "reward2",
            cond: 6,
            take: &[
                (90268, 1),
                (90269, 20),
                (90270, 20),
                (90271, 20),
                (90272, 20),
            ],
            give: &[(90308, 1), (90309, 1), (49040, 2)],
            exp: 80000,
            sp: 0,
            html: "30564-04.html",
        },
    ],
};
