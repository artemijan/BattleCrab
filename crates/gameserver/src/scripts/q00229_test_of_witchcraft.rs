//! Test of Witchcraft (229) — `quests/Q00229_TestOfWitchcraft`. The Warlock /
//! Spellsinger… actually the Necromancer/Warlock-leaning proof (Wizard, Knight
//! or Palus Knight, level 39+). Shadow Orim sends the aspirant to gather the six
//! Gems of Aklantoth (from Iker, Kaira, Lara's revenant, Nestle/Leopold's
//! mercenary), forge the Sword of Binding and a Soultrap Crystal, and bind the
//! Drevanul Prince Zeruel — struck down with that very sword — for the Mark of
//! Witchcraft.
//!
//! Item-gated (cond 1..10). Zeruel is first driven off (attacked while holding
//! the First Brimstone → he flees, cond 5) and later bound: only a killing blow
//! from the Sword of Binding yields the Zeruel Bind Crystal (Java
//! `getKillingBlowWeapon`, approximated by the killer's equipped weapon).

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const GROCER_LARA: i32 = 30063;
const TRADER_ALEXANDRIA: i32 = 30098;
const MAGISTER_IKER: i32 = 30110;
const PRIEST_VADIN: i32 = 30188;
const TRADER_NESTLE: i32 = 30314;
const SIR_KLAUS_VASPER: i32 = 30417;
const LEOPOLD: i32 = 30435;
const MAGISTER_KAIRA: i32 = 30476;
const SHADOW_ORIM: i32 = 30630;
const WARDEN_RODERIK: i32 = 30631;
const WARDEN_ENDRIGO: i32 = 30632;
const FISHER_EVERT: i32 = 30633;
// Items
const SWORD_OF_BINDING: i32 = 3029;
const ORIMS_DIAGRAM: i32 = 3308;
const ALEXANDRIAS_BOOK: i32 = 3309;
const IKERS_LIST: i32 = 3310;
const DIRE_WYRM_FANG: i32 = 3311;
const LETO_LIZARDMAN_CHARM: i32 = 3312;
const ENCHANTED_STONE_GOLEM_HEARTSTONE: i32 = 3313;
const LARAS_MEMO: i32 = 3314;
const NESTLES_MEMO: i32 = 3315;
const LEOPOLDS_JOURNAL: i32 = 3316;
const AKLANTOTH_1ST_GEM: i32 = 3317;
const AKLANTOTH_2ND_GEM: i32 = 3318;
const AKLANTOTH_3RD_GEM: i32 = 3319;
const AKLANTOTH_4TH_GEM: i32 = 3320;
const AKLANTOTH_5TH_GEM: i32 = 3321;
const AKLANTOTH_6TH_GEM: i32 = 3322;
const BRIMSTONE_1ST: i32 = 3323;
const ORIMS_INSTRUCTIONS: i32 = 3324;
const ORIMS_1ST_LETTER: i32 = 3325;
const ORIMS_2ND_LETTER: i32 = 3326;
const SIR_VASPERS_LETTER: i32 = 3327;
const VADINS_CRUCIFIX: i32 = 3328;
const TAMLIN_ORC_AMULET: i32 = 3329;
const VADINS_SANCTIONS: i32 = 3330;
const IKERS_AMULET: i32 = 3331;
const SOULTRAP_CRYSTAL: i32 = 3332;
const PURGATORY_KEY: i32 = 3333;
const ZERUEL_BIND_CRYSTAL: i32 = 3334;
const BRIMSTONE_2ND: i32 = 3335;
// Reward
const MARK_OF_WITCHCRAFT: i32 = 3307;
// Monsters
const DIRE_WYRM: i32 = 20557;
const ENCHANTED_STONE_GOLEM: i32 = 20565;
const LETO_LIZARDMAN: i32 = 20577;
const LETO_LIZARDMAN_ARCHER: i32 = 20578;
const LETO_LIZARDMAN_SOLDIER: i32 = 20579;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const TAMLIN_ORC: i32 = 20601;
const TAMLIN_ORC_ARCHER: i32 = 20602;
const NAMELESS_REVENANT: i32 = 27099;
const SKELETAL_MERCENARY: i32 = 27100;
const DREVANUL_PRINCE_ZERUEL: i32 = 27101;
// Misc
const MIN_LEVEL: i32 = 39;
const WIZARD: i32 = 11;
const KNIGHT: i32 = 4;
const PALUS_KNIGHT: i32 = 32;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

fn all_six_gems(ctx: &QuestCtx) -> bool {
    has(ctx, AKLANTOTH_1ST_GEM)
        && has(ctx, AKLANTOTH_2ND_GEM)
        && has(ctx, AKLANTOTH_3RD_GEM)
        && has(ctx, AKLANTOTH_4TH_GEM)
        && has(ctx, AKLANTOTH_5TH_GEM)
        && has(ctx, AKLANTOTH_6TH_GEM)
}

/// A capped reagent leg gated on the book + Iker's list.
fn ingredient(ctx: &mut QuestCtx, item: i32) {
    if has(ctx, ALEXANDRIAS_BOOK) && has(ctx, IKERS_LIST) && ctx.quest_items_count(item) < 20 {
        ctx.give_items(item, 1);
        ctx.play_sound(if ctx.quest_items_count(item) >= 20 {
            quest_sounds::MIDDLE
        } else {
            quest_sounds::ITEMGET
        });
    }
}

pub struct Q00229TestOfWitchcraft;

impl QuestScript for Q00229TestOfWitchcraft {
    fn id(&self) -> i32 {
        229
    }
    fn name(&self) -> &'static str {
        "Q00229_TestOfWitchcraft"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00229_TestOfWitchcraft"
    }
    fn start_npcs(&self) -> &[i32] {
        &[SHADOW_ORIM]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            SHADOW_ORIM,
            GROCER_LARA,
            TRADER_ALEXANDRIA,
            MAGISTER_IKER,
            PRIEST_VADIN,
            TRADER_NESTLE,
            SIR_KLAUS_VASPER,
            LEOPOLD,
            MAGISTER_KAIRA,
            WARDEN_RODERIK,
            WARDEN_ENDRIGO,
            FISHER_EVERT,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            DIRE_WYRM,
            ENCHANTED_STONE_GOLEM,
            LETO_LIZARDMAN,
            LETO_LIZARDMAN_ARCHER,
            LETO_LIZARDMAN_SOLDIER,
            LETO_LIZARDMAN_WARRIOR,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OVERLORD,
            TAMLIN_ORC,
            TAMLIN_ORC_ARCHER,
            NAMELESS_REVENANT,
            SKELETAL_MERCENARY,
            DREVANUL_PRINCE_ZERUEL,
        ]
    }
    fn attack_npcs(&self) -> &[i32] {
        &[
            NAMELESS_REVENANT,
            SKELETAL_MERCENARY,
            DREVANUL_PRINCE_ZERUEL,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            SWORD_OF_BINDING,
            ORIMS_DIAGRAM,
            ALEXANDRIAS_BOOK,
            IKERS_LIST,
            DIRE_WYRM_FANG,
            LETO_LIZARDMAN_CHARM,
            ENCHANTED_STONE_GOLEM_HEARTSTONE,
            LARAS_MEMO,
            NESTLES_MEMO,
            LEOPOLDS_JOURNAL,
            AKLANTOTH_1ST_GEM,
            AKLANTOTH_2ND_GEM,
            AKLANTOTH_3RD_GEM,
            AKLANTOTH_4TH_GEM,
            AKLANTOTH_5TH_GEM,
            AKLANTOTH_6TH_GEM,
            BRIMSTONE_1ST,
            ORIMS_INSTRUCTIONS,
            ORIMS_1ST_LETTER,
            ORIMS_2ND_LETTER,
            SIR_VASPERS_LETTER,
            VADINS_CRUCIFIX,
            TAMLIN_ORC_AMULET,
            VADINS_SANCTIONS,
            IKERS_AMULET,
            SOULTRAP_CRYSTAL,
            PURGATORY_KEY,
            ZERUEL_BIND_CRYSTAL,
            BRIMSTONE_2ND,
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
                    ctx.give_items(ORIMS_DIAGRAM, 1);
                }
                None
            }
            "30630-04.htm" | "30630-06.htm" | "30630-07.htm" | "30630-12.htm" | "30630-13.htm"
            | "30630-20.htm" | "30630-21.htm" | "30098-02.htm" | "30110-02.htm"
            | "30417-02.htm" => Some(event.to_string()),
            "30630-14.htm" => {
                if has(ctx, ALEXANDRIAS_BOOK) {
                    ctx.take_items(ALEXANDRIAS_BOOK, 1);
                    for g in [
                        AKLANTOTH_1ST_GEM,
                        AKLANTOTH_2ND_GEM,
                        AKLANTOTH_3RD_GEM,
                        AKLANTOTH_4TH_GEM,
                        AKLANTOTH_5TH_GEM,
                        AKLANTOTH_6TH_GEM,
                    ] {
                        ctx.take_items(g, 1);
                    }
                    ctx.give_items(BRIMSTONE_1ST, 1);
                    ctx.set_cond(4, true);
                    ctx.spawn_attacker(DREVANUL_PRINCE_ZERUEL, true);
                    return Some(event.to_string());
                }
                None
            }
            "30630-16.htm" => {
                if has(ctx, BRIMSTONE_1ST) {
                    ctx.take_items(BRIMSTONE_1ST, 1);
                    ctx.give_items(ORIMS_INSTRUCTIONS, 1);
                    ctx.give_items(ORIMS_1ST_LETTER, 1);
                    ctx.give_items(ORIMS_2ND_LETTER, 1);
                    ctx.set_cond(6, true);
                    return Some(event.to_string());
                }
                None
            }
            "30630-22.htm" => {
                if has(ctx, ZERUEL_BIND_CRYSTAL) {
                    ctx.give_adena(372154, true);
                    ctx.give_items(MARK_OF_WITCHCRAFT, 1);
                    ctx.add_exp_and_sp(2058244, 141240);
                    ctx.exit_quest(false, true);
                    ctx.social_action(3);
                    return Some(event.to_string());
                }
                None
            }
            "30063-02.htm" => {
                ctx.give_items(LARAS_MEMO, 1);
                Some(event.to_string())
            }
            "30098-03.htm" => {
                if has(ctx, ORIMS_DIAGRAM) {
                    ctx.take_items(ORIMS_DIAGRAM, 1);
                    ctx.give_items(ALEXANDRIAS_BOOK, 1);
                    ctx.set_cond(2, true);
                    return Some(event.to_string());
                }
                None
            }
            "30110-03.htm" => {
                ctx.give_items(IKERS_LIST, 1);
                Some(event.to_string())
            }
            "30110-08.htm" => {
                ctx.take_items(ORIMS_2ND_LETTER, 1);
                ctx.give_items(IKERS_AMULET, 1);
                ctx.give_items(SOULTRAP_CRYSTAL, 1);
                if has(ctx, SWORD_OF_BINDING) {
                    ctx.set_cond(7, true);
                }
                Some(event.to_string())
            }
            "30314-02.htm" => {
                ctx.give_items(NESTLES_MEMO, 1);
                Some(event.to_string())
            }
            "30417-03.htm" => {
                if has(ctx, ORIMS_1ST_LETTER) {
                    ctx.take_items(ORIMS_1ST_LETTER, 1);
                    ctx.give_items(SIR_VASPERS_LETTER, 1);
                    return Some(event.to_string());
                }
                None
            }
            "30435-02.htm" => {
                if has(ctx, NESTLES_MEMO) {
                    ctx.take_items(NESTLES_MEMO, 1);
                    ctx.give_items(LEOPOLDS_JOURNAL, 1);
                    return Some(event.to_string());
                }
                None
            }
            "30476-02.htm" => {
                ctx.give_items(AKLANTOTH_2ND_GEM, 1);
                if has(ctx, AKLANTOTH_1ST_GEM)
                    && has(ctx, AKLANTOTH_3RD_GEM)
                    && has(ctx, AKLANTOTH_4TH_GEM)
                    && has(ctx, AKLANTOTH_5TH_GEM)
                    && has(ctx, AKLANTOTH_6TH_GEM)
                {
                    ctx.set_cond(3, true);
                }
                Some(event.to_string())
            }
            "30633-02.htm" => {
                ctx.give_items(BRIMSTONE_2ND, 1);
                ctx.set_cond(9, true);
                // TODO(G22): getSummonedNpcCount()<1 guard; spawn Zeruel.
                ctx.spawn_attacker(DREVANUL_PRINCE_ZERUEL, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            NAMELESS_REVENANT => {
                if ctx.npc_script_value() == 0
                    && has(ctx, ALEXANDRIAS_BOOK)
                    && has(ctx, LARAS_MEMO)
                    && !has(ctx, AKLANTOTH_3RD_GEM)
                {
                    ctx.set_npc_script_value(1);
                }
            }
            SKELETAL_MERCENARY => {
                if ctx.npc_script_value() == 0
                    && has(ctx, LEOPOLDS_JOURNAL)
                    && !has(ctx, AKLANTOTH_4TH_GEM)
                    && !has(ctx, AKLANTOTH_5TH_GEM)
                    && !has(ctx, AKLANTOTH_6TH_GEM)
                {
                    ctx.set_npc_script_value(1);
                }
            }
            DREVANUL_PRINCE_ZERUEL => {
                if has(ctx, BRIMSTONE_1ST) {
                    ctx.delete_npc();
                    ctx.set_cond(5, true);
                } else if has(ctx, ORIMS_INSTRUCTIONS)
                    && has(ctx, BRIMSTONE_2ND)
                    && has(ctx, SWORD_OF_BINDING)
                    && has(ctx, SOULTRAP_CRYSTAL)
                    && ctx.npc_script_value() == 0
                    && ctx.equipped_weapon_id() == SWORD_OF_BINDING
                {
                    ctx.set_npc_script_value(1);
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
            DIRE_WYRM => ingredient(ctx, DIRE_WYRM_FANG),
            ENCHANTED_STONE_GOLEM => ingredient(ctx, ENCHANTED_STONE_GOLEM_HEARTSTONE),
            LETO_LIZARDMAN
            | LETO_LIZARDMAN_ARCHER
            | LETO_LIZARDMAN_SOLDIER
            | LETO_LIZARDMAN_WARRIOR
            | LETO_LIZARDMAN_SHAMAN
            | LETO_LIZARDMAN_OVERLORD => ingredient(ctx, LETO_LIZARDMAN_CHARM),
            TAMLIN_ORC | TAMLIN_ORC_ARCHER => {
                if has(ctx, VADINS_CRUCIFIX)
                    && ctx.roll(100) < 50
                    && ctx.quest_items_count(TAMLIN_ORC_AMULET) < 20
                {
                    ctx.give_items(TAMLIN_ORC_AMULET, 1);
                    ctx.play_sound(if ctx.quest_items_count(TAMLIN_ORC_AMULET) >= 20 {
                        quest_sounds::MIDDLE
                    } else {
                        quest_sounds::ITEMGET
                    });
                }
            }
            NAMELESS_REVENANT => {
                if has(ctx, ALEXANDRIAS_BOOK)
                    && has(ctx, LARAS_MEMO)
                    && !has(ctx, AKLANTOTH_3RD_GEM)
                {
                    ctx.take_items(LARAS_MEMO, 1);
                    ctx.give_items(AKLANTOTH_3RD_GEM, 1);
                    ctx.play_sound(quest_sounds::ITEMGET);
                    if has(ctx, AKLANTOTH_1ST_GEM)
                        && has(ctx, AKLANTOTH_2ND_GEM)
                        && has(ctx, AKLANTOTH_4TH_GEM)
                        && has(ctx, AKLANTOTH_5TH_GEM)
                        && has(ctx, AKLANTOTH_6TH_GEM)
                    {
                        ctx.set_cond(3, false);
                    }
                }
            }
            SKELETAL_MERCENARY => {
                if has(ctx, LEOPOLDS_JOURNAL)
                    && !(has(ctx, AKLANTOTH_4TH_GEM)
                        && has(ctx, AKLANTOTH_5TH_GEM)
                        && has(ctx, AKLANTOTH_6TH_GEM))
                {
                    if !has(ctx, AKLANTOTH_4TH_GEM) {
                        ctx.give_items(AKLANTOTH_4TH_GEM, 1);
                        ctx.play_sound(quest_sounds::ITEMGET);
                    } else if !has(ctx, AKLANTOTH_5TH_GEM) {
                        ctx.give_items(AKLANTOTH_5TH_GEM, 1);
                        ctx.play_sound(quest_sounds::ITEMGET);
                    } else if !has(ctx, AKLANTOTH_6TH_GEM) {
                        ctx.take_items(LEOPOLDS_JOURNAL, 1);
                        ctx.give_items(AKLANTOTH_6TH_GEM, 1);
                        ctx.play_sound(quest_sounds::ITEMGET);
                        if has(ctx, AKLANTOTH_1ST_GEM)
                            && has(ctx, AKLANTOTH_2ND_GEM)
                            && has(ctx, AKLANTOTH_3RD_GEM)
                        {
                            ctx.set_cond(3, false);
                        }
                    }
                }
            }
            DREVANUL_PRINCE_ZERUEL => {
                if has(ctx, ORIMS_INSTRUCTIONS)
                    && has(ctx, BRIMSTONE_2ND)
                    && has(ctx, SWORD_OF_BINDING)
                    && has(ctx, SOULTRAP_CRYSTAL)
                    && ctx.equipped_weapon_id() == SWORD_OF_BINDING
                {
                    ctx.take_items(SOULTRAP_CRYSTAL, 1);
                    ctx.give_items(PURGATORY_KEY, 1);
                    ctx.give_items(ZERUEL_BIND_CRYSTAL, 1);
                    ctx.take_items(BRIMSTONE_2ND, 1);
                    ctx.play_sound(quest_sounds::ITEMGET);
                    ctx.set_cond(10, false);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == SHADOW_ORIM {
                let class = ctx.player_class_id();
                if class == WIZARD || class == KNIGHT || class == PALUS_KNIGHT {
                    if ctx.player_level() >= MIN_LEVEL {
                        return Some(if class == WIZARD {
                            "30630-03.htm".to_string()
                        } else {
                            "30630-05.htm".to_string()
                        });
                    }
                    return Some("30630-02.htm".to_string());
                }
                return Some("30630-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == SHADOW_ORIM {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            SHADOW_ORIM => Some(orim_talk(ctx)),
            GROCER_LARA => Some(lara_talk(ctx)),
            TRADER_ALEXANDRIA => Some(alexandria_talk(ctx)),
            MAGISTER_IKER => Some(iker_talk(ctx)),
            PRIEST_VADIN => Some(vadin_talk(ctx)),
            TRADER_NESTLE => Some(nestle_talk(ctx)),
            SIR_KLAUS_VASPER => Some(vasper_talk(ctx)),
            LEOPOLD => Some(leopold_talk(ctx)),
            MAGISTER_KAIRA => Some(kaira_talk(ctx)),
            WARDEN_RODERIK => {
                if has(ctx, ALEXANDRIAS_BOOK)
                    && (has(ctx, LARAS_MEMO) || has(ctx, AKLANTOTH_3RD_GEM))
                {
                    Some("30631-01.htm".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            WARDEN_ENDRIGO => {
                if has(ctx, ALEXANDRIAS_BOOK)
                    && (has(ctx, LARAS_MEMO) || has(ctx, AKLANTOTH_3RD_GEM))
                {
                    Some("30632-01.htm".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            FISHER_EVERT => Some(evert_talk(ctx)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

fn orim_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ORIMS_DIAGRAM) {
        "30630-09.htm".to_string()
    } else if has(ctx, ALEXANDRIAS_BOOK) {
        if all_six_gems(ctx) {
            "30630-11.htm".to_string()
        } else {
            "30630-10.htm".to_string()
        }
    } else if has(ctx, BRIMSTONE_1ST) {
        "30630-15.htm".to_string()
    } else if has(ctx, SWORD_OF_BINDING) && has(ctx, SOULTRAP_CRYSTAL) {
        ctx.set_cond(8, true);
        "30630-18.htm".to_string()
    } else if has(ctx, SWORD_OF_BINDING) && has(ctx, ZERUEL_BIND_CRYSTAL) {
        "30630-19.htm".to_string()
    } else if has(ctx, ORIMS_INSTRUCTIONS) {
        "30630-17.htm".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn lara_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ALEXANDRIAS_BOOK) {
        if !has(ctx, LARAS_MEMO) && !has(ctx, AKLANTOTH_3RD_GEM) {
            "30063-01.htm".to_string()
        } else if !has(ctx, AKLANTOTH_3RD_GEM) && has(ctx, LARAS_MEMO) {
            "30063-03.htm".to_string()
        } else {
            "30063-04.htm".to_string()
        }
    } else if has(ctx, BRIMSTONE_1ST) || has(ctx, ORIMS_INSTRUCTIONS) {
        "30063-05.htm".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn alexandria_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ORIMS_DIAGRAM) {
        "30098-01.htm".to_string()
    } else if has(ctx, ALEXANDRIAS_BOOK) {
        "30098-04.htm".to_string()
    } else if has(ctx, ORIMS_INSTRUCTIONS) && has(ctx, BRIMSTONE_1ST) {
        "30098-05.htm".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn iker_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ALEXANDRIAS_BOOK) {
        if !has(ctx, IKERS_LIST) && !has(ctx, AKLANTOTH_1ST_GEM) {
            "30110-01.htm".to_string()
        } else if has(ctx, IKERS_LIST) {
            if ctx.quest_items_count(DIRE_WYRM_FANG) >= 20
                && ctx.quest_items_count(LETO_LIZARDMAN_CHARM) >= 20
                && ctx.quest_items_count(ENCHANTED_STONE_GOLEM_HEARTSTONE) >= 20
            {
                ctx.take_items(IKERS_LIST, 1);
                ctx.take_items(DIRE_WYRM_FANG, -1);
                ctx.take_items(LETO_LIZARDMAN_CHARM, -1);
                ctx.take_items(ENCHANTED_STONE_GOLEM_HEARTSTONE, -1);
                ctx.give_items(AKLANTOTH_1ST_GEM, 1);
                if has(ctx, AKLANTOTH_2ND_GEM)
                    && has(ctx, AKLANTOTH_3RD_GEM)
                    && has(ctx, AKLANTOTH_4TH_GEM)
                    && has(ctx, AKLANTOTH_5TH_GEM)
                    && has(ctx, AKLANTOTH_6TH_GEM)
                {
                    ctx.set_cond(3, true);
                }
                "30110-05.htm".to_string()
            } else {
                "30110-04.htm".to_string()
            }
        } else {
            "30110-06.htm".to_string()
        }
    } else if has(ctx, ORIMS_INSTRUCTIONS) {
        if !has(ctx, SOULTRAP_CRYSTAL) && !has(ctx, ZERUEL_BIND_CRYSTAL) {
            "30110-07.htm".to_string()
        } else if !has(ctx, ZERUEL_BIND_CRYSTAL) && has(ctx, SOULTRAP_CRYSTAL) {
            "30110-09.htm".to_string()
        } else {
            "30110-10.htm".to_string()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn vadin_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ORIMS_INSTRUCTIONS) && has(ctx, SIR_VASPERS_LETTER) {
        ctx.take_items(SIR_VASPERS_LETTER, 1);
        ctx.give_items(VADINS_CRUCIFIX, 1);
        "30188-01.htm".to_string()
    } else if has(ctx, VADINS_CRUCIFIX) {
        if ctx.quest_items_count(TAMLIN_ORC_AMULET) < 20 {
            "30188-02.htm".to_string()
        } else {
            ctx.take_items(VADINS_CRUCIFIX, 1);
            ctx.take_items(TAMLIN_ORC_AMULET, -1);
            ctx.give_items(VADINS_SANCTIONS, 1);
            "30188-03.htm".to_string()
        }
    } else if has(ctx, ORIMS_INSTRUCTIONS) {
        if has(ctx, VADINS_SANCTIONS) {
            "30188-04.htm".to_string()
        } else if has(ctx, SWORD_OF_BINDING) {
            "30188-05.htm".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn nestle_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, ALEXANDRIAS_BOOK) {
        return ctx.no_quest_html();
    }
    if !has(ctx, LEOPOLDS_JOURNAL)
        && !has(ctx, NESTLES_MEMO)
        && !has(ctx, AKLANTOTH_4TH_GEM)
        && !has(ctx, AKLANTOTH_5TH_GEM)
        && !has(ctx, AKLANTOTH_6TH_GEM)
    {
        "30314-01.htm".to_string()
    } else if has(ctx, NESTLES_MEMO) && !has(ctx, LEOPOLDS_JOURNAL) {
        "30314-03.htm".to_string()
    } else if !has(ctx, NESTLES_MEMO)
        && (has(ctx, LEOPOLDS_JOURNAL)
            || has(ctx, AKLANTOTH_4TH_GEM)
            || has(ctx, AKLANTOTH_5TH_GEM)
            || has(ctx, AKLANTOTH_6TH_GEM))
    {
        "30314-04.htm".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn vasper_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, ORIMS_INSTRUCTIONS) {
        return ctx.no_quest_html();
    }
    if has(ctx, ORIMS_1ST_LETTER) {
        "30417-01.htm".to_string()
    } else if has(ctx, SIR_VASPERS_LETTER) {
        "30417-04.htm".to_string()
    } else if has(ctx, VADINS_SANCTIONS) {
        ctx.give_items(SWORD_OF_BINDING, 1);
        ctx.take_items(VADINS_SANCTIONS, 1);
        if has(ctx, SOULTRAP_CRYSTAL) {
            ctx.set_cond(7, true);
        }
        "30417-05.htm".to_string()
    } else if has(ctx, SWORD_OF_BINDING) {
        "30417-06.htm".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn leopold_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ALEXANDRIAS_BOOK) {
        if has(ctx, NESTLES_MEMO) && !has(ctx, LEOPOLDS_JOURNAL) {
            "30435-01.htm".to_string()
        } else if has(ctx, LEOPOLDS_JOURNAL) && !has(ctx, NESTLES_MEMO) {
            "30435-03.htm".to_string()
        } else if has(ctx, AKLANTOTH_4TH_GEM)
            && has(ctx, AKLANTOTH_5TH_GEM)
            && has(ctx, AKLANTOTH_6TH_GEM)
        {
            "30435-04.htm".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else if has(ctx, BRIMSTONE_1ST) || has(ctx, ORIMS_INSTRUCTIONS) {
        "30435-05.htm".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn kaira_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ALEXANDRIAS_BOOK) {
        if !has(ctx, AKLANTOTH_2ND_GEM) {
            "30476-01.htm".to_string()
        } else {
            "30476-03.htm".to_string()
        }
    } else if has(ctx, BRIMSTONE_1ST) || has(ctx, ORIMS_INSTRUCTIONS) {
        "30476-04.htm".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn evert_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, ORIMS_INSTRUCTIONS) {
        return ctx.no_quest_html();
    }
    if has(ctx, SOULTRAP_CRYSTAL) && has(ctx, SWORD_OF_BINDING) && !has(ctx, BRIMSTONE_2ND) {
        "30633-01.htm".to_string()
    } else if has(ctx, SOULTRAP_CRYSTAL)
        && has(ctx, BRIMSTONE_2ND)
        && !has(ctx, ZERUEL_BIND_CRYSTAL)
    {
        // TODO(G22): getSummonedNpcCount()<1 guard; spawn Zeruel.
        ctx.spawn_attacker(DREVANUL_PRINCE_ZERUEL, true);
        "30633-02.htm".to_string()
    } else if has(ctx, ZERUEL_BIND_CRYSTAL)
        && !has(ctx, SOULTRAP_CRYSTAL)
        && !has(ctx, BRIMSTONE_2ND)
    {
        "30633-03.htm".to_string()
    } else {
        ctx.no_quest_html()
    }
}
