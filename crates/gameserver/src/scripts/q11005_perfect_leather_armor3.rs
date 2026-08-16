//! Perfect Leather Armor3 (11005) — `quests/Q11005_PerfectLeatherArmor3`.
//!
//! Newbie chain, Human line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.
//!
//! Gated on Q11004_PerfectLeatherArmor2 — `addCondCompletedQuest`, checked after the
//! level and race pair.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11005,
    name: "Q11005_PerfectLeatherArmor3",
    html_dir: "quests/Q11005_PerfectLeatherArmor3",
    start_npcs: &[30001],
    talk_npcs: &[30001],
    kill_npcs: &[20103, 20106, 20108, 20110, 20113, 20115],
    quest_items: &[90214, 90215, 90216],
    levels: (15, 20),
    race: super::newbie_chain::HUMAN,
    requires: Some(("Q11004_PerfectLeatherArmor2", "30001-06.html")),
    start_event: "30001-02.htm",
    start_brief: Some((2, 90214, 1803498)),
    plain_events: &["abort.html"],
    brief: None,
    created_html: "30001-01.html",
    started_html: &[(30001, 2, "30001-02a.html"), (30001, 4, "30001-03.html")],
    stages: &[
        Stage {
            monsters: &[20103, 20108, 20106],
            cond: 2,
            item: 90215,
            need: 25,
            chance: 87,
            capped: true,
            next_cond: 3,
            msg: 1803499,
            advance_when: &[(90215, 25)],
        },
        Stage {
            monsters: &[20110, 20113, 20115],
            cond: 3,
            item: 90216,
            need: 20,
            chance: 100,
            capped: true,
            next_cond: 4,
            msg: 1803500,
            advance_when: &[(90216, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 4,
            take: &[(90214, 1), (90215, 25), (90216, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30001-04.html",
        },
        Reward {
            event: "reward2",
            cond: 4,
            take: &[(90214, 1), (90215, 25), (90216, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30001-05.html",
        },
    ],
};
