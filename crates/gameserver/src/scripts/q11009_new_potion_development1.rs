//! New Potion Development1 (11009) — `quests/Q11009_NewPotionDevelopment1`.
//!
//! Newbie chain, Elf line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11009,
    name: "Q11009_NewPotionDevelopment1",
    html_dir: "quests/Q11009_NewPotionDevelopment1",
    start_npcs: &[30220],
    talk_npcs: &[30220, 30150],
    kill_npcs: &[20410, 20393, 20369],
    quest_items: &[90228, 90229, 90230],
    levels: (15, 20),
    race: super::newbie_chain::ELF,
    requires: None,
    start_event: "30220-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30150, 1, "30150-01.htm", 2, 90228, 1803516)),
    created_html: "30220-01.html",
    started_html: &[
        (30220, 1, "30220-02a.html"),
        (30150, 2, "30150-01a.html"),
        (30150, 4, "30150-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20410, 20393],
            cond: 2,
            item: 90229,
            need: 20,
            chance: 92,
            capped: true,
            next_cond: 3,
            msg: 1803517,
            advance_when: &[(90229, 20)],
        },
        Stage {
            monsters: &[20369],
            cond: 3,
            item: 90230,
            need: 20,
            chance: 92,
            capped: true,
            next_cond: 4,
            msg: 1803518,
            advance_when: &[(90230, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 4,
            take: &[(90228, 1), (90229, 20), (90230, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30150-03.html",
        },
        Reward {
            event: "reward2",
            cond: 4,
            take: &[(90228, 1), (90229, 20), (90230, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30150-04.html",
        },
    ],
};
