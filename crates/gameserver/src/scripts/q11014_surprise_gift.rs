//! Surprise Gift (11014) — `quests/Q11014_SurpriseGift`.
//!
//! Newbie chain, Dark Elf line, levels 11–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11014,
    name: "Q11014_SurpriseGift",
    html_dir: "quests/Q11014_SurpriseGift",
    start_npcs: &[30141],
    talk_npcs: &[30136, 30141],
    kill_npcs: &[20015, 20020, 20433, 20392, 20380, 20379, 20105],
    quest_items: &[90243, 90244, 90245, 90246, 90247],
    levels: (11, 20),
    race: super::newbie_chain::DARK_ELF,
    requires: None,
    start_event: "30141-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30136, 1, "30136-01.htm", 2, 90243, 1803531)),
    created_html: "30141-01.html",
    started_html: &[
        (30141, 1, "30141-02a.html"),
        (30136, 2, "30136-01a.html"),
        (30136, 6, "30136-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20015, 20020],
            cond: 2,
            item: 90244,
            need: 10,
            chance: 85,
            capped: true,
            next_cond: 3,
            msg: 1803532,
            advance_when: &[(90244, 10)],
        },
        Stage {
            monsters: &[20433, 20392],
            cond: 3,
            item: 90245,
            need: 10,
            chance: 85,
            capped: true,
            next_cond: 4,
            msg: 1803533,
            advance_when: &[(90245, 10)],
        },
        Stage {
            monsters: &[20379, 20380],
            cond: 4,
            item: 90246,
            need: 10,
            chance: 85,
            capped: true,
            next_cond: 5,
            msg: 1803534,
            advance_when: &[(90246, 10)],
        },
        Stage {
            monsters: &[20105],
            cond: 5,
            item: 90247,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 6,
            msg: 1803535,
            advance_when: &[(90247, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 6,
            take: &[
                (90243, 1),
                (90244, 10),
                (90245, 10),
                (90246, 10),
                (90247, 20),
            ],
            give: &[(90306, 1), (90307, 1), (49041, 2)],
            exp: 80000,
            sp: 0,
            html: "30136-03.html",
        },
        Reward {
            event: "reward2",
            cond: 6,
            take: &[
                (90243, 1),
                (90244, 10),
                (90245, 10),
                (90246, 10),
                (90247, 20),
            ],
            give: &[(90308, 1), (90309, 1), (49041, 2)],
            exp: 80000,
            sp: 0,
            html: "30136-04.html",
        },
    ],
};
