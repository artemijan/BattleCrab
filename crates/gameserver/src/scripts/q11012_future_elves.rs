//! Future Elves (11012) — `quests/Q11012_FutureElves`.
//!
//! The Elf line's capstone: pick a class path at the
//! starter, then collect the reward from that path's trainer. No monsters.
//! Shape and Java citations in [`super::newbie_chain::Capstone`].

use super::newbie_chain::Capstone;

pub const QUEST: Capstone = Capstone {
    id: 11012,
    name: "Q11012_FutureElves",
    html_dir: "quests/Q11012_FutureElves",
    start_npcs: &[30150],
    talk_npcs: &[30150, 30327, 30328, 30414, 30293],
    min_level: 19,
    race: super::newbie_chain::ELF,
    requires: ("Q11011_NewPotionDevelopment3", "30150-04.html"),
    plain_events: &[
        "30150-02.htm",
        "30150-02a.htm",
        "f_knight.html",
        "f_scout.html",
        "m_wizard.html",
        "m_oracle.html",
    ],
    accepts: &[
        ("a_knight.html", 2),
        ("a_scout.html", 3),
        ("a_wizard.html", 4),
        ("a_oracle.html", 5),
    ],
    trainers: &[
        (30327, 19, 2, "30327-01.html"),
        (30328, 22, 3, "30328-01.html"),
        (30414, 26, 4, "30414-01.html"),
        (30293, 29, 5, "30293-01.html"),
    ],
    created: &[(18, "30150-01.html"), (25, "30150-01a.html")],
    started_html: None,
    finish_events: &[
        "30327-02.html",
        "30328-02.html",
        "30414-02.html",
        "30293-02.html",
    ],
    finish_give: &[(49772, 2), (49087, 1)],
};
