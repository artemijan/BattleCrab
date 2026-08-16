//! Prepare For Trade1 (11015) — `quests/Q11015_PrepareForTrade1`.
//!
//! Newbie chain, Dark Elf line, levels 15–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11015,
    name: "Q11015_PrepareForTrade1",
    html_dir: "quests/Q11015_PrepareForTrade1",
    start_npcs: &[30136],
    talk_npcs: &[30136, 30137],
    kill_npcs: &[20380, 20418, 20034, 20038, 20043],
    quest_items: &[90249, 90250, 90251, 90252],
    levels: (15, 20),
    race: super::newbie_chain::DARK_ELF,
    requires: None,
    start_event: "30136-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30137, 1, "30137-01.htm", 2, 90249, 1803536)),
    created_html: "30136-01.html",
    started_html: &[
        (30136, 1, "30136-02a.html"),
        (30137, 2, "30137-01a.html"),
        (30137, 5, "30137-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20380],
            cond: 2,
            item: 90250,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 3,
            msg: 1803537,
            advance_when: &[(90250, 20)],
        },
        Stage {
            monsters: &[20418],
            cond: 3,
            item: 90251,
            need: 10,
            chance: 87,
            capped: true,
            next_cond: 4,
            msg: 1803538,
            advance_when: &[(90251, 10)],
        },
        Stage {
            monsters: &[20034, 20038, 20043],
            cond: 4,
            item: 90252,
            need: 20,
            chance: 90,
            capped: true,
            next_cond: 5,
            msg: 1803539,
            advance_when: &[(90252, 20)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 5,
            take: &[(90249, 1), (90250, 20), (90251, 10), (90252, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5789, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30137-03.html",
        },
        Reward {
            event: "reward2",
            cond: 5,
            take: &[(90249, 1), (90250, 20), (90251, 10), (90252, 20)],
            give: &[(10650, 5), (1073, 40), (90310, 40), (5790, 1000)],
            exp: 70000,
            sp: 3600,
            html: "30137-04.html",
        },
    ],
};
