//! Future Dwarves (10993) — `quests/Q10993_FutureDwarves`.
//!
//! The Dwarf line's capstone: pick a class path at the
//! starter, then collect the reward from that path's trainer. No monsters.
//! Shape and Java citations in [`Capstone`].

use super::newbie_chain::Capstone;

pub const QUEST: Capstone = Capstone {
    id: 10993,
    name: "Q10993_FutureDwarves",
    html_dir: "quests/Q10993_FutureDwarves",
    start_npcs: &[30650],
    talk_npcs: &[30524, 30650, 30527],
    min_level: 19,
    race: super::newbie_chain::DWARF,
    requires: ("Q10999_LoserPriest3", "30650-04.html"),
    plain_events: &["30650-02.htm", "f_scavenger.html", "f_artisan.html"],
    accepts: &[("a_scavenger.html", 2), ("a_artisan.html", 3)],
    trainers: &[
        (30524, 54, 2, "30524-01.html"),
        (30527, 56, 3, "30527-01.html"),
    ],
    created: &[(53, "30650-01.html")],
    started_html: None,
    finish_events: &["30524-02.html", "30527-02.html"],
    finish_give: &[(49772, 2), (49087, 1)],
};
