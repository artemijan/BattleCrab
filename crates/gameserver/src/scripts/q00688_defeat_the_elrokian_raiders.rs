//! Defeat the Elrokian Raiders! (688) — `quests/Q00688_DefeatTheElrokianRaiders`.
//! Dinn (32105, level 75+) pays for Dinosaur Fang Necklaces dropped by Elroki
//! (22214) on the Primeval Isle: 3000 adena each on turn-in, or a `donation`
//! that trades 100 for a 50/50 450000-or-150000 jackpot. Repeatable.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const ELROKI: i32 = 22214;
const DINN: i32 = 32105;
const DINOSAUR_FANG_NECKLACE: i32 = 8785;
const MIN_LEVEL: i32 = 75;
const DROP_RATE: i32 = 448;

pub struct Q00688DefeatTheElrokianRaiders;

impl QuestScript for Q00688DefeatTheElrokianRaiders {
    fn id(&self) -> i32 {
        688
    }
    fn name(&self) -> &'static str {
        "Q00688_DefeatTheElrokianRaiders"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00688_DefeatTheElrokianRaiders"
    }
    fn start_npcs(&self) -> &[i32] {
        &[DINN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[DINN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[ELROKI]
    }
    fn quest_items(&self) -> &[i32] {
        &[DINOSAUR_FANG_NECKLACE]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL {
                    "32105-01.htm"
                } else {
                    "32105-04.html"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            return Some(
                if ctx.quest_items_count(DINOSAUR_FANG_NECKLACE) > 0 {
                    "32105-05.html"
                } else {
                    "32105-12.html"
                }
                .to_string(),
            );
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "32105-02.htm" | "32105-10.html" => Some(event.to_string()),
            "32105-03.html" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "32105-06.html" => {
                let count = ctx.quest_items_count(DINOSAUR_FANG_NECKLACE);
                if count > 0 {
                    ctx.give_adena(3000 * count, true);
                    ctx.take_items(DINOSAUR_FANG_NECKLACE, -1);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "donation" => {
                if ctx.quest_items_count(DINOSAUR_FANG_NECKLACE) < 100 {
                    Some("32105-07.html".to_string())
                } else {
                    let html = if ctx.roll(1000) < 500 {
                        ctx.give_adena(450000, true);
                        "32105-08.html"
                    } else {
                        ctx.give_adena(150000, true);
                        "32105-09.html"
                    };
                    ctx.take_items(DINOSAUR_FANG_NECKLACE, 100);
                    Some(html.to_string())
                }
            }
            "32105-11.html" => {
                let count = ctx.quest_items_count(DINOSAUR_FANG_NECKLACE);
                if count > 0 {
                    ctx.give_adena(3000 * count, true);
                }
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMember(player, 1)`: a cond-1 member. Port is killer-only
        // (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        // `DROP_RATE * RATE_QUEST_DROP` folded into the roll threshold (a
        // rate-in-threshold drop, like Q00262), then `rewardItems` scales the
        // amount by the reward rate.
        let chance = DROP_RATE as f64 * ctx.rate_quest_drop();
        if (ctx.roll(1000) as f64) < chance {
            ctx.reward_items(DINOSAUR_FANG_NECKLACE, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}
