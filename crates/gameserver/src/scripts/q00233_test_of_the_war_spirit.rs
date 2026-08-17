//! Test of the War Spirit (233) — `quests/Q00233_TestOfTheWarSpirit`. The second
//! Overlord 2nd-class proof (Orc race, Orc Shaman, level 39+). Seer Somak sets
//! the aspirant to reassemble four ancient warriors' skeletons — Brakis, Tonar,
//! Hermodt and Kiruna — each via its own seer's totem and a five-bone hunt. The
//! four sets of remains become the Vendetta Totem; thirteen Tamlin Orc heads
//! upgrade it to the War Spirit Totem, and Ancestor Martankus grants the Mark.
//!
//! Pure item-gate (cond 1..5). The four skeleton legs complete in any order —
//! cond 2 fires when the fourth `*_REMAINS1` is forged.

use crate::game_loop::quests::{QuestCtx, QuestScript};

// NPCs
const PRIESTESS_VIVYAN: i32 = 30030;
const TRADER_SARIEN: i32 = 30436;
const SEER_RACOY: i32 = 30507;
const SEER_SOMAK: i32 = 30510;
const SEER_MANAKIA: i32 = 30515;
const SHADOW_ORIM: i32 = 30630;
const ANCESTOR_MARTANKUS: i32 = 30649;
const SEER_PEKIRON: i32 = 30682;
// Items
const VENDETTA_TOTEM: i32 = 2880;
const TAMLIN_ORC_HEAD: i32 = 2881;
const WARSPIRIT_TOTEM: i32 = 2882;
const ORIMS_CONTRACT: i32 = 2883;
const PORTAS_EYE: i32 = 2884;
const EXCUROS_SCALE: i32 = 2885;
const MORDEOS_TALON: i32 = 2886;
const BRAKIS_REMAINS1: i32 = 2887;
const PEKIRONS_TOTEM: i32 = 2888;
const TONARS_SKULL: i32 = 2889;
const TONARS_RIB_BONE: i32 = 2890;
const TONARS_SPINE: i32 = 2891;
const TONARS_ARM_BONE: i32 = 2892;
const TONARS_THIGH_BONE: i32 = 2893;
const TONARS_REMAINS1: i32 = 2894;
const MANAKIAS_TOTEM: i32 = 2895;
const HERMODTS_SKULL: i32 = 2896;
const HERMODTS_RIB_BONE: i32 = 2897;
const HERMODTS_SPINE: i32 = 2898;
const HERMODTS_ARM_BONE: i32 = 2899;
const HERMODTS_THIGH_BONE: i32 = 2900;
const HERMODTS_REMAINS1: i32 = 2901;
const RACOYS_TOTEM: i32 = 2902;
const VIVIANTES_LETTER: i32 = 2903;
const INSECT_DIAGRAM_BOOK: i32 = 2904;
const KIRUNAS_SKULL: i32 = 2905;
const KIRUNAS_RIB_BONE: i32 = 2906;
const KIRUNAS_SPINE: i32 = 2907;
const KIRUNAS_ARM_BONE: i32 = 2908;
const KIRUNAS_THIGH_BONE: i32 = 2909;
const KIRUNAS_REMAINS1: i32 = 2910;
const BRAKIS_REMAINS2: i32 = 2911;
const TONARS_REMAINS2: i32 = 2912;
const HERMODTS_REMAINS2: i32 = 2913;
const KIRUNAS_REMAINS2: i32 = 2914;
// Reward
const MARK_OF_WARSPIRIT: i32 = 2879;
// Monsters
const NOBLE_ANT: i32 = 20089;
const NOBLE_ANT_LEADER: i32 = 20090;
const MEDUSA: i32 = 20158;
const PORTA: i32 = 20213;
const EXCURO: i32 = 20214;
const MORDERO: i32 = 20215;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const TAMLIN_ORC: i32 = 20601;
const TAMLIN_ORC_ARCHER: i32 = 20602;
const STENOA_GORGON_QUEEN: i32 = 27108;
// Misc
const MIN_LEVEL: i32 = 39;
const RACE_ORC: i32 = 3;
const ORC_SHAMAN: i32 = 50;

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// Give the first bone in `bones` the killer is still missing (gated on `gate`).
fn next_bone(ctx: &mut QuestCtx, gate: i32, bones: &[i32]) {
    if !has(ctx, gate) {
        return;
    }
    for &b in bones {
        if ctx.award_once(b) {
            return;
        }
    }
}

/// After forging one skeleton's `*_REMAINS1`, advance to cond 2 if the other
/// three are already assembled.
fn maybe_cond2(ctx: &mut QuestCtx, a: i32, b: i32, c: i32) {
    if has(ctx, a) && has(ctx, b) && has(ctx, c) {
        ctx.set_cond(2, false);
    }
}

pub struct Q00233TestOfTheWarSpirit;

impl QuestScript for Q00233TestOfTheWarSpirit {
    fn id(&self) -> i32 {
        233
    }
    fn name(&self) -> &'static str {
        "Q00233_TestOfTheWarSpirit"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00233_TestOfTheWarSpirit"
    }
    fn start_npcs(&self) -> &[i32] {
        &[SEER_SOMAK]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            SEER_SOMAK,
            PRIESTESS_VIVYAN,
            TRADER_SARIEN,
            SEER_RACOY,
            SEER_MANAKIA,
            SHADOW_ORIM,
            ANCESTOR_MARTANKUS,
            SEER_PEKIRON,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            NOBLE_ANT,
            NOBLE_ANT_LEADER,
            MEDUSA,
            PORTA,
            EXCURO,
            MORDERO,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OVERLORD,
            TAMLIN_ORC,
            TAMLIN_ORC_ARCHER,
            STENOA_GORGON_QUEEN,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            VENDETTA_TOTEM,
            TAMLIN_ORC_HEAD,
            WARSPIRIT_TOTEM,
            ORIMS_CONTRACT,
            PORTAS_EYE,
            EXCUROS_SCALE,
            MORDEOS_TALON,
            BRAKIS_REMAINS1,
            PEKIRONS_TOTEM,
            TONARS_SKULL,
            TONARS_RIB_BONE,
            TONARS_SPINE,
            TONARS_ARM_BONE,
            TONARS_THIGH_BONE,
            TONARS_REMAINS1,
            MANAKIAS_TOTEM,
            HERMODTS_SKULL,
            HERMODTS_RIB_BONE,
            HERMODTS_SPINE,
            HERMODTS_ARM_BONE,
            HERMODTS_THIGH_BONE,
            HERMODTS_REMAINS1,
            RACOYS_TOTEM,
            VIVIANTES_LETTER,
            INSECT_DIAGRAM_BOOK,
            KIRUNAS_SKULL,
            KIRUNAS_RIB_BONE,
            KIRUNAS_SPINE,
            KIRUNAS_ARM_BONE,
            KIRUNAS_THIGH_BONE,
            KIRUNAS_REMAINS1,
            BRAKIS_REMAINS2,
            TONARS_REMAINS2,
            HERMODTS_REMAINS2,
            KIRUNAS_REMAINS2,
        ]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == SEER_SOMAK {
                if ctx.player_race() != RACE_ORC {
                    return Some("30510-01.html".to_string());
                } else if ctx.player_class_id() != ORC_SHAMAN {
                    return Some("30510-02.html".to_string());
                } else if ctx.player_level() < MIN_LEVEL {
                    return Some("30510-03.html".to_string());
                }
                return Some("30510-04.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == SEER_SOMAK {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            SEER_SOMAK => Some(somak_talk(ctx)),
            PRIESTESS_VIVYAN => Some(vivyan_talk(ctx)),
            TRADER_SARIEN => Some(sarien_talk(ctx)),
            SEER_RACOY => Some(racoy_talk(ctx)),
            SEER_MANAKIA => Some(manakia_talk(ctx)),
            SHADOW_ORIM => Some(orim_talk(ctx)),
            ANCESTOR_MARTANKUS => {
                if has(ctx, WARSPIRIT_TOTEM) {
                    Some("30649-01.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            SEER_PEKIRON => Some(pekiron_talk(ctx)),
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
                }
                None
            }
            "30510-05a.html" | "30510-05b.html" | "30510-05c.html" | "30510-05d.html"
            | "30510-05.html" | "30030-02.html" | "30030-03.html" | "30630-02.html"
            | "30630-03.html" | "30649-02.html" => Some(event.to_string()),
            "30030-04.html" => {
                ctx.give_items(VIVIANTES_LETTER, 1);
                Some(event.to_string())
            }
            "30507-02.html" => {
                ctx.give_items(RACOYS_TOTEM, 1);
                Some(event.to_string())
            }
            "30515-02.html" => {
                ctx.give_items(MANAKIAS_TOTEM, 1);
                Some(event.to_string())
            }
            "30630-04.html" => {
                ctx.give_items(ORIMS_CONTRACT, 1);
                Some(event.to_string())
            }
            "30682-02.html" => {
                ctx.give_items(PEKIRONS_TOTEM, 1);
                Some(event.to_string())
            }
            "30649-03.html" => {
                if has(ctx, TONARS_REMAINS2) {
                    ctx.give_adena(161806, true);
                    ctx.give_items(MARK_OF_WARSPIRIT, 1);
                    ctx.add_exp_and_sp(894888, 61408);
                    ctx.exit_quest(false, true);
                    ctx.social_action(3);
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
            NOBLE_ANT | NOBLE_ANT_LEADER => {
                if has(ctx, RACOYS_TOTEM) && has(ctx, INSECT_DIAGRAM_BOOK) {
                    let i0 = ctx.roll(100);
                    if i0 > 65 {
                        next_bone(ctx, RACOYS_TOTEM, &[KIRUNAS_THIGH_BONE, KIRUNAS_ARM_BONE]);
                    } else if i0 > 30 {
                        next_bone(ctx, RACOYS_TOTEM, &[KIRUNAS_SPINE, KIRUNAS_RIB_BONE]);
                    } else {
                        ctx.award_once(KIRUNAS_SKULL);
                    }
                }
            }
            MEDUSA => next_bone(
                ctx,
                MANAKIAS_TOTEM,
                &[
                    HERMODTS_RIB_BONE,
                    HERMODTS_SPINE,
                    HERMODTS_ARM_BONE,
                    HERMODTS_THIGH_BONE,
                ],
            ),
            STENOA_GORGON_QUEEN => {
                if has(ctx, MANAKIAS_TOTEM) {
                    ctx.award_once(HERMODTS_SKULL);
                }
            }
            PORTA => {
                if has(ctx, ORIMS_CONTRACT) {
                    ctx.give_item_randomly(PORTAS_EYE, 2, 10, 1.0, true);
                }
            }
            EXCURO => {
                if has(ctx, ORIMS_CONTRACT) {
                    ctx.give_item_randomly(EXCUROS_SCALE, 5, 10, 1.0, true);
                }
            }
            MORDERO => {
                if has(ctx, ORIMS_CONTRACT) {
                    ctx.give_item_randomly(MORDEOS_TALON, 5, 10, 1.0, true);
                }
            }
            LETO_LIZARDMAN_SHAMAN | LETO_LIZARDMAN_OVERLORD => next_bone(
                ctx,
                PEKIRONS_TOTEM,
                &[
                    TONARS_SKULL,
                    TONARS_RIB_BONE,
                    TONARS_SPINE,
                    TONARS_ARM_BONE,
                    TONARS_THIGH_BONE,
                ],
            ),
            TAMLIN_ORC | TAMLIN_ORC_ARCHER => {
                #[allow(clippy::collapsible_match)]
                if has(ctx, VENDETTA_TOTEM)
                    && ctx.give_item_randomly(TAMLIN_ORC_HEAD, 1, 13, 1.0, true)
                {
                    ctx.set_cond(4, true);
                }
            }
            _ => {}
        }
    }
}

fn somak_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, VENDETTA_TOTEM) && !has(ctx, WARSPIRIT_TOTEM) {
        if has(ctx, BRAKIS_REMAINS1)
            && has(ctx, HERMODTS_REMAINS1)
            && has(ctx, KIRUNAS_REMAINS1)
            && has(ctx, TONARS_REMAINS1)
        {
            ctx.give_items(VENDETTA_TOTEM, 1);
            ctx.take_items(BRAKIS_REMAINS1, 1);
            ctx.take_items(TONARS_REMAINS1, 1);
            ctx.take_items(HERMODTS_REMAINS1, 1);
            ctx.take_items(KIRUNAS_REMAINS1, 1);
            ctx.set_cond(3, false);
            "30510-07.html".to_string()
        } else {
            "30510-06.html".to_string()
        }
    } else if has(ctx, VENDETTA_TOTEM) {
        if ctx.quest_items_count(TAMLIN_ORC_HEAD) < 13 {
            "30510-08.html".to_string()
        } else {
            ctx.take_items(VENDETTA_TOTEM, 1);
            ctx.give_items(WARSPIRIT_TOTEM, 1);
            ctx.give_items(BRAKIS_REMAINS2, 1);
            ctx.give_items(TONARS_REMAINS2, 1);
            ctx.give_items(HERMODTS_REMAINS2, 1);
            ctx.give_items(KIRUNAS_REMAINS2, 1);
            ctx.set_cond(5, false);
            "30510-09.html".to_string()
        }
    } else if has(ctx, WARSPIRIT_TOTEM) {
        "30510-10.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn vivyan_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, RACOYS_TOTEM) && !has(ctx, VIVIANTES_LETTER) && !has(ctx, INSECT_DIAGRAM_BOOK) {
        "30030-01.html".to_string()
    } else if has(ctx, RACOYS_TOTEM) && has(ctx, VIVIANTES_LETTER) && !has(ctx, INSECT_DIAGRAM_BOOK)
    {
        "30030-05.html".to_string()
    } else if has(ctx, RACOYS_TOTEM) && has(ctx, INSECT_DIAGRAM_BOOK) && !has(ctx, VIVIANTES_LETTER)
    {
        "30030-06.html".to_string()
    } else if !has(ctx, RACOYS_TOTEM)
        && (has(ctx, KIRUNAS_REMAINS1) || has(ctx, KIRUNAS_REMAINS2) || has(ctx, VENDETTA_TOTEM))
    {
        "30030-07.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn sarien_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, RACOYS_TOTEM) && has(ctx, VIVIANTES_LETTER) && !has(ctx, INSECT_DIAGRAM_BOOK) {
        ctx.take_items(VIVIANTES_LETTER, 1);
        ctx.give_items(INSECT_DIAGRAM_BOOK, 1);
        "30436-01.html".to_string()
    } else if has(ctx, RACOYS_TOTEM) && has(ctx, INSECT_DIAGRAM_BOOK) && !has(ctx, VIVIANTES_LETTER)
    {
        "30436-02.html".to_string()
    } else if !has(ctx, RACOYS_TOTEM)
        && (has(ctx, KIRUNAS_REMAINS1) || has(ctx, KIRUNAS_REMAINS2) || has(ctx, VENDETTA_TOTEM))
    {
        "30436-03.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn racoy_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, RACOYS_TOTEM)
        && !has(ctx, KIRUNAS_REMAINS1)
        && !has(ctx, KIRUNAS_REMAINS2)
        && !has(ctx, VENDETTA_TOTEM)
    {
        "30507-01.html".to_string()
    } else if has(ctx, RACOYS_TOTEM)
        && !has(ctx, VIVIANTES_LETTER)
        && !has(ctx, INSECT_DIAGRAM_BOOK)
    {
        "30507-03.html".to_string()
    } else if has(ctx, RACOYS_TOTEM) && has(ctx, VIVIANTES_LETTER) && !has(ctx, INSECT_DIAGRAM_BOOK)
    {
        "30507-04.html".to_string()
    } else if has(ctx, RACOYS_TOTEM) && has(ctx, INSECT_DIAGRAM_BOOK) && !has(ctx, VIVIANTES_LETTER)
    {
        if has(ctx, KIRUNAS_SKULL)
            && has(ctx, KIRUNAS_RIB_BONE)
            && has(ctx, KIRUNAS_SPINE)
            && has(ctx, KIRUNAS_ARM_BONE)
            && has(ctx, KIRUNAS_THIGH_BONE)
        {
            ctx.take_items(RACOYS_TOTEM, 1);
            ctx.take_items(INSECT_DIAGRAM_BOOK, 1);
            ctx.take_items(KIRUNAS_SKULL, 1);
            ctx.take_items(KIRUNAS_RIB_BONE, 1);
            ctx.take_items(KIRUNAS_SPINE, 1);
            ctx.take_items(KIRUNAS_ARM_BONE, 1);
            ctx.take_items(KIRUNAS_THIGH_BONE, 1);
            ctx.give_items(KIRUNAS_REMAINS1, 1);
            maybe_cond2(ctx, BRAKIS_REMAINS1, HERMODTS_REMAINS1, TONARS_REMAINS1);
            "30507-06.html".to_string()
        } else {
            "30507-05.html".to_string()
        }
    } else if !has(ctx, RACOYS_TOTEM)
        && (has(ctx, KIRUNAS_REMAINS1) || has(ctx, KIRUNAS_REMAINS2) || has(ctx, VENDETTA_TOTEM))
    {
        "30507-07.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn manakia_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, MANAKIAS_TOTEM)
        && !has(ctx, HERMODTS_REMAINS2)
        && !has(ctx, VENDETTA_TOTEM)
        && !has(ctx, HERMODTS_REMAINS1)
    {
        "30515-01.html".to_string()
    } else if has(ctx, MANAKIAS_TOTEM) {
        if has(ctx, HERMODTS_SKULL)
            && has(ctx, HERMODTS_RIB_BONE)
            && has(ctx, HERMODTS_SPINE)
            && has(ctx, HERMODTS_ARM_BONE)
            && has(ctx, HERMODTS_THIGH_BONE)
        {
            ctx.take_items(MANAKIAS_TOTEM, 1);
            ctx.take_items(HERMODTS_SKULL, 1);
            ctx.take_items(HERMODTS_RIB_BONE, 1);
            ctx.take_items(HERMODTS_SPINE, 1);
            ctx.take_items(HERMODTS_ARM_BONE, 1);
            ctx.take_items(HERMODTS_THIGH_BONE, 1);
            ctx.give_items(HERMODTS_REMAINS1, 1);
            maybe_cond2(ctx, BRAKIS_REMAINS1, KIRUNAS_REMAINS1, TONARS_REMAINS1);
            "30515-04.html".to_string()
        } else {
            "30515-03.html".to_string()
        }
    } else if !has(ctx, MANAKIAS_TOTEM)
        && (has(ctx, HERMODTS_REMAINS1) || has(ctx, HERMODTS_REMAINS2) || has(ctx, VENDETTA_TOTEM))
    {
        "30515-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn orim_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, ORIMS_CONTRACT)
        && !has(ctx, BRAKIS_REMAINS1)
        && !has(ctx, BRAKIS_REMAINS2)
        && !has(ctx, VENDETTA_TOTEM)
    {
        "30630-01.html".to_string()
    } else if has(ctx, ORIMS_CONTRACT) {
        if ctx.quest_items_count(PORTAS_EYE)
            + ctx.quest_items_count(EXCUROS_SCALE)
            + ctx.quest_items_count(MORDEOS_TALON)
            < 30
        {
            "30630-05.html".to_string()
        } else {
            ctx.take_items(ORIMS_CONTRACT, 1);
            ctx.take_items(PORTAS_EYE, -1);
            ctx.take_items(EXCUROS_SCALE, -1);
            ctx.take_items(MORDEOS_TALON, -1);
            ctx.give_items(BRAKIS_REMAINS1, 1);
            maybe_cond2(ctx, HERMODTS_REMAINS1, KIRUNAS_REMAINS1, TONARS_REMAINS1);
            "30630-06.html".to_string()
        }
    } else if !has(ctx, ORIMS_CONTRACT)
        && (has(ctx, BRAKIS_REMAINS1) || has(ctx, BRAKIS_REMAINS2) || has(ctx, VENDETTA_TOTEM))
    {
        "30630-07.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn pekiron_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, PEKIRONS_TOTEM)
        && !has(ctx, TONARS_REMAINS1)
        && !has(ctx, TONARS_REMAINS2)
        && !has(ctx, VENDETTA_TOTEM)
    {
        "30682-01.html".to_string()
    } else if has(ctx, PEKIRONS_TOTEM) {
        if has(ctx, TONARS_SKULL)
            && has(ctx, TONARS_RIB_BONE)
            && has(ctx, TONARS_SPINE)
            && has(ctx, TONARS_ARM_BONE)
            && has(ctx, TONARS_THIGH_BONE)
        {
            ctx.take_items(PEKIRONS_TOTEM, 1);
            ctx.take_items(TONARS_SKULL, 1);
            ctx.take_items(TONARS_RIB_BONE, 1);
            ctx.take_items(TONARS_SPINE, 1);
            ctx.take_items(TONARS_ARM_BONE, 1);
            ctx.take_items(TONARS_THIGH_BONE, 1);
            ctx.give_items(TONARS_REMAINS1, 1);
            maybe_cond2(ctx, BRAKIS_REMAINS1, HERMODTS_REMAINS1, KIRUNAS_REMAINS1);
            "30682-04.html".to_string()
        } else {
            "30682-03.html".to_string()
        }
    } else if !has(ctx, PEKIRONS_TOTEM)
        && (has(ctx, TONARS_REMAINS1) || has(ctx, TONARS_REMAINS2) || has(ctx, VENDETTA_TOTEM))
    {
        "30682-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}
