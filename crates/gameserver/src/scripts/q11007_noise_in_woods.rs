//! Noise In Woods (11007) — `quests/Q11007_NoiseInWoods`.
//!
//! Newbie chain, Elf line, levels 2–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11007,
    name: "Q11007_NoiseInWoods",
    html_dir: "quests/Q11007_NoiseInWoods",
    start_npcs: &[30599],
    talk_npcs: &[30599, 30218],
    kill_npcs: &[20525, 20325, 20468, 20469, 20470, 20509],
    quest_items: &[90217, 90218, 90219, 90220, 90221],
    levels: (2, 20),
    race: super::newbie_chain::ELF,
    requires: None,
    start_event: "30599-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30218, 1, "30218-01.htm", 2, 90217, 1803507)),
    created_html: "30599-01.html",
    started_html: &[
        (30599, 1, "30599-02a.html"),
        (30218, 2, "30218-01a.html"),
        (30218, 6, "30218-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20525],
            cond: 2,
            item: 90218,
            need: 10,
            chance: 100,
            capped: true,
            next_cond: 3,
            msg: 1803508,
            advance_when: &[(90218, 10)],
        },
        Stage {
            monsters: &[20325],
            cond: 3,
            item: 90219,
            need: 10,
            chance: 100,
            capped: true,
            next_cond: 4,
            msg: 1803509,
            advance_when: &[(90219, 10)],
        },
        Stage {
            monsters: &[20468, 20469, 20470],
            cond: 4,
            item: 90220,
            need: 10,
            chance: 100,
            capped: true,
            next_cond: 5,
            msg: 1803510,
            advance_when: &[(90220, 10)],
        },
        Stage {
            monsters: &[20509],
            cond: 5,
            item: 90221,
            need: 20,
            chance: 100,
            capped: true,
            next_cond: 6,
            msg: 1803511,
            advance_when: &[(90221, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 6,
            take: &[
                (90217, 1),
                (90218, 10),
                (90219, 10),
                (90220, 10),
                (90221, 20),
            ],
            give: &[(49046, 1), (49041, 2), (49039, 1)],
            exp: 70000,
            sp: 0,
            html: "30218-04.html",
        },
        Reward {
            event: "reward2",
            cond: 6,
            take: &[
                (90217, 1),
                (90218, 10),
                (90219, 10),
                (90220, 10),
                (90221, 20),
            ],
            give: &[(49045, 1), (49041, 2), (49039, 1)],
            exp: 70000,
            sp: 0,
            html: "30218-03.html",
        },
    ],
};
