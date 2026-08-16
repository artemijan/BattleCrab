//! Shilens Hunt (11013) — `quests/Q11013_ShilensHunt`.
//!
//! Newbie chain, Dark Elf line, levels 2–20. The shape is
//! [`super::newbie_chain`]'s; this file is the table that fills it in, and
//! every id and count below is Java's.
//!
//! **This quest's stages are uncapped**: Java omits both the
//! `getQuestItemsCount(...) < need` guard and the `getRandom` roll, so every
//! kill drops and the count runs past the requirement.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11013,
    name: "Q11013_ShilensHunt",
    html_dir: "quests/Q11013_ShilensHunt",
    start_npcs: &[30600],
    talk_npcs: &[30600, 30141],
    kill_npcs: &[20456, 20003, 20004, 20005, 20007, 20386, 20387, 20388],
    quest_items: &[90237, 90238, 90239, 90240, 90241, 90242],
    levels: (2, 20),
    race: super::newbie_chain::DARK_ELF,
    requires: None,
    start_event: "30600-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30141, 1, "30141-01.htm", 2, 90237, 1803525)),
    created_html: "30600-01.html",
    started_html: &[
        (30600, 1, "30600-02a.html"),
        (30141, 2, "30141-01a.html"),
        (30141, 7, "30141-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20456],
            cond: 2,
            item: 90238,
            need: 10,
            chance: 100,
            capped: false,
            next_cond: 3,
            msg: 1803526,
            advance_when: &[(90238, 10)],
        },
        Stage {
            monsters: &[20003],
            cond: 3,
            item: 90239,
            need: 10,
            chance: 100,
            capped: false,
            next_cond: 4,
            msg: 1803527,
            advance_when: &[(90239, 10)],
        },
        Stage {
            monsters: &[20004, 20005],
            cond: 4,
            item: 90240,
            need: 10,
            chance: 100,
            capped: false,
            next_cond: 5,
            msg: 1803528,
            advance_when: &[(90240, 10)],
        },
        Stage {
            monsters: &[20007],
            cond: 5,
            item: 90241,
            need: 10,
            chance: 100,
            capped: false,
            next_cond: 6,
            msg: 1803529,
            advance_when: &[(90241, 10)],
        },
        Stage {
            monsters: &[20386, 20387, 20388],
            cond: 6,
            item: 90242,
            need: 10,
            chance: 100,
            capped: false,
            next_cond: 7,
            msg: 1803530,
            advance_when: &[(90242, 10)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 7,
            take: &[
                (90237, 1),
                (90238, 10),
                (90239, 10),
                (90240, 10),
                (90241, 10),
                (90242, 10),
            ],
            give: &[(49050, 1), (49041, 2), (49039, 1)],
            exp: 70000,
            sp: 0,
            html: "30141-03.html",
        },
        Reward {
            event: "reward2",
            cond: 7,
            take: &[
                (90237, 1),
                (90238, 10),
                (90239, 10),
                (90240, 10),
                (90241, 10),
                (90242, 10),
            ],
            give: &[(49049, 1), (49041, 2), (49039, 1)],
            exp: 70000,
            sp: 0,
            html: "30141-03.html",
        },
    ],
};
