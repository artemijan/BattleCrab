//! Test of Magus (228) — `quests/Q00228_TestOfMagus`. The wizard 2nd-class
//! proof (Wizard / Elven Wizard / Dark Wizard, level 39+). Bard Rukal sends the
//! mage to Parina and Elder Casian for the Lilac Charm, three Golden Seeds from
//! the Singing Flowers, then the Score of Elements — after which the four
//! elemental spirits (Serpent/Earth, Salamander/Fire, Sylph/Wind, Undine/Water)
//! each trade a charm-gathered pile of reagents for an elemental Tone. All four
//! Tones earn the Mark of Magus.
//!
//! Pure item-gate (cond 1..6). Each element is an independent charm→gather→tone
//! loop; the final cond fires when the fourth Tone is turned in.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const PARINA: i32 = 30391;
const EARTH_SNAKE: i32 = 30409;
const FLAME_SALAMANDER: i32 = 30411;
const WIND_SYLPH: i32 = 30412;
const WATER_UNDINE: i32 = 30413;
const ELDER_CASIAN: i32 = 30612;
const BARD_RUKAL: i32 = 30629;
// Items
const RUKALS_LETTER: i32 = 2841;
const PARINAS_LETTER: i32 = 2842;
const LILAC_CHARM: i32 = 2843;
const GOLDEN_SEED_1ST: i32 = 2844;
const GOLDEN_SEED_2ND: i32 = 2845;
const GOLDEN_SEED_3RD: i32 = 2846;
const SCORE_OF_ELEMENTS: i32 = 2847;
const DAZZLING_DROP: i32 = 2848;
const FLAME_CRYSTAL: i32 = 2849;
const HARPYS_FEATHER: i32 = 2850;
const WYRMS_WINGBONE: i32 = 2851;
const WINDSUS_MANE: i32 = 2852;
const ENCHANTED_MONSTER_EYE_SHELL: i32 = 2853;
const ENCHANTED_GOLEM_POWDER: i32 = 2854;
const ENCHANTED_IRON_GOLEM_SCRAP: i32 = 2855;
const TONE_OF_WATER: i32 = 2856;
const TONE_OF_FIRE: i32 = 2857;
const TONE_OF_WIND: i32 = 2858;
const TONE_OF_EARTH: i32 = 2859;
const SALAMANDER_CHARM: i32 = 2860;
const SYLPH_CHARM: i32 = 2861;
const UNDINE_CHARM: i32 = 2862;
const SERPENT_CHARM: i32 = 2863;
// Reward
const MARK_OF_MAGUS: i32 = 2840;
// Monsters
const HARPY: i32 = 20145;
const MARSH_STAKATO: i32 = 20157;
const WYRM: i32 = 20176;
const MARSH_STAKATO_WORKER: i32 = 20230;
const TOAD_LORD: i32 = 20231;
const MARSH_STAKATO_SOLDIER: i32 = 20232;
const MARSH_STAKATO_DRONE: i32 = 20234;
const WINDSUS: i32 = 20553;
const ENCHANTED_MONSTEREYE: i32 = 20564;
const ENCHANTED_STOLEN_GOLEM: i32 = 20565;
const ENCHANTED_IRON_GOLEM: i32 = 20566;
const SINGING_FLOWER_PHANTASM: i32 = 27095;
const SINGING_FLOWER_NIGTMATE: i32 = 27096;
const SINGING_FLOWER_DARKLING: i32 = 27097;
const GHOST_FIRE: i32 = 27098;
// Misc
const MIN_LEVEL: i32 = 39;
const WIZARD: i32 = 11;
const ELVEN_WIZARD: i32 = 26;
const DARK_WIZARD: i32 = 39;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// Give one reagent up to `cap`; play the "milestone" sound at the cap.
fn gather(ctx: &mut QuestCtx, item: i32, cap: i64) {
    ctx.give_items(item, 1);
    if ctx.quest_items_count(item) >= cap {
        ctx.play_sound(quest_sounds::MIDDLE);
    } else {
        ctx.play_sound(quest_sounds::ITEMGET);
    }
}

/// A Golden Seed drop off a Singing Flower; advances to cond 4 once all three
/// seeds are held.
fn seed(ctx: &mut QuestCtx, own: i32, a: i32, b: i32) {
    if has(ctx, LILAC_CHARM) && ctx.award_once(own) && has(ctx, a) && has(ctx, b) {
        ctx.set_cond(4, false);
    }
}

/// After turning in an elemental Tone, advance to cond 6 once all four are held.
fn maybe_cond6(ctx: &mut QuestCtx, a: i32, b: i32, c: i32) {
    if has(ctx, a) && has(ctx, b) && has(ctx, c) {
        ctx.set_cond(6, true);
    }
}

pub struct Q00228TestOfMagus;

impl QuestScript for Q00228TestOfMagus {
    fn id(&self) -> i32 {
        228
    }
    fn name(&self) -> &'static str {
        "Q00228_TestOfMagus"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00228_TestOfMagus"
    }
    fn start_npcs(&self) -> &[i32] {
        &[BARD_RUKAL]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            BARD_RUKAL,
            PARINA,
            EARTH_SNAKE,
            FLAME_SALAMANDER,
            WIND_SYLPH,
            WATER_UNDINE,
            ELDER_CASIAN,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            HARPY,
            MARSH_STAKATO,
            WYRM,
            MARSH_STAKATO_WORKER,
            TOAD_LORD,
            MARSH_STAKATO_SOLDIER,
            MARSH_STAKATO_DRONE,
            WINDSUS,
            ENCHANTED_MONSTEREYE,
            ENCHANTED_STOLEN_GOLEM,
            ENCHANTED_IRON_GOLEM,
            SINGING_FLOWER_PHANTASM,
            SINGING_FLOWER_NIGTMATE,
            SINGING_FLOWER_DARKLING,
            GHOST_FIRE,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            RUKALS_LETTER,
            PARINAS_LETTER,
            LILAC_CHARM,
            GOLDEN_SEED_1ST,
            GOLDEN_SEED_2ND,
            GOLDEN_SEED_3RD,
            SCORE_OF_ELEMENTS,
            DAZZLING_DROP,
            FLAME_CRYSTAL,
            HARPYS_FEATHER,
            WYRMS_WINGBONE,
            WINDSUS_MANE,
            ENCHANTED_MONSTER_EYE_SHELL,
            ENCHANTED_GOLEM_POWDER,
            ENCHANTED_IRON_GOLEM_SCRAP,
            TONE_OF_WATER,
            TONE_OF_FIRE,
            TONE_OF_WIND,
            TONE_OF_EARTH,
            SALAMANDER_CHARM,
            SYLPH_CHARM,
            UNDINE_CHARM,
            SERPENT_CHARM,
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
                    ctx.give_items(RUKALS_LETTER, 1);
                }
                None
            }
            "30629-09.html" | "30409-02.html" => Some(event.to_string()),
            "30629-10.html" => {
                if has(ctx, GOLDEN_SEED_3RD) {
                    ctx.take_items(LILAC_CHARM, 1);
                    ctx.take_items(GOLDEN_SEED_1ST, 1);
                    ctx.take_items(GOLDEN_SEED_2ND, 1);
                    ctx.take_items(GOLDEN_SEED_3RD, 1);
                    ctx.give_items(SCORE_OF_ELEMENTS, 1);
                    ctx.set_cond(5, true);
                    return Some(event.to_string());
                }
                None
            }
            "30391-02.html" => ctx
                .swap_quest_item(RUKALS_LETTER, PARINAS_LETTER, 2)
                .then(|| event.to_string()),
            "30409-03.html" => {
                ctx.give_items(SERPENT_CHARM, 1);
                Some(event.to_string())
            }
            "30412-02.html" => {
                ctx.give_items(SYLPH_CHARM, 1);
                Some(event.to_string())
            }
            "30612-02.html" => {
                ctx.take_items(PARINAS_LETTER, 1);
                ctx.give_items(LILAC_CHARM, 1);
                ctx.set_cond(3, true);
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
            HARPY => {
                if has(ctx, SCORE_OF_ELEMENTS)
                    && has(ctx, SYLPH_CHARM)
                    && ctx.quest_items_count(HARPYS_FEATHER) < 20
                {
                    gather(ctx, HARPYS_FEATHER, 20);
                }
            }
            MARSH_STAKATO
            | MARSH_STAKATO_WORKER
            | TOAD_LORD
            | MARSH_STAKATO_SOLDIER
            | MARSH_STAKATO_DRONE => {
                if has(ctx, SCORE_OF_ELEMENTS)
                    && has(ctx, UNDINE_CHARM)
                    && ctx.quest_items_count(DAZZLING_DROP) < 20
                {
                    gather(ctx, DAZZLING_DROP, 20);
                }
            }
            WYRM => {
                if has(ctx, SCORE_OF_ELEMENTS)
                    && has(ctx, SYLPH_CHARM)
                    && ctx.quest_items_count(WYRMS_WINGBONE) < 10
                    && ctx.roll(2) == 0
                {
                    gather(ctx, WYRMS_WINGBONE, 10);
                }
            }
            WINDSUS => {
                if has(ctx, SCORE_OF_ELEMENTS)
                    && has(ctx, SYLPH_CHARM)
                    && ctx.quest_items_count(WINDSUS_MANE) < 10
                    && ctx.roll(2) == 0
                {
                    gather(ctx, WINDSUS_MANE, 10);
                }
            }
            ENCHANTED_MONSTEREYE => {
                if has(ctx, SCORE_OF_ELEMENTS)
                    && has(ctx, SERPENT_CHARM)
                    && ctx.quest_items_count(ENCHANTED_MONSTER_EYE_SHELL) < 10
                {
                    gather(ctx, ENCHANTED_MONSTER_EYE_SHELL, 10);
                }
            }
            ENCHANTED_STOLEN_GOLEM => {
                if has(ctx, SCORE_OF_ELEMENTS)
                    && has(ctx, SERPENT_CHARM)
                    && ctx.quest_items_count(ENCHANTED_GOLEM_POWDER) < 10
                {
                    gather(ctx, ENCHANTED_GOLEM_POWDER, 10);
                }
            }
            ENCHANTED_IRON_GOLEM => {
                if has(ctx, SCORE_OF_ELEMENTS)
                    && has(ctx, SERPENT_CHARM)
                    && ctx.quest_items_count(ENCHANTED_IRON_GOLEM_SCRAP) < 10
                {
                    gather(ctx, ENCHANTED_IRON_GOLEM_SCRAP, 10);
                }
            }
            SINGING_FLOWER_PHANTASM => seed(ctx, GOLDEN_SEED_1ST, GOLDEN_SEED_2ND, GOLDEN_SEED_3RD),
            SINGING_FLOWER_NIGTMATE => seed(ctx, GOLDEN_SEED_2ND, GOLDEN_SEED_1ST, GOLDEN_SEED_3RD),
            SINGING_FLOWER_DARKLING => seed(ctx, GOLDEN_SEED_3RD, GOLDEN_SEED_1ST, GOLDEN_SEED_2ND),
            GHOST_FIRE
                if has(ctx, SCORE_OF_ELEMENTS)
                    && has(ctx, SALAMANDER_CHARM)
                    && ctx.quest_items_count(FLAME_CRYSTAL) < 5
                    && ctx.roll(2) == 0 =>
            {
                gather(ctx, FLAME_CRYSTAL, 5);
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == BARD_RUKAL {
                let class = ctx.player_class_id();
                if class == WIZARD || class == ELVEN_WIZARD || class == DARK_WIZARD {
                    return Some(if ctx.player_level() < MIN_LEVEL {
                        "30629-02.html".to_string()
                    } else {
                        "30629-03.htm".to_string()
                    });
                }
                return Some("30629-01.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == BARD_RUKAL {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            BARD_RUKAL => Some(rukal_talk(ctx)),
            PARINA => Some(parina_talk(ctx)),
            EARTH_SNAKE => Some(earth_talk(ctx)),
            FLAME_SALAMANDER => Some(flame_talk(ctx)),
            WIND_SYLPH => Some(sylph_talk(ctx)),
            WATER_UNDINE => Some(undine_talk(ctx)),
            ELDER_CASIAN => Some(casian_talk(ctx)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

fn rukal_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, RUKALS_LETTER) {
        "30629-05.html".to_string()
    } else if has(ctx, PARINAS_LETTER) {
        "30629-06.html".to_string()
    } else if has(ctx, LILAC_CHARM) {
        if has(ctx, GOLDEN_SEED_1ST) && has(ctx, GOLDEN_SEED_2ND) && has(ctx, GOLDEN_SEED_3RD) {
            "30629-08.html".to_string()
        } else {
            "30629-07.html".to_string()
        }
    } else if has(ctx, SCORE_OF_ELEMENTS) {
        if has(ctx, TONE_OF_WATER)
            && has(ctx, TONE_OF_FIRE)
            && has(ctx, TONE_OF_WIND)
            && has(ctx, TONE_OF_EARTH)
        {
            ctx.give_adena(372154, true);
            ctx.give_items(MARK_OF_MAGUS, 1);
            ctx.add_exp_and_sp(2058244, 141240);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            "30629-12.html".to_string()
        } else {
            "30629-11.html".to_string()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn parina_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, RUKALS_LETTER) {
        "30391-01.html".to_string()
    } else if has(ctx, PARINAS_LETTER) {
        "30391-03.html".to_string()
    } else if has(ctx, LILAC_CHARM) {
        "30391-04.html".to_string()
    } else if has(ctx, SCORE_OF_ELEMENTS) {
        "30391-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn earth_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, SCORE_OF_ELEMENTS) {
        return ctx.no_quest_html();
    }
    if !has(ctx, TONE_OF_EARTH) && !has(ctx, SERPENT_CHARM) {
        "30409-01.html".to_string()
    } else if has(ctx, SERPENT_CHARM) {
        if ctx.quest_items_count(ENCHANTED_MONSTER_EYE_SHELL) >= 10
            && ctx.quest_items_count(ENCHANTED_GOLEM_POWDER) >= 10
            && ctx.quest_items_count(ENCHANTED_IRON_GOLEM_SCRAP) >= 10
        {
            ctx.take_items(ENCHANTED_MONSTER_EYE_SHELL, -1);
            ctx.take_items(ENCHANTED_GOLEM_POWDER, -1);
            ctx.take_items(ENCHANTED_IRON_GOLEM_SCRAP, -1);
            ctx.give_items(TONE_OF_EARTH, 1);
            ctx.take_items(SERPENT_CHARM, 1);
            maybe_cond6(ctx, TONE_OF_FIRE, TONE_OF_WATER, TONE_OF_WIND);
            "30409-05.html".to_string()
        } else {
            "30409-04.html".to_string()
        }
    } else {
        "30409-06.html".to_string()
    }
}

fn flame_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, SCORE_OF_ELEMENTS) {
        return ctx.no_quest_html();
    }
    if !has(ctx, TONE_OF_FIRE) && !has(ctx, SALAMANDER_CHARM) {
        ctx.give_items(SALAMANDER_CHARM, 1);
        "30411-01.html".to_string()
    } else if has(ctx, SALAMANDER_CHARM) {
        if ctx.quest_items_count(FLAME_CRYSTAL) < 5 {
            "30411-02.html".to_string()
        } else {
            ctx.take_items(FLAME_CRYSTAL, -1);
            ctx.give_items(TONE_OF_FIRE, 1);
            ctx.take_items(SALAMANDER_CHARM, 1);
            maybe_cond6(ctx, TONE_OF_WATER, TONE_OF_WIND, TONE_OF_EARTH);
            "30411-03.html".to_string()
        }
    } else {
        "30411-04.html".to_string()
    }
}

fn sylph_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, SCORE_OF_ELEMENTS) {
        return ctx.no_quest_html();
    }
    if !has(ctx, TONE_OF_WIND) && !has(ctx, SYLPH_CHARM) {
        "30412-01.html".to_string()
    } else if has(ctx, SYLPH_CHARM) {
        if ctx.quest_items_count(HARPYS_FEATHER) >= 20
            && ctx.quest_items_count(WYRMS_WINGBONE) >= 10
            && ctx.quest_items_count(WINDSUS_MANE) >= 10
        {
            ctx.take_items(HARPYS_FEATHER, -1);
            ctx.take_items(WYRMS_WINGBONE, -1);
            ctx.take_items(WINDSUS_MANE, -1);
            ctx.give_items(TONE_OF_WIND, 1);
            ctx.take_items(SYLPH_CHARM, 1);
            maybe_cond6(ctx, TONE_OF_WATER, TONE_OF_FIRE, TONE_OF_EARTH);
            "30412-04.html".to_string()
        } else {
            "30412-03.html".to_string()
        }
    } else {
        "30412-05.html".to_string()
    }
}

fn undine_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, SCORE_OF_ELEMENTS) {
        return ctx.no_quest_html();
    }
    if !has(ctx, TONE_OF_WATER) && !has(ctx, UNDINE_CHARM) {
        ctx.give_items(UNDINE_CHARM, 1);
        "30413-01.html".to_string()
    } else if has(ctx, UNDINE_CHARM) {
        if ctx.quest_items_count(DAZZLING_DROP) < 20 {
            "30413-02.html".to_string()
        } else {
            ctx.take_items(DAZZLING_DROP, -1);
            ctx.give_items(TONE_OF_WATER, 1);
            ctx.take_items(UNDINE_CHARM, 1);
            maybe_cond6(ctx, TONE_OF_FIRE, TONE_OF_WIND, TONE_OF_EARTH);
            "30413-03.html".to_string()
        }
    } else {
        "30413-04.html".to_string()
    }
}

fn casian_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, PARINAS_LETTER) {
        "30612-01.html".to_string()
    } else if has(ctx, LILAC_CHARM) {
        if has(ctx, GOLDEN_SEED_1ST) && has(ctx, GOLDEN_SEED_2ND) && has(ctx, GOLDEN_SEED_3RD) {
            "30612-04.html".to_string()
        } else {
            "30612-03.html".to_string()
        }
    } else if has(ctx, SCORE_OF_ELEMENTS) {
        "30612-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}
