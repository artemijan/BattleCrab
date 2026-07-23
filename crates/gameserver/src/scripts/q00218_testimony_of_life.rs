//! Testimony of Life (218) — `quests/Q00218_TestimonyOfLife`. The Elf 2nd-class
//! prerequisite (Elf race, `ELF_2ND_GROUP`, level 37+). Master Cardien starts a
//! long alchemical errand to brew the Water of Life: forge a Pure Mithril Cup
//! (Pushkin), copy Andariel's scripture (Arkenia/Adonius), reassemble Talin's
//! Spear from six pieces (Isael), and slay the Unicorn of Eva *with that spear*
//! for its tears — distilled by Thalia and blessed by Asterios into the Camomile
//! Charm and the Mark of Life.
//!
//! Item-gated (cond 1..21). Several capped gather legs cross-gate their cond;
//! the Unicorn only yields its tears to a killing blow struck with Talin's Spear
//! (Java `getKillingBlowWeapon`, approximated by the killer's equipped weapon).

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const HIERARCH_ASTERIOS: i32 = 30154;
const BLACKSMITH_PUSHKIN: i32 = 30300;
const THALIA: i32 = 30371;
const PRIEST_ADONIUS: i32 = 30375;
const ARKENIA: i32 = 30419;
const MASTER_CARDIEN: i32 = 30460;
const ISAEL_SILVERSHADOW: i32 = 30655;
// Items
const TALINS_SPEAR: i32 = 3026;
const CARDIENS_LETTER: i32 = 3141;
const CAMOMILE_CHARM: i32 = 3142;
const HIERARCHS_LETTER: i32 = 3143;
const MOONFLOWER_CHARM: i32 = 3144;
const GRAIL_DIAGRAM: i32 = 3145;
const THALIAS_1ST_LETTER: i32 = 3146;
const THALIAS_2ND_LETTER: i32 = 3147;
const THALIAS_INSTRUCTIONS: i32 = 3148;
const PUSHKINS_LIST: i32 = 3149;
const PURE_MITHRIL_CUP: i32 = 3150;
const ARKENIAS_CONTRACT: i32 = 3151;
const ARKENIAS_INSTRUCTIONS: i32 = 3152;
const ADONIUS_LIST: i32 = 3153;
const ANDARIEL_SCRIPTURE_COPY: i32 = 3154;
const STARDUST: i32 = 3155;
const ISAELS_INSTRUCTIONS: i32 = 3156;
const ISAELS_LETTER: i32 = 3157;
const GRAIL_OF_PURITY: i32 = 3158;
const TEARS_OF_UNICORN: i32 = 3159;
const WATER_OF_LIFE: i32 = 3160;
const PURE_MITHRIL_ORE: i32 = 3161;
const ANT_SOLDIER_ACID: i32 = 3162;
const WYRMS_TALON: i32 = 3163;
const SPIDER_ICHOR: i32 = 3164;
const HARPYS_DOWN: i32 = 3165;
const TALINS_SPEAR_BLADE: i32 = 3166;
const TALINS_SPEAR_SHAFT: i32 = 3167;
const TALINS_RUBY: i32 = 3168;
const TALINS_AQUAMARINE: i32 = 3169;
const TALINS_AMETHYST: i32 = 3170;
const TALINS_PERIDOT: i32 = 3171;
// Reward
const MARK_OF_LIFE: i32 = 3140;
// Monsters
const ANT_RECRUIT: i32 = 20082;
const ANT_PATROL: i32 = 20084;
const ANT_GUARD: i32 = 20086;
const ANT_SOLDIER: i32 = 20087;
const ANT_WARRIOR_CAPTAIN: i32 = 20088;
const HARPY: i32 = 20145;
const WYRM: i32 = 20176;
const MARSH_SPIDER: i32 = 20233;
const GUARDIAN_BASILISK: i32 = 20550;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const UNICORN_OF_EVA: i32 = 27077;
// Misc
const MIN_LEVEL: i32 = 37;
const LEVEL: i32 = 38;
const RACE_ELF: i32 = 1;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// A capped gather leg gated on `list`: give `amount` of `own` up to `cap`, and
/// once `own` and both `others` reach their caps advance to `cond`.
fn gather(
    ctx: &mut QuestCtx,
    list: i32,
    own: i32,
    amount: i64,
    cap: i64,
    others: &[(i32, i64)],
    cond: i32,
) {
    if has(ctx, MOONFLOWER_CHARM) && has(ctx, list) && ctx.quest_items_count(own) < cap {
        ctx.give_items(own, amount);
        if ctx.quest_items_count(own) == cap {
            ctx.play_sound(quest_sounds::MIDDLE);
            if others.iter().all(|&(o, c)| ctx.quest_items_count(o) >= c) {
                ctx.set_cond(cond, false);
            }
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}

pub struct Q00218TestimonyOfLife;

impl QuestScript for Q00218TestimonyOfLife {
    fn id(&self) -> i32 {
        218
    }
    fn name(&self) -> &'static str {
        "Q00218_TestimonyOfLife"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00218_TestimonyOfLife"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MASTER_CARDIEN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            MASTER_CARDIEN,
            HIERARCH_ASTERIOS,
            BLACKSMITH_PUSHKIN,
            THALIA,
            PRIEST_ADONIUS,
            ARKENIA,
            ISAEL_SILVERSHADOW,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            ANT_RECRUIT,
            ANT_PATROL,
            ANT_GUARD,
            ANT_SOLDIER,
            ANT_WARRIOR_CAPTAIN,
            HARPY,
            WYRM,
            MARSH_SPIDER,
            GUARDIAN_BASILISK,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OVERLORD,
            UNICORN_OF_EVA,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            TALINS_SPEAR,
            CARDIENS_LETTER,
            CAMOMILE_CHARM,
            HIERARCHS_LETTER,
            MOONFLOWER_CHARM,
            GRAIL_DIAGRAM,
            THALIAS_1ST_LETTER,
            THALIAS_2ND_LETTER,
            THALIAS_INSTRUCTIONS,
            PUSHKINS_LIST,
            PURE_MITHRIL_CUP,
            ARKENIAS_CONTRACT,
            ARKENIAS_INSTRUCTIONS,
            ADONIUS_LIST,
            ANDARIEL_SCRIPTURE_COPY,
            STARDUST,
            ISAELS_INSTRUCTIONS,
            ISAELS_LETTER,
            GRAIL_OF_PURITY,
            TEARS_OF_UNICORN,
            WATER_OF_LIFE,
            PURE_MITHRIL_ORE,
            ANT_SOLDIER_ACID,
            WYRMS_TALON,
            SPIDER_ICHOR,
            HARPYS_DOWN,
            TALINS_SPEAR_BLADE,
            TALINS_SPEAR_SHAFT,
            TALINS_RUBY,
            TALINS_AQUAMARINE,
            TALINS_AMETHYST,
            TALINS_PERIDOT,
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
                    if !has(ctx, CARDIENS_LETTER) {
                        ctx.give_items(CARDIENS_LETTER, 1);
                    }
                    ctx.play_sound(quest_sounds::MIDDLE);
                    return Some("30460-04.htm".to_string());
                }
                None
            }
            "30154-02.html" | "30154-03.html" | "30154-04.html" | "30154-05.html"
            | "30154-06.html" | "30300-02.html" | "30300-03.html" | "30300-04.html"
            | "30300-05.html" | "30300-09.html" | "30300-07a.html" | "30371-02.html"
            | "30371-10.html" | "30419-02.html" | "30419-03.html" => Some(event.to_string()),
            "30154-07.html" => {
                if has(ctx, CARDIENS_LETTER) {
                    ctx.take_items(CARDIENS_LETTER, 1);
                    ctx.give_items(HIERARCHS_LETTER, 1);
                    ctx.give_items(MOONFLOWER_CHARM, 1);
                    ctx.set_cond(2, true);
                    return Some(event.to_string());
                }
                None
            }
            "30300-06.html" => {
                if has(ctx, GRAIL_DIAGRAM) {
                    ctx.take_items(GRAIL_DIAGRAM, 1);
                    ctx.give_items(PUSHKINS_LIST, 1);
                    ctx.set_cond(4, true);
                    return Some(event.to_string());
                }
                None
            }
            "30300-10.html" => {
                if has(ctx, PUSHKINS_LIST) {
                    ctx.take_items(PUSHKINS_LIST, 1);
                    ctx.give_items(PURE_MITHRIL_CUP, 1);
                    ctx.take_items(PURE_MITHRIL_ORE, -1);
                    ctx.take_items(ANT_SOLDIER_ACID, -1);
                    ctx.take_items(WYRMS_TALON, -1);
                    ctx.set_cond(6, true);
                    return Some(event.to_string());
                }
                None
            }
            "30371-03.html" => {
                if has(ctx, HIERARCHS_LETTER) {
                    ctx.take_items(HIERARCHS_LETTER, 1);
                    ctx.give_items(GRAIL_DIAGRAM, 1);
                    ctx.set_cond(3, true);
                    return Some(event.to_string());
                }
                None
            }
            "30371-11.html" => {
                if has(ctx, STARDUST) {
                    ctx.give_items(THALIAS_2ND_LETTER, 1);
                    ctx.take_items(STARDUST, 1);
                    ctx.set_cond(14, true);
                    return Some(event.to_string());
                }
                None
            }
            "30419-04.html" => {
                if has(ctx, THALIAS_1ST_LETTER) {
                    ctx.take_items(THALIAS_1ST_LETTER, 1);
                    ctx.give_items(ARKENIAS_CONTRACT, 1);
                    ctx.give_items(ARKENIAS_INSTRUCTIONS, 1);
                    ctx.set_cond(8, true);
                    return Some(event.to_string());
                }
                None
            }
            "30375-02.html" => {
                if has(ctx, ARKENIAS_INSTRUCTIONS) {
                    ctx.take_items(ARKENIAS_INSTRUCTIONS, 1);
                    ctx.give_items(ADONIUS_LIST, 1);
                    ctx.set_cond(9, true);
                    return Some(event.to_string());
                }
                None
            }
            "30655-02.html" => {
                if has(ctx, THALIAS_2ND_LETTER) {
                    ctx.take_items(THALIAS_2ND_LETTER, 1);
                    ctx.give_items(ISAELS_INSTRUCTIONS, 1);
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
        match ctx.npc_id {
            ANT_RECRUIT | ANT_PATROL | ANT_GUARD | ANT_SOLDIER | ANT_WARRIOR_CAPTAIN => gather(
                ctx,
                PUSHKINS_LIST,
                ANT_SOLDIER_ACID,
                2,
                20,
                &[(PURE_MITHRIL_ORE, 10), (WYRMS_TALON, 20)],
                5,
            ),
            WYRM => gather(
                ctx,
                PUSHKINS_LIST,
                WYRMS_TALON,
                4,
                20,
                &[(PURE_MITHRIL_ORE, 10), (ANT_SOLDIER_ACID, 20)],
                5,
            ),
            GUARDIAN_BASILISK => gather(
                ctx,
                PUSHKINS_LIST,
                PURE_MITHRIL_ORE,
                2,
                10,
                &[(WYRMS_TALON, 20), (ANT_SOLDIER_ACID, 20)],
                5,
            ),
            HARPY => gather(
                ctx,
                ADONIUS_LIST,
                HARPYS_DOWN,
                4,
                20,
                &[(SPIDER_ICHOR, 20)],
                10,
            ),
            MARSH_SPIDER => gather(
                ctx,
                ADONIUS_LIST,
                SPIDER_ICHOR,
                4,
                20,
                &[(HARPYS_DOWN, 20)],
                10,
            ),
            LETO_LIZARDMAN_SHAMAN | LETO_LIZARDMAN_OVERLORD => {
                if has(ctx, ISAELS_INSTRUCTIONS) {
                    for part in [
                        TALINS_SPEAR_BLADE,
                        TALINS_SPEAR_SHAFT,
                        TALINS_RUBY,
                        TALINS_AQUAMARINE,
                        TALINS_AMETHYST,
                        TALINS_PERIDOT,
                    ] {
                        if !has(ctx, part) {
                            ctx.give_items(part, 1);
                            ctx.play_sound(quest_sounds::MIDDLE);
                            break;
                        }
                    }
                }
            }
            UNICORN_OF_EVA => {
                if !has(ctx, TEARS_OF_UNICORN)
                    && has(ctx, MOONFLOWER_CHARM)
                    && has(ctx, TALINS_SPEAR)
                    && has(ctx, GRAIL_OF_PURITY)
                    && ctx.equipped_weapon_id() == TALINS_SPEAR
                {
                    ctx.take_items(TALINS_SPEAR, 1);
                    ctx.take_items(GRAIL_OF_PURITY, 1);
                    ctx.give_items(TEARS_OF_UNICORN, 1);
                    ctx.set_cond(19, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == MASTER_CARDIEN {
                if ctx.player_race() != RACE_ELF {
                    return Some("30460-01.html".to_string());
                } else if ctx.player_level() < MIN_LEVEL {
                    return Some("30460-02.html".to_string());
                } else if ctx.is_in_category("ELF_2ND_GROUP") {
                    return Some("30460-03.htm".to_string());
                }
                return Some("30460-01a.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == MASTER_CARDIEN {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            MASTER_CARDIEN => Some(cardien_talk(ctx)),
            HIERARCH_ASTERIOS => Some(asterios_talk(ctx)),
            BLACKSMITH_PUSHKIN => Some(pushkin_talk(ctx)),
            THALIA => Some(thalia_talk(ctx)),
            ARKENIA => Some(arkenia_talk(ctx)),
            PRIEST_ADONIUS => Some(adonius_talk(ctx)),
            ISAEL_SILVERSHADOW => Some(isael_talk(ctx)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

fn cardien_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, CARDIENS_LETTER) {
        "30460-05.html".to_string()
    } else if has(ctx, MOONFLOWER_CHARM) {
        "30460-06.html".to_string()
    } else if has(ctx, CAMOMILE_CHARM) {
        ctx.give_adena(342288, true);
        ctx.give_items(MARK_OF_LIFE, 1);
        ctx.add_exp_and_sp(1886832, 125918);
        ctx.exit_quest(false, true);
        ctx.social_action(3);
        "30460-07.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn asterios_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, CARDIENS_LETTER) {
        "30154-01.html".to_string()
    } else if has(ctx, MOONFLOWER_CHARM) {
        if !has(ctx, WATER_OF_LIFE) {
            "30154-08.html".to_string()
        } else {
            ctx.give_items(CAMOMILE_CHARM, 1);
            ctx.take_items(MOONFLOWER_CHARM, 1);
            ctx.take_items(WATER_OF_LIFE, 1);
            ctx.set_cond(21, true);
            "30154-09.html".to_string()
        }
    } else if has(ctx, CAMOMILE_CHARM) {
        "30154-10.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn pushkin_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, MOONFLOWER_CHARM) {
        return ctx.no_quest_html();
    }
    if has(ctx, GRAIL_DIAGRAM) {
        "30300-01.html".to_string()
    } else if has(ctx, PUSHKINS_LIST) {
        if ctx.quest_items_count(PURE_MITHRIL_ORE) >= 10
            && ctx.quest_items_count(ANT_SOLDIER_ACID) >= 20
            && ctx.quest_items_count(WYRMS_TALON) >= 20
        {
            "30300-08.html".to_string()
        } else {
            "30300-07.html".to_string()
        }
    } else if has(ctx, PURE_MITHRIL_CUP) {
        "30300-11.html".to_string()
    } else {
        "30300-12.html".to_string()
    }
}

fn thalia_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, MOONFLOWER_CHARM) {
        return ctx.no_quest_html();
    }
    if has(ctx, HIERARCHS_LETTER) {
        "30371-01.html".to_string()
    } else if has(ctx, GRAIL_DIAGRAM) {
        "30371-04.html".to_string()
    } else if has(ctx, PUSHKINS_LIST) {
        "30371-05.html".to_string()
    } else if has(ctx, PURE_MITHRIL_CUP) {
        ctx.give_items(THALIAS_1ST_LETTER, 1);
        ctx.take_items(PURE_MITHRIL_CUP, 1);
        ctx.set_cond(7, true);
        "30371-06.html".to_string()
    } else if has(ctx, THALIAS_1ST_LETTER) {
        "30371-07.html".to_string()
    } else if has(ctx, ARKENIAS_CONTRACT) {
        "30371-08.html".to_string()
    } else if has(ctx, STARDUST) {
        "30371-09.html".to_string()
    } else if has(ctx, THALIAS_INSTRUCTIONS) {
        if ctx.player_level() >= LEVEL {
            ctx.take_items(THALIAS_INSTRUCTIONS, 1);
            ctx.give_items(THALIAS_2ND_LETTER, 1);
            ctx.set_cond(14, true);
            "30371-13.html".to_string()
        } else {
            "30371-12.html".to_string()
        }
    } else if has(ctx, THALIAS_2ND_LETTER) {
        "30371-14.html".to_string()
    } else if has(ctx, ISAELS_INSTRUCTIONS) {
        "30371-15.html".to_string()
    } else if has(ctx, TALINS_SPEAR) && has(ctx, ISAELS_LETTER) {
        ctx.take_items(ISAELS_LETTER, 1);
        ctx.give_items(GRAIL_OF_PURITY, 1);
        ctx.set_cond(18, true);
        "30371-16.html".to_string()
    } else if has(ctx, TALINS_SPEAR) && has(ctx, GRAIL_OF_PURITY) {
        "30371-17.html".to_string()
    } else if has(ctx, TEARS_OF_UNICORN) {
        ctx.take_items(TEARS_OF_UNICORN, 1);
        ctx.give_items(WATER_OF_LIFE, 1);
        ctx.set_cond(20, true);
        "30371-18.html".to_string()
    } else if has(ctx, CAMOMILE_CHARM) || has(ctx, WATER_OF_LIFE) {
        "30371-19.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn arkenia_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, MOONFLOWER_CHARM) {
        return ctx.no_quest_html();
    }
    if has(ctx, THALIAS_1ST_LETTER) {
        "30419-01.html".to_string()
    } else if has(ctx, ARKENIAS_INSTRUCTIONS) || has(ctx, ADONIUS_LIST) {
        "30419-05.html".to_string()
    } else if has(ctx, ANDARIEL_SCRIPTURE_COPY) {
        ctx.take_items(ARKENIAS_CONTRACT, 1);
        ctx.take_items(ANDARIEL_SCRIPTURE_COPY, 1);
        ctx.give_items(STARDUST, 1);
        ctx.set_cond(12, true);
        "30419-06.html".to_string()
    } else if has(ctx, STARDUST) {
        "30419-07.html".to_string()
    } else if !has(ctx, ARKENIAS_CONTRACT) {
        "30419-08.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn adonius_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, MOONFLOWER_CHARM) {
        return ctx.no_quest_html();
    }
    if has(ctx, ARKENIAS_INSTRUCTIONS) {
        "30375-01.html".to_string()
    } else if has(ctx, ADONIUS_LIST) {
        if ctx.quest_items_count(SPIDER_ICHOR) >= 20 && ctx.quest_items_count(HARPYS_DOWN) >= 20 {
            ctx.take_items(ADONIUS_LIST, 1);
            ctx.give_items(ANDARIEL_SCRIPTURE_COPY, 1);
            ctx.take_items(SPIDER_ICHOR, -1);
            ctx.take_items(HARPYS_DOWN, -1);
            ctx.set_cond(11, true);
            "30375-04.html".to_string()
        } else {
            "30375-03.html".to_string()
        }
    } else if has(ctx, ANDARIEL_SCRIPTURE_COPY) {
        "30375-05.html".to_string()
    } else {
        "30375-06.html".to_string()
    }
}

fn isael_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, MOONFLOWER_CHARM) {
        return ctx.no_quest_html();
    }
    if has(ctx, THALIAS_2ND_LETTER) {
        "30655-01.html".to_string()
    } else if has(ctx, ISAELS_INSTRUCTIONS) {
        if has(ctx, TALINS_SPEAR_BLADE)
            && has(ctx, TALINS_SPEAR_SHAFT)
            && has(ctx, TALINS_RUBY)
            && has(ctx, TALINS_AQUAMARINE)
            && has(ctx, TALINS_AMETHYST)
            && has(ctx, TALINS_PERIDOT)
        {
            ctx.give_items(TALINS_SPEAR, 1);
            ctx.take_items(ISAELS_INSTRUCTIONS, 1);
            ctx.give_items(ISAELS_LETTER, 1);
            ctx.take_items(TALINS_SPEAR_BLADE, 1);
            ctx.take_items(TALINS_SPEAR_SHAFT, 1);
            ctx.take_items(TALINS_RUBY, 1);
            ctx.take_items(TALINS_AQUAMARINE, 1);
            ctx.take_items(TALINS_AMETHYST, 1);
            ctx.take_items(TALINS_PERIDOT, 1);
            ctx.set_cond(17, true);
            "30655-04.html".to_string()
        } else {
            "30655-03.html".to_string()
        }
    } else if has(ctx, TALINS_SPEAR) && has(ctx, ISAELS_LETTER) {
        "30655-05.html".to_string()
    } else if has(ctx, GRAIL_OF_PURITY) || has(ctx, WATER_OF_LIFE) || has(ctx, CAMOMILE_CHARM) {
        "30655-06.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}
