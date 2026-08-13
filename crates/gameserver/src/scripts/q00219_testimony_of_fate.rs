//! Testimony of Fate (219) — `quests/Q00219_TestimonyOfFate`. The Dark Elf
//! 2nd-class prerequisite (Dark Elf race, `DELF_2ND_GROUP`, level 37+). Magister
//! Kaira sends the aspirant to lay Kasandra's spirit to rest (Metheus, a herb
//! hunt for Master Ixia's belladonna, and Alder's skull), then earn Tetrarch
//! Thifiell's Palus Charm and complete Arkenia's alchemy — four overlord skulls
//! into Red Fairy Dust, a Black Willow Leaf into Blight Treant Sap — for the
//! Mark of Fate.
//!
//! Item-gated (cond 1..18). Two order-independent collections: five poison
//! reagents (each to 10 → cond 7) and four overlord skulls the Bloody Pixy
//! transmutes into Red Fairy Dust.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const MAGISTER_ROA: i32 = 30114;
const WAREHOUSE_KEEPER_NORMAN: i32 = 30210;
const TETRARCH_THIFIELL: i32 = 30358;
const ARKENIA: i32 = 30419;
const MASTER_IXIA: i32 = 30463;
const MAGISTER_KAIRA: i32 = 30476;
const ALDERS_SPIRIT: i32 = 30613;
const BROTHER_METHEUS: i32 = 30614;
const BLOODY_PIXY: i32 = 31845;
const BLIGHT_TREANT: i32 = 31850;
// Items
const KAIRAS_LETTER: i32 = 3173;
const METHEUSS_FUNERAL_JAR: i32 = 3174;
const KASANDRAS_REMAINS: i32 = 3175;
const HERBALISM_TEXTBOOK: i32 = 3176;
const IXIAS_LIST: i32 = 3177;
const MEDUSAS_ICHOR: i32 = 3178;
const MARSH_SPIDER_FLUIDS: i32 = 3179;
const DEAD_SEEKER_DUNG: i32 = 3180;
const TYRANTS_BLOOD: i32 = 3181;
const NIGHTSHADE_ROOT: i32 = 3182;
const BELLADONNA: i32 = 3183;
const ALDERS_SKULL1: i32 = 3184;
const ALDERS_SKULL2: i32 = 3185;
const ALDERS_RECEIPT: i32 = 3186;
const REVELATIONS_MANUSCRIPT: i32 = 3187;
const KAIRAS_RECOMMENDATION: i32 = 3189;
const KAIRAS_INSTRUCTIONS: i32 = 3188;
const PALUS_CHARM: i32 = 3190;
const THIFIELLS_LETTER: i32 = 3191;
const ARKENIAS_NOTE: i32 = 3192;
const PIXY_GARNET: i32 = 3193;
const GRANDISS_SKULL: i32 = 3194;
const KARUL_BUGBEAR_SKULL: i32 = 3195;
const BREKA_OVERLORD_SKULL: i32 = 3196;
const LETO_OVERLORD_SKULL: i32 = 3197;
const RED_FAIRY_DUST: i32 = 3198;
const TIMIRIRAN_SEED: i32 = 3199;
const BLACK_WILLOW_LEAF: i32 = 3200;
const BLIGHT_TREANT_SAP: i32 = 3201;
const ARKENIAS_LETTER: i32 = 3202;
// Reward
const MARK_OF_FATE: i32 = 3172;
// Monsters
const HANGMAN_TREE: i32 = 20144;
const MARSH_STAKATO: i32 = 20157;
const MEDUSA: i32 = 20158;
const TYRANT: i32 = 20192;
const TYRANT_KINGPIN: i32 = 20193;
const DEAD_SEEKER: i32 = 20202;
const MARSH_STAKATO_WORKER: i32 = 20230;
const MARSH_STAKATO_SOLDIER: i32 = 20232;
const MARSH_SPIDER: i32 = 20233;
const MARSH_STAKATO_DRONE: i32 = 20234;
const BREKA_ORC_OVERLORD: i32 = 20270;
const GRANDIS: i32 = 20554;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const KARUL_BUGBEAR: i32 = 20600;
const BLACK_WILLOW_LURKER: i32 = 27079;
// Misc
const MIN_LEVEL: i32 = 37;
const RACE_DARK_ELF: i32 = 2;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// A poison-reagent leg: collect `own` up to 10, and once all five reagents are
/// at 10 advance to cond 7. `others` are the other four reagents.
fn herb_kill(ctx: &mut QuestCtx, own: i32, others: [i32; 4]) {
    if has(ctx, IXIAS_LIST) && ctx.quest_items_count(own) < 10 {
        if ctx.quest_items_count(own) == 9 {
            ctx.give_items(own, 1);
            ctx.play_sound(quest_sounds::MIDDLE);
            if others.iter().all(|&o| ctx.quest_items_count(o) >= 10) {
                ctx.set_cond(7, false);
            }
        } else {
            ctx.give_items(own, 1);
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}

/// An overlord-skull leg for the Bloody Pixy's Red Fairy Dust.
fn skull_kill(ctx: &mut QuestCtx, skull: i32) {
    if has(ctx, PALUS_CHARM) && has(ctx, ARKENIAS_NOTE) && has(ctx, PIXY_GARNET) && !has(ctx, skull)
    {
        ctx.give_items(skull, 1);
        ctx.play_sound(quest_sounds::MIDDLE);
    }
}

pub struct Q00219TestimonyOfFate;

impl QuestScript for Q00219TestimonyOfFate {
    fn id(&self) -> i32 {
        219
    }
    fn name(&self) -> &'static str {
        "Q00219_TestimonyOfFate"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00219_TestimonyOfFate"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MAGISTER_KAIRA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            MAGISTER_KAIRA,
            MAGISTER_ROA,
            WAREHOUSE_KEEPER_NORMAN,
            TETRARCH_THIFIELL,
            ARKENIA,
            MASTER_IXIA,
            ALDERS_SPIRIT,
            BROTHER_METHEUS,
            BLOODY_PIXY,
            BLIGHT_TREANT,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            HANGMAN_TREE,
            MARSH_STAKATO,
            MEDUSA,
            TYRANT,
            TYRANT_KINGPIN,
            DEAD_SEEKER,
            MARSH_STAKATO_WORKER,
            MARSH_STAKATO_SOLDIER,
            MARSH_SPIDER,
            MARSH_STAKATO_DRONE,
            BREKA_ORC_OVERLORD,
            GRANDIS,
            LETO_LIZARDMAN_OVERLORD,
            KARUL_BUGBEAR,
            BLACK_WILLOW_LURKER,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            KAIRAS_LETTER,
            METHEUSS_FUNERAL_JAR,
            KASANDRAS_REMAINS,
            HERBALISM_TEXTBOOK,
            IXIAS_LIST,
            MEDUSAS_ICHOR,
            MARSH_SPIDER_FLUIDS,
            DEAD_SEEKER_DUNG,
            TYRANTS_BLOOD,
            NIGHTSHADE_ROOT,
            BELLADONNA,
            ALDERS_SKULL1,
            ALDERS_SKULL2,
            ALDERS_RECEIPT,
            REVELATIONS_MANUSCRIPT,
            KAIRAS_RECOMMENDATION,
            KAIRAS_INSTRUCTIONS,
            PALUS_CHARM,
            THIFIELLS_LETTER,
            ARKENIAS_NOTE,
            PIXY_GARNET,
            GRANDISS_SKULL,
            KARUL_BUGBEAR_SKULL,
            BREKA_OVERLORD_SKULL,
            LETO_OVERLORD_SKULL,
            RED_FAIRY_DUST,
            TIMIRIRAN_SEED,
            BLACK_WILLOW_LEAF,
            BLIGHT_TREANT_SAP,
            ARKENIAS_LETTER,
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
                    ctx.give_items(KAIRAS_LETTER, 1);
                }
                None
            }
            "30476-04.htm" | "30476-13.html" | "30476-14.html" | "30114-02.html"
            | "30114-03.html" | "30463-02a.html" => Some(event.to_string()),
            "30476-12.html" => ctx
                .swap_quest_item(REVELATIONS_MANUSCRIPT, KAIRAS_RECOMMENDATION, 15)
                .then(|| event.to_string()),
            "30114-04.html" => ctx
                .swap_quest_item(ALDERS_SKULL2, ALDERS_RECEIPT, 12)
                .then(|| event.to_string()),
            "30419-02.html" => ctx
                .swap_quest_item(THIFIELLS_LETTER, ARKENIAS_NOTE, 17)
                .then(|| event.to_string()),
            "30419-05.html" => {
                if has(ctx, ARKENIAS_NOTE)
                    && has(ctx, RED_FAIRY_DUST)
                    && has(ctx, BLIGHT_TREANT_SAP)
                {
                    ctx.take_items(ARKENIAS_NOTE, 1);
                    ctx.take_items(RED_FAIRY_DUST, 1);
                    ctx.take_items(BLIGHT_TREANT_SAP, 1);
                    ctx.give_items(ARKENIAS_LETTER, 1);
                    ctx.set_cond(18, true);
                    return Some(event.to_string());
                }
                None
            }
            "31845-02.html" => {
                ctx.give_items(PIXY_GARNET, 1);
                Some(event.to_string())
            }
            "31850-02.html" => {
                ctx.give_items(TIMIRIRAN_SEED, 1);
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
            HANGMAN_TREE => {
                if has(ctx, METHEUSS_FUNERAL_JAR) && !has(ctx, KASANDRAS_REMAINS) {
                    ctx.take_items(METHEUSS_FUNERAL_JAR, 1);
                    ctx.give_items(KASANDRAS_REMAINS, 1);
                    ctx.set_cond(3, true);
                }
            }
            MARSH_STAKATO | MARSH_STAKATO_WORKER | MARSH_STAKATO_SOLDIER | MARSH_STAKATO_DRONE => {
                herb_kill(
                    ctx,
                    NIGHTSHADE_ROOT,
                    [
                        MEDUSAS_ICHOR,
                        MARSH_SPIDER_FLUIDS,
                        DEAD_SEEKER_DUNG,
                        TYRANTS_BLOOD,
                    ],
                );
            }
            MEDUSA => herb_kill(
                ctx,
                MEDUSAS_ICHOR,
                [
                    MARSH_SPIDER_FLUIDS,
                    DEAD_SEEKER_DUNG,
                    TYRANTS_BLOOD,
                    NIGHTSHADE_ROOT,
                ],
            ),
            TYRANT | TYRANT_KINGPIN => herb_kill(
                ctx,
                TYRANTS_BLOOD,
                [
                    MEDUSAS_ICHOR,
                    MARSH_SPIDER_FLUIDS,
                    DEAD_SEEKER_DUNG,
                    NIGHTSHADE_ROOT,
                ],
            ),
            DEAD_SEEKER => herb_kill(
                ctx,
                DEAD_SEEKER_DUNG,
                [
                    MEDUSAS_ICHOR,
                    MARSH_SPIDER_FLUIDS,
                    TYRANTS_BLOOD,
                    NIGHTSHADE_ROOT,
                ],
            ),
            MARSH_SPIDER => herb_kill(
                ctx,
                MARSH_SPIDER_FLUIDS,
                [
                    MEDUSAS_ICHOR,
                    DEAD_SEEKER_DUNG,
                    TYRANTS_BLOOD,
                    NIGHTSHADE_ROOT,
                ],
            ),
            BREKA_ORC_OVERLORD => skull_kill(ctx, BREKA_OVERLORD_SKULL),
            GRANDIS => skull_kill(ctx, GRANDISS_SKULL),
            LETO_LIZARDMAN_OVERLORD => skull_kill(ctx, LETO_OVERLORD_SKULL),
            KARUL_BUGBEAR => skull_kill(ctx, KARUL_BUGBEAR_SKULL),
            BLACK_WILLOW_LURKER => {
                if has(ctx, PALUS_CHARM)
                    && has(ctx, ARKENIAS_NOTE)
                    && has(ctx, TIMIRIRAN_SEED)
                    && !has(ctx, BLIGHT_TREANT_SAP)
                    && !has(ctx, BLACK_WILLOW_LEAF)
                {
                    ctx.give_items(BLACK_WILLOW_LEAF, 1);
                    ctx.play_sound(quest_sounds::MIDDLE);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == MAGISTER_KAIRA {
                if ctx.player_race() != RACE_DARK_ELF {
                    return Some("30476-01.html".to_string());
                } else if ctx.player_level() < MIN_LEVEL {
                    return Some("30476-02.html".to_string());
                } else if ctx.is_in_category("DELF_2ND_GROUP") {
                    return Some("30476-03.htm".to_string());
                }
                return Some("30476-01a.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == MAGISTER_KAIRA {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            MAGISTER_KAIRA => Some(kaira_talk(ctx)),
            BROTHER_METHEUS => Some(metheus_talk(ctx)),
            MASTER_IXIA => Some(ixia_talk(ctx)),
            MAGISTER_ROA => Some(roa_talk(ctx)),
            WAREHOUSE_KEEPER_NORMAN => Some(norman_talk(ctx)),
            TETRARCH_THIFIELL => Some(thifiell_talk(ctx)),
            ARKENIA => Some(arkenia_talk(ctx)),
            ALDERS_SPIRIT => {
                if has(ctx, ALDERS_SKULL1) || has(ctx, ALDERS_SKULL2) {
                    Some("30613-01.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            BLOODY_PIXY => Some(pixy_talk(ctx)),
            BLIGHT_TREANT => Some(treant_talk(ctx)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

fn kaira_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, KAIRAS_LETTER) {
        "30476-06.html".to_string()
    } else if has(ctx, METHEUSS_FUNERAL_JAR) || has(ctx, KASANDRAS_REMAINS) {
        "30476-07.html".to_string()
    } else if has(ctx, HERBALISM_TEXTBOOK) || has(ctx, IXIAS_LIST) {
        ctx.set_cond(5, true);
        "30476-08.html".to_string()
    } else if has(ctx, ALDERS_SKULL1) {
        ctx.take_items(ALDERS_SKULL1, 1);
        ctx.give_items(ALDERS_SKULL2, 1);
        ctx.set_cond(10, true);
        "30476-09.html".to_string()
    } else if has(ctx, ALDERS_SKULL2) || has(ctx, ALDERS_RECEIPT) {
        ctx.set_cond(11, true);
        "30476-10.html".to_string()
    } else if has(ctx, REVELATIONS_MANUSCRIPT) {
        "30476-11.html".to_string()
    } else if has(ctx, KAIRAS_INSTRUCTIONS) {
        ctx.give_items(KAIRAS_RECOMMENDATION, 1);
        ctx.take_items(KAIRAS_INSTRUCTIONS, 1);
        ctx.set_cond(15, true);
        "30476-15.html".to_string()
    } else if has(ctx, KAIRAS_RECOMMENDATION) {
        "30476-16.html".to_string()
    } else if has(ctx, PALUS_CHARM) {
        "30476-17.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn metheus_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, KAIRAS_LETTER) {
        ctx.take_items(KAIRAS_LETTER, 1);
        ctx.give_items(METHEUSS_FUNERAL_JAR, 1);
        ctx.set_cond(2, true);
        "30614-01.html".to_string()
    } else if has(ctx, METHEUSS_FUNERAL_JAR) && !has(ctx, KASANDRAS_REMAINS) {
        "30614-02.html".to_string()
    } else if has(ctx, KASANDRAS_REMAINS) && !has(ctx, METHEUSS_FUNERAL_JAR) {
        ctx.take_items(KASANDRAS_REMAINS, 1);
        ctx.give_items(HERBALISM_TEXTBOOK, 1);
        ctx.set_cond(4, true);
        "30614-03.html".to_string()
    } else if has(ctx, HERBALISM_TEXTBOOK) || has(ctx, IXIAS_LIST) {
        ctx.set_cond(5, true);
        "30614-04.html".to_string()
    } else if has(ctx, BELLADONNA) {
        ctx.take_items(BELLADONNA, 1);
        ctx.give_items(ALDERS_SKULL1, 1);
        ctx.set_cond(9, true);
        "30614-05.html".to_string()
    } else if has(ctx, ALDERS_SKULL1)
        || has(ctx, ALDERS_SKULL2)
        || has(ctx, ALDERS_RECEIPT)
        || has(ctx, REVELATIONS_MANUSCRIPT)
        || has(ctx, KAIRAS_INSTRUCTIONS)
        || has(ctx, KAIRAS_RECOMMENDATION)
    {
        "30614-06.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn ixia_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, HERBALISM_TEXTBOOK) {
        ctx.take_items(HERBALISM_TEXTBOOK, 1);
        ctx.give_items(IXIAS_LIST, 1);
        ctx.set_cond(6, true);
        "30463-01.html".to_string()
    } else if has(ctx, IXIAS_LIST) {
        if ctx.quest_items_count(MEDUSAS_ICHOR) >= 10
            && ctx.quest_items_count(MARSH_SPIDER_FLUIDS) >= 10
            && ctx.quest_items_count(DEAD_SEEKER_DUNG) >= 10
            && ctx.quest_items_count(TYRANTS_BLOOD) >= 10
            && ctx.quest_items_count(NIGHTSHADE_ROOT) >= 10
        {
            ctx.take_items(IXIAS_LIST, 1);
            ctx.take_items(MEDUSAS_ICHOR, -1);
            ctx.take_items(MARSH_SPIDER_FLUIDS, -1);
            ctx.take_items(DEAD_SEEKER_DUNG, -1);
            ctx.take_items(TYRANTS_BLOOD, -1);
            ctx.take_items(NIGHTSHADE_ROOT, -1);
            ctx.give_items(BELLADONNA, 1);
            ctx.set_cond(8, true);
            "30463-03.html".to_string()
        } else {
            "30463-02.html".to_string()
        }
    } else if has(ctx, BELLADONNA) {
        "30463-04.html".to_string()
    } else if has(ctx, ALDERS_SKULL1)
        || has(ctx, ALDERS_SKULL2)
        || has(ctx, ALDERS_RECEIPT)
        || has(ctx, REVELATIONS_MANUSCRIPT)
        || has(ctx, KAIRAS_INSTRUCTIONS)
        || has(ctx, KAIRAS_RECOMMENDATION)
    {
        "30463-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn roa_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ALDERS_SKULL2) {
        "30114-01.html".to_string()
    } else if has(ctx, ALDERS_RECEIPT) {
        "30114-05.html".to_string()
    } else if has(ctx, REVELATIONS_MANUSCRIPT)
        || has(ctx, KAIRAS_INSTRUCTIONS)
        || has(ctx, KAIRAS_RECOMMENDATION)
    {
        "30114-06.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn norman_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ALDERS_RECEIPT) {
        ctx.take_items(ALDERS_RECEIPT, 1);
        ctx.give_items(REVELATIONS_MANUSCRIPT, 1);
        ctx.set_cond(13, true);
        "30210-01.html".to_string()
    } else if has(ctx, REVELATIONS_MANUSCRIPT) {
        "30210-02.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn thifiell_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, KAIRAS_RECOMMENDATION) {
        ctx.take_items(KAIRAS_RECOMMENDATION, 1);
        ctx.give_items(PALUS_CHARM, 1);
        ctx.give_items(THIFIELLS_LETTER, 1);
        ctx.set_cond(16, true);
        "30358-01.html".to_string()
    } else if has(ctx, PALUS_CHARM) {
        if has(ctx, THIFIELLS_LETTER) {
            "30358-02.html".to_string()
        } else if has(ctx, ARKENIAS_NOTE) {
            "30358-03.html".to_string()
        } else if has(ctx, ARKENIAS_LETTER) {
            ctx.give_adena(247708, true);
            ctx.give_items(MARK_OF_FATE, 1);
            ctx.add_exp_and_sp(1365470, 91124);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            "30358-04.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn arkenia_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, PALUS_CHARM) {
        return ctx.no_quest_html();
    }
    if has(ctx, THIFIELLS_LETTER) {
        "30419-01.html".to_string()
    } else if has(ctx, ARKENIAS_NOTE) && !has(ctx, RED_FAIRY_DUST) && !has(ctx, BLIGHT_TREANT_SAP) {
        "30419-03.html".to_string()
    } else if has(ctx, ARKENIAS_NOTE) && has(ctx, RED_FAIRY_DUST) && has(ctx, BLIGHT_TREANT_SAP) {
        "30419-04.html".to_string()
    } else if has(ctx, ARKENIAS_LETTER) {
        "30419-06.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn pixy_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, PALUS_CHARM) || !has(ctx, ARKENIAS_NOTE) {
        return ctx.no_quest_html();
    }
    if !has(ctx, RED_FAIRY_DUST) && !has(ctx, PIXY_GARNET) {
        "31845-01.html".to_string()
    } else if !has(ctx, RED_FAIRY_DUST)
        && has(ctx, PIXY_GARNET)
        && !has(ctx, GRANDISS_SKULL)
        && !has(ctx, KARUL_BUGBEAR_SKULL)
        && !has(ctx, BREKA_OVERLORD_SKULL)
        && !has(ctx, LETO_OVERLORD_SKULL)
    {
        "31845-03.html".to_string()
    } else if !has(ctx, RED_FAIRY_DUST)
        && has(ctx, PIXY_GARNET)
        && has(ctx, GRANDISS_SKULL)
        && has(ctx, KARUL_BUGBEAR_SKULL)
        && has(ctx, BREKA_OVERLORD_SKULL)
        && has(ctx, LETO_OVERLORD_SKULL)
    {
        ctx.take_items(PIXY_GARNET, 1);
        ctx.take_items(GRANDISS_SKULL, 1);
        ctx.take_items(KARUL_BUGBEAR_SKULL, 1);
        ctx.take_items(BREKA_OVERLORD_SKULL, 1);
        ctx.take_items(LETO_OVERLORD_SKULL, 1);
        ctx.give_items(RED_FAIRY_DUST, 1);
        "31845-04.html".to_string()
    } else if !has(ctx, PIXY_GARNET) && has(ctx, RED_FAIRY_DUST) {
        "31845-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn treant_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, PALUS_CHARM) || !has(ctx, ARKENIAS_NOTE) {
        return ctx.no_quest_html();
    }
    if !has(ctx, BLIGHT_TREANT_SAP) && !has(ctx, TIMIRIRAN_SEED) {
        "31850-01.html".to_string()
    } else if has(ctx, TIMIRIRAN_SEED)
        && !has(ctx, BLIGHT_TREANT_SAP)
        && !has(ctx, BLACK_WILLOW_LEAF)
    {
        "31850-03.html".to_string()
    } else if has(ctx, TIMIRIRAN_SEED)
        && has(ctx, BLACK_WILLOW_LEAF)
        && !has(ctx, BLIGHT_TREANT_SAP)
    {
        ctx.take_items(TIMIRIRAN_SEED, 1);
        ctx.take_items(BLACK_WILLOW_LEAF, 1);
        ctx.give_items(BLIGHT_TREANT_SAP, 1);
        "31850-04.html".to_string()
    } else if has(ctx, BLIGHT_TREANT_SAP) && !has(ctx, TIMIRIRAN_SEED) {
        "31850-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}
