//! New Potion Development3 (11011) — `quests/Q11011_NewPotionDevelopment3`.
//!
//! Newbie chain, Elf line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.
//!
//! Gated on Q11010_NewPotionDevelopment2 — `addCondCompletedQuest`, checked after the
//! level and race pair.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11011,
    name: "Q11011_NewPotionDevelopment3",
    html_dir: "quests/Q11011_NewPotionDevelopment3",
    start_npcs: &[30150],
    talk_npcs: &[30150],
    kill_npcs: &[20039, 20043],
    quest_items: &[90234, 90235, 90236],
    levels: (15, 20),
    race: super::newbie_chain::ELF,
    requires: Some(("Q11010_NewPotionDevelopment2", "30150-05.html")),
    start_event: "30150-02.htm",
    start_brief: Some((2, 90234, 1803522)),
    plain_events: &["abort.html"],
    brief: None,
    created_html: "30150-01.html",
    started_html: &[(30150, 2, "30150-02a.html"), (30150, 4, "30150-03.html")],
    stages: &[
        Stage {
            monsters: &[20039],
            cond: 2,
            item: 90235,
            need: 20,
            chance: 95,
            capped: true,
            next_cond: 3,
            msg: 1803523,
            advance_when: &[(90235, 20)],
        },
        Stage {
            monsters: &[20043],
            cond: 3,
            item: 90236,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 4,
            msg: 1803524,
            advance_when: &[(90236, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 4,
            take: &[(90234, 1), (90235, 20), (90236, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30150-04.html",
        },
        Reward {
            event: "reward2",
            cond: 4,
            take: &[(90234, 1), (90235, 20), (90236, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30150-04.html",
        },
    ],
};
