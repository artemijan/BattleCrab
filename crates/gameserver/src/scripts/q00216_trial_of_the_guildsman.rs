//! Trial of the Guildsman (216) — `quests/Q00216_TrialOfTheGuildsman`. The
//! dwarven-crafter trial (Artisan / Scavenger, level 35+, 2000-adena entry fee).
//! Valkon starts a sprawling gather-and-craft: berry for Altran's recipe, then
//! two parallel supply chains — Norman/Duning's Journeyman Gems (bone powder,
//! whetstones, pigment, yarn, and 30 Duning's Keys off Breka orcs) and Pinter's
//! Journeyman Deco Beads (70 amber beads off ants) — culminating in seven
//! crafted Journeyman Rings for the Mark of the Guildsman.
//!
//! Java selects a party member per kill via `getRandomPartyMemberState` +
//! `checkPartyMember`; per this port's G11 policy that reduces to the killer,
//! with `checkPartyMember`'s predicate folded into each `onKill` gate. The
//! final Journeyman Rings are crafted through the recipe system (quest 216 only
//! grants the recipe and checks the count), so tests supply the rings directly.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const WAREHOUSE_KEEPER_VALKON: i32 = 30103;
const WAREHOUSE_KEEPER_NORMAN: i32 = 30210;
const BLACKSMITH_ALTRAN: i32 = 30283;
const BLACKSMITH_PINTER: i32 = 30298;
const BLACKSMITH_DUNING: i32 = 30688;
// Items
const ADENA: i32 = 57;
const RECIPE_JOURNEYMAN_RING: i32 = 3024;
const RECIPE_AMBER_BEAD: i32 = 3025;
const VALKONS_RECOMMENDATION: i32 = 3120;
const MANDRAGORA_BERRY: i32 = 3121;
const ALLTRANS_INSTRUCTIONS: i32 = 3122;
const ALLTRANS_1ST_RECOMMENDATION: i32 = 3123;
const ALLTRANS_2ND_RECOMMENDATION: i32 = 3124;
const NORMANS_INSTRUCTIONS: i32 = 3125;
const NORMANS_RECEIPT: i32 = 3126;
const DUNINGS_INSTRUCTIONS: i32 = 3127;
const DUNINGS_KEY: i32 = 3128;
const NORMANS_LIST: i32 = 3129;
const GRAY_BONE_POWDER: i32 = 3130;
const GRANITE_WHETSTONE: i32 = 3131;
const RED_PIGMENT: i32 = 3132;
const BRAIDED_YARN: i32 = 3133;
const JOURNEYMAN_GEM: i32 = 3134;
const PINTERS_INSTRUCTIONS: i32 = 3135;
const AMBER_BEAD: i32 = 3136;
const AMBER_LUMP: i32 = 3137;
const JOURNEYMAN_DECO_BEADS: i32 = 3138;
const JOURNEYMAN_RING: i32 = 3139;
// Reward
const MARK_OF_GUILDSMAN: i32 = 3119;
// Monsters
const ANT: i32 = 20079;
const ANT_CAPTAIN: i32 = 20080;
const ANT_OVERSEER: i32 = 20081;
const GRANITE_GOLEM: i32 = 20083;
const MANDRAGORA_SPROUT1: i32 = 20154;
const MANDRAGORA_SAPLONG: i32 = 20155;
const MANDRAGORA_BLOSSOM: i32 = 20156;
const SILENOS: i32 = 20168;
const STRAIN: i32 = 20200;
const GHOUL: i32 = 20201;
const DEAD_SEEKER: i32 = 20202;
const MANDRAGORA_SPROUT2: i32 = 20223;
const BREKA_ORC: i32 = 20267;
const BREKA_ORC_ARCHER: i32 = 20268;
const BREKA_ORC_SHAMAN: i32 = 20269;
const BREKA_ORC_OVERLORD: i32 = 20270;
const BREKA_ORC_WARRIOR: i32 = 20271;
// Misc
const MIN_LEVEL: i32 = 35;
const SCAVENGER: i32 = 54;
const ARTISAN: i32 = 56;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// A simple "give N of a material, capped-check at 70" reward leg gated on the
/// `checkPartyMember` predicate (folded onto the killer per G11).
fn material_leg(ctx: &mut QuestCtx, gate_ok: bool, item: i32, amount: i64) {
    if gate_ok {
        ctx.give_items(item, amount);
        if ctx.quest_items_count(item) == 70 {
            ctx.play_sound(quest_sounds::MIDDLE);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}

pub struct Q00216TrialOfTheGuildsman;

impl QuestScript for Q00216TrialOfTheGuildsman {
    fn id(&self) -> i32 {
        216
    }
    fn name(&self) -> &'static str {
        "Q00216_TrialOfTheGuildsman"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00216_TrialOfTheGuildsman"
    }
    fn start_npcs(&self) -> &[i32] {
        &[WAREHOUSE_KEEPER_VALKON]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            WAREHOUSE_KEEPER_VALKON,
            WAREHOUSE_KEEPER_NORMAN,
            BLACKSMITH_ALTRAN,
            BLACKSMITH_PINTER,
            BLACKSMITH_DUNING,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            ANT,
            ANT_CAPTAIN,
            ANT_OVERSEER,
            GRANITE_GOLEM,
            MANDRAGORA_SPROUT1,
            MANDRAGORA_SAPLONG,
            MANDRAGORA_BLOSSOM,
            SILENOS,
            STRAIN,
            GHOUL,
            DEAD_SEEKER,
            MANDRAGORA_SPROUT2,
            BREKA_ORC,
            BREKA_ORC_ARCHER,
            BREKA_ORC_SHAMAN,
            BREKA_ORC_OVERLORD,
            BREKA_ORC_WARRIOR,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            RECIPE_JOURNEYMAN_RING,
            RECIPE_AMBER_BEAD,
            VALKONS_RECOMMENDATION,
            MANDRAGORA_BERRY,
            ALLTRANS_INSTRUCTIONS,
            ALLTRANS_1ST_RECOMMENDATION,
            ALLTRANS_2ND_RECOMMENDATION,
            NORMANS_INSTRUCTIONS,
            NORMANS_RECEIPT,
            DUNINGS_INSTRUCTIONS,
            DUNINGS_KEY,
            NORMANS_LIST,
            GRAY_BONE_POWDER,
            GRANITE_WHETSTONE,
            RED_PIGMENT,
            BRAIDED_YARN,
            JOURNEYMAN_GEM,
            PINTERS_INSTRUCTIONS,
            AMBER_BEAD,
            AMBER_LUMP,
            JOURNEYMAN_DECO_BEADS,
            JOURNEYMAN_RING,
        ]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == WAREHOUSE_KEEPER_VALKON {
                let class = ctx.player_class_id();
                if class == ARTISAN || class == SCAVENGER {
                    return Some(if ctx.player_level() < MIN_LEVEL {
                        "30103-02.html".to_string()
                    } else {
                        "30103-03.htm".to_string()
                    });
                }
                return Some("30103-01.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == WAREHOUSE_KEEPER_VALKON {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            WAREHOUSE_KEEPER_VALKON => Some(valkon_talk(ctx)),
            WAREHOUSE_KEEPER_NORMAN => Some(norman_talk(ctx)),
            BLACKSMITH_ALTRAN => Some(altran_talk(ctx)),
            BLACKSMITH_PINTER => Some(pinter_talk(ctx)),
            BLACKSMITH_DUNING => Some(duning_talk(ctx)),
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => {
                if ctx.quest_items_count(ADENA) >= 2000 {
                    ctx.start_quest();
                    ctx.take_items(ADENA, 2000);
                    if !has(ctx, VALKONS_RECOMMENDATION) {
                        ctx.give_items(VALKONS_RECOMMENDATION, 1);
                    }
                    ctx.play_sound(quest_sounds::MIDDLE);
                    None
                } else {
                    Some("30103-05b.htm".to_string())
                }
            }
            "30103-04.htm" | "30103-05.htm" | "30103-05a.html" | "30103-06a.html"
            | "30103-06b.html" | "30103-06c.html" | "30103-07a.html" | "30103-07b.html"
            | "30103-07c.html" | "30210-02.html" | "30210-03.html" | "30210-08.html"
            | "30210-09.html" | "30210-11a.html" | "30283-03a.html" | "30283-03b.html"
            | "30283-04.html" | "30298-03.html" | "30298-05a.html" => Some(event.to_string()),
            "30103-09a.html" => {
                if has(ctx, ALLTRANS_INSTRUCTIONS) && ctx.quest_items_count(JOURNEYMAN_RING) >= 7 {
                    ctx.give_adena(187606, true);
                    ctx.give_items(MARK_OF_GUILDSMAN, 1);
                    ctx.add_exp_and_sp(1029478, 66768);
                    ctx.exit_quest(false, true);
                    ctx.social_action(3);
                    return Some(event.to_string());
                }
                None
            }
            "30103-09b.html" => {
                if has(ctx, ALLTRANS_INSTRUCTIONS) && ctx.quest_items_count(JOURNEYMAN_RING) >= 7 {
                    ctx.give_adena(93803, true);
                    ctx.give_items(MARK_OF_GUILDSMAN, 1);
                    ctx.add_exp_and_sp(514739, 33384);
                    ctx.exit_quest(false, true);
                    ctx.social_action(3);
                    return Some(event.to_string());
                }
                None
            }
            "30210-04.html" => {
                if has(ctx, ALLTRANS_1ST_RECOMMENDATION) {
                    ctx.take_items(ALLTRANS_1ST_RECOMMENDATION, 1);
                    ctx.give_items(NORMANS_INSTRUCTIONS, 1);
                    ctx.give_items(NORMANS_RECEIPT, 1);
                    return Some(event.to_string());
                }
                None
            }
            "30210-10.html" => {
                if has(ctx, NORMANS_INSTRUCTIONS) {
                    ctx.take_items(NORMANS_INSTRUCTIONS, 1);
                    ctx.take_items(DUNINGS_KEY, -1);
                    ctx.give_items(NORMANS_LIST, 1);
                    return Some(event.to_string());
                }
                None
            }
            "30283-03.html" => {
                if has(ctx, VALKONS_RECOMMENDATION) && has(ctx, MANDRAGORA_BERRY) {
                    ctx.give_items(RECIPE_JOURNEYMAN_RING, 1);
                    ctx.take_items(VALKONS_RECOMMENDATION, 1);
                    ctx.take_items(MANDRAGORA_BERRY, 1);
                    ctx.give_items(ALLTRANS_INSTRUCTIONS, 1);
                    ctx.give_items(ALLTRANS_1ST_RECOMMENDATION, 1);
                    ctx.give_items(ALLTRANS_2ND_RECOMMENDATION, 1);
                    ctx.set_cond(5, true);
                    return Some(event.to_string());
                }
                None
            }
            "30298-04.html" => {
                if ctx.player_class_id() == SCAVENGER {
                    if has(ctx, ALLTRANS_2ND_RECOMMENDATION) {
                        ctx.take_items(ALLTRANS_2ND_RECOMMENDATION, 1);
                        ctx.give_items(PINTERS_INSTRUCTIONS, 1);
                        return Some(event.to_string());
                    }
                    None
                } else if has(ctx, ALLTRANS_2ND_RECOMMENDATION) {
                    ctx.give_items(RECIPE_AMBER_BEAD, 1);
                    ctx.take_items(ALLTRANS_2ND_RECOMMENDATION, 1);
                    ctx.give_items(PINTERS_INSTRUCTIONS, 1);
                    Some("30298-05.html".to_string())
                } else {
                    None
                }
            }
            "30688-02.html" => {
                if has(ctx, NORMANS_RECEIPT) {
                    ctx.take_items(NORMANS_RECEIPT, 1);
                    ctx.give_items(DUNINGS_INSTRUCTIONS, 1);
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
            ANT | ANT_CAPTAIN | ANT_OVERSEER => {
                // checkPartyMember: needs both instruction sets, amber < 70.
                if has(ctx, ALLTRANS_INSTRUCTIONS)
                    && has(ctx, PINTERS_INSTRUCTIONS)
                    && ctx.quest_items_count(AMBER_BEAD) < 70
                {
                    let class = ctx.player_class_id();
                    let mut count = 0;
                    if class == SCAVENGER && ctx.npc_is_spoiled() {
                        count += 5;
                    }
                    if ctx.roll(2) == 0 && class == ARTISAN {
                        ctx.give_items(AMBER_LUMP, 1);
                        ctx.play_sound(quest_sounds::MIDDLE);
                    }
                    if ctx.quest_items_count(AMBER_BEAD) + count < 70 {
                        count += 5;
                    }
                    if count > 0 {
                        ctx.give_item_randomly(AMBER_BEAD, count, 70, 1.0, true);
                    }
                }
            }
            GRANITE_GOLEM => material_leg(
                ctx,
                has(ctx, ALLTRANS_INSTRUCTIONS)
                    && has(ctx, NORMANS_LIST)
                    && ctx.quest_items_count(GRANITE_WHETSTONE) < 70,
                GRANITE_WHETSTONE,
                7,
            ),
            SILENOS => material_leg(
                ctx,
                has(ctx, ALLTRANS_INSTRUCTIONS)
                    && has(ctx, NORMANS_LIST)
                    && ctx.quest_items_count(BRAIDED_YARN) < 70,
                BRAIDED_YARN,
                10,
            ),
            STRAIN | GHOUL => material_leg(
                ctx,
                has(ctx, ALLTRANS_INSTRUCTIONS)
                    && has(ctx, NORMANS_LIST)
                    && ctx.quest_items_count(GRAY_BONE_POWDER) < 70,
                GRAY_BONE_POWDER,
                5,
            ),
            DEAD_SEEKER => material_leg(
                ctx,
                has(ctx, ALLTRANS_INSTRUCTIONS)
                    && has(ctx, NORMANS_LIST)
                    && ctx.quest_items_count(RED_PIGMENT) < 70,
                RED_PIGMENT,
                7,
            ),
            MANDRAGORA_SPROUT1 | MANDRAGORA_SAPLONG | MANDRAGORA_BLOSSOM | MANDRAGORA_SPROUT2 => {
                if has(ctx, VALKONS_RECOMMENDATION) && !has(ctx, MANDRAGORA_BERRY) {
                    ctx.give_items(MANDRAGORA_BERRY, 1);
                    ctx.set_cond(4, true);
                }
            }
            BREKA_ORC | BREKA_ORC_ARCHER | BREKA_ORC_SHAMAN | BREKA_ORC_OVERLORD
            | BREKA_ORC_WARRIOR
                if has(ctx, ALLTRANS_INSTRUCTIONS)
                    && has(ctx, NORMANS_INSTRUCTIONS)
                    && has(ctx, DUNINGS_INSTRUCTIONS)
                    && ctx.quest_items_count(DUNINGS_KEY) < 30 =>
            {
                if ctx.quest_items_count(DUNINGS_KEY) >= 29 {
                    ctx.give_items(DUNINGS_KEY, 1);
                    ctx.take_items(DUNINGS_INSTRUCTIONS, 1);
                    ctx.play_sound(quest_sounds::MIDDLE);
                } else {
                    ctx.give_items(DUNINGS_KEY, 1);
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
            _ => {}
        }
    }
}

fn valkon_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, VALKONS_RECOMMENDATION) {
        ctx.set_cond(3, true);
        "30103-07.html".to_string()
    } else if has(ctx, ALLTRANS_INSTRUCTIONS) {
        if ctx.quest_items_count(JOURNEYMAN_RING) < 7 {
            "30103-08.html".to_string()
        } else {
            "30103-09.html".to_string()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn norman_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, ALLTRANS_INSTRUCTIONS) {
        return ctx.no_quest_html();
    }
    if has(ctx, ALLTRANS_1ST_RECOMMENDATION) {
        "30210-01.html".to_string()
    } else if has(ctx, NORMANS_INSTRUCTIONS) && has(ctx, NORMANS_RECEIPT) {
        "30210-05.html".to_string()
    } else if has(ctx, NORMANS_INSTRUCTIONS) && has(ctx, DUNINGS_INSTRUCTIONS) {
        "30210-06.html".to_string()
    } else if has(ctx, NORMANS_INSTRUCTIONS) && ctx.quest_items_count(DUNINGS_KEY) >= 30 {
        "30210-07.html".to_string()
    } else if has(ctx, NORMANS_LIST) {
        if ctx.quest_items_count(GRAY_BONE_POWDER) >= 70
            && ctx.quest_items_count(GRANITE_WHETSTONE) >= 70
            && ctx.quest_items_count(RED_PIGMENT) >= 70
            && ctx.quest_items_count(BRAIDED_YARN) >= 70
        {
            ctx.take_items(NORMANS_LIST, 1);
            ctx.take_items(GRAY_BONE_POWDER, -1);
            ctx.take_items(GRANITE_WHETSTONE, -1);
            ctx.take_items(RED_PIGMENT, -1);
            ctx.take_items(BRAIDED_YARN, -1);
            ctx.give_items(JOURNEYMAN_GEM, 7);
            if ctx.quest_items_count(JOURNEYMAN_DECO_BEADS) >= 7 {
                ctx.set_cond(6, true);
            }
            "30210-12.html".to_string()
        } else {
            "30210-11.html".to_string()
        }
    } else if !has(ctx, NORMANS_INSTRUCTIONS)
        && !has(ctx, NORMANS_LIST)
        && (has(ctx, JOURNEYMAN_GEM) || has(ctx, JOURNEYMAN_RING))
    {
        "30210-13.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn altran_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, VALKONS_RECOMMENDATION) {
        if !has(ctx, MANDRAGORA_BERRY) {
            ctx.set_cond(2, true);
            "30283-01.html".to_string()
        } else {
            "30283-02.html".to_string()
        }
    } else if has(ctx, ALLTRANS_INSTRUCTIONS) {
        if ctx.quest_items_count(JOURNEYMAN_RING) < 7 {
            "30283-04.html".to_string()
        } else {
            "30283-05.html".to_string()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn pinter_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, ALLTRANS_INSTRUCTIONS) {
        return ctx.no_quest_html();
    }
    if has(ctx, ALLTRANS_2ND_RECOMMENDATION) {
        "30298-02.html".to_string()
    } else if has(ctx, PINTERS_INSTRUCTIONS) {
        if ctx.quest_items_count(AMBER_BEAD) < 70 {
            "30298-06.html".to_string()
        } else {
            ctx.take_items(RECIPE_AMBER_BEAD, 1);
            ctx.take_items(PINTERS_INSTRUCTIONS, 1);
            ctx.take_items(AMBER_BEAD, -1);
            ctx.take_items(AMBER_LUMP, -1);
            ctx.give_items(JOURNEYMAN_DECO_BEADS, 7);
            if ctx.quest_items_count(JOURNEYMAN_GEM) >= 7 {
                ctx.set_cond(6, true);
            }
            "30298-07.html".to_string()
        }
    } else if !has(ctx, PINTERS_INSTRUCTIONS)
        && (has(ctx, JOURNEYMAN_DECO_BEADS) || has(ctx, JOURNEYMAN_RING))
    {
        "30298-08.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn duning_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ALLTRANS_INSTRUCTIONS) && has(ctx, NORMANS_INSTRUCTIONS) {
        if has(ctx, NORMANS_RECEIPT) && !has(ctx, DUNINGS_INSTRUCTIONS) {
            return "30688-01.html".to_string();
        }
        if has(ctx, DUNINGS_INSTRUCTIONS)
            && !has(ctx, NORMANS_RECEIPT)
            && ctx.quest_items_count(DUNINGS_KEY) < 30
        {
            "30688-03.html".to_string()
        } else if ctx.quest_items_count(DUNINGS_KEY) >= 30 && !has(ctx, DUNINGS_INSTRUCTIONS) {
            "30688-04.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else if has(ctx, ALLTRANS_INSTRUCTIONS)
        && !has(ctx, NORMANS_INSTRUCTIONS)
        && !has(ctx, DUNINGS_INSTRUCTIONS)
    {
        "30688-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}
