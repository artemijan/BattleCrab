//! Perfect Leather Armor1 (11003) — `quests/Q11003_PerfectLeatherArmor1`.
//!
//! Newbie chain, Human line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11003,
    name: "Q11003_PerfectLeatherArmor1",
    html_dir: "quests/Q11003_PerfectLeatherArmor1",
    start_npcs: &[30035],
    talk_npcs: &[30035, 30001],
    kill_npcs: &[20103, 20106, 20108, 20110, 20113, 20115],
    quest_items: &[90208, 90209, 90210],
    levels: (15, 20),
    race: super::newbie_chain::HUMAN,
    requires: None,
    start_event: "30035-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30001, 1, "30001-01.htm", 2, 90208, 1803498)),
    created_html: "30035-01.html",
    started_html: &[
        (30035, 1, "30035-02a.html"),
        (30001, 2, "30001-01a.html"),
        (30001, 4, "30001-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20103, 20108, 20106],
            cond: 2,
            item: 90209,
            need: 25,
            chance: 87,
            capped: true,
            next_cond: 3,
            msg: 1803499,
            advance_when: &[(90209, 25)],
        },
        Stage {
            monsters: &[20110, 20113, 20115],
            cond: 3,
            item: 90210,
            need: 20,
            chance: 100,
            capped: true,
            next_cond: 4,
            msg: 1803500,
            advance_when: &[(90210, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 4,
            take: &[(90208, 1), (90209, 25), (90210, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30001-03.html",
        },
        Reward {
            event: "reward2",
            cond: 4,
            take: &[(90208, 1), (90209, 25), (90210, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30001-04.html",
        },
    ],
};
