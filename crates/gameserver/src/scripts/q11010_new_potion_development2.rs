//! New Potion Development2 (11010) — `quests/Q11010_NewPotionDevelopment2`.
//!
//! Newbie chain, Elf line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.
//!
//! Gated on Q11009_NewPotionDevelopment1 — `addCondCompletedQuest`, checked after the
//! level and race pair.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11010,
    name: "Q11010_NewPotionDevelopment2",
    html_dir: "quests/Q11010_NewPotionDevelopment2",
    start_npcs: &[30150],
    talk_npcs: &[30150],
    kill_npcs: &[20410, 20393, 20369],
    quest_items: &[90231, 90232, 90233],
    levels: (15, 20),
    race: super::newbie_chain::ELF,
    requires: Some(("Q11009_NewPotionDevelopment1", "30150-06.html")),
    start_event: "30150-02.htm",
    start_brief: Some((2, 90231, 1803516)),
    plain_events: &["abort.html"],
    brief: None,
    created_html: "30150-01.html",
    started_html: &[(30150, 2, "30150-02a.html"), (30150, 4, "30150-03.html")],
    stages: &[
        Stage {
            monsters: &[20410, 20393],
            cond: 2,
            item: 90232,
            need: 20,
            chance: 92,
            capped: true,
            next_cond: 3,
            msg: 1803517,
            advance_when: &[(90232, 20)],
        },
        Stage {
            monsters: &[20369],
            cond: 3,
            item: 90233,
            need: 20,
            chance: 92,
            capped: true,
            next_cond: 4,
            msg: 1803518,
            advance_when: &[(90233, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 4,
            take: &[(90231, 1), (90232, 20), (90233, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30150-04.html",
        },
        Reward {
            event: "reward2",
            cond: 4,
            take: &[(90231, 1), (90232, 20), (90233, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30150-05.html",
        },
    ],
};
