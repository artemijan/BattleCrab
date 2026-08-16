//! Temples Decorations (10996) — `quests/Q10996_TemplesDecorations`.
//!
//! Newbie chain, Dwarf line, levels 11–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 10996,
    name: "Q10996_TemplesDecorations",
    html_dir: "quests/Q10996_TemplesDecorations",
    start_npcs: &[30516],
    talk_npcs: &[30538, 30516],
    kill_npcs: &[20370, 20510, 20528, 20323, 20521, 20526],
    quest_items: &[90290, 90291, 90292, 90293, 90294],
    levels: (11, 20),
    race: super::newbie_chain::DWARF,
    requires: None,
    start_event: "30516-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30538, 1, "30538-01.htm", 2, 90290, 1803574)),
    created_html: "30516-01.html",
    started_html: &[
        (30516, 1, "30516-02a.html"),
        (30538, 2, "30538-01a.html"),
        (30538, 6, "30538-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20370],
            cond: 2,
            item: 90291,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 3,
            msg: 1803575,
            advance_when: &[(90291, 20)],
        },
        Stage {
            monsters: &[20510],
            cond: 3,
            item: 90292,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 4,
            msg: 1803576,
            advance_when: &[(90292, 20)],
        },
        Stage {
            monsters: &[20528, 20323],
            cond: 4,
            item: 90293,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 5,
            msg: 1803577,
            advance_when: &[(90293, 20)],
        },
        Stage {
            monsters: &[20521, 20526],
            cond: 5,
            item: 90294,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 6,
            msg: 1803578,
            advance_when: &[(90294, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 6,
            take: &[
                (90290, 1),
                (90291, 20),
                (90292, 20),
                (90293, 20),
                (90294, 20),
            ],
            give: &[(90306, 1), (90307, 1), (49041, 2)],
            exp: 80000,
            sp: 0,
            html: "30538-03.html",
        },
        Reward {
            event: "reward2",
            cond: 6,
            take: &[
                (90290, 1),
                (90291, 20),
                (90292, 20),
                (90293, 20),
                (90294, 20),
            ],
            give: &[(90308, 1), (90309, 1), (49041, 2)],
            exp: 80000,
            sp: 0,
            html: "30538-04.html",
        },
    ],
};
