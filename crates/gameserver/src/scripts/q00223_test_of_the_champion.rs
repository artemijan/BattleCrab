//! Test of the Champion (223) — `quests/Q00223_TestOfTheChampion`. The Warlord
//! (and Orc Raider) 2nd-class proof: Veteran Ascalon sends a level-39 Warrior or
//! Orc Raider on a long relay of letters between four NPCs, interleaved with four
//! hunt-and-collect legs, ending with the Mark of the Champion.
//!
//! The quest is a pure item-gated state machine — every `cond` is cosmetic
//! progress, and each turn-in is gated on which letter/insignia the player is
//! carrying, exactly as Java's `hasQuestItems` chain. Three of the hunt legs
//! (Bloody Axe Elite, Harpy, Road Scavenger) additionally spring an `onAttack`
//! ambush: the first hit on a mob rolls a chance to conjure one or two quest
//! monsters (`HARPY_MATRIARCH`/`ROAD_COLLECTOR`, or an extra elite) that also
//! drop the leg's quest item, gated by `scriptValue` so it fires only once.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const TRADER_GROOT: i32 = 30093;
const CAPTAIN_MOUEN: i32 = 30196;
const VETERAN_ASCALON: i32 = 30624;
const MASON: i32 = 30625;
// Items
const ASCALONS_1ST_LETTER: i32 = 3277;
const MASONS_LETTER: i32 = 3278;
const IRON_ROSE_RING: i32 = 3279;
const ASCALONS_2ND_LETTER: i32 = 3280;
const WHITE_ROSE_INSIGNIA: i32 = 3281;
const GROOTS_LETTER: i32 = 3282;
const ASCALONS_3RD_LETTER: i32 = 3283;
const MOUENS_1ST_ORDER: i32 = 3284;
const MOUENS_2ND_ORDER: i32 = 3285;
const MOUENS_LETTER: i32 = 3286;
const HARPYS_EGG: i32 = 3287;
const MEDUSA_VENOM: i32 = 3288;
const WINDSUS_BILE: i32 = 3289;
const BLOODY_AXE_HEAD: i32 = 3290;
const ROAD_RATMAN_HEAD: i32 = 3291;
const LETO_LIZARDMAN_FANG: i32 = 3292;
// Reward
const MARK_OF_CHAMPION: i32 = 3276;
// Monsters
const HARPY: i32 = 20145;
const MEDUSA: i32 = 20158;
const ROAD_SCAVENGER: i32 = 20551;
const WINDSUS: i32 = 20553;
const LETO_LIZARDMAN: i32 = 20577;
const LETO_LIZARDMAN_ARCHER: i32 = 20578;
const LETO_LIZARDMAN_SOLDIER: i32 = 20579;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OCERLORD: i32 = 20582;
const BLOODY_AXE_ELITE: i32 = 20780;
// Quest monsters (ambush spawns)
const HARPY_MATRIARCH: i32 = 27088;
const ROAD_COLLECTOR: i32 = 27089;
// Misc
const MIN_LEVEL: i32 = 39;
const WARRIOR: i32 = 1;
const ORC_RAIDER: i32 = 45;
const LAST_ATTACKER: &str = "lastAttacker";

const KILL_NPCS: &[i32] = &[
    HARPY,
    MEDUSA,
    WINDSUS,
    ROAD_SCAVENGER,
    LETO_LIZARDMAN,
    LETO_LIZARDMAN_ARCHER,
    LETO_LIZARDMAN_SOLDIER,
    LETO_LIZARDMAN_WARRIOR,
    LETO_LIZARDMAN_SHAMAN,
    LETO_LIZARDMAN_OCERLORD,
    BLOODY_AXE_ELITE,
    HARPY_MATRIARCH,
    ROAD_COLLECTOR,
];

pub struct Q00223TestOfTheChampion;

impl QuestScript for Q00223TestOfTheChampion {
    fn id(&self) -> i32 {
        223
    }
    fn name(&self) -> &'static str {
        "Q00223_TestOfTheChampion"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00223_TestOfTheChampion"
    }
    fn start_npcs(&self) -> &[i32] {
        &[VETERAN_ASCALON]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[VETERAN_ASCALON, TRADER_GROOT, CAPTAIN_MOUEN, MASON]
    }
    fn kill_npcs(&self) -> &[i32] {
        KILL_NPCS
    }
    fn attack_npcs(&self) -> &[i32] {
        &[HARPY, ROAD_SCAVENGER, BLOODY_AXE_ELITE]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            ASCALONS_1ST_LETTER,
            MASONS_LETTER,
            IRON_ROSE_RING,
            ASCALONS_2ND_LETTER,
            WHITE_ROSE_INSIGNIA,
            GROOTS_LETTER,
            ASCALONS_3RD_LETTER,
            MOUENS_1ST_ORDER,
            MOUENS_2ND_ORDER,
            MOUENS_LETTER,
            HARPYS_EGG,
            MEDUSA_VENOM,
            WINDSUS_BILE,
            BLOODY_AXE_HEAD,
            ROAD_RATMAN_HEAD,
            LETO_LIZARDMAN_FANG,
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
                    ctx.play_sound(quest_sounds::MIDDLE);
                    ctx.give_items(ASCALONS_1ST_LETTER, 1);
                }
                None
            }
            "30624-05.htm" | "30196-02.html" | "30625-02.html" => Some(event.to_string()),
            "30624-10.html" => {
                if ctx.quest_items_count(MASONS_LETTER) > 0 {
                    ctx.take_items(MASONS_LETTER, 1);
                    ctx.give_items(ASCALONS_2ND_LETTER, 1);
                    ctx.set_cond(5, true);
                    return Some(event.to_string());
                }
                None
            }
            "30624-14.html" => {
                if ctx.quest_items_count(GROOTS_LETTER) > 0 {
                    ctx.take_items(GROOTS_LETTER, 1);
                    ctx.give_items(ASCALONS_3RD_LETTER, 1);
                    ctx.set_cond(9, true);
                    return Some(event.to_string());
                }
                None
            }
            "30093-02.html" => {
                if ctx.quest_items_count(ASCALONS_2ND_LETTER) > 0 {
                    ctx.take_items(ASCALONS_2ND_LETTER, 1);
                    ctx.give_items(WHITE_ROSE_INSIGNIA, 1);
                    ctx.set_cond(6, true);
                    return Some(event.to_string());
                }
                None
            }
            "30196-03.html" => {
                if ctx.quest_items_count(ASCALONS_3RD_LETTER) > 0 {
                    ctx.take_items(ASCALONS_3RD_LETTER, 1);
                    ctx.give_items(MOUENS_1ST_ORDER, 1);
                    ctx.set_cond(10, true);
                    return Some(event.to_string());
                }
                None
            }
            "30196-06.html" => {
                if ctx.quest_items_count(ROAD_RATMAN_HEAD) >= 10 {
                    ctx.take_items(MOUENS_1ST_ORDER, 1);
                    ctx.give_items(MOUENS_2ND_ORDER, 1);
                    ctx.take_items(ROAD_RATMAN_HEAD, -1);
                    ctx.set_cond(12, true);
                    return Some(event.to_string());
                }
                None
            }
            "30625-03.html" => {
                if ctx.quest_items_count(ASCALONS_1ST_LETTER) > 0 {
                    ctx.take_items(ASCALONS_1ST_LETTER, 1);
                    ctx.give_items(IRON_ROSE_RING, 1);
                    ctx.set_cond(2, true);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let player = ctx.player;
        match ctx.npc_id {
            HARPY => {
                if ctx.npc_script_value() == 0 {
                    ctx.set_npc_var_int(LAST_ATTACKER, player);
                    if ctx.quest_items_count(WHITE_ROSE_INSIGNIA) > 0
                        && ctx.quest_items_count(HARPYS_EGG) < 30
                        && ctx.roll(2) == 0
                    {
                        // 70% one matriarch, else two.
                        ctx.spawn_attacker(HARPY_MATRIARCH, false);
                        if ctx.roll(10) >= 7 {
                            ctx.spawn_attacker(HARPY_MATRIARCH, false);
                        }
                    }
                    ctx.set_npc_script_value(1);
                } else if ctx.npc_script_value() == 1 {
                    ctx.set_npc_script_value(2);
                }
            }
            ROAD_SCAVENGER => {
                if ctx.npc_script_value() == 0 {
                    ctx.set_npc_var_int(LAST_ATTACKER, player);
                    if ctx.quest_items_count(MOUENS_1ST_ORDER) > 0
                        && ctx.quest_items_count(ROAD_RATMAN_HEAD) < 10
                        && ctx.roll(2) == 0
                    {
                        ctx.spawn_attacker(ROAD_COLLECTOR, false);
                        if ctx.roll(10) >= 7 {
                            ctx.spawn_attacker(ROAD_COLLECTOR, false);
                        }
                    }
                    ctx.set_npc_script_value(1);
                } else if ctx.npc_script_value() == 1 {
                    ctx.set_npc_script_value(2);
                }
            }
            BLOODY_AXE_ELITE => {
                if ctx.npc_script_value() == 0 {
                    ctx.set_npc_var_int(LAST_ATTACKER, player);
                    if ctx.quest_items_count(IRON_ROSE_RING) > 0
                        && ctx.quest_items_count(BLOODY_AXE_HEAD) < 10
                        && ctx.roll(2) == 0
                    {
                        ctx.spawn_attacker(BLOODY_AXE_ELITE, false);
                    }
                    ctx.set_npc_script_value(1);
                } else if ctx.npc_script_value() == 1 {
                    ctx.set_npc_script_value(2);
                }
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            HARPY | HARPY_MATRIARCH => {
                if ctx.quest_items_count(WHITE_ROSE_INSIGNIA) > 0
                    && ctx.quest_items_count(HARPYS_EGG) < 30
                {
                    let at_end = ctx.quest_items_count(HARPYS_EGG) >= 28;
                    ctx.give_items(HARPYS_EGG, 2);
                    if at_end {
                        ctx.play_sound(quest_sounds::MIDDLE);
                        maybe_finish_insignia(ctx);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            MEDUSA => {
                if ctx.quest_items_count(WHITE_ROSE_INSIGNIA) > 0
                    && ctx.quest_items_count(MEDUSA_VENOM) < 30
                {
                    let at_end = ctx.quest_items_count(MEDUSA_VENOM) >= 27;
                    ctx.give_items(MEDUSA_VENOM, 3);
                    if at_end {
                        ctx.play_sound(quest_sounds::MIDDLE);
                        maybe_finish_insignia(ctx);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            WINDSUS => {
                if ctx.quest_items_count(WHITE_ROSE_INSIGNIA) > 0
                    && ctx.quest_items_count(WINDSUS_BILE) < 30
                {
                    let at_end = ctx.quest_items_count(WINDSUS_BILE) >= 27;
                    ctx.give_items(WINDSUS_BILE, 3);
                    if at_end {
                        ctx.play_sound(quest_sounds::MIDDLE);
                        maybe_finish_insignia(ctx);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            ROAD_SCAVENGER | ROAD_COLLECTOR => {
                if ctx.quest_items_count(MOUENS_1ST_ORDER) > 0
                    && ctx.quest_items_count(ROAD_RATMAN_HEAD) < 10
                {
                    let at_end = ctx.quest_items_count(ROAD_RATMAN_HEAD) >= 9;
                    ctx.give_items(ROAD_RATMAN_HEAD, 1);
                    if at_end {
                        ctx.set_cond(11, true);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            LETO_LIZARDMAN | LETO_LIZARDMAN_ARCHER | LETO_LIZARDMAN_SOLDIER
            | LETO_LIZARDMAN_WARRIOR | LETO_LIZARDMAN_SHAMAN | LETO_LIZARDMAN_OCERLORD => {
                if ctx.quest_items_count(MOUENS_2ND_ORDER) > 0
                    && ctx.quest_items_count(LETO_LIZARDMAN_FANG) < 10
                {
                    let at_end = ctx.quest_items_count(LETO_LIZARDMAN_FANG) >= 9;
                    ctx.give_items(LETO_LIZARDMAN_FANG, 1);
                    if at_end {
                        ctx.set_cond(13, true);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            BLOODY_AXE_ELITE => {
                if ctx.quest_items_count(IRON_ROSE_RING) > 0
                    && ctx.quest_items_count(BLOODY_AXE_HEAD) < 10
                {
                    let at_end = ctx.quest_items_count(BLOODY_AXE_HEAD) >= 9;
                    ctx.give_items(BLOODY_AXE_HEAD, 1);
                    if at_end {
                        ctx.set_cond(3, true);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == VETERAN_ASCALON {
                let class = ctx.player_class_id();
                if class == WARRIOR || class == ORC_RAIDER {
                    if ctx.player_level() >= MIN_LEVEL {
                        return Some(if class == WARRIOR {
                            "30624-03.htm".to_string()
                        } else {
                            "30624-04.html".to_string()
                        });
                    }
                    return Some("30624-01.html".to_string());
                }
                return Some("30624-02.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == VETERAN_ASCALON {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        // Started.
        match ctx.npc_id {
            VETERAN_ASCALON => Some(ascalon_talk(ctx)),
            TRADER_GROOT => Some(groot_talk(ctx)),
            CAPTAIN_MOUEN => Some(mouen_talk(ctx)),
            MASON => Some(mason_talk(ctx)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

/// The three insignia hunt legs share one completion check: once all three
/// stacks reach 30, advance to cond 7.
fn maybe_finish_insignia(ctx: &mut QuestCtx) {
    if ctx.quest_items_count(HARPYS_EGG) >= 30
        && ctx.quest_items_count(MEDUSA_VENOM) >= 30
        && ctx.quest_items_count(WINDSUS_BILE) >= 30
    {
        ctx.set_cond(7, false);
    }
}

fn ascalon_talk(ctx: &mut QuestCtx) -> String {
    if ctx.quest_items_count(ASCALONS_1ST_LETTER) > 0 {
        "30624-07.html".to_string()
    } else if ctx.quest_items_count(IRON_ROSE_RING) > 0 {
        "30624-08.html".to_string()
    } else if ctx.quest_items_count(MASONS_LETTER) > 0 {
        "30624-09.html".to_string()
    } else if ctx.quest_items_count(ASCALONS_2ND_LETTER) > 0 {
        "30624-11.html".to_string()
    } else if ctx.quest_items_count(WHITE_ROSE_INSIGNIA) > 0 {
        "30624-12.html".to_string()
    } else if ctx.quest_items_count(GROOTS_LETTER) > 0 {
        "30624-13.html".to_string()
    } else if ctx.quest_items_count(ASCALONS_3RD_LETTER) > 0 {
        "30624-15.html".to_string()
    } else if ctx.quest_items_count(MOUENS_1ST_ORDER) > 0
        || ctx.quest_items_count(MOUENS_2ND_ORDER) > 0
    {
        "30624-16.html".to_string()
    } else if ctx.quest_items_count(MOUENS_LETTER) > 0 {
        ctx.give_adena(229764, true);
        ctx.give_items(MARK_OF_CHAMPION, 1);
        ctx.add_exp_and_sp(1270742, 87200);
        ctx.exit_quest(false, true);
        ctx.social_action(3);
        "30624-17.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn groot_talk(ctx: &mut QuestCtx) -> String {
    if ctx.quest_items_count(ASCALONS_2ND_LETTER) > 0 {
        "30093-01.html".to_string()
    } else if ctx.quest_items_count(WHITE_ROSE_INSIGNIA) > 0 {
        if ctx.quest_items_count(HARPYS_EGG) >= 30
            && ctx.quest_items_count(MEDUSA_VENOM) >= 30
            && ctx.quest_items_count(WINDSUS_BILE) >= 30
        {
            ctx.take_items(WHITE_ROSE_INSIGNIA, 1);
            ctx.give_items(GROOTS_LETTER, 1);
            ctx.take_items(HARPYS_EGG, -1);
            ctx.take_items(MEDUSA_VENOM, -1);
            ctx.take_items(WINDSUS_BILE, -1);
            ctx.set_cond(8, true);
            "30093-04.html".to_string()
        } else {
            "30093-03.html".to_string()
        }
    } else if ctx.quest_items_count(GROOTS_LETTER) > 0 {
        "30093-05.html".to_string()
    } else if ctx.quest_items_count(ASCALONS_3RD_LETTER) > 0
        || ctx.quest_items_count(MOUENS_1ST_ORDER) > 0
        || ctx.quest_items_count(MOUENS_2ND_ORDER) > 0
        || ctx.quest_items_count(MOUENS_LETTER) > 0
    {
        "30093-06.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn mouen_talk(ctx: &mut QuestCtx) -> String {
    if ctx.quest_items_count(ASCALONS_3RD_LETTER) > 0 {
        "30196-01.html".to_string()
    } else if ctx.quest_items_count(MOUENS_1ST_ORDER) > 0 {
        if ctx.quest_items_count(ROAD_RATMAN_HEAD) < 10 {
            "30196-04.html".to_string()
        } else {
            "30196-05.html".to_string()
        }
    } else if ctx.quest_items_count(MOUENS_2ND_ORDER) > 0 {
        if ctx.quest_items_count(LETO_LIZARDMAN_FANG) < 10 {
            "30196-07.html".to_string()
        } else {
            ctx.take_items(MOUENS_2ND_ORDER, 1);
            ctx.give_items(MOUENS_LETTER, 1);
            ctx.take_items(LETO_LIZARDMAN_FANG, -1);
            ctx.set_cond(14, true);
            "30196-08.html".to_string()
        }
    } else if ctx.quest_items_count(MOUENS_LETTER) > 0 {
        "30196-09.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn mason_talk(ctx: &mut QuestCtx) -> String {
    if ctx.quest_items_count(ASCALONS_1ST_LETTER) > 0 {
        "30625-01.html".to_string()
    } else if ctx.quest_items_count(IRON_ROSE_RING) > 0 {
        if ctx.quest_items_count(BLOODY_AXE_HEAD) < 10 {
            "30625-04.html".to_string()
        } else {
            ctx.give_items(MASONS_LETTER, 1);
            ctx.take_items(IRON_ROSE_RING, 1);
            ctx.take_items(BLOODY_AXE_HEAD, -1);
            ctx.set_cond(4, true);
            "30625-05.html".to_string()
        }
    } else if ctx.quest_items_count(MASONS_LETTER) > 0 {
        "30625-06.html".to_string()
    } else if ctx.quest_items_count(ASCALONS_2ND_LETTER) > 0
        || ctx.quest_items_count(WHITE_ROSE_INSIGNIA) > 0
        || ctx.quest_items_count(GROOTS_LETTER) > 0
        || ctx.quest_items_count(ASCALONS_3RD_LETTER) > 0
        || ctx.quest_items_count(MOUENS_1ST_ORDER) > 0
        || ctx.quest_items_count(MOUENS_2ND_ORDER) > 0
        || ctx.quest_items_count(MOUENS_LETTER) > 0
    {
        "30625-07.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}
