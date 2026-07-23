//! 1000 Years, the End of Lamentation (344) — `quests/Q00344_1000YearsTheEndOfLamentation`.
//! Gilmore (30754, level 48–52) sends the player into the Cave of Trials for
//! Articles of Sacrifice off the Cave Servants. Turning a batch in is a gamble:
//! usually it just sells for 60 adena each, but the more you hand over the
//! likelier you instead get one of four ancient relics — and each relic goes to
//! a specific Aden scholar (Kaien / Rodemai / Garvarentz / Orven) who trades it
//! for a random B-grade prize. `memoState` tracks which relic is in hand.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const KAIEN: i32 = 30623;
const GARVARENTZ: i32 = 30704;
const GILMORE: i32 = 30754;
const RODEMAI: i32 = 30756;
const ORVEN: i32 = 30857;

const ARTICLES: i32 = 4269;
const OLD_KEY: i32 = 4270;
const OLD_HILT: i32 = 4271;
const TOTEM_NECKLACE: i32 = 4272;
const CRUCIFIX: i32 = 4273;
const MIN_LEVEL: i32 = 48;

/// `MONSTER_CHANCES`: per-mob Article drop chance (0..1 for `giveItemRandomly`).
fn article_chance(npc_id: i32) -> Option<f64> {
    let v = match npc_id {
        20236 => 0.58,
        20238 => 0.75,
        20237 => 0.78,
        20239 => 0.79,
        20240 => 0.85,
        20272 => 0.58,
        20273 => 0.78,
        20274 => 0.75,
        20275 => 0.79,
        20276 => 0.85,
        _ => return None,
    };
    Some(v)
}

fn has_any_relic(ctx: &QuestCtx) -> bool {
    [OLD_KEY, OLD_HILT, TOTEM_NECKLACE, CRUCIFIX]
        .iter()
        .any(|&id| ctx.quest_items_count(id) > 0)
}

pub struct Q003441000YearsTheEndOfLamentation;

impl QuestScript for Q003441000YearsTheEndOfLamentation {
    fn id(&self) -> i32 {
        344
    }
    fn name(&self) -> &'static str {
        "Q00344_1000YearsTheEndOfLamentation"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00344_1000YearsTheEndOfLamentation"
    }
    fn start_npcs(&self) -> &[i32] {
        &[GILMORE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[KAIEN, GARVARENTZ, GILMORE, RODEMAI, ORVEN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            20236, 20238, 20237, 20239, 20240, 20272, 20273, 20274, 20275, 20276,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[ARTICLES, OLD_KEY, OLD_HILT, TOTEM_NECKLACE, CRUCIFIX]
    }

    /// `addCondMaxLevel(52, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 52).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30754-03.htm" | "30754-16.html" => Some(event.to_string()),
            "30754-04.htm" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "30754-08.html" => {
                if !ctx.is_cond(1) {
                    return None;
                }
                let count = ctx.quest_items_count(ARTICLES);
                if count < 1 {
                    return Some("30754-07.html".to_string());
                }
                ctx.take_items(ARTICLES, -1);
                if i64::from(ctx.roll(1000)) >= count {
                    // The common case: articles just sell for adena.
                    ctx.give_adena(count * 60, true);
                    Some(event.to_string())
                } else {
                    // Jackpot: a random relic (more articles = better odds).
                    ctx.set_cond(2, true);
                    let (memo, relic) = match ctx.roll(4) {
                        0 => (1, OLD_HILT),
                        1 => (2, OLD_KEY),
                        2 => (3, TOTEM_NECKLACE),
                        _ => (4, CRUCIFIX),
                    };
                    ctx.set_memo_state(memo);
                    ctx.give_items(relic, 1);
                    Some("30754-09.html".to_string())
                }
            }
            "30754-17.html" => {
                if ctx.is_cond(1) {
                    ctx.exit_quest(true, true);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "relic_info" => Some(
                match ctx.memo_state() {
                    1 => "30754-10.html",
                    2 => "30754-11.html",
                    3 => "30754-12.html",
                    4 => "30754-13.html",
                    _ => return None,
                }
                .to_string(),
            ),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMemberState(killer, 1, 3, npc)` — a cond-1 member. Port is
        // killer-only (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        if let Some(chance) = article_chance(ctx.npc_id) {
            ctx.give_item_randomly(ARTICLES, 1, 0, chance, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        match ctx.npc_id {
            GILMORE => {
                if ctx.is_created() {
                    return Some(
                        if ctx.player_level() >= MIN_LEVEL {
                            "30754-02.htm"
                        } else {
                            "30754-01.htm"
                        }
                        .to_string(),
                    );
                }
                if ctx.is_started() {
                    if ctx.is_cond(1) {
                        return Some(
                            if ctx.quest_items_count(ARTICLES) > 0 {
                                "30754-06.html"
                            } else {
                                "30754-05.html"
                            }
                            .to_string(),
                        );
                    }
                    if has_any_relic(ctx) {
                        return Some("30754-14.html".to_string());
                    }
                    // Relic already handed to a scholar — resume collecting.
                    ctx.set_cond(1, false);
                    return Some("30754-15.html".to_string());
                }
                Some(ctx.already_completed_html())
            }
            KAIEN => Some(self.exchange(
                ctx,
                1,
                OLD_HILT,
                &[(52, 1874, 25), (76, 1887, 10), (98, 951, 1), (100, 133, 1)],
                "30623-01.html",
                "30623-02.html",
            )),
            RODEMAI => Some(self.exchange(
                ctx,
                2,
                OLD_KEY,
                &[(39, 1879, 55), (89, 951, 1), (100, 885, 1)],
                "30756-01.html",
                "30756-02.html",
            )),
            GARVARENTZ => Some(self.exchange(
                ctx,
                3,
                TOTEM_NECKLACE,
                &[(47, 1882, 70), (97, 1881, 50), (100, 191, 1)],
                "30704-01.html",
                "30704-02.html",
            )),
            ORVEN => Some(self.exchange(
                ctx,
                4,
                CRUCIFIX,
                &[(49, 1875, 19), (69, 952, 5), (100, 2437, 1)],
                "30857-01.html",
                "30857-02.html",
            )),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

impl Q003441000YearsTheEndOfLamentation {
    /// A scholar's relic exchange: at the matching `memoState`, trade the held
    /// `relic` for a random reward from `table` (`(upper-bound /100, item, count)`),
    /// reset to cond 1 and show `ok`; otherwise `no_relic`. Returns the no-quest
    /// page when the memo doesn't match.
    fn exchange(
        &self,
        ctx: &mut QuestCtx,
        memo: i32,
        relic: i32,
        table: &[(i32, i32, i64)],
        ok: &str,
        no_relic: &str,
    ) -> String {
        if ctx.memo_state() != memo {
            return ctx.no_quest_html();
        }
        if ctx.quest_items_count(relic) == 0 {
            return no_relic.to_string();
        }
        ctx.take_items(relic, -1);
        let rand = ctx.roll(100);
        for &(bound, item, count) in table {
            if rand <= bound {
                ctx.reward_items(item, count);
                break;
            }
        }
        ctx.set_cond(1, false);
        ok.to_string()
    }
}
