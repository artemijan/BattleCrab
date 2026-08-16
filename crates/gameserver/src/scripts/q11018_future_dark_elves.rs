//! Future Dark Elves (11018) — `quests/Q11018_FutureDarkElves`.
//!
//! The Dark Elf line's capstone: pick a class path at the
//! starter, then collect the reward from that path's trainer. No monsters.
//! Shape and Java citations in [`super::newbie_chain::Capstone`].

use super::newbie_chain::Capstone;

pub const QUEST: Capstone = Capstone {
    id: 11018,
    name: "Q11018_FutureDarkElves",
    html_dir: "quests/Q11018_FutureDarkElves",
    start_npcs: &[30137],
    talk_npcs: &[30329, 30137, 30416, 30421, 30330],
    min_level: 19,
    race: super::newbie_chain::DARK_ELF,
    requires: ("Q11017_PrepareForTrade3", "30137-04.html"),
    plain_events: &[
        "30137-02.htm",
        "30137-02a.htm",
        "f_PalusKnight.html",
        "f_assassin.html",
        "m_wizard.html",
        "m_shillien.html",
    ],
    accepts: &[
        ("a_PalusKnight.html", 2),
        ("a_assassin.html", 3),
        ("a_wizard.html", 4),
        ("a_shillien.html", 5),
    ],
    trainers: &[
        (30329, 32, 2, "30329-01.html"),
        (30416, 35, 3, "30416-01.html"),
        (30421, 39, 4, "30421-01.html"),
        (30330, 39, 5, "30330-01.html"),
    ],
    created: &[(31, "30137-01.html"), (38, "30137-01a.html")],
    started_html: None,
    finish_events: &[
        "30329-02.html",
        "30416-02.html",
        "30421-02.html",
        "30330-02.html",
    ],
    finish_give: &[(49772, 2), (49087, 1)],
};
