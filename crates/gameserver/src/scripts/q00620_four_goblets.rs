//! Four Goblets (620) — `quests/Q00620_FourGoblets`. The level-74-80 Imperial
//! Tomb (Tomb of the Ancient Kings) quest: kill the tomb's undead for Ancient
//! Relics, Grave Passes and Sealed Boxes, and the four Guardian bosses for the
//! four Goblets of the four kings. Ghost of Wigoth / the Chamberlain open Sealed
//! Boxes for a random treasure or trade 1,000 Relics for a Sealed-recipe; the
//! four goblets returned to the Nameless Spirit yield the Antique Brooch (free
//! tomb re-entry). Repeatable.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const NAMELESS_SPIRIT: i32 = 31453;
const GHOST_OF_WIGOTH_1: i32 = 31452;
const GHOST_OF_WIGOTH_2: i32 = 31454;
const CONQ_SM: i32 = 31921;
const EMPER_SM: i32 = 31922;
const SAGES_SM: i32 = 31923;
const JUDGE_SM: i32 = 31924;
const GHOST_CHAMBERLAIN_1: i32 = 31919;
const GHOST_CHAMBERLAIN_2: i32 = 31920;
// Items
const ADENA: i32 = 57;
const ANTIQUE_BROOCH: i32 = 7262;
const ENTRANCE_PASS: i32 = 7075;
const GRAVE_PASS: i32 = 7261;
const RELIC: i32 = 7254;
const SEALED_BOX: i32 = 7255;
const GOBLETS: [i32; 4] = [7256, 7257, 7258, 7259];
// Bosses
const BOSS_1: i32 = 25339;
const BOSS_2: i32 = 25342;
const BOSS_3: i32 = 25346;
const BOSS_4: i32 = 25349;
// Misc
const MIN_LEVEL: i32 = 74;
const MAX_LEVEL: i32 = 80;
/// Tomb undead kill-ids (Java iterates 18120..=18256).
const TOMB_LO: i32 = 18120;
const TOMB_HI: i32 = 18256;

fn count(ctx: &QuestCtx, item: i32) -> i64 {
    ctx.quest_items_count(item)
}

fn goblet_count(ctx: &QuestCtx) -> i64 {
    GOBLETS.iter().map(|&g| count(ctx, g)).sum()
}

fn has_all_goblets(ctx: &QuestCtx) -> bool {
    GOBLETS.iter().all(|&g| count(ctx, g) >= 1)
}

/// Open a Sealed Box: consume one and roll the treasure table (Java's identical
/// events "11" and "19"). Returns `true` if some reward was granted.
fn open_sealed_box(ctx: &mut QuestCtx) -> bool {
    if !ctx.take_items(SEALED_BOX, 1) {
        return false;
    }
    let give = |ctx: &mut QuestCtx, id: i32, n: i64| ctx.give_items(id, n);
    match ctx.roll(5) {
        0 => {
            give(ctx, ADENA, 10000);
            true
        }
        1 => {
            if ctx.roll(1000) < 848 {
                let i = ctx.roll(1000);
                let (id, n) = if i < 43 {
                    (1884, 42)
                } else if i < 66 {
                    (1895, 36)
                } else if i < 184 {
                    (1876, 4)
                } else if i < 250 {
                    (1881, 6)
                } else if i < 287 {
                    (5549, 8)
                } else if i < 484 {
                    (1874, 1)
                } else if i < 681 {
                    (1889, 1)
                } else if i < 799 {
                    (1877, 1)
                } else if i < 902 {
                    (1894, 1)
                } else {
                    (4043, 1)
                };
                give(ctx, id, n);
                true
            } else if ctx.roll(1000) < 323 {
                let i = ctx.roll(1000);
                let id = if i < 335 {
                    1888
                } else if i < 556 {
                    4040
                } else if i < 725 {
                    1890
                } else if i < 872 {
                    5550
                } else if i < 962 {
                    1893
                } else if i < 986 {
                    4046
                } else {
                    4048
                };
                give(ctx, id, 1);
                true
            } else {
                false
            }
        }
        2 => {
            if ctx.roll(1000) < 847 {
                let i = ctx.roll(1000);
                let (id, n) = if i < 148 {
                    (1878, 8)
                } else if i < 175 {
                    (1882, 24)
                } else if i < 273 {
                    (1879, 4)
                } else if i < 322 {
                    (1880, 6)
                } else if i < 357 {
                    (1885, 6)
                } else if i < 554 {
                    (1875, 1)
                } else if i < 685 {
                    (1883, 1)
                } else if i < 803 {
                    (5220, 1)
                } else if i < 901 {
                    (4039, 1)
                } else {
                    (4044, 1)
                };
                give(ctx, id, n);
                true
            } else if ctx.roll(1000) < 251 {
                let i = ctx.roll(1000);
                let id = if i < 350 {
                    1887
                } else if i < 587 {
                    4042
                } else if i < 798 {
                    1886
                } else if i < 922 {
                    4041
                } else if i < 966 {
                    1892
                } else if i < 996 {
                    1891
                } else {
                    4047
                };
                give(ctx, id, 1);
                true
            } else {
                false
            }
        }
        3 => {
            if ctx.roll(1000) < 31 {
                let i = ctx.roll(1000);
                let id = if i < 223 {
                    730
                } else if i < 893 {
                    948
                } else {
                    960
                };
                give(ctx, id, 1);
                true
            } else if ctx.roll(1000) < 50 {
                let i = ctx.roll(1000);
                let id = if i < 202 {
                    729
                } else if i < 928 {
                    947
                } else {
                    959
                };
                give(ctx, id, 1);
                true
            } else {
                false
            }
        }
        _ => {
            if ctx.roll(1000) < 329 {
                let i = ctx.roll(1000);
                let id = match i {
                    _ if i < 88 => 6698,
                    _ if i < 185 => 6699,
                    _ if i < 238 => 6700,
                    _ if i < 262 => 6701,
                    _ if i < 292 => 6702,
                    _ if i < 356 => 6703,
                    _ if i < 420 => 6704,
                    _ if i < 482 => 6705,
                    _ if i < 554 => 6706,
                    _ if i < 576 => 6707,
                    _ if i < 640 => 6708,
                    _ if i < 704 => 6709,
                    _ if i < 777 => 6710,
                    _ if i < 799 => 6711,
                    _ if i < 863 => 6712,
                    _ if i < 927 => 6713,
                    _ => 6714,
                };
                give(ctx, id, 1);
                true
            } else if ctx.roll(1000) < 54 {
                let i = ctx.roll(1000);
                let id = match i {
                    _ if i < 100 => 6688,
                    _ if i < 198 => 6689,
                    _ if i < 298 => 6690,
                    _ if i < 398 => 6691,
                    _ if i < 499 => 7579,
                    _ if i < 601 => 6693,
                    _ if i < 703 => 6694,
                    _ if i < 801 => 6695,
                    _ if i < 902 => 6696,
                    _ => 6697,
                };
                give(ctx, id, 1);
                true
            } else {
                false
            }
        }
    }
}

pub struct Q00620FourGoblets;

impl QuestScript for Q00620FourGoblets {
    fn id(&self) -> i32 {
        620
    }
    fn name(&self) -> &'static str {
        "Q00620_FourGoblets"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00620_FourGoblets"
    }
    fn start_npcs(&self) -> &[i32] {
        &[NAMELESS_SPIRIT]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            NAMELESS_SPIRIT,
            GHOST_OF_WIGOTH_1,
            GHOST_OF_WIGOTH_2,
            CONQ_SM,
            EMPER_SM,
            SAGES_SM,
            JUDGE_SM,
            GHOST_CHAMBERLAIN_1,
            GHOST_CHAMBERLAIN_2,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        // The whole tomb-undead range (18120..=18256) plus the four bosses.
        static IDS: std::sync::OnceLock<Vec<i32>> = std::sync::OnceLock::new();
        IDS.get_or_init(|| {
            let mut v: Vec<i32> = (TOMB_LO..=TOMB_HI).collect();
            v.extend([BOSS_1, BOSS_2, BOSS_3, BOSS_4]);
            v
        })
    }
    fn quest_items(&self) -> &[i32] {
        &[
            ANTIQUE_BROOCH,
            SEALED_BOX,
            7256,
            7257,
            7258,
            7259,
            GRAVE_PASS,
            ENTRANCE_PASS,
        ]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let html = match ctx.npc_id {
            NAMELESS_SPIRIT => nameless_talk(ctx),
            GHOST_OF_WIGOTH_1 => match ctx.cond() {
                1 if goblet_count(ctx) == 1 => "31452-01.html".to_string(),
                1 if goblet_count(ctx) > 1 => "31452-02.html".to_string(),
                2 => "31452-02.html".to_string(),
                _ => ctx.no_quest_html(),
            },
            GHOST_OF_WIGOTH_2 => wigoth2_talk(ctx),
            CONQ_SM => "31921-E.htm".to_string(),
            EMPER_SM => "31922-E.htm".to_string(),
            SAGES_SM => "31923-E.htm".to_string(),
            JUDGE_SM => "31924-E.htm".to_string(),
            GHOST_CHAMBERLAIN_1 => "31919-1.htm".to_string(),
            _ => ctx.no_quest_html(),
        };
        Some(html)
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "accept" => {
                let lvl = ctx.player_level();
                if (MIN_LEVEL..=MAX_LEVEL).contains(&lvl) {
                    ctx.start_quest();
                    ctx.give_items(ENTRANCE_PASS, 1);
                    Some("31453-13.htm".to_string())
                } else {
                    Some("31453-12.htm".to_string())
                }
            }
            // Open a Sealed Box at Ghost of Wigoth 2 ("11") or Chamberlain ("19").
            "11" | "19" => {
                if count(ctx, SEALED_BOX) < 1 {
                    return None;
                }
                let (ok, none_a, none_b) = if event == "11" {
                    ("31454-13.htm", "31454-14.htm", "31454-15.htm")
                } else {
                    ("31919-3.htm", "31919-4.htm", "31919-5.htm")
                };
                if open_sealed_box(ctx) {
                    Some(ok.to_string())
                } else if ctx.roll(2) == 0 {
                    Some(none_a.to_string())
                } else {
                    Some(none_b.to_string())
                }
            }
            // Turn in all four goblets for the Antique Brooch.
            "12" => {
                if has_all_goblets(ctx) {
                    for g in GOBLETS {
                        ctx.take_items(g, -1);
                    }
                    if count(ctx, ANTIQUE_BROOCH) < 1 {
                        ctx.give_items(ANTIQUE_BROOCH, 1);
                    }
                    ctx.set_cond(2, true);
                    Some("31453-16.htm".to_string())
                } else {
                    Some("31453-14.htm".to_string())
                }
            }
            "13" => {
                ctx.exit_quest(true, true);
                Some("31453-18.htm".to_string())
            }
            "14" => Some(if ctx.cond() == 2 {
                "31453-19.htm".to_string()
            } else {
                "31453-13.htm".to_string()
            }),
            // Chamberlain teleports (free with the brooch, else a Grave Pass).
            "15" => tomb_teleport(ctx, 178298, -84574, -7216, "31919-0.htm"),
            "16" => tomb_teleport(ctx, 186942, -75602, -2834, "31920-0.htm"),
            "17" => {
                // Nameless Spirit's teleport out (always allowed).
                if count(ctx, ANTIQUE_BROOCH) >= 1 {
                    ctx.teleport_to(169590, -90218, -2914);
                    None
                } else {
                    ctx.take_items(GRAVE_PASS, 1);
                    ctx.teleport_to(169590, -90218, -2914);
                    Some("31452-6.htm".to_string())
                }
            }
            "18" => Some(
                match goblet_count(ctx) {
                    0..=2 => "31452-3.htm",
                    3 => "31452-4.htm",
                    _ => "31452-5.htm",
                }
                .to_string(),
            ),
            // Trade 1,000 Relics for a Sealed recipe (the event id is the recipe
            // item id; Java's `getInt(event)` reads item 0 — a datapack bug we
            // fix by honoring the button's intent).
            "6881" | "6883" | "6885" | "6887" | "7580" | "6891" | "6893" | "6895" | "6897"
            | "6899" => {
                if ctx.take_items(RELIC, 1000) {
                    if let Ok(recipe) = event.parse::<i32>() {
                        ctx.give_items(recipe, 1);
                    }
                    Some("31454-17.htm".to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        let npc_id = ctx.npc_id;
        if ctx.cond() > 0 && (TOMB_LO..=TOMB_HI).contains(&npc_id) {
            if ctx.roll(100) < 15 {
                ctx.give_items(SEALED_BOX, 1);
                ctx.play_sound(quest_sounds::ITEMGET);
            }
            if count(ctx, GRAVE_PASS) < 1 {
                ctx.give_items(GRAVE_PASS, 1);
            }
            if count(ctx, RELIC) < 1000 {
                ctx.give_items(RELIC, 1);
            }
        }
        // A boss yields its king's goblet (once).
        let goblet = match npc_id {
            BOSS_1 => Some(GOBLETS[0]),
            BOSS_2 => Some(GOBLETS[1]),
            BOSS_3 => Some(GOBLETS[2]),
            BOSS_4 => Some(GOBLETS[3]),
            _ => None,
        };
        if let Some(g) = goblet
            && count(ctx, g) < 1
        {
            ctx.give_items(g, 1);
        }
    }
}

/// A Chamberlain teleport into the tomb: free with the Antique Brooch, else it
/// costs a Grave Pass; without either, the refusal html.
fn tomb_teleport(ctx: &mut QuestCtx, x: i32, y: i32, z: i32, refuse: &str) -> Option<String> {
    if count(ctx, ANTIQUE_BROOCH) >= 1 {
        ctx.teleport_to(x, y, z);
        None
    } else if count(ctx, GRAVE_PASS) >= 1 {
        ctx.take_items(GRAVE_PASS, 1);
        ctx.teleport_to(x, y, z);
        None
    } else {
        Some(refuse.to_string())
    }
}

fn nameless_talk(ctx: &QuestCtx) -> String {
    if ctx.is_created() {
        return if (MIN_LEVEL..=MAX_LEVEL).contains(&ctx.player_level()) {
            "31453-1.htm".to_string()
        } else {
            "31453-12.htm".to_string()
        };
    }
    match ctx.cond() {
        1 => {
            if has_all_goblets(ctx) {
                "31453-15.htm".to_string()
            } else {
                "31453-14.htm".to_string()
            }
        }
        2 => "31453-17.htm".to_string(),
        _ => ctx.no_quest_html(),
    }
}

fn wigoth2_talk(ctx: &QuestCtx) -> String {
    let relics = count(ctx, RELIC) >= 1000;
    let boxes = count(ctx, SEALED_BOX) >= 1;
    let all = has_all_goblets(ctx);
    let many = goblet_count(ctx) > 1;
    match (relics, boxes) {
        (true, true) => {
            if all {
                "31454-4.htm"
            } else if many {
                "31454-8.htm"
            } else {
                "31454-12.htm"
            }
        }
        (true, false) => {
            if all {
                "31454-3.htm"
            } else if many {
                "31454-7.htm"
            } else {
                "31454-11.htm"
            }
        }
        (false, true) => {
            if all {
                "31454-2.htm"
            } else if many {
                "31454-6.htm"
            } else {
                "31454-10.htm"
            }
        }
        (false, false) => {
            if all {
                "31454-1.htm"
            } else if many {
                "31454-5.htm"
            } else {
                "31454-9.htm"
            }
        }
    }
    .to_string()
}
