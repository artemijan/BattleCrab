//! Trial of the Pilgrim (215) — `quests/Q00215_TrialOfThePilgrim`. The healer
//! trial (`HEAL_GROUP`, level 35+). Hermit Santiago starts a long pilgrimage
//! between the priests and elders of every race — Tanapi, Martankus, Gauri,
//! Gerald, Dorf, Primos, Petron, Andellia, Uruha, Casian — slaying a Lava
//! Salamander, Nahir and a Black Willow along the way to assemble the Book of
//! Sage and earn the Mark of the Pilgrim.
//!
//! `memoState`-driven (states 1..17). One money sink of note: Gerald sells the
//! Book of Gerald for 5000 adena, later refunded when the finished badge and
//! book are returned to him.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const PRIEST_PETRON: i32 = 30036;
const PRIEST_PRIMOS: i32 = 30117;
const ANDELLIA: i32 = 30362;
const GAURI_TWINKLEROCK: i32 = 30550;
const SEER_TANAPI: i32 = 30571;
const ELDER_CASIAN: i32 = 30612;
const HERMIT_SANTIAGO: i32 = 30648;
const ANCESTOR_MARTANKUS: i32 = 30649;
const PRIEST_OF_THE_EARTH_GERALD: i32 = 30650;
const WANDERER_DORF: i32 = 30651;
const URUHA: i32 = 30652;
// Items
const ADENA: i32 = 57;
const BOOK_OF_SAGE: i32 = 2722;
const VOUCHER_OF_TRIAL: i32 = 2723;
const SPIRIT_OF_FLAME: i32 = 2724;
const ESSENSE_OF_FLAME: i32 = 2725;
const BOOK_OF_GERALD: i32 = 2726;
const GREY_BADGE: i32 = 2727;
const PICTURE_OF_NAHIR: i32 = 2728;
const HAIR_OF_NAHIR: i32 = 2729;
const STATUE_OF_EINHASAD: i32 = 2730;
const BOOK_OF_DARKNESS: i32 = 2731;
const DEBRIS_OF_WILLOW: i32 = 2732;
const TAG_OF_RUMOR: i32 = 2733;
// Reward
const MARK_OF_PILGRIM: i32 = 2721;
// Quest monsters
const LAVA_SALAMANDER: i32 = 27116;
const NAHIR: i32 = 27117;
const BLACK_WILLOW: i32 = 27118;
// Misc
const MIN_LEVEL: i32 = 35;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

pub struct Q00215TrialOfThePilgrim;

impl QuestScript for Q00215TrialOfThePilgrim {
    fn id(&self) -> i32 {
        215
    }
    fn name(&self) -> &'static str {
        "Q00215_TrialOfThePilgrim"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00215_TrialOfThePilgrim"
    }
    fn start_npcs(&self) -> &[i32] {
        &[HERMIT_SANTIAGO]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            HERMIT_SANTIAGO,
            PRIEST_PETRON,
            PRIEST_PRIMOS,
            ANDELLIA,
            GAURI_TWINKLEROCK,
            SEER_TANAPI,
            ELDER_CASIAN,
            ANCESTOR_MARTANKUS,
            PRIEST_OF_THE_EARTH_GERALD,
            WANDERER_DORF,
            URUHA,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[LAVA_SALAMANDER, NAHIR, BLACK_WILLOW]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            BOOK_OF_SAGE,
            VOUCHER_OF_TRIAL,
            SPIRIT_OF_FLAME,
            ESSENSE_OF_FLAME,
            BOOK_OF_GERALD,
            GREY_BADGE,
            PICTURE_OF_NAHIR,
            HAIR_OF_NAHIR,
            STATUE_OF_EINHASAD,
            BOOK_OF_DARKNESS,
            DEBRIS_OF_WILLOW,
            TAG_OF_RUMOR,
        ]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        let memo = ctx.memo_state();
        match event {
            "ACCEPT" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    ctx.set_memo_state(1);
                    ctx.give_items(VOUCHER_OF_TRIAL, 1);
                    ctx.play_sound(quest_sounds::MIDDLE);
                }
                None
            }
            "30648-05.html" | "30648-06.html" | "30648-07.html" | "30648-08.html" => {
                Some(event.to_string())
            }
            "30362-05.html" => {
                if memo == 15 && has(ctx, BOOK_OF_DARKNESS) {
                    ctx.take_items(BOOK_OF_DARKNESS, 1);
                    ctx.set_memo_state(16);
                    ctx.set_cond(16, true);
                    return Some(event.to_string());
                }
                None
            }
            "30362-04.html" => {
                if memo == 15 && has(ctx, BOOK_OF_DARKNESS) {
                    ctx.set_memo_state(16);
                    ctx.set_cond(16, true);
                    return Some(event.to_string());
                }
                None
            }
            "30649-04.html" => {
                if memo == 4 && has(ctx, ESSENSE_OF_FLAME) {
                    ctx.give_items(SPIRIT_OF_FLAME, 1);
                    ctx.take_items(ESSENSE_OF_FLAME, 1);
                    ctx.set_memo_state(5);
                    ctx.set_cond(5, true);
                    return Some(event.to_string());
                }
                None
            }
            "30650-02.html" => {
                if memo == 6 && has(ctx, TAG_OF_RUMOR) {
                    if ctx.quest_items_count(ADENA) >= 5000 {
                        ctx.give_items(BOOK_OF_GERALD, 1);
                        ctx.take_items(ADENA, 5000);
                        ctx.set_memo_state(7);
                        return Some(event.to_string());
                    }
                    return Some("30650-03.html".to_string());
                }
                None
            }
            "30650-03.html" => {
                if memo == 6 && has(ctx, TAG_OF_RUMOR) {
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "30652-02.html" => {
                if memo == 14 && has(ctx, DEBRIS_OF_WILLOW) {
                    ctx.give_items(BOOK_OF_DARKNESS, 1);
                    ctx.take_items(DEBRIS_OF_WILLOW, 1);
                    ctx.set_memo_state(15);
                    ctx.set_cond(15, true);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let memo = ctx.memo_state();
        match ctx.npc_id {
            LAVA_SALAMANDER => {
                if memo == 3 && !has(ctx, ESSENSE_OF_FLAME) {
                    ctx.set_memo_state(4);
                    ctx.set_cond(4, true);
                    ctx.give_items(ESSENSE_OF_FLAME, 1);
                }
            }
            NAHIR => {
                if memo == 10 && !has(ctx, HAIR_OF_NAHIR) {
                    ctx.set_memo_state(11);
                    ctx.set_cond(11, true);
                    ctx.give_items(HAIR_OF_NAHIR, 1);
                }
            }
            BLACK_WILLOW => {
                if memo == 13 && !has(ctx, DEBRIS_OF_WILLOW) {
                    ctx.set_memo_state(14);
                    ctx.set_cond(14, true);
                    ctx.give_items(DEBRIS_OF_WILLOW, 1);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let memo = ctx.memo_state();
        if ctx.is_created() {
            if ctx.npc_id == HERMIT_SANTIAGO {
                if !ctx.is_in_category("HEAL_GROUP") {
                    return Some("30648-02.html".to_string());
                } else if ctx.player_level() < MIN_LEVEL {
                    return Some("30648-01.html".to_string());
                }
                return Some("30648-03.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == HERMIT_SANTIAGO {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            HERMIT_SANTIAGO => {
                if memo >= 1 {
                    if !has(ctx, BOOK_OF_SAGE) {
                        Some("30648-09.html".to_string())
                    } else {
                        ctx.give_items(MARK_OF_PILGRIM, 1);
                        ctx.add_exp_and_sp(133300, 0);
                        ctx.exit_quest(false, true);
                        ctx.social_action(3);
                        Some("30648-10.html".to_string())
                    }
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            SEER_TANAPI => match memo {
                1 if has(ctx, VOUCHER_OF_TRIAL) => {
                    ctx.take_items(VOUCHER_OF_TRIAL, 1);
                    ctx.set_memo_state(2);
                    ctx.set_cond(2, true);
                    Some("30571-01.html".to_string())
                }
                2 => Some("30571-02.html".to_string()),
                5 if has(ctx, SPIRIT_OF_FLAME) => {
                    ctx.set_cond(6, true);
                    Some("30571-03.html".to_string())
                }
                _ => Some(ctx.no_quest_html()),
            },
            ANCESTOR_MARTANKUS => match memo {
                2 => {
                    ctx.set_memo_state(3);
                    ctx.set_cond(3, true);
                    Some("30649-01.html".to_string())
                }
                3 => Some("30649-02.html".to_string()),
                4 if has(ctx, ESSENSE_OF_FLAME) => Some("30649-03.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            GAURI_TWINKLEROCK => match memo {
                5 if has(ctx, SPIRIT_OF_FLAME) => {
                    ctx.take_items(SPIRIT_OF_FLAME, 1);
                    ctx.give_items(TAG_OF_RUMOR, 1);
                    ctx.set_memo_state(6);
                    ctx.set_cond(7, true);
                    Some("30550-01.html".to_string())
                }
                6 => Some("30550-02.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            PRIEST_OF_THE_EARTH_GERALD => {
                if memo == 6 && has(ctx, TAG_OF_RUMOR) {
                    Some("30650-01.html".to_string())
                } else if has(ctx, GREY_BADGE) && has(ctx, BOOK_OF_GERALD) {
                    ctx.give_adena(5000, true);
                    ctx.take_items(BOOK_OF_GERALD, 1);
                    Some("30650-04.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            WANDERER_DORF => match memo {
                6 if has(ctx, TAG_OF_RUMOR) => {
                    ctx.give_items(GREY_BADGE, 1);
                    ctx.take_items(TAG_OF_RUMOR, 1);
                    ctx.set_memo_state(8);
                    Some("30651-01.html".to_string())
                }
                7 if has(ctx, TAG_OF_RUMOR) => {
                    ctx.give_items(GREY_BADGE, 1);
                    ctx.take_items(TAG_OF_RUMOR, 1);
                    ctx.set_memo_state(8);
                    Some("30651-02.html".to_string())
                }
                8 => {
                    ctx.set_cond(8, true);
                    Some("30651-03.html".to_string())
                }
                _ => Some(ctx.no_quest_html()),
            },
            PRIEST_PRIMOS => match memo {
                8 => {
                    ctx.set_memo_state(9);
                    ctx.set_cond(9, true);
                    Some("30117-01.html".to_string())
                }
                9 => {
                    ctx.set_memo_state(9);
                    ctx.set_cond(9, true);
                    Some("30117-02.html".to_string())
                }
                _ => Some(ctx.no_quest_html()),
            },
            PRIEST_PETRON => match memo {
                9 => {
                    ctx.give_items(PICTURE_OF_NAHIR, 1);
                    ctx.set_memo_state(10);
                    ctx.set_cond(10, true);
                    Some("30036-01.html".to_string())
                }
                10 => Some("30036-02.html".to_string()),
                11 => {
                    ctx.take_items(PICTURE_OF_NAHIR, 1);
                    ctx.take_items(HAIR_OF_NAHIR, 1);
                    ctx.give_items(STATUE_OF_EINHASAD, 1);
                    ctx.set_memo_state(12);
                    ctx.set_cond(12, true);
                    Some("30036-03.html".to_string())
                }
                12 if has(ctx, STATUE_OF_EINHASAD) => Some("30036-04.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            ANDELLIA => match memo {
                12 => {
                    ctx.set_memo_state(13);
                    ctx.set_cond(13, true);
                    Some("30362-01.html".to_string())
                }
                13 => Some("30362-02.html".to_string()),
                14 => Some("30362-02a.html".to_string()),
                15 => {
                    if has(ctx, BOOK_OF_DARKNESS) {
                        Some("30362-03.html".to_string())
                    } else {
                        Some("30362-07.html".to_string())
                    }
                }
                16 => Some("30362-06.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            URUHA => match memo {
                14 if has(ctx, DEBRIS_OF_WILLOW) => Some("30652-01.html".to_string()),
                15 if has(ctx, BOOK_OF_DARKNESS) => Some("30652-03.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            ELDER_CASIAN => match memo {
                16 => {
                    ctx.set_memo_state(17);
                    if !has(ctx, BOOK_OF_SAGE) {
                        ctx.give_items(BOOK_OF_SAGE, 1);
                    }
                    ctx.take_items(GREY_BADGE, 1);
                    ctx.take_items(SPIRIT_OF_FLAME, 1);
                    ctx.take_items(STATUE_OF_EINHASAD, 1);
                    if has(ctx, BOOK_OF_DARKNESS) {
                        ctx.add_exp_and_sp(5000, 500);
                        ctx.take_items(BOOK_OF_DARKNESS, 1);
                    }
                    Some("30612-01.html".to_string())
                }
                17 => {
                    ctx.set_cond(17, true);
                    Some("30612-02.html".to_string())
                }
                _ => Some(ctx.no_quest_html()),
            },
            _ => Some(ctx.no_quest_html()),
        }
    }
}
