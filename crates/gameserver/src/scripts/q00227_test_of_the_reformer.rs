//! Test of the Reformer (227) — `quests/Q00227_TestOfTheReformer`. The
//! Prophet/Shillien-Elder-leaning proof (Cleric or Shillien Oracle, level 39+).
//! Priestess Pupina sends the reformer to pry Talianus… no — to wrest Araurune's
//! Huge Nail from a Nameless Revenant, infiltrate the Ol Mahum through Preacher
//! Sla, Katari, Kakan and Nyakuri, and reassemble a skeleton's bones for Ramus,
//! all to earn the Mark of the Reformer.
//!
//! `memoState`-driven (states 1..18). This is the quest that motivated the
//! `on_attack` skill/summon extension: two of its `onAttack` gates read the
//! **striking skill** (`ctx.attack_skill_id()`). The Nameless Revenant only
//! surrenders its diary pages when first struck with **Disrupt Undead**; the
//! Crimson Werewolf flees ("coward!") unless engaged with a mage attack skill,
//! and is only credited to the player who last hit it with one.

use crate::game_loop::npc::ai;
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const PRIESTESS_PUPINA: i32 = 30118;
const PREACHER_SLA: i32 = 30666;
const RAMUS: i32 = 30667;
const KATARI: i32 = 30668;
const KAKAN: i32 = 30669;
const NYAKURI: i32 = 30670;
const OL_MAHUM_PILGRIM: i32 = 30732;
// Items
const BOOK_OF_REFORM: i32 = 2822;
const LETTER_OF_INTRODUCTION: i32 = 2823;
const SLAS_LETTER: i32 = 2824;
const GREETINGS: i32 = 2825;
const OL_MAHUM_MONEY: i32 = 2826;
const KATARIS_LETTER: i32 = 2827;
const NYAKURIS_LETTER: i32 = 2828;
const UNDEAD_LIST: i32 = 2829;
const RAMUSS_LETTER: i32 = 2830;
const RIPPED_DIARY: i32 = 2831;
const HUGE_NAIL: i32 = 2832;
const LETTER_OF_BETRAYER: i32 = 2833;
const BONE_FRAGMENT4: i32 = 2834;
const BONE_FRAGMENT5: i32 = 2835;
const BONE_FRAGMENT6: i32 = 2836;
const BONE_FRAGMENT7: i32 = 2837;
const BONE_FRAGMENT8: i32 = 2838;
const KAKANS_LETTER: i32 = 3037;
const LETTER_GREETINGS1: i32 = 5567;
const LETTER_GREETINGS2: i32 = 5568;
// Reward
const MARK_OF_REFORMER: i32 = 2821;
// Monsters
const MISERY_SKELETON: i32 = 20022;
const SKELETON_ARCHER: i32 = 20100;
const SKELETON_MARKSMAN: i32 = 20102;
const SKELETON_LORD: i32 = 20104;
const SILENT_HORROR: i32 = 20404;
const NAMELESS_REVENANT: i32 = 27099;
const ARURAUNE: i32 = 27128;
const OL_MAHUM_INSPECTOR: i32 = 27129;
const OL_MAHUM_BETRAYER: i32 = 27130;
const CRIMSON_WEREWOLF: i32 = 27131;
const KRUDEL_LIZARDMAN: i32 = 27132;
// Skills
const DISRUPT_UNDEAD: i32 = 1031;
/// The basic mage attack skills that count as "engaging" the Crimson Werewolf.
const MAGE_ATTACK_SKILLS: [i32; 9] = [1031, 1069, 1147, 1164, 1168, 1177, 1184, 1201, 1206];
// Misc
const MIN_LEVEL: i32 = 39;
const CLERIC: i32 = 15;
const SHILLIEN_ORACLE: i32 = 42;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// A skeleton bone-fragment leg; once all five are held, advance to cond 19.
fn bone(ctx: &mut QuestCtx, own: i32, a: i32, b: i32, c: i32, d: i32) {
    if ctx.memo_state() == 16 && !has(ctx, own) {
        ctx.give_items(own, 1);
        ctx.play_sound(quest_sounds::ITEMGET);
        if has(ctx, a) && has(ctx, b) && has(ctx, c) && has(ctx, d) {
            ctx.set_memo_state(17);
            ctx.set_cond(19, false);
        }
    }
}

pub struct Q00227TestOfTheReformer;

impl QuestScript for Q00227TestOfTheReformer {
    fn id(&self) -> i32 {
        227
    }
    fn name(&self) -> &'static str {
        "Q00227_TestOfTheReformer"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00227_TestOfTheReformer"
    }
    fn start_npcs(&self) -> &[i32] {
        &[PRIESTESS_PUPINA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            PRIESTESS_PUPINA,
            PREACHER_SLA,
            RAMUS,
            KATARI,
            KAKAN,
            NYAKURI,
            OL_MAHUM_PILGRIM,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            MISERY_SKELETON,
            SKELETON_ARCHER,
            SKELETON_MARKSMAN,
            SKELETON_LORD,
            SILENT_HORROR,
            NAMELESS_REVENANT,
            ARURAUNE,
            OL_MAHUM_INSPECTOR,
            OL_MAHUM_BETRAYER,
            CRIMSON_WEREWOLF,
            KRUDEL_LIZARDMAN,
        ]
    }
    fn attack_npcs(&self) -> &[i32] {
        &[NAMELESS_REVENANT, CRIMSON_WEREWOLF]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            BOOK_OF_REFORM,
            LETTER_OF_INTRODUCTION,
            SLAS_LETTER,
            GREETINGS,
            OL_MAHUM_MONEY,
            KATARIS_LETTER,
            NYAKURIS_LETTER,
            UNDEAD_LIST,
            RAMUSS_LETTER,
            RIPPED_DIARY,
            HUGE_NAIL,
            LETTER_OF_BETRAYER,
            BONE_FRAGMENT4,
            BONE_FRAGMENT5,
            BONE_FRAGMENT6,
            BONE_FRAGMENT7,
            BONE_FRAGMENT8,
            KAKANS_LETTER,
            LETTER_GREETINGS1,
            LETTER_GREETINGS2,
        ]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let memo = ctx.memo_state();
        if ctx.is_created() {
            if ctx.npc_id == PRIESTESS_PUPINA {
                let class = ctx.player_class_id();
                if class == CLERIC || class == SHILLIEN_ORACLE {
                    return Some(if ctx.player_level() >= MIN_LEVEL {
                        "30118-03.htm".to_string()
                    } else {
                        "30118-01.html".to_string()
                    });
                }
                return Some("30118-02.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == PRIESTESS_PUPINA {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            PRIESTESS_PUPINA => Some(pupina_talk(ctx, memo)),
            PREACHER_SLA => Some(sla_talk(ctx, memo)),
            RAMUS => Some(ramus_talk(ctx, memo)),
            KATARI => Some(katari_talk(ctx, memo)),
            KAKAN => Some(kakan_talk(ctx, memo)),
            NYAKURI => Some(nyakuri_talk(ctx, memo)),
            OL_MAHUM_PILGRIM => {
                if memo == 7 {
                    ctx.give_items(OL_MAHUM_MONEY, 1);
                    ctx.set_memo_state(8);
                    Some("30732-01.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    ctx.set_memo_state(1);
                    ctx.play_sound(quest_sounds::MIDDLE);
                    ctx.give_items(BOOK_OF_REFORM, 1);
                }
                None
            }
            "30118-06.html" => {
                if has(ctx, BOOK_OF_REFORM) {
                    ctx.take_items(BOOK_OF_REFORM, 1);
                    ctx.give_items(LETTER_OF_INTRODUCTION, 1);
                    ctx.take_items(HUGE_NAIL, 1);
                    ctx.set_memo_state(4);
                    ctx.set_cond(4, true);
                    return Some(event.to_string());
                }
                None
            }
            "30666-02.html" | "30666-03.html" | "30669-02.html" | "30669-05.html"
            | "30670-02.html" => Some(event.to_string()),
            "30666-04.html" => {
                ctx.take_items(LETTER_OF_INTRODUCTION, 1);
                ctx.give_items(SLAS_LETTER, 1);
                ctx.set_memo_state(5);
                ctx.set_cond(5, true);
                Some(event.to_string())
            }
            "30669-03.html" => {
                ctx.set_cond(12, true);
                // `if (npc.getSummonedNpcCount() < 1)`: the staged duel — a
                // decoy Ol Mahum Pilgrim at Java's fixed spot, the Crimson
                // Werewolf beside it, and the werewolf pointed at the *decoy*
                // (`addDamageHate` 99999), not the player.
                if ctx.summoned_npc_count() < 1 {
                    let pilgrim = ctx.spawn_bystander_at(OL_MAHUM_PILGRIM, -9282, -89975, -2331);
                    let wolf = ctx.spawn_bystander_at(CRIMSON_WEREWOLF, -9382, -89852, -2333);
                    if let (Some(pilgrim), Some(wolf)) = (pilgrim, wolf) {
                        ai::seed_attack(ctx.world, wolf, pilgrim);
                    }
                }
                Some(event.to_string())
            }
            "30670-03.html" => {
                ctx.set_cond(15, true);
                // The same staging for Krudel Lizardman, with Java's gate the
                // port had dropped here.
                if ctx.summoned_npc_count() < 1 {
                    let pilgrim = ctx.spawn_bystander_at(OL_MAHUM_PILGRIM, 125947, -180049, -1778);
                    let lizard = ctx.spawn_bystander_at(KRUDEL_LIZARDMAN, 126019, -179983, -1781);
                    if let (Some(pilgrim), Some(lizard)) = (pilgrim, lizard) {
                        ai::seed_attack(ctx.world, lizard, pilgrim);
                    }
                }
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            MISERY_SKELETON => bone(
                ctx,
                BONE_FRAGMENT7,
                BONE_FRAGMENT4,
                BONE_FRAGMENT5,
                BONE_FRAGMENT6,
                BONE_FRAGMENT8,
            ),
            SKELETON_ARCHER => bone(
                ctx,
                BONE_FRAGMENT8,
                BONE_FRAGMENT4,
                BONE_FRAGMENT5,
                BONE_FRAGMENT6,
                BONE_FRAGMENT7,
            ),
            SKELETON_MARKSMAN => bone(
                ctx,
                BONE_FRAGMENT6,
                BONE_FRAGMENT4,
                BONE_FRAGMENT5,
                BONE_FRAGMENT7,
                BONE_FRAGMENT8,
            ),
            SKELETON_LORD => bone(
                ctx,
                BONE_FRAGMENT5,
                BONE_FRAGMENT4,
                BONE_FRAGMENT6,
                BONE_FRAGMENT7,
                BONE_FRAGMENT8,
            ),
            SILENT_HORROR => bone(
                ctx,
                BONE_FRAGMENT4,
                BONE_FRAGMENT5,
                BONE_FRAGMENT6,
                BONE_FRAGMENT7,
                BONE_FRAGMENT8,
            ),
            NAMELESS_REVENANT => {
                if ctx.memo_state() == 1
                    && ctx.npc_script_value() == 1
                    && !has(ctx, HUGE_NAIL)
                    && has(ctx, BOOK_OF_REFORM)
                    && ctx.quest_items_count(RIPPED_DIARY) < 7
                {
                    if ctx.quest_items_count(RIPPED_DIARY) == 6 {
                        ctx.spawn_attacker(ARURAUNE, true);
                        ctx.take_items(RIPPED_DIARY, -1);
                        ctx.set_cond(2, false);
                    } else {
                        ctx.give_items(RIPPED_DIARY, 1);
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            ARURAUNE => {
                if !has(ctx, HUGE_NAIL) {
                    ctx.give_items(HUGE_NAIL, 1);
                    ctx.play_sound(quest_sounds::ITEMGET);
                    ctx.set_memo_state(3);
                    ctx.set_cond(3, false);
                }
            }
            OL_MAHUM_INSPECTOR => {
                if ctx.memo_state() == 6 {
                    ctx.set_memo_state(7);
                    ctx.set_cond(7, true);
                }
            }
            OL_MAHUM_BETRAYER => {
                if ctx.memo_state() == 8 {
                    ctx.set_memo_state(9);
                    ctx.set_cond(9, false);
                    ctx.give_items(LETTER_OF_BETRAYER, 1);
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
            CRIMSON_WEREWOLF => {
                if ctx.npc_script_value() == ctx.player && ctx.memo_state() == 11 {
                    ctx.set_memo_state(12);
                    ctx.set_cond(13, true);
                }
            }
            KRUDEL_LIZARDMAN if ctx.memo_state() == 13 => {
                ctx.set_memo_state(14);
                ctx.set_cond(16, true);
            }
            _ => {}
        }
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            NAMELESS_REVENANT => {
                // Disrupt Undead marks it as "properly reformed" (scriptValue 1);
                // any other skill spoils it (scriptValue 2). A melee swing (no
                // skill) leaves it untouched, as in Java.
                if let Some(skill) = ctx.attack_skill_id() {
                    if skill == DISRUPT_UNDEAD {
                        ctx.set_npc_script_value(1);
                    } else {
                        ctx.set_npc_script_value(2);
                    }
                }
            }
            CRIMSON_WEREWOLF => {
                let engaged = ctx
                    .attack_skill_id()
                    .is_some_and(|s| MAGE_ATTACK_SKILLS.contains(&s));
                if engaged {
                    // Credit the mage who engaged it (Java stores the attacker's
                    // object id in `scriptValue`).
                    ctx.set_npc_script_value(ctx.player);
                } else {
                    // "Cowardly guy!" — a melee/non-mage hit makes it flee.
                    ctx.delete_npc();
                }
            }
            _ => {}
        }
    }
}

fn pupina_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    if memo == 3 {
        if has(ctx, HUGE_NAIL) {
            "30118-05.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else if (1..3).contains(&memo) {
        "30118-04a.html".to_string()
    } else if memo >= 4 {
        "30118-07.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn sla_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        4 if has(ctx, LETTER_OF_INTRODUCTION) => "30666-01.html".to_string(),
        m if (11..18).contains(&m) => "30666-06b.html".to_string(),
        5 if has(ctx, SLAS_LETTER) => "30666-05.html".to_string(),
        10 => {
            let with_money = has(ctx, OL_MAHUM_MONEY);
            if with_money {
                ctx.take_items(OL_MAHUM_MONEY, 1);
            }
            ctx.give_items(GREETINGS, 1);
            ctx.give_items(LETTER_GREETINGS1, 1);
            ctx.give_items(LETTER_GREETINGS2, 1);
            ctx.set_memo_state(11);
            ctx.set_cond(11, true);
            if with_money {
                "30666-06.html".to_string()
            } else {
                "30666-06a.html".to_string()
            }
        }
        18 if has(ctx, KATARIS_LETTER)
            && has(ctx, KAKANS_LETTER)
            && has(ctx, NYAKURIS_LETTER)
            && has(ctx, RAMUSS_LETTER) =>
        {
            ctx.give_adena(226528, true);
            ctx.give_items(MARK_OF_REFORMER, 1);
            ctx.add_exp_and_sp(1252844, 85972);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            "30666-07.html".to_string()
        }
        _ => ctx.no_quest_html(),
    }
}

fn ramus_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        15 if has(ctx, LETTER_GREETINGS2) && !has(ctx, UNDEAD_LIST) => {
            ctx.give_items(UNDEAD_LIST, 1);
            ctx.take_items(LETTER_GREETINGS2, 1);
            ctx.set_memo_state(16);
            ctx.set_cond(18, true);
            "30667-01.html".to_string()
        }
        16 => "30667-02.html".to_string(),
        17 if has(ctx, UNDEAD_LIST) => {
            ctx.take_items(UNDEAD_LIST, 1);
            ctx.give_items(RAMUSS_LETTER, 1);
            ctx.take_items(BONE_FRAGMENT4, 1);
            ctx.take_items(BONE_FRAGMENT5, 1);
            ctx.take_items(BONE_FRAGMENT6, 1);
            ctx.take_items(BONE_FRAGMENT7, 1);
            ctx.take_items(BONE_FRAGMENT8, 1);
            ctx.set_memo_state(18);
            ctx.set_cond(20, true);
            "30667-03.html".to_string()
        }
        _ => ctx.no_quest_html(),
    }
}

fn katari_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        5 | 6 => {
            ctx.take_items(SLAS_LETTER, 1);
            ctx.set_memo_state(6);
            ctx.set_cond(6, true);
            ctx.spawn_attacker(OL_MAHUM_INSPECTOR, true);
            "30668-01.html".to_string()
        }
        7 | 8 => {
            if memo == 7 {
                ctx.set_memo_state(8);
            }
            ctx.set_cond(8, true);
            ctx.spawn_attacker(OL_MAHUM_BETRAYER, true);
            "30668-02.html".to_string()
        }
        9 if has(ctx, LETTER_OF_BETRAYER) => {
            ctx.give_items(KATARIS_LETTER, 1);
            ctx.take_items(LETTER_OF_BETRAYER, 1);
            ctx.set_memo_state(10);
            ctx.set_cond(10, true);
            "30668-03.html".to_string()
        }
        m if m >= 10 => "30668-04.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}

fn kakan_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        11 if has(ctx, GREETINGS) => "30669-01.html".to_string(),
        12 if has(ctx, GREETINGS) && !has(ctx, KAKANS_LETTER) => {
            ctx.take_items(GREETINGS, 1);
            ctx.give_items(KAKANS_LETTER, 1);
            ctx.set_memo_state(13);
            ctx.set_cond(14, true);
            "30669-04.html".to_string()
        }
        _ => ctx.no_quest_html(),
    }
}

fn nyakuri_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        13 if has(ctx, LETTER_GREETINGS1) => "30670-01.html".to_string(),
        14 if has(ctx, LETTER_GREETINGS1) && !has(ctx, NYAKURIS_LETTER) => {
            ctx.give_items(NYAKURIS_LETTER, 1);
            ctx.take_items(LETTER_GREETINGS1, 1);
            ctx.set_memo_state(15);
            ctx.set_cond(17, true);
            "30670-04.html".to_string()
        }
        _ => ctx.no_quest_html(),
    }
}
