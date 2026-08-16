//! Preparation For Dungeon (11008) — `quests/Q11008_PreparationForDungeon`.
//!
//! Newbie chain, Elf line, levels 11–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11008,
    name: "Q11008_PreparationForDungeon",
    html_dir: "quests/Q11008_PreparationForDungeon",
    start_npcs: &[30218],
    talk_npcs: &[30218, 30220],
    kill_npcs: &[20471, 20472, 20473, 20013, 20019, 20308, 20460, 20466],
    quest_items: &[90222, 90223, 90224, 90225],
    levels: (11, 20),
    race: super::newbie_chain::ELF,
    requires: None,
    start_event: "30218-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30220, 1, "30220-01.htm", 2, 90222, 1803512)),
    created_html: "30218-01.html",
    started_html: &[
        (30218, 1, "30218-02a.html"),
        (30220, 2, "30220-01a.html"),
        (30220, 5, "30220-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20471, 20472, 20473],
            cond: 2,
            item: 90223,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 3,
            msg: 1803513,
            advance_when: &[(90223, 20)],
        },
        Stage {
            monsters: &[20013, 20019],
            cond: 3,
            item: 90224,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 4,
            msg: 1803514,
            advance_when: &[(90224, 20)],
        },
        Stage {
            monsters: &[20308, 20460, 20466],
            cond: 4,
            item: 90225,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 5,
            msg: 1803515,
            advance_when: &[(90225, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 5,
            take: &[(90222, 1), (90223, 20), (90224, 20), (90225, 20)],
            give: &[(90306, 1), (90307, 1), (49041, 2)],
            exp: 80000,
            sp: 0,
            html: "30220-03.html",
        },
        Reward {
            event: "reward2",
            cond: 5,
            take: &[(90222, 1), (90223, 20), (90224, 20), (90225, 20)],
            give: &[(90308, 1), (90309, 1), (49041, 2)],
            exp: 80000,
            sp: 0,
            html: "30220-04.html",
        },
    ],
};
