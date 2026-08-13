//! Testimony of Prosperity (221) — `quests/Q00221_TestimonyOfProsperity`. The
//! Dwarf 2nd-class prerequisite (Dwarf race, `DWARF_2ND_GROUP`, level 37+).
//! Warehouse Keeper Parman sets the aspirant to earn the First Ring of
//! Testimony by assembling four proofs — Piotur's Blessed Seed, Lilith's Elven
//! Wafer, Emily's Recipe, and the Old Account Book (five guild contributions
//! collected for Iron Gate's Lockirin) — then, with the Second Ring, to forge
//! the Key of Titan and open the Box of Titan for Maphr's Tablet Fragment and
//! the Mark of Prosperity.
//!
//! Item-gated (cond 1..9). The Old Account Book leg is a five-guild contribution
//! subsystem (Shari/Mion+Maryse/Torocco/Bolter/Toma → Spiron/Balanki/Keef/
//! Filaur/Arin receipts). The Key of Titan is recipe-crafted, so tests supply it.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const WAREHOUSE_KEEPER_WILFORD: i32 = 30005;
const WAREHOUSE_KEEPER_PARMAN: i32 = 30104;
const LILITH: i32 = 30368;
const GUARD_BRIGHT: i32 = 30466;
const TRADER_SHARI: i32 = 30517;
const TRADER_MION: i32 = 30519;
const IRON_GATES_LOCKIRIN: i32 = 30531;
const GOLDEN_WHEELS_SPIRON: i32 = 30532;
const SILVER_SCALES_BALANKI: i32 = 30533;
const BRONZE_KEYS_KEEF: i32 = 30534;
const GRAY_PILLAR_MEMBER_FILAUR: i32 = 30535;
const BLACK_ANVILS_ARIN: i32 = 30536;
const MARYSE_REDBONNET: i32 = 30553;
const MINER_BOLTER: i32 = 30554;
const CARRIER_TOROCCO: i32 = 30555;
const MASTER_TOMA: i32 = 30556;
const PIOTUR: i32 = 30597;
const EMILY: i32 = 30620;
const MAESTRO_NIKOLA: i32 = 30621;
const BOX_OF_TITAN: i32 = 30622;
// Items
const ADENA: i32 = 57;
const ANIMAL_SKIN: i32 = 1867;
const RECIPE_TITAN_KEY: i32 = 3023;
const KEY_OF_TITAN: i32 = 3030;
const RING_OF_TESTIMONY_1ST: i32 = 3239;
const RING_OF_TESTIMONY_2ND: i32 = 3240;
const OLD_ACCOUNT_BOOK: i32 = 3241;
const BLESSED_SEED: i32 = 3242;
const EMILYS_RECIPE: i32 = 3243;
const LILITHS_ELVEN_WAFER: i32 = 3244;
const MAPHR_TABLET_FRAGMENT: i32 = 3245;
const COLLECTION_LICENSE: i32 = 3246;
const LOCKIRINS_1ST_NOTICE: i32 = 3247;
const LOCKIRINS_2ND_NOTICE: i32 = 3248;
const LOCKIRINS_3RD_NOTICE: i32 = 3249;
const LOCKIRINS_4TH_NOTICE: i32 = 3250;
const LOCKIRINS_5TH_NOTICE: i32 = 3251;
const CONTRIBUTION_OF_SHARI: i32 = 3252;
const CONTRIBUTION_OF_MION: i32 = 3253;
const CONTRIBUTION_OF_MARYSE: i32 = 3254;
const MARYSES_REQUEST: i32 = 3255;
const CONTRIBUTION_OF_TOMA: i32 = 3256;
const RECEIPT_OF_BOLTER: i32 = 3257;
const RECEIPT_OF_CONTRIBUTION_1ST: i32 = 3258;
const RECEIPT_OF_CONTRIBUTION_2ND: i32 = 3259;
const RECEIPT_OF_CONTRIBUTION_3RD: i32 = 3260;
const RECEIPT_OF_CONTRIBUTION_4TH: i32 = 3261;
const RECEIPT_OF_CONTRIBUTION_5TH: i32 = 3262;
const PROCURATION_OF_TOROCCO: i32 = 3263;
const BRIGHTS_LIST: i32 = 3264;
const MANDRAGORA_PETAL: i32 = 3265;
const CRIMSON_MOSS: i32 = 3266;
const MANDRAGORA_BOUGUET: i32 = 3267;
const PARMANS_INSTRUCTIONS: i32 = 3268;
const PARMANS_LETTER: i32 = 3269;
const CLAY_DOUGH: i32 = 3270;
const PATTERN_OF_KEYHOLE: i32 = 3271;
const NIKOLAS_LIST: i32 = 3272;
const STAKATO_SHELL: i32 = 3273;
const TOAD_LORD_SAC: i32 = 3274;
const MARSH_SPIDER_THORN: i32 = 3275;
const CRYSTAL_BROOCH: i32 = 3428;
// Reward
const MARK_OF_PROSPERITY: i32 = 3238;
// Monsters
const MANDRAGORA_SPROUT1: i32 = 20154;
const MANDRAGORA_SAPLING: i32 = 20155;
const MANDRAGORA_BLOSSOM: i32 = 20156;
const MARSH_STAKATO: i32 = 20157;
const MANDRAGORA_SPROUT2: i32 = 20223;
const GIANT_CRIMSON_ANT: i32 = 20228;
const MARSH_STAKATO_WORKER: i32 = 20230;
const TOAD_LORD: i32 = 20231;
const MARSH_STAKATO_SOLDIER: i32 = 20232;
const MARSH_SPIDER: i32 = 20233;
const MARSH_STAKATO_DRONE: i32 = 20234;
// Misc
const MIN_LEVEL: i32 = 37;
const RACE_DWARF: i32 = 4;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// The four First-Ring proofs are complete → cond 2.
fn maybe_cond2(ctx: &mut QuestCtx) {
    if has(ctx, OLD_ACCOUNT_BOOK)
        && has(ctx, BLESSED_SEED)
        && has(ctx, EMILYS_RECIPE)
        && has(ctx, LILITHS_ELVEN_WAFER)
    {
        ctx.set_cond(2, true);
    }
}

pub struct Q00221TestimonyOfProsperity;

impl QuestScript for Q00221TestimonyOfProsperity {
    fn id(&self) -> i32 {
        221
    }
    fn name(&self) -> &'static str {
        "Q00221_TestimonyOfProsperity"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00221_TestimonyOfProsperity"
    }
    fn start_npcs(&self) -> &[i32] {
        &[WAREHOUSE_KEEPER_PARMAN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            WAREHOUSE_KEEPER_PARMAN,
            WAREHOUSE_KEEPER_WILFORD,
            LILITH,
            GUARD_BRIGHT,
            TRADER_SHARI,
            TRADER_MION,
            IRON_GATES_LOCKIRIN,
            GOLDEN_WHEELS_SPIRON,
            SILVER_SCALES_BALANKI,
            BRONZE_KEYS_KEEF,
            GRAY_PILLAR_MEMBER_FILAUR,
            BLACK_ANVILS_ARIN,
            MARYSE_REDBONNET,
            MINER_BOLTER,
            CARRIER_TOROCCO,
            MASTER_TOMA,
            PIOTUR,
            EMILY,
            MAESTRO_NIKOLA,
            BOX_OF_TITAN,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            MANDRAGORA_SPROUT1,
            MANDRAGORA_SAPLING,
            MANDRAGORA_BLOSSOM,
            MARSH_STAKATO,
            MANDRAGORA_SPROUT2,
            GIANT_CRIMSON_ANT,
            MARSH_STAKATO_WORKER,
            TOAD_LORD,
            MARSH_STAKATO_SOLDIER,
            MARSH_SPIDER,
            MARSH_STAKATO_DRONE,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            RECIPE_TITAN_KEY,
            KEY_OF_TITAN,
            RING_OF_TESTIMONY_1ST,
            RING_OF_TESTIMONY_2ND,
            OLD_ACCOUNT_BOOK,
            BLESSED_SEED,
            EMILYS_RECIPE,
            LILITHS_ELVEN_WAFER,
            MAPHR_TABLET_FRAGMENT,
            COLLECTION_LICENSE,
            LOCKIRINS_1ST_NOTICE,
            LOCKIRINS_2ND_NOTICE,
            LOCKIRINS_3RD_NOTICE,
            LOCKIRINS_4TH_NOTICE,
            LOCKIRINS_5TH_NOTICE,
            CONTRIBUTION_OF_SHARI,
            CONTRIBUTION_OF_MION,
            CONTRIBUTION_OF_MARYSE,
            MARYSES_REQUEST,
            CONTRIBUTION_OF_TOMA,
            RECEIPT_OF_BOLTER,
            RECEIPT_OF_CONTRIBUTION_1ST,
            RECEIPT_OF_CONTRIBUTION_2ND,
            RECEIPT_OF_CONTRIBUTION_3RD,
            RECEIPT_OF_CONTRIBUTION_4TH,
            RECEIPT_OF_CONTRIBUTION_5TH,
            PROCURATION_OF_TOROCCO,
            BRIGHTS_LIST,
            MANDRAGORA_PETAL,
            CRIMSON_MOSS,
            MANDRAGORA_BOUGUET,
            PARMANS_INSTRUCTIONS,
            PARMANS_LETTER,
            CLAY_DOUGH,
            PATTERN_OF_KEYHOLE,
            NIKOLAS_LIST,
            STAKATO_SHELL,
            TOAD_LORD_SAC,
            MARSH_SPIDER_THORN,
            CRYSTAL_BROOCH,
        ]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => {
                ctx.accept_with_item(RING_OF_TESTIMONY_1ST);
                None
            }
            "30104-04a.html" | "30104-04b.html" | "30104-04c.html" | "30104-04d.html"
            | "30104-05.html" | "30104-08a.html" | "30104-08b.html" | "30104-08c.html"
            | "30005-02.html" | "30005-03.html" | "30368-02.html" | "30466-02.html"
            | "30531-02.html" | "30620-02.html" | "30621-02.html" | "30621-03.html" => {
                Some(event.to_string())
            }
            "30104-08.html" => {
                ctx.take_items(RING_OF_TESTIMONY_1ST, 1);
                ctx.give_items(RING_OF_TESTIMONY_2ND, 1);
                ctx.take_items(OLD_ACCOUNT_BOOK, 1);
                ctx.take_items(BLESSED_SEED, 1);
                ctx.take_items(EMILYS_RECIPE, 1);
                ctx.take_items(LILITHS_ELVEN_WAFER, 1);
                ctx.give_items(PARMANS_LETTER, 1);
                ctx.set_cond(4, true);
                Some(event.to_string())
            }
            "30005-04.html" => {
                ctx.give_items(CRYSTAL_BROOCH, 1);
                Some(event.to_string())
            }
            "30368-03.html" => {
                if has(ctx, CRYSTAL_BROOCH) {
                    ctx.give_items(LILITHS_ELVEN_WAFER, 1);
                    ctx.take_items(CRYSTAL_BROOCH, 1);
                    maybe_cond2(ctx);
                    return Some(event.to_string());
                }
                None
            }
            "30466-03.html" => {
                ctx.give_items(BRIGHTS_LIST, 1);
                Some(event.to_string())
            }
            "30531-03.html" => {
                ctx.give_items(COLLECTION_LICENSE, 1);
                ctx.give_items(LOCKIRINS_1ST_NOTICE, 1);
                ctx.give_items(LOCKIRINS_2ND_NOTICE, 1);
                ctx.give_items(LOCKIRINS_3RD_NOTICE, 1);
                ctx.give_items(LOCKIRINS_4TH_NOTICE, 1);
                ctx.give_items(LOCKIRINS_5TH_NOTICE, 1);
                Some(event.to_string())
            }
            "30534-03a.html" => {
                if ctx.quest_items_count(ADENA) < 5000 {
                    Some(event.to_string())
                } else if has(ctx, PROCURATION_OF_TOROCCO) {
                    ctx.take_items(ADENA, 5000);
                    ctx.give_items(RECEIPT_OF_CONTRIBUTION_3RD, 1);
                    ctx.take_items(PROCURATION_OF_TOROCCO, 1);
                    Some("30534-03b.html".to_string())
                } else {
                    None
                }
            }
            "30555-02.html" => {
                ctx.give_items(PROCURATION_OF_TOROCCO, 1);
                Some(event.to_string())
            }
            "30597-02.html" => {
                ctx.give_items(BLESSED_SEED, 1);
                maybe_cond2(ctx);
                Some(event.to_string())
            }
            "30620-03.html" => {
                if has(ctx, MANDRAGORA_BOUGUET) {
                    ctx.give_items(EMILYS_RECIPE, 1);
                    ctx.take_items(MANDRAGORA_BOUGUET, 1);
                    maybe_cond2(ctx);
                    return Some(event.to_string());
                }
                None
            }
            "30621-04.html" => {
                ctx.give_items(CLAY_DOUGH, 1);
                ctx.set_cond(5, true);
                Some(event.to_string())
            }
            "30622-02.html" => ctx
                .swap_quest_item(CLAY_DOUGH, PATTERN_OF_KEYHOLE, 6)
                .then(|| event.to_string()),
            "30622-04.html" => {
                if has(ctx, KEY_OF_TITAN) {
                    ctx.take_items(KEY_OF_TITAN, 1);
                    ctx.give_items(MAPHR_TABLET_FRAGMENT, 1);
                    ctx.take_items(NIKOLAS_LIST, 1);
                    ctx.take_items(RECIPE_TITAN_KEY, 1);
                    ctx.take_items(STAKATO_SHELL, -1);
                    ctx.take_items(TOAD_LORD_SAC, -1);
                    ctx.take_items(MARSH_SPIDER_THORN, -1);
                    ctx.set_cond(9, true);
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
            MANDRAGORA_SPROUT1 | MANDRAGORA_SAPLING | MANDRAGORA_BLOSSOM | MANDRAGORA_SPROUT2 => {
                ingredient_kill(ctx, MANDRAGORA_PETAL, 20)
            }
            GIANT_CRIMSON_ANT => ingredient_kill(ctx, CRIMSON_MOSS, 10),
            MARSH_STAKATO | MARSH_STAKATO_WORKER | MARSH_STAKATO_SOLDIER | MARSH_STAKATO_DRONE => {
                key_material(
                    ctx,
                    STAKATO_SHELL,
                    20,
                    [(TOAD_LORD_SAC, 10), (MARSH_SPIDER_THORN, 10)],
                );
            }
            TOAD_LORD => {
                key_material(
                    ctx,
                    TOAD_LORD_SAC,
                    10,
                    [(STAKATO_SHELL, 20), (MARSH_SPIDER_THORN, 10)],
                );
            }
            MARSH_SPIDER => {
                key_material(
                    ctx,
                    MARSH_SPIDER_THORN,
                    10,
                    [(STAKATO_SHELL, 20), (TOAD_LORD_SAC, 10)],
                );
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == WAREHOUSE_KEEPER_PARMAN {
                if ctx.player_race() != RACE_DWARF {
                    return Some("30104-01.html".to_string());
                } else if ctx.player_level() < MIN_LEVEL {
                    return Some("30104-02.html".to_string());
                } else if ctx.is_in_category("DWARF_2ND_GROUP") {
                    return Some("30104-03.htm".to_string());
                }
                return Some("30104-01a.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == WAREHOUSE_KEEPER_PARMAN {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            WAREHOUSE_KEEPER_PARMAN => Some(parman_talk(ctx)),
            WAREHOUSE_KEEPER_WILFORD => Some(wilford_talk(ctx)),
            LILITH => Some(lilith_talk(ctx)),
            GUARD_BRIGHT => Some(bright_talk(ctx)),
            IRON_GATES_LOCKIRIN => Some(lockirin_talk(ctx)),
            TRADER_SHARI => Some(contribution_giver(
                ctx,
                RECEIPT_OF_CONTRIBUTION_1ST,
                CONTRIBUTION_OF_SHARI,
                LOCKIRINS_1ST_NOTICE,
                "30517-01.html",
                "30517-02.html",
            )),
            TRADER_MION => Some(contribution_giver(
                ctx,
                RECEIPT_OF_CONTRIBUTION_2ND,
                CONTRIBUTION_OF_MION,
                LOCKIRINS_2ND_NOTICE,
                "30519-01.html",
                "30519-02.html",
            )),
            MASTER_TOMA => Some(contribution_giver(
                ctx,
                RECEIPT_OF_CONTRIBUTION_5TH,
                CONTRIBUTION_OF_TOMA,
                LOCKIRINS_5TH_NOTICE,
                "30556-01.html",
                "30556-02.html",
            )),
            GOLDEN_WHEELS_SPIRON => Some(spiron_talk(ctx)),
            SILVER_SCALES_BALANKI => Some(balanki_talk(ctx)),
            BRONZE_KEYS_KEEF => Some(keef_talk(ctx)),
            GRAY_PILLAR_MEMBER_FILAUR => Some(filaur_talk(ctx)),
            BLACK_ANVILS_ARIN => Some(arin_talk(ctx)),
            MARYSE_REDBONNET => Some(maryse_talk(ctx)),
            MINER_BOLTER => Some(bolter_talk(ctx)),
            CARRIER_TOROCCO => Some(torocco_talk(ctx)),
            PIOTUR => Some(piotur_talk(ctx)),
            EMILY => Some(emily_talk(ctx)),
            MAESTRO_NIKOLA => Some(nikola_talk(ctx)),
            BOX_OF_TITAN => Some(box_talk(ctx)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

/// A Key-of-Titan reagent leg (gated on Ring 2nd + Nikola's List); cond 8 once
/// all three of shell/sac/thorn reach their caps.
/// One of Emily's two ingredient legs: gathered one per kill while Bright's
/// list is open and the recipe is not yet in hand.
fn ingredient_kill(ctx: &mut QuestCtx, item: i32, cap: i64) {
    if has(ctx, RING_OF_TESTIMONY_1ST)
        && has(ctx, BRIGHTS_LIST)
        && !has(ctx, EMILYS_RECIPE)
        && ctx.quest_items_count(item) < cap
    {
        ctx.give_items(item, 1);
        ctx.play_sound(if ctx.quest_items_count(item) == cap {
            quest_sounds::MIDDLE
        } else {
            quest_sounds::ITEMGET
        });
    }
}

fn key_material(ctx: &mut QuestCtx, own: i32, cap: i64, others: [(i32, i64); 2]) {
    if has(ctx, RING_OF_TESTIMONY_2ND)
        && has(ctx, NIKOLAS_LIST)
        && !has(ctx, CLAY_DOUGH)
        && !has(ctx, PATTERN_OF_KEYHOLE)
        && ctx.quest_items_count(own) < cap
    {
        ctx.give_items(own, 1);
        if ctx.quest_items_count(own) == cap {
            ctx.play_sound(quest_sounds::MIDDLE);
            if others.iter().all(|&(o, c)| ctx.quest_items_count(o) >= c) {
                ctx.set_cond(8, false);
            }
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}

/// Shari/Mion/Toma each hand over a raw contribution once, gated on not yet
/// having the corresponding receipt / notice.
fn contribution_giver(
    ctx: &mut QuestCtx,
    receipt: i32,
    contribution: i32,
    notice: i32,
    give_html: &str,
    have_html: &str,
) -> String {
    if !(has(ctx, RING_OF_TESTIMONY_1ST) && has(ctx, COLLECTION_LICENSE)) {
        return ctx.no_quest_html();
    }
    if !has(ctx, receipt) && !has(ctx, contribution) && !has(ctx, notice) {
        ctx.give_items(contribution, 1);
        give_html.to_string()
    } else if has(ctx, contribution) && !has(ctx, notice) && !has(ctx, receipt) {
        have_html.to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn parman_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, RING_OF_TESTIMONY_1ST) {
        if has(ctx, OLD_ACCOUNT_BOOK)
            && has(ctx, BLESSED_SEED)
            && has(ctx, EMILYS_RECIPE)
            && has(ctx, LILITHS_ELVEN_WAFER)
        {
            "30104-06.html".to_string()
        } else {
            "30104-05.html".to_string()
        }
    } else if has(ctx, PARMANS_INSTRUCTIONS) {
        ctx.take_items(PARMANS_INSTRUCTIONS, 1);
        ctx.give_items(RING_OF_TESTIMONY_2ND, 1);
        ctx.give_items(PARMANS_LETTER, 1);
        ctx.set_cond(4, true);
        "30104-10.html".to_string()
    } else if has(ctx, RING_OF_TESTIMONY_2ND) {
        if has(ctx, PARMANS_LETTER) {
            "30104-11.html".to_string()
        } else if has(ctx, CLAY_DOUGH) || has(ctx, PATTERN_OF_KEYHOLE) || has(ctx, NIKOLAS_LIST) {
            "30104-12.html".to_string()
        } else if has(ctx, MAPHR_TABLET_FRAGMENT) {
            ctx.give_adena(217682, true);
            ctx.give_items(MARK_OF_PROSPERITY, 1);
            ctx.add_exp_and_sp(1199958, 80080);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            "30104-13.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn wilford_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, RING_OF_TESTIMONY_1ST) {
        if !has(ctx, LILITHS_ELVEN_WAFER) && !has(ctx, CRYSTAL_BROOCH) {
            "30005-01.html".to_string()
        } else if has(ctx, CRYSTAL_BROOCH) && !has(ctx, LILITHS_ELVEN_WAFER) {
            "30005-05.html".to_string()
        } else if has(ctx, LILITHS_ELVEN_WAFER) {
            "30005-06.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else if has(ctx, RING_OF_TESTIMONY_2ND) {
        "30005-07.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn lilith_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, RING_OF_TESTIMONY_1ST) {
        if has(ctx, CRYSTAL_BROOCH) && !has(ctx, LILITHS_ELVEN_WAFER) {
            "30368-01.html".to_string()
        } else {
            "30368-04.html".to_string()
        }
    } else if has(ctx, RING_OF_TESTIMONY_2ND) {
        "30368-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn bright_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, RING_OF_TESTIMONY_1ST) {
        if !has(ctx, EMILYS_RECIPE) && !has(ctx, BRIGHTS_LIST) && !has(ctx, MANDRAGORA_BOUGUET) {
            "30466-01.html".to_string()
        } else if has(ctx, BRIGHTS_LIST) && !has(ctx, EMILYS_RECIPE) {
            if ctx.quest_items_count(MANDRAGORA_PETAL) < 20
                || ctx.quest_items_count(CRIMSON_MOSS) < 10
            {
                "30466-04.html".to_string()
            } else {
                ctx.take_items(BRIGHTS_LIST, 1);
                ctx.take_items(MANDRAGORA_PETAL, -1);
                ctx.take_items(CRIMSON_MOSS, -1);
                ctx.give_items(MANDRAGORA_BOUGUET, 1);
                "30466-05.html".to_string()
            }
        } else if has(ctx, MANDRAGORA_BOUGUET)
            && !has(ctx, EMILYS_RECIPE)
            && !has(ctx, BRIGHTS_LIST)
        {
            "30466-06.html".to_string()
        } else if has(ctx, EMILYS_RECIPE) {
            "30466-07.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else if has(ctx, RING_OF_TESTIMONY_2ND) {
        "30466-08.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn lockirin_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, RING_OF_TESTIMONY_1ST) {
        if !has(ctx, COLLECTION_LICENSE) && !has(ctx, OLD_ACCOUNT_BOOK) {
            "30531-01.html".to_string()
        } else if has(ctx, COLLECTION_LICENSE) {
            if has(ctx, RECEIPT_OF_CONTRIBUTION_1ST)
                && has(ctx, RECEIPT_OF_CONTRIBUTION_2ND)
                && has(ctx, RECEIPT_OF_CONTRIBUTION_3RD)
                && has(ctx, RECEIPT_OF_CONTRIBUTION_4TH)
                && has(ctx, RECEIPT_OF_CONTRIBUTION_5TH)
            {
                ctx.give_items(OLD_ACCOUNT_BOOK, 1);
                ctx.take_items(COLLECTION_LICENSE, 1);
                ctx.take_items(RECEIPT_OF_CONTRIBUTION_1ST, 1);
                ctx.take_items(RECEIPT_OF_CONTRIBUTION_2ND, 1);
                ctx.take_items(RECEIPT_OF_CONTRIBUTION_3RD, 1);
                ctx.take_items(RECEIPT_OF_CONTRIBUTION_4TH, 1);
                ctx.take_items(RECEIPT_OF_CONTRIBUTION_5TH, 1);
                ctx.play_sound(quest_sounds::MIDDLE);
                maybe_cond2(ctx);
                "30531-05.html".to_string()
            } else {
                "30531-04.html".to_string()
            }
        } else if has(ctx, OLD_ACCOUNT_BOOK) {
            "30531-06.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else if has(ctx, RING_OF_TESTIMONY_2ND) {
        "30531-07.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn spiron_talk(ctx: &mut QuestCtx) -> String {
    if !(has(ctx, RING_OF_TESTIMONY_1ST) && has(ctx, COLLECTION_LICENSE)) {
        return ctx.no_quest_html();
    }
    if has(ctx, LOCKIRINS_1ST_NOTICE)
        && !has(ctx, CONTRIBUTION_OF_SHARI)
        && !has(ctx, RECEIPT_OF_CONTRIBUTION_1ST)
    {
        ctx.take_items(LOCKIRINS_1ST_NOTICE, 1);
        "30532-01.html".to_string()
    } else if !has(ctx, RECEIPT_OF_CONTRIBUTION_1ST)
        && !has(ctx, CONTRIBUTION_OF_SHARI)
        && !has(ctx, LOCKIRINS_1ST_NOTICE)
    {
        "30532-02.html".to_string()
    } else if has(ctx, CONTRIBUTION_OF_SHARI)
        && !has(ctx, RECEIPT_OF_CONTRIBUTION_1ST)
        && !has(ctx, LOCKIRINS_1ST_NOTICE)
    {
        ctx.take_items(CONTRIBUTION_OF_SHARI, 1);
        ctx.give_items(RECEIPT_OF_CONTRIBUTION_1ST, 1);
        "30532-03.html".to_string()
    } else if has(ctx, RECEIPT_OF_CONTRIBUTION_1ST) {
        "30532-04.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn balanki_talk(ctx: &mut QuestCtx) -> String {
    if !(has(ctx, RING_OF_TESTIMONY_1ST) && has(ctx, COLLECTION_LICENSE)) {
        return ctx.no_quest_html();
    }
    let contribs =
        ctx.quest_items_count(CONTRIBUTION_OF_MION) + ctx.quest_items_count(CONTRIBUTION_OF_MARYSE);
    if has(ctx, LOCKIRINS_2ND_NOTICE) && !has(ctx, RECEIPT_OF_CONTRIBUTION_2ND) && contribs < 2 {
        ctx.take_items(LOCKIRINS_2ND_NOTICE, 1);
        "30533-01.html".to_string()
    } else if !has(ctx, LOCKIRINS_2ND_NOTICE)
        && !has(ctx, RECEIPT_OF_CONTRIBUTION_2ND)
        && contribs < 2
    {
        "30533-02.html".to_string()
    } else if !has(ctx, LOCKIRINS_2ND_NOTICE)
        && !has(ctx, RECEIPT_OF_CONTRIBUTION_2ND)
        && has(ctx, CONTRIBUTION_OF_MION)
        && has(ctx, CONTRIBUTION_OF_MARYSE)
    {
        ctx.take_items(CONTRIBUTION_OF_MION, 1);
        ctx.take_items(CONTRIBUTION_OF_MARYSE, 1);
        ctx.give_items(RECEIPT_OF_CONTRIBUTION_2ND, 1);
        "30533-03.html".to_string()
    } else if has(ctx, RECEIPT_OF_CONTRIBUTION_2ND) {
        "30533-04.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn keef_talk(ctx: &mut QuestCtx) -> String {
    if !(has(ctx, RING_OF_TESTIMONY_1ST) && has(ctx, COLLECTION_LICENSE)) {
        return ctx.no_quest_html();
    }
    if has(ctx, LOCKIRINS_3RD_NOTICE)
        && !has(ctx, RECEIPT_OF_CONTRIBUTION_3RD)
        && !has(ctx, PROCURATION_OF_TOROCCO)
    {
        ctx.take_items(LOCKIRINS_3RD_NOTICE, 1);
        "30534-01.html".to_string()
    } else if !has(ctx, RECEIPT_OF_CONTRIBUTION_3RD)
        && !has(ctx, PROCURATION_OF_TOROCCO)
        && !has(ctx, LOCKIRINS_3RD_NOTICE)
    {
        "30534-02.html".to_string()
    } else if has(ctx, PROCURATION_OF_TOROCCO)
        && !has(ctx, LOCKIRINS_3RD_NOTICE)
        && !has(ctx, RECEIPT_OF_CONTRIBUTION_3RD)
    {
        "30534-03.html".to_string()
    } else if has(ctx, RECEIPT_OF_CONTRIBUTION_3RD) {
        "30534-04.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn filaur_talk(ctx: &mut QuestCtx) -> String {
    if !(has(ctx, RING_OF_TESTIMONY_1ST) && has(ctx, COLLECTION_LICENSE)) {
        return ctx.no_quest_html();
    }
    if has(ctx, LOCKIRINS_4TH_NOTICE)
        && !has(ctx, RECEIPT_OF_CONTRIBUTION_4TH)
        && !has(ctx, RECEIPT_OF_BOLTER)
    {
        ctx.take_items(LOCKIRINS_4TH_NOTICE, 1);
        "30535-01.html".to_string()
    } else if !has(ctx, RECEIPT_OF_CONTRIBUTION_4TH)
        && !has(ctx, RECEIPT_OF_BOLTER)
        && !has(ctx, LOCKIRINS_4TH_NOTICE)
    {
        "30535-02.html".to_string()
    } else if has(ctx, RECEIPT_OF_BOLTER)
        && !has(ctx, RECEIPT_OF_CONTRIBUTION_4TH)
        && !has(ctx, LOCKIRINS_4TH_NOTICE)
    {
        ctx.take_items(RECEIPT_OF_BOLTER, 1);
        ctx.give_items(RECEIPT_OF_CONTRIBUTION_4TH, 1);
        "30535-03.html".to_string()
    } else if has(ctx, RECEIPT_OF_CONTRIBUTION_4TH) {
        "30535-04.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn arin_talk(ctx: &mut QuestCtx) -> String {
    if !(has(ctx, RING_OF_TESTIMONY_1ST) && has(ctx, COLLECTION_LICENSE)) {
        return ctx.no_quest_html();
    }
    if has(ctx, LOCKIRINS_5TH_NOTICE)
        && !has(ctx, RECEIPT_OF_CONTRIBUTION_5TH)
        && !has(ctx, CONTRIBUTION_OF_TOMA)
    {
        ctx.take_items(LOCKIRINS_5TH_NOTICE, 1);
        "30536-01.html".to_string()
    } else if !has(ctx, RECEIPT_OF_CONTRIBUTION_5TH)
        && !has(ctx, CONTRIBUTION_OF_TOMA)
        && !has(ctx, LOCKIRINS_5TH_NOTICE)
    {
        "30536-02.html".to_string()
    } else if has(ctx, CONTRIBUTION_OF_TOMA)
        && !has(ctx, RECEIPT_OF_CONTRIBUTION_5TH)
        && !has(ctx, LOCKIRINS_5TH_NOTICE)
    {
        ctx.take_items(CONTRIBUTION_OF_TOMA, 1);
        ctx.give_items(RECEIPT_OF_CONTRIBUTION_5TH, 1);
        "30536-03.html".to_string()
    } else if has(ctx, RECEIPT_OF_CONTRIBUTION_5TH) {
        "30536-04.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn maryse_talk(ctx: &mut QuestCtx) -> String {
    if !(has(ctx, RING_OF_TESTIMONY_1ST) && has(ctx, COLLECTION_LICENSE)) {
        return ctx.no_quest_html();
    }
    if !has(ctx, RECEIPT_OF_CONTRIBUTION_2ND)
        && !has(ctx, CONTRIBUTION_OF_MARYSE)
        && !has(ctx, LOCKIRINS_2ND_NOTICE)
        && !has(ctx, MARYSES_REQUEST)
    {
        ctx.give_items(MARYSES_REQUEST, 1);
        "30553-01.html".to_string()
    } else if has(ctx, MARYSES_REQUEST)
        && !has(ctx, RECEIPT_OF_CONTRIBUTION_2ND)
        && !has(ctx, CONTRIBUTION_OF_MARYSE)
        && !has(ctx, LOCKIRINS_2ND_NOTICE)
    {
        if ctx.quest_items_count(ANIMAL_SKIN) < 10 {
            "30553-02.html".to_string()
        } else {
            ctx.take_items(ANIMAL_SKIN, 10);
            ctx.give_items(CONTRIBUTION_OF_MARYSE, 1);
            ctx.take_items(MARYSES_REQUEST, 1);
            "30553-03.html".to_string()
        }
    } else if has(ctx, CONTRIBUTION_OF_MARYSE) {
        "30553-04.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn bolter_talk(ctx: &mut QuestCtx) -> String {
    if !(has(ctx, RING_OF_TESTIMONY_1ST) && has(ctx, COLLECTION_LICENSE)) {
        return ctx.no_quest_html();
    }
    if !has(ctx, RECEIPT_OF_CONTRIBUTION_4TH)
        && !has(ctx, RECEIPT_OF_BOLTER)
        && !has(ctx, LOCKIRINS_4TH_NOTICE)
    {
        ctx.give_items(RECEIPT_OF_BOLTER, 1);
        "30554-01.html".to_string()
    } else if has(ctx, RECEIPT_OF_BOLTER) {
        "30554-02.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn torocco_talk(ctx: &mut QuestCtx) -> String {
    if !(has(ctx, RING_OF_TESTIMONY_1ST) && has(ctx, COLLECTION_LICENSE)) {
        return ctx.no_quest_html();
    }
    if !has(ctx, RECEIPT_OF_CONTRIBUTION_3RD)
        && !has(ctx, PROCURATION_OF_TOROCCO)
        && !has(ctx, LOCKIRINS_3RD_NOTICE)
    {
        "30555-01.html".to_string()
    } else if has(ctx, PROCURATION_OF_TOROCCO) {
        "30555-03.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn piotur_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, RING_OF_TESTIMONY_1ST) {
        if !has(ctx, BLESSED_SEED) {
            "30597-01.html".to_string()
        } else {
            "30597-03.html".to_string()
        }
    } else if has(ctx, RING_OF_TESTIMONY_2ND) {
        "30597-04.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn emily_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, RING_OF_TESTIMONY_1ST) {
        if has(ctx, MANDRAGORA_BOUGUET) && !has(ctx, EMILYS_RECIPE) && !has(ctx, BRIGHTS_LIST) {
            "30620-01.html".to_string()
        } else if has(ctx, EMILYS_RECIPE) {
            "30620-04.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else if has(ctx, RING_OF_TESTIMONY_2ND) {
        "30620-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn nikola_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, RING_OF_TESTIMONY_2ND) {
        return ctx.no_quest_html();
    }
    if !has(ctx, CLAY_DOUGH)
        && !has(ctx, PATTERN_OF_KEYHOLE)
        && !has(ctx, NIKOLAS_LIST)
        && !has(ctx, MAPHR_TABLET_FRAGMENT)
    {
        ctx.take_items(PARMANS_LETTER, 1);
        "30621-01.html".to_string()
    } else if has(ctx, CLAY_DOUGH) {
        "30621-05.html".to_string()
    } else if has(ctx, PATTERN_OF_KEYHOLE) {
        ctx.give_items(RECIPE_TITAN_KEY, 1);
        ctx.take_items(PATTERN_OF_KEYHOLE, 1);
        ctx.give_items(NIKOLAS_LIST, 1);
        ctx.set_cond(7, true);
        "30621-06.html".to_string()
    } else if has(ctx, NIKOLAS_LIST) && !has(ctx, KEY_OF_TITAN) {
        "30621-07.html".to_string()
    } else if has(ctx, NIKOLAS_LIST) && has(ctx, KEY_OF_TITAN) {
        "30621-08.html".to_string()
    } else if has(ctx, MAPHR_TABLET_FRAGMENT) {
        "30621-09.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn box_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, RING_OF_TESTIMONY_2ND) {
        return ctx.no_quest_html();
    }
    if has(ctx, CLAY_DOUGH) && !has(ctx, PATTERN_OF_KEYHOLE) {
        "30622-01.html".to_string()
    } else if has(ctx, KEY_OF_TITAN) && !has(ctx, MAPHR_TABLET_FRAGMENT) {
        "30622-03.html".to_string()
    } else if !has(ctx, KEY_OF_TITAN) && !has(ctx, CLAY_DOUGH) {
        "30622-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}
