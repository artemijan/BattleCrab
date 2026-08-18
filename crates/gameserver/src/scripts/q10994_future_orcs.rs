//! Future Orcs (10994) — `quests/Q10994_FutureOrcs`.
//!
//! The Orc line's capstone: pick a class path at the
//! starter, then collect the reward from that path's trainer. No monsters.
//! Shape and Java citations in [`Capstone`].

use super::newbie_chain::Capstone;

pub const QUEST: Capstone = Capstone {
    id: 10994,
    name: "Q10994_FutureOrcs",
    html_dir: "quests/Q10994_FutureOrcs",
    start_npcs: &[30560],
    talk_npcs: &[30560, 30570, 30587, 30585],
    min_level: 19,
    race: super::newbie_chain::ORC,
    requires: ("Q11023_RedGemNecklace3", "30560-04.html"),
    plain_events: &[
        "30560-02.htm",
        "30560-02a.htm",
        "f_raider.html",
        "f_monk.html",
        "m_shaman.html",
    ],
    accepts: &[
        ("a_raider.html", 2),
        ("a_monk.html", 3),
        ("a_shaman.html", 4),
    ],
    trainers: &[
        (30570, 45, 2, "30570-01.html"),
        (30587, 47, 3, "30587-01.html"),
        (30585, 50, 4, "30585-01.html"),
    ],
    created: &[(44, "30560-01.html"), (49, "30560-01a.html")],
    started_html: None,
    finish_events: &["30570-02.html", "30587-02.html", "30585-02.html"],
    finish_give: &[(49772, 2), (49087, 1)],
};
