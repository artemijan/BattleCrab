//! Testimony of Trust (217) — `quests/Q00217_TestimonyOfTrust`. The Human
//! 2nd-class prerequisite (Human race, `HUMAN_2ND_GROUP`, level 37+). High
//! Priest Hollint sends the aspirant on a diplomatic circuit of every race —
//! Asterios (Elf), Thifiell/Clayton (Dark Elf), Kakai/Manakia (Orc), Lockirin/
//! Nikola (Dwarf) — earning a Scroll of Trust from each, then the Recommendation
//! of Hollin and the Mark of Trust from High Priest Biotin.
//!
//! `memoState`-driven (states 1..19). The Elf leg conjures the wind/verdure
//! spirits Luell and Actea from Lireins and Dryads at a flat 33% (Java reads a
//! "flag" player var it never increments — kept faithful); the Dark Elf leg is
//! a three-reagent gather, and the Orc/Dwarf legs relay letters and hunt.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const HIGH_PRIEST_BIOTIN: i32 = 30031;
const HIERARCH_ASTERIOS: i32 = 30154;
const HIGH_PRIEST_HOLLINT: i32 = 30191;
const TETRARCH_THIFIELL: i32 = 30358;
const MAGISTER_CLAYTON: i32 = 30464;
const SEER_MANAKIA: i32 = 30515;
const IRON_GATES_LOCKIRIN: i32 = 30531;
const FLAME_LORD_KAKAI: i32 = 30565;
const MAESTRO_NIKOLA: i32 = 30621;
const CARDINAL_SERESIN: i32 = 30657;
// Items
const LETTER_TO_ELF: i32 = 2735;
const LETTER_TO_DARKELF: i32 = 2736;
const LETTER_TO_DWARF: i32 = 2737;
const LETTER_TO_ORC: i32 = 2738;
const LETTER_TO_SERESIN: i32 = 2739;
const SCROLL_OF_DARKELF_TRUST: i32 = 2740;
const SCROLL_OF_ELF_TRUST: i32 = 2741;
const SCROLL_OF_DWARF_TRUST: i32 = 2742;
const SCROLL_OF_ORC_TRUST: i32 = 2743;
const RECOMMENDATION_OF_HOLLIN: i32 = 2744;
const ORDER_OF_ASTERIOS: i32 = 2745;
const BREATH_OF_WINDS: i32 = 2746;
const SEED_OF_VERDURE: i32 = 2747;
const LETTER_OF_THIFIELL: i32 = 2748;
const BLOOD_OF_GUARDIAN_BASILISK: i32 = 2749;
const GIANT_APHID: i32 = 2750;
const STAKATOS_FLUIDS: i32 = 2751;
const BASILISK_PLASMA: i32 = 2752;
const HONEY_DEW: i32 = 2753;
const STAKATO_ICHOR: i32 = 2754;
const ORDER_OF_CLAYTON: i32 = 2755;
const PARASITE_OF_LOTA: i32 = 2756;
const LETTER_TO_MANAKIA: i32 = 2757;
const LETTER_OF_MANAKIA: i32 = 2758;
const LETTER_TO_NICHOLA: i32 = 2759;
const ORDER_OF_NICHOLA: i32 = 2760;
const HEART_OF_PORTA: i32 = 2761;
// Reward
const MARK_OF_TRUST: i32 = 2734;
// Monsters
const DRYAD: i32 = 20013;
const DRYAD_ELDER: i32 = 20019;
const LIREIN: i32 = 20036;
const LIREIN_ELDER: i32 = 20044;
const ANT_RECRUIT: i32 = 20082;
const ANT_PATROL: i32 = 20084;
const ANT_GUARD: i32 = 20086;
const ANT_SOLDIER: i32 = 20087;
const ANT_WARRIOR_CAPTAIN: i32 = 20088;
const MARSH_STAKATO: i32 = 20157;
const PORTA: i32 = 20213;
const MARSH_STAKATO_WORKER: i32 = 20230;
const MARSH_STAKATO_SOLDIER: i32 = 20232;
const MARSH_STAKATO_DRONE: i32 = 20234;
const GUARDIAN_BASILISK: i32 = 20550;
const WINDSUS: i32 = 20553;
const LUELL_OF_ZEPHYR_WINDS: i32 = 27120;
const ACTEA_OF_VERDANT_WILDS: i32 = 27121;
// Misc
const MIN_LEVEL: i32 = 37;
const RACE_HUMAN: i32 = 0;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// The Dark Elf reagent legs: collect an intermediate up to `cap`, and on the
/// fifth convert it to `final_item`, advancing to cond 7 once all three of
/// plasma/honeydew/ichor are held. `others` are the other two final items.
fn reagent_kill(
    ctx: &mut QuestCtx,
    cap: i64,
    intermediate: i32,
    final_item: i32,
    others: [i32; 2],
) {
    if ctx.memo_state() == 6
        && ctx.quest_items_count(intermediate) < cap
        && has(ctx, ORDER_OF_CLAYTON)
        && !has(ctx, final_item)
    {
        if ctx.quest_items_count(intermediate) >= 4 {
            ctx.give_items(final_item, 1);
            ctx.take_items(intermediate, -1);
            ctx.play_sound(quest_sounds::MIDDLE);
            if has(ctx, others[0]) && has(ctx, others[1]) {
                ctx.set_cond(7, false);
            }
        } else {
            ctx.give_items(intermediate, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}

/// The elemental guardian legs: each dryad/lirein kill has a `flag * 33`%
/// chance to summon the matching guardian, where `flag` rises with the number
/// of failed attempts.
fn guardian_kill(ctx: &mut QuestCtx, guardian: i32) {
    if ctx.memo_state() != 2 {
        return;
    }
    let flag = ctx
        .get_var("flag")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    if ctx.roll(100) < flag * 33 {
        ctx.spawn_attacker(guardian, true);
        ctx.play_sound(quest_sounds::BEFORE_BATTLE);
    }
}

pub struct Q00217TestimonyOfTrust;

impl QuestScript for Q00217TestimonyOfTrust {
    fn id(&self) -> i32 {
        217
    }
    fn name(&self) -> &'static str {
        "Q00217_TestimonyOfTrust"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00217_TestimonyOfTrust"
    }
    fn start_npcs(&self) -> &[i32] {
        &[HIGH_PRIEST_HOLLINT]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            HIGH_PRIEST_HOLLINT,
            HIGH_PRIEST_BIOTIN,
            HIERARCH_ASTERIOS,
            TETRARCH_THIFIELL,
            MAGISTER_CLAYTON,
            SEER_MANAKIA,
            IRON_GATES_LOCKIRIN,
            FLAME_LORD_KAKAI,
            MAESTRO_NIKOLA,
            CARDINAL_SERESIN,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            DRYAD,
            DRYAD_ELDER,
            LIREIN,
            LIREIN_ELDER,
            ANT_RECRUIT,
            ANT_PATROL,
            ANT_GUARD,
            ANT_SOLDIER,
            ANT_WARRIOR_CAPTAIN,
            MARSH_STAKATO,
            PORTA,
            MARSH_STAKATO_WORKER,
            MARSH_STAKATO_SOLDIER,
            MARSH_STAKATO_DRONE,
            GUARDIAN_BASILISK,
            WINDSUS,
            LUELL_OF_ZEPHYR_WINDS,
            ACTEA_OF_VERDANT_WILDS,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            LETTER_TO_ELF,
            LETTER_TO_DARKELF,
            LETTER_TO_DWARF,
            LETTER_TO_ORC,
            LETTER_TO_SERESIN,
            SCROLL_OF_DARKELF_TRUST,
            SCROLL_OF_ELF_TRUST,
            SCROLL_OF_DWARF_TRUST,
            SCROLL_OF_ORC_TRUST,
            RECOMMENDATION_OF_HOLLIN,
            ORDER_OF_ASTERIOS,
            BREATH_OF_WINDS,
            SEED_OF_VERDURE,
            LETTER_OF_THIFIELL,
            BLOOD_OF_GUARDIAN_BASILISK,
            GIANT_APHID,
            STAKATOS_FLUIDS,
            BASILISK_PLASMA,
            HONEY_DEW,
            STAKATO_ICHOR,
            ORDER_OF_CLAYTON,
            PARASITE_OF_LOTA,
            LETTER_TO_MANAKIA,
            LETTER_OF_MANAKIA,
            LETTER_TO_NICHOLA,
            ORDER_OF_NICHOLA,
            HEART_OF_PORTA,
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
                    ctx.give_items(LETTER_TO_ELF, 1);
                    ctx.give_items(LETTER_TO_DARKELF, 1);
                    ctx.play_sound(quest_sounds::MIDDLE);
                }
                None
            }
            "30154-02.html" | "30657-02.html" => Some(event.to_string()),
            "30154-03.html" => {
                if has(ctx, LETTER_TO_ELF) {
                    ctx.take_items(LETTER_TO_ELF, 1);
                    ctx.give_items(ORDER_OF_ASTERIOS, 1);
                    ctx.set_memo_state(2);
                    ctx.set_cond(2, true);
                    return Some(event.to_string());
                }
                None
            }
            "30358-02.html" => {
                if has(ctx, LETTER_TO_DARKELF) {
                    ctx.take_items(LETTER_TO_DARKELF, 1);
                    ctx.give_items(LETTER_OF_THIFIELL, 1);
                    ctx.set_memo_state(5);
                    ctx.set_cond(5, true);
                    return Some(event.to_string());
                }
                None
            }
            "30515-02.html" => {
                if has(ctx, LETTER_TO_MANAKIA) {
                    ctx.take_items(LETTER_TO_MANAKIA, 1);
                    ctx.set_memo_state(11);
                    ctx.set_cond(14, true);
                    return Some(event.to_string());
                }
                None
            }
            "30531-02.html" => {
                if has(ctx, LETTER_TO_DWARF) {
                    ctx.take_items(LETTER_TO_DWARF, 1);
                    ctx.give_items(LETTER_TO_NICHOLA, 1);
                    ctx.set_memo_state(15);
                    ctx.set_cond(18, true);
                    return Some(event.to_string());
                }
                None
            }
            "30565-02.html" => {
                if has(ctx, LETTER_TO_ORC) {
                    ctx.take_items(LETTER_TO_ORC, 1);
                    ctx.give_items(LETTER_TO_MANAKIA, 1);
                    ctx.set_memo_state(10);
                    ctx.set_cond(13, true);
                    return Some(event.to_string());
                }
                None
            }
            "30621-02.html" => {
                if has(ctx, LETTER_TO_NICHOLA) {
                    ctx.take_items(LETTER_TO_NICHOLA, 1);
                    ctx.give_items(ORDER_OF_NICHOLA, 1);
                    ctx.set_memo_state(16);
                    ctx.set_cond(19, true);
                    return Some(event.to_string());
                }
                None
            }
            "30657-03.html" => {
                if memo == 8 && has(ctx, LETTER_TO_SERESIN) {
                    ctx.give_items(LETTER_TO_DWARF, 1);
                    ctx.give_items(LETTER_TO_ORC, 1);
                    ctx.take_items(LETTER_TO_SERESIN, 1);
                    ctx.set_memo_state(9);
                    ctx.set_cond(12, true);
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
            DRYAD | DRYAD_ELDER => guardian_kill(ctx, ACTEA_OF_VERDANT_WILDS),
            LIREIN | LIREIN_ELDER => guardian_kill(ctx, LUELL_OF_ZEPHYR_WINDS),
            ACTEA_OF_VERDANT_WILDS => {
                if memo == 2 && !has(ctx, SEED_OF_VERDURE) {
                    ctx.give_items(SEED_OF_VERDURE, 1);
                    if has(ctx, BREATH_OF_WINDS) {
                        ctx.set_memo_state(3);
                        ctx.play_sound(quest_sounds::MIDDLE);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            LUELL_OF_ZEPHYR_WINDS => {
                if memo == 2 && !has(ctx, BREATH_OF_WINDS) {
                    ctx.give_items(BREATH_OF_WINDS, 1);
                    if has(ctx, SEED_OF_VERDURE) {
                        ctx.set_memo_state(3);
                        ctx.set_cond(3, true);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            ANT_RECRUIT | ANT_GUARD => {
                reagent_kill(
                    ctx,
                    5,
                    GIANT_APHID,
                    HONEY_DEW,
                    [BASILISK_PLASMA, STAKATO_ICHOR],
                );
            }
            ANT_PATROL | ANT_SOLDIER | ANT_WARRIOR_CAPTAIN => {
                reagent_kill(
                    ctx,
                    10,
                    GIANT_APHID,
                    HONEY_DEW,
                    [BASILISK_PLASMA, STAKATO_ICHOR],
                );
            }
            MARSH_STAKATO | MARSH_STAKATO_WORKER => {
                reagent_kill(
                    ctx,
                    10,
                    STAKATOS_FLUIDS,
                    STAKATO_ICHOR,
                    [BASILISK_PLASMA, HONEY_DEW],
                );
            }
            MARSH_STAKATO_SOLDIER | MARSH_STAKATO_DRONE => {
                reagent_kill(
                    ctx,
                    5,
                    STAKATOS_FLUIDS,
                    STAKATO_ICHOR,
                    [BASILISK_PLASMA, HONEY_DEW],
                );
            }
            GUARDIAN_BASILISK => {
                reagent_kill(
                    ctx,
                    10,
                    BLOOD_OF_GUARDIAN_BASILISK,
                    BASILISK_PLASMA,
                    [STAKATO_ICHOR, HONEY_DEW],
                );
            }
            WINDSUS => {
                if memo == 11 && ctx.quest_items_count(PARASITE_OF_LOTA) < 10 {
                    ctx.give_items(PARASITE_OF_LOTA, 2);
                    if ctx.quest_items_count(PARASITE_OF_LOTA) == 10 {
                        ctx.set_memo_state(12);
                        ctx.set_cond(15, true);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            PORTA => {
                if memo == 16 && !has(ctx, HEART_OF_PORTA) {
                    ctx.give_items(HEART_OF_PORTA, 1);
                    ctx.set_cond(20, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let memo = ctx.memo_state();
        if ctx.is_created() {
            if ctx.npc_id == HIGH_PRIEST_HOLLINT {
                if ctx.player_race() != RACE_HUMAN {
                    return Some("30191-02.html".to_string());
                } else if ctx.player_level() < MIN_LEVEL {
                    return Some("30191-01.html".to_string());
                } else if ctx.is_in_category("HUMAN_2ND_GROUP") {
                    return Some("30191-03.htm".to_string());
                } else if ctx.is_in_category("FIRST_CLASS_GROUP") {
                    return Some("30191-01a.html".to_string());
                }
                return Some("30191-01b.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == HIGH_PRIEST_HOLLINT {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            HIGH_PRIEST_HOLLINT => Some(hollint_talk(ctx, memo)),
            HIGH_PRIEST_BIOTIN => {
                if memo == 19 && has(ctx, RECOMMENDATION_OF_HOLLIN) {
                    ctx.give_adena(252212, true);
                    ctx.give_items(MARK_OF_TRUST, 1);
                    ctx.add_exp_and_sp(1390298, 92782);
                    ctx.exit_quest(false, true);
                    ctx.social_action(3);
                    Some("30031-01.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            HIERARCH_ASTERIOS => Some(asterios_talk(ctx, memo)),
            TETRARCH_THIFIELL => Some(thifiell_talk(ctx, memo)),
            MAGISTER_CLAYTON => Some(clayton_talk(ctx, memo)),
            SEER_MANAKIA => Some(manakia_talk(ctx, memo)),
            IRON_GATES_LOCKIRIN => Some(lockirin_talk(ctx, memo)),
            FLAME_LORD_KAKAI => Some(kakai_talk(ctx, memo)),
            MAESTRO_NIKOLA => Some(nikola_talk(ctx, memo)),
            CARDINAL_SERESIN => Some(seresin_talk(ctx, memo)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

fn hollint_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        7 if has(ctx, SCROLL_OF_ELF_TRUST) && has(ctx, SCROLL_OF_DARKELF_TRUST) => {
            ctx.give_items(LETTER_TO_SERESIN, 1);
            ctx.take_items(SCROLL_OF_DARKELF_TRUST, 1);
            ctx.take_items(SCROLL_OF_ELF_TRUST, 1);
            ctx.set_memo_state(8);
            ctx.set_cond(10, true);
            "30191-05.html".to_string()
        }
        18 if has(ctx, SCROLL_OF_DWARF_TRUST) && has(ctx, SCROLL_OF_ORC_TRUST) => {
            ctx.take_items(SCROLL_OF_DWARF_TRUST, 1);
            ctx.take_items(SCROLL_OF_ORC_TRUST, 1);
            ctx.give_items(RECOMMENDATION_OF_HOLLIN, 1);
            ctx.set_memo_state(19);
            ctx.set_cond(23, true);
            "30191-06.html".to_string()
        }
        19 => "30191-07.html".to_string(),
        1 => "30191-08.html".to_string(),
        8 => "30191-09.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}

fn asterios_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        1 if has(ctx, LETTER_TO_ELF) => "30154-01.html".to_string(),
        2 if has(ctx, ORDER_OF_ASTERIOS) => "30154-04.html".to_string(),
        3 if has(ctx, BREATH_OF_WINDS) && has(ctx, SEED_OF_VERDURE) => {
            ctx.give_items(SCROLL_OF_ELF_TRUST, 1);
            ctx.take_items(ORDER_OF_ASTERIOS, 1);
            ctx.take_items(BREATH_OF_WINDS, 1);
            ctx.take_items(SEED_OF_VERDURE, 1);
            ctx.set_memo_state(4);
            ctx.set_cond(4, true);
            "30154-05.html".to_string()
        }
        4 => "30154-06.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}

fn thifiell_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        4 if has(ctx, LETTER_TO_DARKELF) => "30358-01.html".to_string(),
        6 if has(ctx, ORDER_OF_CLAYTON)
            && (ctx.quest_items_count(STAKATO_ICHOR)
                + ctx.quest_items_count(HONEY_DEW)
                + ctx.quest_items_count(BASILISK_PLASMA))
                == 3 =>
        {
            ctx.give_items(SCROLL_OF_DARKELF_TRUST, 1);
            ctx.take_items(BASILISK_PLASMA, -1);
            ctx.take_items(HONEY_DEW, -1);
            ctx.take_items(STAKATO_ICHOR, -1);
            ctx.take_items(ORDER_OF_CLAYTON, 1);
            ctx.set_memo_state(7);
            ctx.set_cond(9, true);
            "30358-03.html".to_string()
        }
        7 => "30358-04.html".to_string(),
        5 => "30358-05.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}

fn clayton_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        5 if has(ctx, LETTER_OF_THIFIELL) => {
            ctx.take_items(LETTER_OF_THIFIELL, 1);
            ctx.give_items(ORDER_OF_CLAYTON, 1);
            ctx.set_memo_state(6);
            ctx.set_cond(6, true);
            "30464-01.html".to_string()
        }
        6 => {
            if has(ctx, ORDER_OF_CLAYTON)
                && (ctx.quest_items_count(STAKATO_ICHOR)
                    + ctx.quest_items_count(HONEY_DEW)
                    + ctx.quest_items_count(BASILISK_PLASMA))
                    < 3
            {
                "30464-02.html".to_string()
            } else {
                ctx.set_cond(8, true);
                "30464-03.html".to_string()
            }
        }
        _ => ctx.no_quest_html(),
    }
}

fn manakia_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    if has(ctx, LETTER_TO_MANAKIA) {
        "30515-01.html".to_string()
    } else if memo == 11 {
        "30515-03.html".to_string()
    } else if memo == 12 {
        if ctx.quest_items_count(PARASITE_OF_LOTA) == 10 {
            ctx.take_items(PARASITE_OF_LOTA, -1);
            ctx.give_items(LETTER_OF_MANAKIA, 1);
            ctx.set_memo_state(13);
            ctx.set_cond(16, true);
            "30515-04.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else if memo == 13 {
        "30515-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn lockirin_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        14 if has(ctx, LETTER_TO_DWARF) => "30531-01.html".to_string(),
        15 => "30531-03.html".to_string(),
        17 => {
            ctx.give_items(SCROLL_OF_DWARF_TRUST, 1);
            ctx.set_memo_state(18);
            ctx.set_cond(22, true);
            "30531-04.html".to_string()
        }
        18 => "30531-05.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}

fn kakai_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        9 if has(ctx, LETTER_TO_ORC) => "30565-01.html".to_string(),
        10 => "30565-03.html".to_string(),
        13 => {
            ctx.give_items(SCROLL_OF_ORC_TRUST, 1);
            ctx.take_items(LETTER_OF_MANAKIA, 1);
            ctx.set_memo_state(14);
            ctx.set_cond(17, true);
            "30565-04.html".to_string()
        }
        14 => "30565-05.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}

fn nikola_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        15 if has(ctx, LETTER_TO_NICHOLA) => "30621-01.html".to_string(),
        16 => {
            if !has(ctx, HEART_OF_PORTA) {
                "30621-03.html".to_string()
            } else {
                ctx.take_items(ORDER_OF_NICHOLA, 1);
                ctx.take_items(HEART_OF_PORTA, 1);
                ctx.set_memo_state(17);
                ctx.set_cond(21, true);
                "30621-04.html".to_string()
            }
        }
        17 => "30621-05.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}

fn seresin_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        8 if has(ctx, LETTER_TO_SERESIN) => "30657-01.html".to_string(),
        9 => "30657-04.html".to_string(),
        18 => "30657-05.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}
