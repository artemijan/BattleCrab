//! Help With Temple Restoration (11002) — `quests/Q11002_HelpWithTempleRestoration`.
//!
//! Newbie chain, Human line, levels 11–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11002,
    name: "Q11002_HelpWithTempleRestoration",
    html_dir: "quests/Q11002_HelpWithTempleRestoration",
    start_npcs: &[30283],
    talk_npcs: &[30035, 30283],
    kill_npcs: &[20098, 20096, 20343, 20342, 20016, 20101],
    quest_items: &[90204, 90205, 90206, 90207],
    levels: (11, 20),
    race: super::newbie_chain::HUMAN,
    requires: None,
    start_event: "30283-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30035, 1, "30035-01.htm", 2, 90204, 1803494)),
    created_html: "30283-01.html",
    started_html: &[
        (30283, 1, "30283-02a.html"),
        (30035, 2, "30035-01a.html"),
        (30035, 5, "30035-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20098, 20096],
            cond: 2,
            item: 90205,
            need: 20,
            chance: 84,
            capped: true,
            next_cond: 3,
            msg: 1803495,
            advance_when: &[(90205, 20)],
        },
        Stage {
            monsters: &[20343, 20342],
            cond: 3,
            item: 90206,
            need: 25,
            chance: 87,
            capped: true,
            next_cond: 4,
            msg: 1803496,
            advance_when: &[(90206, 25)],
        },
        Stage {
            monsters: &[20101, 20016],
            cond: 4,
            item: 90207,
            need: 20,
            chance: 84,
            capped: true,
            next_cond: 5,
            msg: 1803497,
            advance_when: &[(90207, 20), (90207, 10)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 5,
            take: &[(90204, 1), (90205, 20), (90206, 25), (90207, 20)],
            give: &[(90306, 1), (90307, 1), (49041, 2)],
            exp: 80000,
            sp: 0,
            html: "30035-03.html",
        },
        Reward {
            event: "reward2",
            cond: 5,
            take: &[(90204, 1), (90205, 20), (90206, 25), (90207, 20)],
            give: &[(90308, 1), (90309, 1), (49041, 2)],
            exp: 80000,
            sp: 0,
            html: "30035-04.html",
        },
    ],
};
