//! Rise and Fall of the Elroki Tribe (643) — `quests/Q00643_RiseAndFallOfTheElrokiTribe`.
//! Singsing (32106, level 75+) buys Bones of a Plains Dinosaur off the Primeval
//! Isle dinosaurs (per-mob rate-in-threshold drops, 1–2 each): sell them at 1374
//! adena apiece, or exchange 300 at Karakawei (32117) for 5 of a random B-grade
//! weapon piece. Repeatable.
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const SINGSING: i32 = 32106;
const KARAKAWEI: i32 = 32117;
const BONES: i32 = 8776;
const MIN_LEVEL: i32 = 75;
const CHANCE_MOBS1: i32 = 116;
const CHANCE_MOBS2: i32 = 360;
const CHANCE_DEINO: i32 = 558;
const DEINONYCHUS: i32 = 22203;
/// The weapon pieces the exchange awards (5 of a random one).
const PIECE: [i32; 11] = [
    8712, 8713, 8714, 8715, 8716, 8717, 8718, 8719, 8720, 8721, 8722,
];
const MOBS2: [i32; 4] = [22742, 22743, 22744, 22745];

fn mobs1() -> &'static [i32] {
    &[
        22200, 22201, 22202, 22204, 22205, 22208, 22209, 22210, 22211, 22212, 22213, 22219, 22220,
        22221, 22222, 22224, 22225, 22226, 22227,
    ]
}

fn kill_ids() -> &'static [i32] {
    static IDS: OnceLock<Vec<i32>> = OnceLock::new();
    IDS.get_or_init(|| {
        let mut v = mobs1().to_vec();
        v.extend(MOBS2);
        v.push(DEINONYCHUS);
        v
    })
}

pub struct Q00643RiseAndFallOfTheElrokiTribe {
    /// Java stores `isFirstTalk` as a **mutable field on the quest singleton**,
    /// so the very first talk to Karakawei on the whole server sees `32117-01`
    /// and everyone after sees `32117-03` — a per-server (not per-player) flag.
    /// Kept faithful with an atomic; `build_registry` makes one instance.
    first_talk: AtomicBool,
}

impl Q00643RiseAndFallOfTheElrokiTribe {
    pub fn new() -> Self {
        Self {
            first_talk: AtomicBool::new(true),
        }
    }
}

impl Default for Q00643RiseAndFallOfTheElrokiTribe {
    fn default() -> Self {
        Self::new()
    }
}

impl QuestScript for Q00643RiseAndFallOfTheElrokiTribe {
    fn id(&self) -> i32 {
        643
    }
    fn name(&self) -> &'static str {
        "Q00643_RiseAndFallOfTheElrokiTribe"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00643_RiseAndFallOfTheElrokiTribe"
    }
    fn start_npcs(&self) -> &[i32] {
        &[SINGSING]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[SINGSING, KARAKAWEI]
    }
    fn kill_npcs(&self) -> &[i32] {
        kill_ids()
    }
    fn quest_items(&self) -> &[i32] {
        &[BONES]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "32106-01.htm"
                } else {
                    "32106-06.html"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            if ctx.npc_id == SINGSING {
                return Some(
                    if ctx.quest_items_count(BONES) > 0 {
                        "32106-08.html"
                    } else {
                        "32106-14.html"
                    }
                    .to_string(),
                );
            }
            if ctx.npc_id == KARAKAWEI {
                // Global first-talk flag (see the struct docs) — swap returns the
                // prior value.
                return Some(
                    if self.first_talk.swap(false, Ordering::Relaxed) {
                        "32117-01.html"
                    } else {
                        "32117-03.html"
                    }
                    .to_string(),
                );
            }
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "32106-02.htm" | "32106-04.htm" | "32106-05.html" | "32106-10.html"
            | "32106-13.html" | "32117-02.html" | "32117-06.html" | "32117-07.html" => {
                Some(event.to_string())
            }
            "quest_accept" => {
                if ctx.player_level() >= MIN_LEVEL {
                    ctx.start_quest();
                    Some("32106-03.html".to_string())
                } else {
                    Some("32106-07.html".to_string())
                }
            }
            "32106-09.html" => {
                let bones = ctx.quest_items_count(BONES);
                ctx.give_adena(1374 * bones, true);
                ctx.take_items(BONES, -1);
                Some(event.to_string())
            }
            "exit" => {
                let html = if ctx.quest_items_count(BONES) == 0 {
                    "32106-11.html"
                } else {
                    ctx.give_adena(1374 * ctx.quest_items_count(BONES), true);
                    "32106-12.html"
                };
                ctx.exit_quest(true, true);
                Some(html.to_string())
            }
            "exchange" => {
                if ctx.quest_items_count(BONES) < 300 {
                    Some("32117-04.html".to_string())
                } else {
                    let idx = ctx.roll(PIECE.len() as i32) as usize;
                    ctx.reward_items(PIECE[idx], 5);
                    ctx.take_items(BONES, 300);
                    ctx.play_sound(quest_sounds::MIDDLE);
                    Some("32117-05.html".to_string())
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMember(player, 1)` — a cond-1 member. Port is killer-only
        // (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        let rate = ctx.rate_quest_drop();
        let npc = ctx.npc_id;
        if mobs1().contains(&npc) {
            // Always pays; the roll only decides 2 vs 1.
            let count = if (ctx.roll(1000) as f64) < (CHANCE_MOBS1 as f64 * rate) {
                2
            } else {
                1
            };
            ctx.reward_items(BONES, count);
            ctx.play_sound(quest_sounds::ITEMGET);
        } else if MOBS2.contains(&npc) {
            if (ctx.roll(1000) as f64) < (CHANCE_MOBS2 as f64 * rate) {
                ctx.reward_items(BONES, 1);
                ctx.play_sound(quest_sounds::ITEMGET);
            }
        } else if npc == DEINONYCHUS && (ctx.roll(1000) as f64) < (CHANCE_DEINO as f64 * rate) {
            ctx.reward_items(BONES, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}
