//! Test of the Lord (232) — `quests/Q00232_TestOfTheLord`. The Overlord
//! 2nd-class proof (Orc race, Orc Shaman, level 39+). Flame Lord Kakai sends the
//! aspirant to the five Orc clan chiefs (Varkees, Tantus, Hatos, Takuna,
//! Chianta), each of whom hands out a charm, takes a gathered pile of trophies,
//! and forges a ceremonial artifact. With all five, Kakai forges the Bear Fang
//! Necklace; Ancestor Martankus then sends the aspirant to slay the Ragna Orc
//! chief for the Immortal Flame, and the Mark of the Lord.
//!
//! Pure item-gate (cond 1..7). The five crafting legs complete in any order —
//! cond 2 fires when the fifth ceremonial artifact is forged.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const SEER_SOMAK: i32 = 30510;
const SEER_MANAKIA: i32 = 30515;
const TRADER_JAKAL: i32 = 30558;
const BLACKSMITH_SUMARI: i32 = 30564;
const FLAME_LORD_KAKAI: i32 = 30565;
const ATUBA_CHIEF_VARKEES: i32 = 30566;
const NERUGA_CHIEF_TANTUS: i32 = 30567;
const URUTU_CHIEF_HATOS: i32 = 30568;
const DUDA_MARA_CHIEF_TAKUNA: i32 = 30641;
const GANDI_CHIEF_CHIANTA: i32 = 30642;
const FIRST_ORC: i32 = 30643;
const ANCESTOR_MARTANKUS: i32 = 30649;
// Items
const ADENA: i32 = 57;
const BONE_ARROW: i32 = 1341;
const ORDEAL_NECKLACE: i32 = 3391;
const VARKEES_CHARM: i32 = 3392;
const TANTUS_CHARM: i32 = 3393;
const HATOS_CHARM: i32 = 3394;
const TAKUNA_CHARM: i32 = 3395;
const CHIANTA_CHARM: i32 = 3396;
const MANAKIAS_ORDERS: i32 = 3397;
const BREKA_ORC_FANG: i32 = 3398;
const MANAKIAS_AMULET: i32 = 3399;
const HUGE_ORC_FANG: i32 = 3400;
const SUMARIS_LETTER: i32 = 3401;
const URUTU_BLADE: i32 = 3402;
const TIMAK_ORC_SKULL: i32 = 3403;
const SWORD_INTO_SKULL: i32 = 3404;
const NERUGA_AXE_BLADE: i32 = 3405;
const AXE_OF_CEREMONY: i32 = 3406;
const MARSH_SPIDER_FEELER: i32 = 3407;
const MARSH_SPIDER_FEET: i32 = 3408;
const HANDIWORK_SPIDER_BROOCH: i32 = 3409;
const ENCHANTED_MONSTER_CORNEA: i32 = 3410;
const MONSTER_EYE_WOODCARVING: i32 = 3411;
const BEAR_FANG_NECKLACE: i32 = 3412;
const MARTANKUS_CHARM: i32 = 3413;
const RAGNA_ORC_HEAD: i32 = 3414;
const RAGNA_CHIEF_NOTICE: i32 = 3415;
const IMMORTAL_FLAME: i32 = 3416;
// Reward
const MARK_OF_LORD: i32 = 3390;
// Monsters
const MARSH_SPIDER: i32 = 20233;
const BREKA_ORC_SHAMAN: i32 = 20269;
const BREKA_ORC_OVERLORD: i32 = 20270;
const ENCHANTED_MONSTEREYE: i32 = 20564;
const TIMAK_ORC: i32 = 20583;
const TIMAK_ORC_ARCHER: i32 = 20584;
const TIMAK_ORC_SOLDIER: i32 = 20585;
const TIMAK_ORC_WARRIOR: i32 = 20586;
const TIMAK_ORC_SHAMAN: i32 = 20587;
const TIMAK_ORC_OVERLORD: i32 = 20588;
const RAGNA_ORC_OVERLORD: i32 = 20778;
const RAGNA_ORC_SEER: i32 = 20779;
// Misc
const MIN_LEVEL: i32 = 39;
const RACE_ORC: i32 = 3;
const ORC_SHAMAN: i32 = 50;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// Once the fifth ceremonial artifact is forged, advance to cond 2. Called from
/// each chief with the *other four* artifacts as arguments.
fn maybe_cond2(ctx: &mut QuestCtx, a: i32, b: i32, c: i32, d: i32) {
    if has(ctx, a) && has(ctx, b) && has(ctx, c) && has(ctx, d) {
        ctx.set_cond(2, true);
    }
}

/// A two-material spider leg: 2 feelers up to 10, then 2 feet up to 10.
fn spider_kill(ctx: &mut QuestCtx) {
    if has(ctx, ORDEAL_NECKLACE) && has(ctx, TAKUNA_CHARM) && !has(ctx, HANDIWORK_SPIDER_BROOCH) {
        if ctx.quest_items_count(MARSH_SPIDER_FEELER) < 10 {
            ctx.give_items(MARSH_SPIDER_FEELER, 2);
            ctx.play_sound(if ctx.quest_items_count(MARSH_SPIDER_FEELER) >= 10 {
                quest_sounds::MIDDLE
            } else {
                quest_sounds::ITEMGET
            });
        } else if ctx.quest_items_count(MARSH_SPIDER_FEET) < 10 {
            ctx.give_items(MARSH_SPIDER_FEET, 2);
            ctx.play_sound(if ctx.quest_items_count(MARSH_SPIDER_FEET) >= 10 {
                quest_sounds::MIDDLE
            } else {
                quest_sounds::ITEMGET
            });
        }
    }
}

pub struct Q00232TestOfTheLord;

impl QuestScript for Q00232TestOfTheLord {
    fn id(&self) -> i32 {
        232
    }
    fn name(&self) -> &'static str {
        "Q00232_TestOfTheLord"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00232_TestOfTheLord"
    }
    fn start_npcs(&self) -> &[i32] {
        &[FLAME_LORD_KAKAI]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            FLAME_LORD_KAKAI,
            SEER_SOMAK,
            SEER_MANAKIA,
            TRADER_JAKAL,
            BLACKSMITH_SUMARI,
            ATUBA_CHIEF_VARKEES,
            NERUGA_CHIEF_TANTUS,
            URUTU_CHIEF_HATOS,
            DUDA_MARA_CHIEF_TAKUNA,
            GANDI_CHIEF_CHIANTA,
            FIRST_ORC,
            ANCESTOR_MARTANKUS,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            MARSH_SPIDER,
            BREKA_ORC_SHAMAN,
            BREKA_ORC_OVERLORD,
            ENCHANTED_MONSTEREYE,
            TIMAK_ORC,
            TIMAK_ORC_ARCHER,
            TIMAK_ORC_SOLDIER,
            TIMAK_ORC_WARRIOR,
            TIMAK_ORC_SHAMAN,
            TIMAK_ORC_OVERLORD,
            RAGNA_ORC_OVERLORD,
            RAGNA_ORC_SEER,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            ORDEAL_NECKLACE,
            VARKEES_CHARM,
            TANTUS_CHARM,
            HATOS_CHARM,
            TAKUNA_CHARM,
            CHIANTA_CHARM,
            MANAKIAS_ORDERS,
            BREKA_ORC_FANG,
            MANAKIAS_AMULET,
            HUGE_ORC_FANG,
            SUMARIS_LETTER,
            URUTU_BLADE,
            TIMAK_ORC_SKULL,
            SWORD_INTO_SKULL,
            NERUGA_AXE_BLADE,
            AXE_OF_CEREMONY,
            MARSH_SPIDER_FEELER,
            MARSH_SPIDER_FEET,
            HANDIWORK_SPIDER_BROOCH,
            ENCHANTED_MONSTER_CORNEA,
            MONSTER_EYE_WOODCARVING,
            BEAR_FANG_NECKLACE,
            MARTANKUS_CHARM,
            RAGNA_ORC_HEAD,
            RAGNA_CHIEF_NOTICE,
            IMMORTAL_FLAME,
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
                    ctx.give_items(ORDEAL_NECKLACE, 1);
                }
                None
            }
            "30565-05a.html" | "30558-03a.html" | "30643-02.html" | "30643-03.html"
            | "30649-02.html" | "30649-03.html" => Some(event.to_string()),
            "30565-08.html" => {
                if has(ctx, HUGE_ORC_FANG) {
                    ctx.take_items(ORDEAL_NECKLACE, 1);
                    ctx.take_items(HUGE_ORC_FANG, 1);
                    ctx.take_items(SWORD_INTO_SKULL, 1);
                    ctx.take_items(AXE_OF_CEREMONY, 1);
                    ctx.take_items(HANDIWORK_SPIDER_BROOCH, 1);
                    ctx.take_items(MONSTER_EYE_WOODCARVING, 1);
                    ctx.give_items(BEAR_FANG_NECKLACE, 1);
                    ctx.set_cond(3, true);
                    return Some(event.to_string());
                }
                None
            }
            "30558-02.html" => {
                if ctx.quest_items_count(ADENA) >= 1000 {
                    ctx.take_items(ADENA, 1000);
                    ctx.give_items(NERUGA_AXE_BLADE, 1);
                    return Some(event.to_string());
                }
                None
            }
            "30566-02.html" => {
                ctx.give_items(VARKEES_CHARM, 1);
                Some(event.to_string())
            }
            "30567-02.html" => {
                ctx.give_items(TANTUS_CHARM, 1);
                Some(event.to_string())
            }
            "30568-02.html" => {
                ctx.give_items(HATOS_CHARM, 1);
                Some(event.to_string())
            }
            "30641-02.html" => {
                ctx.give_items(TAKUNA_CHARM, 1);
                Some(event.to_string())
            }
            "30642-02.html" => {
                ctx.give_items(CHIANTA_CHARM, 1);
                Some(event.to_string())
            }
            "30649-04.html" => {
                if has(ctx, BEAR_FANG_NECKLACE) {
                    ctx.take_items(BEAR_FANG_NECKLACE, 1);
                    ctx.give_items(MARTANKUS_CHARM, 1);
                    ctx.set_cond(4, true);
                    return Some(event.to_string());
                }
                None
            }
            "30649-07.html" => {
                // TODO(G22): getSummonedNpcCount()<1 guard; spawn First Orc.
                ctx.spawn_near_npc(FIRST_ORC, false);
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
            MARSH_SPIDER => spider_kill(ctx),
            BREKA_ORC_SHAMAN | BREKA_ORC_OVERLORD => {
                if has(ctx, ORDEAL_NECKLACE)
                    && has(ctx, VARKEES_CHARM)
                    && has(ctx, MANAKIAS_ORDERS)
                    && !has(ctx, HUGE_ORC_FANG)
                    && !has(ctx, MANAKIAS_AMULET)
                    && ctx.quest_items_count(BREKA_ORC_FANG) < 20
                {
                    ctx.give_items(BREKA_ORC_FANG, 2);
                    ctx.play_sound(if ctx.quest_items_count(BREKA_ORC_FANG) >= 20 {
                        quest_sounds::MIDDLE
                    } else {
                        quest_sounds::ITEMGET
                    });
                }
            }
            ENCHANTED_MONSTEREYE => {
                if has(ctx, ORDEAL_NECKLACE)
                    && has(ctx, CHIANTA_CHARM)
                    && !has(ctx, MONSTER_EYE_WOODCARVING)
                    && ctx.quest_items_count(ENCHANTED_MONSTER_CORNEA) < 20
                {
                    ctx.give_items(ENCHANTED_MONSTER_CORNEA, 1);
                    ctx.play_sound(if ctx.quest_items_count(ENCHANTED_MONSTER_CORNEA) >= 20 {
                        quest_sounds::MIDDLE
                    } else {
                        quest_sounds::ITEMGET
                    });
                }
            }
            TIMAK_ORC | TIMAK_ORC_ARCHER | TIMAK_ORC_SOLDIER | TIMAK_ORC_WARRIOR
            | TIMAK_ORC_SHAMAN | TIMAK_ORC_OVERLORD => {
                if has(ctx, ORDEAL_NECKLACE)
                    && has(ctx, HATOS_CHARM)
                    && !has(ctx, SWORD_INTO_SKULL)
                    && ctx.quest_items_count(TIMAK_ORC_SKULL) < 10
                {
                    ctx.give_items(TIMAK_ORC_SKULL, 1);
                    ctx.play_sound(if ctx.quest_items_count(TIMAK_ORC_SKULL) >= 10 {
                        quest_sounds::MIDDLE
                    } else {
                        quest_sounds::ITEMGET
                    });
                }
            }
            RAGNA_ORC_OVERLORD | RAGNA_ORC_SEER => {
                if has(ctx, MARTANKUS_CHARM) {
                    if !has(ctx, RAGNA_CHIEF_NOTICE) {
                        ctx.give_items(RAGNA_CHIEF_NOTICE, 1);
                        ctx.play_sound(quest_sounds::MIDDLE);
                    } else if !has(ctx, RAGNA_ORC_HEAD) {
                        ctx.give_items(RAGNA_ORC_HEAD, 1);
                        ctx.set_cond(5, true);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == FLAME_LORD_KAKAI {
                if ctx.player_race() != RACE_ORC {
                    return Some("30565-01.html".to_string());
                } else if ctx.player_class_id() != ORC_SHAMAN {
                    return Some("30565-02.html".to_string());
                } else if ctx.player_level() < MIN_LEVEL {
                    return Some("30565-03.html".to_string());
                }
                return Some("30565-04.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == FLAME_LORD_KAKAI {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            FLAME_LORD_KAKAI => Some(kakai_talk(ctx)),
            SEER_SOMAK => Some(somak_talk(ctx)),
            SEER_MANAKIA => Some(manakia_talk(ctx)),
            TRADER_JAKAL => Some(jakal_talk(ctx)),
            BLACKSMITH_SUMARI => Some(sumari_talk(ctx)),
            ATUBA_CHIEF_VARKEES => Some(varkees_talk(ctx)),
            NERUGA_CHIEF_TANTUS => Some(tantus_talk(ctx)),
            URUTU_CHIEF_HATOS => Some(hatos_talk(ctx)),
            DUDA_MARA_CHIEF_TAKUNA => Some(takuna_talk(ctx)),
            GANDI_CHIEF_CHIANTA => Some(chianta_talk(ctx)),
            FIRST_ORC => {
                if has(ctx, MARTANKUS_CHARM) || has(ctx, IMMORTAL_FLAME) {
                    ctx.set_cond(7, true);
                    Some("30643-01.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            ANCESTOR_MARTANKUS => Some(martankus_talk(ctx)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

fn kakai_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ORDEAL_NECKLACE) {
        if has(ctx, HUGE_ORC_FANG)
            && has(ctx, SWORD_INTO_SKULL)
            && has(ctx, AXE_OF_CEREMONY)
            && has(ctx, MONSTER_EYE_WOODCARVING)
            && has(ctx, HANDIWORK_SPIDER_BROOCH)
        {
            "30565-07.html".to_string()
        } else {
            "30565-06.html".to_string()
        }
    } else if has(ctx, BEAR_FANG_NECKLACE) {
        "30565-09.html".to_string()
    } else if has(ctx, MARTANKUS_CHARM) {
        "30565-10.html".to_string()
    } else if has(ctx, IMMORTAL_FLAME) {
        ctx.give_adena(161806, true);
        ctx.give_items(MARK_OF_LORD, 1);
        ctx.add_exp_and_sp(894888, 61408);
        ctx.exit_quest(false, true);
        ctx.social_action(3);
        "30565-11.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn somak_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ORDEAL_NECKLACE)
        && has(ctx, HATOS_CHARM)
        && has(ctx, SUMARIS_LETTER)
        && !has(ctx, SWORD_INTO_SKULL)
        && !has(ctx, URUTU_BLADE)
    {
        ctx.take_items(SUMARIS_LETTER, 1);
        ctx.give_items(URUTU_BLADE, 1);
        "30510-01.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, HATOS_CHARM) && has(ctx, URUTU_BLADE) {
        "30510-02.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, SWORD_INTO_SKULL) {
        "30510-03.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn manakia_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ORDEAL_NECKLACE)
        && has(ctx, VARKEES_CHARM)
        && !has(ctx, HUGE_ORC_FANG)
        && !has(ctx, MANAKIAS_AMULET)
        && !has(ctx, MANAKIAS_ORDERS)
    {
        ctx.give_items(MANAKIAS_ORDERS, 1);
        "30515-01.html".to_string()
    } else if has(ctx, VARKEES_CHARM)
        && has(ctx, ORDEAL_NECKLACE)
        && has(ctx, MANAKIAS_ORDERS)
        && !has(ctx, HUGE_ORC_FANG)
        && !has(ctx, MANAKIAS_AMULET)
    {
        if ctx.quest_items_count(BREKA_ORC_FANG) < 20 {
            "30515-02.html".to_string()
        } else {
            ctx.take_items(MANAKIAS_ORDERS, 1);
            ctx.take_items(BREKA_ORC_FANG, -1);
            ctx.give_items(MANAKIAS_AMULET, 1);
            "30515-03.html".to_string()
        }
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, VARKEES_CHARM) && has(ctx, MANAKIAS_AMULET) {
        "30515-04.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, HUGE_ORC_FANG) {
        "30515-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn jakal_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ORDEAL_NECKLACE)
        && has(ctx, TANTUS_CHARM)
        && !has(ctx, AXE_OF_CEREMONY)
        && !has(ctx, NERUGA_AXE_BLADE)
    {
        if ctx.quest_items_count(ADENA) >= 1000 {
            "30558-01.html".to_string()
        } else {
            "30558-03.html".to_string()
        }
    } else if has(ctx, ORDEAL_NECKLACE)
        && has(ctx, TANTUS_CHARM)
        && has(ctx, NERUGA_AXE_BLADE)
        && !has(ctx, AXE_OF_CEREMONY)
    {
        "30558-04.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, AXE_OF_CEREMONY) && !has(ctx, TANTUS_CHARM) {
        "30558-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn sumari_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, HATOS_CHARM)
        && has(ctx, ORDEAL_NECKLACE)
        && !has(ctx, SWORD_INTO_SKULL)
        && !has(ctx, URUTU_BLADE)
        && !has(ctx, SUMARIS_LETTER)
    {
        ctx.give_items(SUMARIS_LETTER, 1);
        "30564-01.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, HATOS_CHARM) && has(ctx, SUMARIS_LETTER) {
        "30564-02.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, HATOS_CHARM) && has(ctx, URUTU_BLADE) {
        "30564-03.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, SWORD_INTO_SKULL) {
        "30564-04.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn varkees_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ORDEAL_NECKLACE) && !has(ctx, HUGE_ORC_FANG) && !has(ctx, VARKEES_CHARM) {
        "30566-01.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE)
        && has(ctx, VARKEES_CHARM)
        && !has(ctx, HUGE_ORC_FANG)
        && !has(ctx, MANAKIAS_AMULET)
    {
        "30566-03.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE)
        && has(ctx, VARKEES_CHARM)
        && has(ctx, MANAKIAS_AMULET)
        && !has(ctx, HUGE_ORC_FANG)
    {
        ctx.take_items(VARKEES_CHARM, 1);
        ctx.take_items(MANAKIAS_AMULET, 1);
        ctx.give_items(HUGE_ORC_FANG, 1);
        maybe_cond2(
            ctx,
            AXE_OF_CEREMONY,
            SWORD_INTO_SKULL,
            HANDIWORK_SPIDER_BROOCH,
            MONSTER_EYE_WOODCARVING,
        );
        "30566-04.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, HUGE_ORC_FANG) && !has(ctx, VARKEES_CHARM) {
        "30566-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn tantus_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ORDEAL_NECKLACE) && !has(ctx, AXE_OF_CEREMONY) && !has(ctx, TANTUS_CHARM) {
        "30567-01.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, TANTUS_CHARM) && !has(ctx, AXE_OF_CEREMONY) {
        if !has(ctx, NERUGA_AXE_BLADE) || ctx.quest_items_count(BONE_ARROW) < 1000 {
            "30567-03.html".to_string()
        } else {
            ctx.take_items(BONE_ARROW, 1000);
            ctx.take_items(TANTUS_CHARM, 1);
            ctx.take_items(NERUGA_AXE_BLADE, 1);
            ctx.give_items(AXE_OF_CEREMONY, 1);
            maybe_cond2(
                ctx,
                HUGE_ORC_FANG,
                SWORD_INTO_SKULL,
                HANDIWORK_SPIDER_BROOCH,
                MONSTER_EYE_WOODCARVING,
            );
            "30567-04.html".to_string()
        }
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, AXE_OF_CEREMONY) && !has(ctx, TANTUS_CHARM) {
        "30567-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn hatos_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ORDEAL_NECKLACE) && !has(ctx, SWORD_INTO_SKULL) && !has(ctx, HATOS_CHARM) {
        "30568-01.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, HATOS_CHARM) && !has(ctx, SWORD_INTO_SKULL) {
        if has(ctx, URUTU_BLADE) && ctx.quest_items_count(TIMAK_ORC_SKULL) >= 10 {
            ctx.take_items(HATOS_CHARM, 1);
            ctx.take_items(URUTU_BLADE, 1);
            ctx.take_items(TIMAK_ORC_SKULL, -1);
            ctx.give_items(SWORD_INTO_SKULL, 1);
            maybe_cond2(
                ctx,
                HUGE_ORC_FANG,
                AXE_OF_CEREMONY,
                HANDIWORK_SPIDER_BROOCH,
                MONSTER_EYE_WOODCARVING,
            );
            "30568-04.html".to_string()
        } else {
            "30568-03.html".to_string()
        }
    } else if has(ctx, ORDEAL_NECKLACE) && has(ctx, SWORD_INTO_SKULL) && !has(ctx, HATOS_CHARM) {
        "30568-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn takuna_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ORDEAL_NECKLACE) && !has(ctx, HANDIWORK_SPIDER_BROOCH) && !has(ctx, TAKUNA_CHARM) {
        "30641-01.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE)
        && has(ctx, TAKUNA_CHARM)
        && !has(ctx, HANDIWORK_SPIDER_BROOCH)
    {
        if ctx.quest_items_count(MARSH_SPIDER_FEELER) >= 10
            && ctx.quest_items_count(MARSH_SPIDER_FEET) >= 10
        {
            ctx.take_items(TAKUNA_CHARM, 1);
            ctx.take_items(MARSH_SPIDER_FEELER, -1);
            ctx.take_items(MARSH_SPIDER_FEET, -1);
            ctx.give_items(HANDIWORK_SPIDER_BROOCH, 1);
            maybe_cond2(
                ctx,
                HUGE_ORC_FANG,
                AXE_OF_CEREMONY,
                SWORD_INTO_SKULL,
                MONSTER_EYE_WOODCARVING,
            );
            "30641-04.html".to_string()
        } else {
            "30641-03.html".to_string()
        }
    } else if has(ctx, ORDEAL_NECKLACE)
        && has(ctx, HANDIWORK_SPIDER_BROOCH)
        && !has(ctx, TAKUNA_CHARM)
    {
        "30641-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn chianta_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ORDEAL_NECKLACE) && !has(ctx, MONSTER_EYE_WOODCARVING) && !has(ctx, CHIANTA_CHARM) {
        "30642-01.html".to_string()
    } else if has(ctx, ORDEAL_NECKLACE)
        && has(ctx, CHIANTA_CHARM)
        && !has(ctx, MONSTER_EYE_WOODCARVING)
    {
        if ctx.quest_items_count(ENCHANTED_MONSTER_CORNEA) < 20 {
            "30642-03.html".to_string()
        } else {
            ctx.take_items(CHIANTA_CHARM, 1);
            ctx.take_items(ENCHANTED_MONSTER_CORNEA, -1);
            ctx.give_items(MONSTER_EYE_WOODCARVING, 1);
            maybe_cond2(
                ctx,
                HUGE_ORC_FANG,
                AXE_OF_CEREMONY,
                SWORD_INTO_SKULL,
                HANDIWORK_SPIDER_BROOCH,
            );
            "30642-04.html".to_string()
        }
    } else if has(ctx, ORDEAL_NECKLACE)
        && has(ctx, MONSTER_EYE_WOODCARVING)
        && !has(ctx, CHIANTA_CHARM)
    {
        "30642-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn martankus_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, BEAR_FANG_NECKLACE) {
        "30649-01.html".to_string()
    } else if has(ctx, MARTANKUS_CHARM)
        && !has(ctx, RAGNA_CHIEF_NOTICE)
        && !has(ctx, RAGNA_ORC_HEAD)
    {
        "30649-05.html".to_string()
    } else if has(ctx, MARTANKUS_CHARM) && has(ctx, RAGNA_CHIEF_NOTICE) && has(ctx, RAGNA_ORC_HEAD)
    {
        ctx.take_items(MARTANKUS_CHARM, 1);
        ctx.take_items(RAGNA_ORC_HEAD, 1);
        ctx.take_items(RAGNA_CHIEF_NOTICE, 1);
        ctx.give_items(IMMORTAL_FLAME, 1);
        ctx.set_cond(6, true);
        "30649-06.html".to_string()
    } else if has(ctx, IMMORTAL_FLAME) {
        // TODO(G22): getSummonedNpcCount()<1 guard; spawn First Orc.
        ctx.spawn_near_npc(FIRST_ORC, false);
        "30649-08.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}
