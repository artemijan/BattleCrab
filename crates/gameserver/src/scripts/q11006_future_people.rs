//! Future People (11006) — `quests/Q11006_FuturePeople`.
//!
//! The Human line's capstone: pick a class path at the
//! starter, then collect the reward from that path's trainer. No monsters.
//! Shape and Java citations in [`super::newbie_chain::Capstone`].
//!
//! **Java bug, reproduced.** `a_cleric.html` sets cond **5** — the same cond
//! `a_wizard.html` sets — while Zigaunt, the cleric trainer, answers only at
//! cond 6. A player who picks the cleric path is therefore served by Parina
//! (the wizard trainer, cond 5) and Zigaunt's page is unreachable. Both
//! hand out the same reward, so the quest still completes.

use super::newbie_chain::Capstone;

pub const QUEST: Capstone = Capstone {
    id: 11006,
    name: "Q11006_FuturePeople",
    html_dir: "quests/Q11006_FuturePeople",
    start_npcs: &[30001],
    talk_npcs: &[30136, 30001, 30391, 30022, 30010, 30417, 30379],
    min_level: 19,
    race: super::newbie_chain::HUMAN,
    requires: ("Q11005_PerfectLeatherArmor3", "30001-04.html"),
    plain_events: &[
        "30001-02.htm",
        "30001-02a.htm",
        "f_warrior.html",
        "f_knight.html",
        "f_rogue.html",
        "m_wizard.html",
        "m_cleric.html",
    ],
    accepts: &[
        ("a_warrior.html", 2),
        ("a_knight.html", 3),
        ("a_rogue.html", 4),
        ("a_wizard.html", 5),
        ("a_cleric.html", 5),
    ],
    trainers: &[
        (30391, 11, 5, "30391-01.html"),
        (30022, 15, 6, "30022-01.html"),
        (30010, 1, 2, "30010-01.html"),
        (30417, 1, 3, "30417-01.html"),
        (30379, 1, 4, "30379-01.html"),
    ],
    created: &[(0, "30001-01.html"), (10, "30001-01a.html")],
    started_html: None,
    finish_events: &[
        "30391-02.html",
        "30022-02.html",
        "30010-02.html",
        "30417-02.html",
        "30379-02.html",
    ],
    finish_give: &[(49772, 2), (49087, 1)],
};
