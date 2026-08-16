//! Prepare For Trade2 (11016) — `quests/Q11016_PrepareForTrade2`.
//!
//! Newbie chain, Dark Elf line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.
//!
//! Gated on Q11015_PrepareForTrade1 — `addCondCompletedQuest`, checked after the
//! level and race pair.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11016,
    name: "Q11016_PrepareForTrade2",
    html_dir: "quests/Q11016_PrepareForTrade2",
    start_npcs: &[30137],
    talk_npcs: &[30137],
    kill_npcs: &[20380, 20418, 20034, 20038, 20043],
    quest_items: &[90253, 90254, 90255, 90256],
    levels: (15, 20),
    race: super::newbie_chain::DARK_ELF,
    requires: Some(("Q11015_PrepareForTrade1", "30137-06.html")),
    start_event: "30137-02.htm",
    start_brief: Some((2, 90253, 1803536)),
    plain_events: &["abort.html"],
    brief: None,
    created_html: "30137-01.html",
    started_html: &[(30137, 2, "30137-02a.html"), (30137, 5, "30137-03.html")],
    stages: &[
        Stage {
            monsters: &[20380],
            cond: 2,
            item: 90254,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 3,
            msg: 1803537,
            advance_when: &[(90254, 20)],
        },
        Stage {
            monsters: &[20418],
            cond: 3,
            item: 90255,
            need: 10,
            chance: 87,
            capped: true,
            next_cond: 4,
            msg: 1803538,
            advance_when: &[(90255, 10)],
        },
        Stage {
            monsters: &[20034, 20038, 20043],
            cond: 4,
            item: 90256,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 5,
            msg: 1803539,
            advance_when: &[(90256, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 5,
            take: &[(90253, 1), (90254, 20), (90255, 10), (90256, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30137-04.html",
        },
        Reward {
            event: "reward2",
            cond: 5,
            take: &[(90253, 1), (90254, 20), (90255, 10), (90256, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30137-05.html",
        },
    ],
};
