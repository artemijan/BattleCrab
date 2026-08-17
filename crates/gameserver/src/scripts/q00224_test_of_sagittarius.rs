//! Test of Sagittarius (224) — `quests/Q00224_TestOfSagittarius`. The archer
//! 2nd-class proof (Rogue / Elven Scout / Assassin → Hawkeye / Silver Ranger /
//! Phantom Ranger, level 39+). Guild President Bernard sends the hopeful on a
//! long relay through Hamil, Aron, Vokian and Gauen, gathering hunter's runes
//! and the four materials of the Crescent Moon Bow, then proving mastery by
//! felling Serpent Demon Kadesh **with that very bow**.
//!
//! This is a `memoState`-driven machine (states 1..14) rather than a pure
//! item-gate: most `onKill` legs check `isMemoState(n)` before dropping. Two
//! mechanics are worth calling out:
//!   * The four bow-materials (mithril clip, stakato chitin, reinforced
//!     bowstring, manashen's horn) each drop from their own mob but only
//!     advance to state 11 when the *other three* are already held — the set
//!     completes in whatever order the player farms it.
//!   * Kadesh is conjured probabilistically while farming Blood of Lizardman
//!     (chance climbs as the stack grows), and only yields the Talisman of
//!     Kadesh if the killing blow was struck with the Crescent Moon Bow;
//!     any other weapon just respawns him.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const PREFECT_VOKIAN: i32 = 30514;
const SAGITTARIUS_HAMIL: i32 = 30626;
const SIR_ARON_TANFORD: i32 = 30653;
const GUILD_PRESIDENT_BERNARD: i32 = 30702;
const MAGISTER_GAUEN: i32 = 30717;
// Items
const WOODEN_ARROW: i32 = 17;
const CRESCENT_MOON_BOW: i32 = 3028;
const BERNARDS_INTRODUCTION: i32 = 3294;
const HAMILS_1ST_LETTER: i32 = 3295;
const HAMILS_2ND_LETTER: i32 = 3296;
const HAMILS_3RD_LETTER: i32 = 3297;
const HUNTERS_1ST_RUNE: i32 = 3298;
const HUNTERS_2ND_RUNE: i32 = 3299;
const TALISMAN_OF_KADESH: i32 = 3300;
const TALISMAN_OF_SNAKE: i32 = 3301;
const MITHRIL_CLIP: i32 = 3302;
const STAKATO_CHITIN: i32 = 3303;
const REINFORCED_BOWSTRING: i32 = 3304;
const MANASHENS_HORN: i32 = 3305;
const BLOOD_OF_LIZARDMAN: i32 = 3306;
// Reward
const MARK_OF_SAGITTARIUS: i32 = 3293;
// Monsters
const ANT: i32 = 20079;
const ANT_CAPTAIN: i32 = 20080;
const ANT_OVERSEER: i32 = 20081;
const ANT_RECRUIT: i32 = 20082;
const ANT_PATROL: i32 = 20084;
const ANT_GUARD: i32 = 20086;
const NOBLE_ANT: i32 = 20089;
const NOBLE_ANT_LEADER: i32 = 20090;
const MARSH_STAKATO_WORKER: i32 = 20230;
const MARSH_STAKATO_SOLDIER: i32 = 20232;
const MARSH_SPIDER: i32 = 20233;
const MARSH_STAKATO_DRONE: i32 = 20234;
const BREKA_ORC_SHAMAN: i32 = 20269;
const BREKA_ORC_OVERLORD: i32 = 20270;
const ROAD_SCAVENGER: i32 = 20551;
const MANASHEN_GARGOYLE: i32 = 20563;
const LETO_LIZARDMAN: i32 = 20577;
const LETO_LIZARDMAN_ARCHER: i32 = 20578;
const LETO_LIZARDMAN_SOLDIER: i32 = 20579;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const SERPENT_DEMON_KADESH: i32 = 27090;
// Misc
const MIN_LEVEL: i32 = 39;
const ROGUE: i32 = 7;
const ELVEN_SCOUT: i32 = 22;
const ASSASSIN: i32 = 35;

const KILL_NPCS: &[i32] = &[
    ANT,
    ANT_CAPTAIN,
    ANT_OVERSEER,
    ANT_RECRUIT,
    ANT_PATROL,
    ANT_GUARD,
    NOBLE_ANT,
    NOBLE_ANT_LEADER,
    MARSH_STAKATO_WORKER,
    MARSH_STAKATO_SOLDIER,
    MARSH_SPIDER,
    MARSH_STAKATO_DRONE,
    BREKA_ORC_SHAMAN,
    BREKA_ORC_OVERLORD,
    ROAD_SCAVENGER,
    MANASHEN_GARGOYLE,
    LETO_LIZARDMAN,
    LETO_LIZARDMAN_ARCHER,
    LETO_LIZARDMAN_SOLDIER,
    LETO_LIZARDMAN_WARRIOR,
    LETO_LIZARDMAN_SHAMAN,
    LETO_LIZARDMAN_OVERLORD,
    SERPENT_DEMON_KADESH,
];

pub struct Q00224TestOfSagittarius;

impl QuestScript for Q00224TestOfSagittarius {
    fn id(&self) -> i32 {
        224
    }
    fn name(&self) -> &'static str {
        "Q00224_TestOfSagittarius"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00224_TestOfSagittarius"
    }
    fn start_npcs(&self) -> &[i32] {
        &[GUILD_PRESIDENT_BERNARD]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            GUILD_PRESIDENT_BERNARD,
            PREFECT_VOKIAN,
            SAGITTARIUS_HAMIL,
            SIR_ARON_TANFORD,
            MAGISTER_GAUEN,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[
            CRESCENT_MOON_BOW,
            BERNARDS_INTRODUCTION,
            HAMILS_1ST_LETTER,
            HAMILS_2ND_LETTER,
            HAMILS_3RD_LETTER,
            HUNTERS_1ST_RUNE,
            HUNTERS_2ND_RUNE,
            TALISMAN_OF_KADESH,
            TALISMAN_OF_SNAKE,
            MITHRIL_CLIP,
            STAKATO_CHITIN,
            REINFORCED_BOWSTRING,
            MANASHENS_HORN,
            BLOOD_OF_LIZARDMAN,
        ]
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
                    ctx.give_items(BERNARDS_INTRODUCTION, 1);
                }
                None
            }
            "30626-02.html" | "30626-06.html" => Some(event.to_string()),
            "30514-02.html" => {
                if ctx.quest_items_count(HAMILS_2ND_LETTER) > 0 {
                    ctx.take_items(HAMILS_2ND_LETTER, 1);
                    ctx.set_memo_state(6);
                    ctx.set_cond(6, true);
                    return Some(event.to_string());
                }
                None
            }
            "30626-03.html" => {
                if ctx.quest_items_count(BERNARDS_INTRODUCTION) > 0 {
                    ctx.take_items(BERNARDS_INTRODUCTION, 1);
                    ctx.give_items(HAMILS_1ST_LETTER, 1);
                    ctx.set_memo_state(2);
                    ctx.set_cond(2, true);
                    return Some(event.to_string());
                }
                None
            }
            "30626-07.html" => {
                if ctx.quest_items_count(HUNTERS_1ST_RUNE) >= 10 {
                    ctx.give_items(HAMILS_2ND_LETTER, 1);
                    ctx.take_items(HUNTERS_1ST_RUNE, -1);
                    ctx.set_memo_state(5);
                    ctx.set_cond(5, true);
                    return Some(event.to_string());
                }
                None
            }
            "30653-02.html" => {
                if ctx.quest_items_count(HAMILS_1ST_LETTER) > 0 {
                    ctx.take_items(HAMILS_1ST_LETTER, 1);
                    ctx.set_memo_state(3);
                    ctx.set_cond(3, true);
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
        match ctx.npc_id {
            ANT | ANT_CAPTAIN | ANT_OVERSEER | ANT_RECRUIT | ANT_PATROL | ANT_GUARD | NOBLE_ANT
            | NOBLE_ANT_LEADER => {
                if ctx.memo_state() == 3 && ctx.quest_items_count(HUNTERS_1ST_RUNE) < 10 {
                    if ctx.quest_items_count(HUNTERS_1ST_RUNE) == 9 {
                        ctx.give_items(HUNTERS_1ST_RUNE, 1);
                        ctx.set_memo_state(4);
                        ctx.set_cond(4, true);
                    } else {
                        ctx.give_items(HUNTERS_1ST_RUNE, 1);
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            MARSH_STAKATO_WORKER | MARSH_STAKATO_SOLDIER | MARSH_STAKATO_DRONE => {
                material_kill(
                    ctx,
                    STAKATO_CHITIN,
                    MITHRIL_CLIP,
                    REINFORCED_BOWSTRING,
                    MANASHENS_HORN,
                );
            }
            MARSH_SPIDER => {
                material_kill(
                    ctx,
                    REINFORCED_BOWSTRING,
                    MITHRIL_CLIP,
                    MANASHENS_HORN,
                    STAKATO_CHITIN,
                );
            }
            ROAD_SCAVENGER => {
                material_kill(
                    ctx,
                    MITHRIL_CLIP,
                    REINFORCED_BOWSTRING,
                    MANASHENS_HORN,
                    STAKATO_CHITIN,
                );
            }
            MANASHEN_GARGOYLE => {
                material_kill(
                    ctx,
                    MANASHENS_HORN,
                    MITHRIL_CLIP,
                    REINFORCED_BOWSTRING,
                    STAKATO_CHITIN,
                );
            }
            BREKA_ORC_SHAMAN | BREKA_ORC_OVERLORD => {
                if ctx.memo_state() == 6 && ctx.quest_items_count(HUNTERS_2ND_RUNE) < 10 {
                    if ctx.quest_items_count(HUNTERS_2ND_RUNE) == 9 {
                        ctx.give_items(HUNTERS_2ND_RUNE, 1);
                        ctx.give_items(TALISMAN_OF_SNAKE, 1);
                        ctx.set_memo_state(7);
                        ctx.set_cond(7, true);
                    } else {
                        ctx.give_items(HUNTERS_2ND_RUNE, 1);
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            LETO_LIZARDMAN
            | LETO_LIZARDMAN_ARCHER
            | LETO_LIZARDMAN_SOLDIER
            | LETO_LIZARDMAN_WARRIOR
            | LETO_LIZARDMAN_SHAMAN
            | LETO_LIZARDMAN_OVERLORD => {
                if ctx.memo_state() == 13 && ctx.quest_items_count(BLOOD_OF_LIZARDMAN) < 140 {
                    // Chance to conjure Kadesh climbs as the Blood stack grows.
                    let blood = ctx.quest_items_count(BLOOD_OF_LIZARDMAN) as i32;
                    if ((blood - 10) * 5) > ctx.roll(100) {
                        // `addSpawn(..., 300000)` — Kadesh gives up after five
                        // minutes. Not a progression gate (the quest only cares
                        // that he was killed) but without it an unfought Kadesh
                        // stands in the field forever, and the next roll spawns
                        // another beside him.
                        if let Some(kadesh) = ctx.spawn_near_npc(SERPENT_DEMON_KADESH, true) {
                            ctx.schedule_despawn(kadesh, 300_000);
                        }
                        ctx.take_items(BLOOD_OF_LIZARDMAN, -1);
                        ctx.play_sound(quest_sounds::BEFORE_BATTLE);
                    } else {
                        ctx.give_items(BLOOD_OF_LIZARDMAN, 1);
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            SERPENT_DEMON_KADESH
                if ctx.memo_state() == 13 && ctx.quest_items_count(TALISMAN_OF_KADESH) == 0 =>
            {
                // Java gates on `npc.getKillingBlowWeapon()`; we approximate
                // with the killer's currently-equipped weapon at kill time,
                // which is the weapon that struck the finishing blow.
                if ctx.equipped_weapon_id() == CRESCENT_MOON_BOW {
                    ctx.give_items(TALISMAN_OF_KADESH, 1);
                    ctx.set_memo_state(14);
                    ctx.set_cond(14, true);
                } else {
                    ctx.spawn_near_npc(SERPENT_DEMON_KADESH, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let memo = ctx.memo_state();
        if ctx.is_created() {
            if ctx.npc_id == GUILD_PRESIDENT_BERNARD {
                let class = ctx.player_class_id();
                if class == ROGUE || class == ELVEN_SCOUT || class == ASSASSIN {
                    return Some(if ctx.player_level() >= MIN_LEVEL {
                        "30702-03.htm".to_string()
                    } else {
                        "30702-01.html".to_string()
                    });
                }
                return Some("30702-02.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == GUILD_PRESIDENT_BERNARD {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        // Started.
        match ctx.npc_id {
            GUILD_PRESIDENT_BERNARD => {
                if ctx.quest_items_count(BERNARDS_INTRODUCTION) > 0 {
                    Some("30702-05.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            PREFECT_VOKIAN => Some(vokian_talk(ctx, memo)),
            SAGITTARIUS_HAMIL => Some(hamil_talk(ctx, memo)),
            SIR_ARON_TANFORD => Some(aron_talk(ctx, memo)),
            MAGISTER_GAUEN => Some(gauen_talk(ctx, memo)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

/// One of the four bow-materials: `<mob>` drops `own` (once), and only when the
/// other three materials are already in hand does the set complete → state 11.
fn material_kill(ctx: &mut QuestCtx, own: i32, a: i32, b: i32, c: i32) {
    if ctx.memo_state() == 10 && ctx.quest_items_count(own) == 0 {
        if ctx.quest_items_count(a) > 0
            && ctx.quest_items_count(b) > 0
            && ctx.quest_items_count(c) > 0
        {
            ctx.give_items(own, 1);
            ctx.set_memo_state(11);
            ctx.set_cond(11, true);
        } else {
            ctx.give_items(own, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}

fn vokian_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        5 if ctx.quest_items_count(HAMILS_2ND_LETTER) > 0 => "30514-01.html".to_string(),
        6 => "30514-03.html".to_string(),
        7 if ctx.quest_items_count(TALISMAN_OF_SNAKE) > 0 => {
            ctx.take_items(TALISMAN_OF_SNAKE, 1);
            ctx.set_memo_state(8);
            ctx.set_cond(8, true);
            "30514-04.html".to_string()
        }
        8 => "30514-05.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}

fn hamil_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        1 if ctx.quest_items_count(BERNARDS_INTRODUCTION) > 0 => "30626-01.html".to_string(),
        2 if ctx.quest_items_count(HAMILS_1ST_LETTER) > 0 => "30626-04.html".to_string(),
        4 if ctx.quest_items_count(HUNTERS_1ST_RUNE) == 10 => "30626-05.html".to_string(),
        5 if ctx.quest_items_count(HAMILS_2ND_LETTER) > 0 => "30626-08.html".to_string(),
        8 => {
            ctx.give_items(HAMILS_3RD_LETTER, 1);
            ctx.take_items(HUNTERS_2ND_RUNE, -1);
            ctx.set_memo_state(9);
            ctx.set_cond(9, true);
            "30626-09.html".to_string()
        }
        9 if ctx.quest_items_count(HAMILS_3RD_LETTER) > 0 => "30626-10.html".to_string(),
        12 if ctx.quest_items_count(CRESCENT_MOON_BOW) > 0 => {
            ctx.set_cond(13, true);
            ctx.set_memo_state(13);
            "30626-11.html".to_string()
        }
        13 => "30626-12.html".to_string(),
        14 if ctx.quest_items_count(TALISMAN_OF_KADESH) > 0 => {
            ctx.give_adena(161806, true);
            ctx.give_items(MARK_OF_SAGITTARIUS, 1);
            ctx.add_exp_and_sp(894888, 61408);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            "30626-13.html".to_string()
        }
        _ => ctx.no_quest_html(),
    }
}

fn aron_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        2 if ctx.quest_items_count(HAMILS_1ST_LETTER) > 0 => "30653-01.html".to_string(),
        3 => "30653-03.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}

fn gauen_talk(ctx: &mut QuestCtx, memo: i32) -> String {
    match memo {
        9 if ctx.quest_items_count(HAMILS_3RD_LETTER) > 0 => {
            ctx.take_items(HAMILS_3RD_LETTER, 1);
            ctx.set_memo_state(10);
            ctx.set_cond(10, true);
            "30717-01.html".to_string()
        }
        10 => "30717-03.html".to_string(),
        12 => "30717-04.html".to_string(),
        11 if ctx.quest_items_count(STAKATO_CHITIN) > 0
            && ctx.quest_items_count(MITHRIL_CLIP) > 0
            && ctx.quest_items_count(REINFORCED_BOWSTRING) > 0
            && ctx.quest_items_count(MANASHENS_HORN) > 0 =>
        {
            ctx.give_items(WOODEN_ARROW, 10);
            ctx.give_items(CRESCENT_MOON_BOW, 1);
            ctx.take_items(MITHRIL_CLIP, 1);
            ctx.take_items(STAKATO_CHITIN, 1);
            ctx.take_items(REINFORCED_BOWSTRING, 1);
            ctx.take_items(MANASHENS_HORN, 1);
            ctx.set_memo_state(12);
            ctx.set_cond(12, true);
            "30717-02.html".to_string()
        }
        _ => ctx.no_quest_html(),
    }
}
