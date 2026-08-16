//! Tombs of Ancestors (11001) — `quests/Q11001_TombsOfAncestors`.
//!
//! The Human newbie line's first step (levels 2–20). The Newbie Guide in
//! Talking Island sends you to Altran, who wants ten each of wolf pelts, orc
//! amulets and werewolf fangs; the reward is a weapon of your choosing plus
//! starter jewellery and 70 000 XP. Shape and Java citations in
//! [`super::newbie_chain`].
//!
//! Two details are load-bearing and easy to lose:
//!
//! - **Cond 4 needs *both* drops.** Orc Warriors give Broken Swords and
//!   Werewolves give Fangs, and each branch's `advance_when` lists the pair —
//!   so whichever one you finish second is what advances the quest. Listing
//!   only a branch's own item would strand at cond 4 whoever capped the other
//!   first.
//! - **The turn-in does not take the Broken Swords.** Java's `reward1`/
//!   `reward2` consume the memo, pelts, amulets and fangs and leave the swords
//!   behind; `quest_items` is what clears them, on `exit_quest`. Kept as-is —
//!   folding them into `take` would change what a player who aborts keeps.

use super::newbie_chain::{Chain, Reward, Stage};

pub const QUEST: Chain = Chain {
    id: 11001,
    name: "Q11001_TombsOfAncestors",
    html_dir: "quests/Q11001_TombsOfAncestors",
    start_npcs: &[30598],
    talk_npcs: &[30598, 30283],
    kill_npcs: &[20120, 20442, 20130, 20131, 20006, 20093, 20132],
    quest_items: &[90199, 90200, 90201, 90202, 90203],
    levels: (2, 20),
    race: super::newbie_chain::HUMAN,
    requires: None,
    start_event: "30598-02.htm",
    start_brief: None,
    plain_events: &[],
    brief: Some((30283, 1, "30283-01.htm", 2, 90199, 1_803_490)),
    created_html: "30598-01.html",
    started_html: &[
        (30598, 1, "30598-02a.html"),
        (30283, 2, "30283-01a.html"),
        (30283, 5, "30283-02.html"),
    ],
    stages: &[
        Stage {
            monsters: &[20120, 20442],
            cond: 2,
            item: 90200,
            need: 10,
            chance: 93,
            capped: true,
            next_cond: 3,
            msg: 1_803_491,
            advance_when: &[(90200, 10)],
        },
        Stage {
            monsters: &[20130, 20131, 20006],
            cond: 3,
            item: 90201,
            need: 10,
            chance: 93,
            capped: true,
            next_cond: 4,
            msg: 1_803_492,
            advance_when: &[(90201, 10)],
        },
        // The two halves of cond 4 — see the module note.
        Stage {
            monsters: &[20093],
            cond: 4,
            item: 90203,
            need: 10,
            chance: 89,
            capped: true,
            next_cond: 5,
            msg: 1_803_493,
            advance_when: &[(90203, 10), (90202, 10)],
        },
        Stage {
            monsters: &[20132],
            cond: 4,
            item: 90202,
            need: 10,
            chance: 100,
            capped: true,
            next_cond: 5,
            msg: 1_803_493,
            advance_when: &[(90202, 10), (90203, 10)],
        },
    ],
    rewards: &[
        Reward {
            event: "reward1",
            cond: 5,
            take: &[(90199, 1), (90200, 10), (90201, 10), (90202, 10)],
            give: &[(49043, 1), (49041, 2), (49039, 1)],
            exp: 70_000,
            sp: 0,
            // Java's own comment: "Need other html" — both branches render
            // Altran's completion page rather than one of their own.
            html: "30283-03.html",
        },
        Reward {
            event: "reward2",
            cond: 5,
            take: &[(90199, 1), (90200, 10), (90201, 10), (90202, 10)],
            give: &[(49044, 1), (49041, 2), (49039, 1)],
            exp: 70_000,
            sp: 0,
            html: "30283-03.html",
        },
    ],
};
