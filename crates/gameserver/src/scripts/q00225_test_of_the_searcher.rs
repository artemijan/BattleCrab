//! Test of the Searcher (225) — `quests/Q00225_TestOfTheSearcher`. The scout /
//! scavenger 2nd-class proof (Rogue / Elven Scout / Assassin / Scavenger,
//! level 39+). Master Luther points the hopeful at Captain Alex, who runs them
//! through a long detective errand for Militiaman Leirynn — collecting Delu
//! totems and Chief Kalkis's fang, chasing a wine catalog and red spore dust,
//! then reassembling four torn map pieces into the map that leads to a buried
//! chest of gold, all to earn the Mark of the Searcher.
//!
//! Pure item-gated state machine (`memoState` is set to 1 at accept but never
//! read — `cond` and the item chain carry everything). Two mechanics of note:
//!   * The two treasure-map halves (Solt's and Makel's) are each assembled from
//!     four torn pieces dropped by Road Scavengers / Hangman Trees; the Hangman
//!     drops (and the 3→map conversion) are 50/50 rolls, the Scavenger ones are
//!     deterministic. Both halves in hand advances to cond 15.
//!   * The Ancient Tree hands over a Rusted Key and conjures a Strong Wooden
//!     Chest beside itself; opening the chest (`deleteMe`) trades the key for 20
//!     Gold Bars.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const CAPTAIN_ALEX: i32 = 30291;
const TYRA: i32 = 30420;
const TREE: i32 = 30627;
const STRONG_WOODEN_CHEST: i32 = 30628;
const MASTER_LUTHER: i32 = 30690;
const MILITIAMAN_LEIRYNN: i32 = 30728;
const DRUNKARD_BORYS: i32 = 30729;
const BODYGUARD_JAX: i32 = 30730;
// Items
const LUTHERS_LETTER: i32 = 2784;
const ALEXS_WARRANT: i32 = 2785;
const LEIRYNNS_1ST_ORDER: i32 = 2786;
const DELU_TOTEM: i32 = 2787;
const LEIRYNNS_2ND_ORDER: i32 = 2788;
const CHIEF_KALKIS_FANG: i32 = 2789;
const LEIRYNNS_REPORT: i32 = 2790;
const STRINGE_MAP: i32 = 2791;
const LAMBERTS_MAP: i32 = 2792;
const ALEXS_LETTER: i32 = 2793;
const ALEXS_ORDER: i32 = 2794;
const WINE_CATALOG: i32 = 2795;
const TYRAS_CONTRACT: i32 = 2796;
const RED_SPORE_DUST: i32 = 2797;
const MALRUKIAN_WINE: i32 = 2798;
const OLD_ORDER: i32 = 2799;
const JAXS_DIARY: i32 = 2800;
const TORN_MAP_PIECE_1ST: i32 = 2801;
const TORN_MAP_PIECE_2ND: i32 = 2802;
const SOLTS_MAP: i32 = 2803;
const MAKELS_MAP: i32 = 2804;
const COMBINED_MAP: i32 = 2805;
const RUSTED_KEY: i32 = 2806;
const GOLD_BAR: i32 = 2807;
const ALEXS_RECOMMEND: i32 = 2808;
// Reward
const MARK_OF_SEARCHER: i32 = 2809;
// Monsters
const HANGMAN_TREE: i32 = 20144;
const ROAD_SCAVENGER: i32 = 20551;
const GIANT_FUNGUS: i32 = 20555;
const DELU_LIZARDMAN_SHAMAN: i32 = 20781;
const NEER_BODYGUARD: i32 = 27092;
const DELU_CHIEF_KALKIS: i32 = 27093;
// Misc
const MIN_LEVEL: i32 = 39;
const ROGUE: i32 = 7;
const ELVEN_SCOUT: i32 = 22;
const ASSASSIN: i32 = 35;
const SCAVENGER: i32 = 54;

pub struct Q00225TestOfTheSearcher;

impl QuestScript for Q00225TestOfTheSearcher {
    fn id(&self) -> i32 {
        225
    }
    fn name(&self) -> &'static str {
        "Q00225_TestOfTheSearcher"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00225_TestOfTheSearcher"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MASTER_LUTHER]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            MASTER_LUTHER,
            CAPTAIN_ALEX,
            TYRA,
            TREE,
            STRONG_WOODEN_CHEST,
            MILITIAMAN_LEIRYNN,
            DRUNKARD_BORYS,
            BODYGUARD_JAX,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            HANGMAN_TREE,
            ROAD_SCAVENGER,
            GIANT_FUNGUS,
            DELU_LIZARDMAN_SHAMAN,
            NEER_BODYGUARD,
            DELU_CHIEF_KALKIS,
        ]
    }
    fn attack_npcs(&self) -> &[i32] {
        &[DELU_LIZARDMAN_SHAMAN]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            LUTHERS_LETTER,
            ALEXS_WARRANT,
            LEIRYNNS_1ST_ORDER,
            DELU_TOTEM,
            LEIRYNNS_2ND_ORDER,
            CHIEF_KALKIS_FANG,
            LEIRYNNS_REPORT,
            STRINGE_MAP,
            LAMBERTS_MAP,
            ALEXS_LETTER,
            ALEXS_ORDER,
            WINE_CATALOG,
            TYRAS_CONTRACT,
            RED_SPORE_DUST,
            MALRUKIAN_WINE,
            OLD_ORDER,
            JAXS_DIARY,
            TORN_MAP_PIECE_1ST,
            TORN_MAP_PIECE_2ND,
            SOLTS_MAP,
            MAKELS_MAP,
            COMBINED_MAP,
            RUSTED_KEY,
            GOLD_BAR,
            ALEXS_RECOMMEND,
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
                    ctx.give_items(LUTHERS_LETTER, 1);
                }
                None
            }
            "30291-05.html" | "30291-01t.html" | "30291-06.html" | "30730-01a.html"
            | "30730-01b.html" | "30730-01c.html" | "30730-02.html" | "30730-02a.html"
            | "30730-02b.html" => Some(event.to_string()),
            "30291-07.html" => {
                if ctx.quest_items_count(LEIRYNNS_REPORT) > 0
                    && ctx.quest_items_count(STRINGE_MAP) > 0
                {
                    ctx.take_items(LEIRYNNS_REPORT, 1);
                    ctx.take_items(STRINGE_MAP, 1);
                    ctx.give_items(LAMBERTS_MAP, 1);
                    ctx.give_items(ALEXS_LETTER, 1);
                    ctx.give_items(ALEXS_ORDER, 1);
                    ctx.set_cond(8, true);
                    return Some(event.to_string());
                }
                None
            }
            "30420-01a.html" => {
                if ctx.quest_items_count(WINE_CATALOG) > 0 {
                    ctx.take_items(WINE_CATALOG, 1);
                    ctx.give_items(TYRAS_CONTRACT, 1);
                    ctx.set_cond(10, true);
                    return Some(event.to_string());
                }
                None
            }
            "30627-01a.html" => {
                // `if (npc.getSummonedNpcCount() < 5)` — note the guard wraps
                // the **whole** block in Java, so a sixth attempt gets neither
                // the key nor the chest nor the cond bump.
                if ctx.summoned_npc_count() < 5 {
                    ctx.give_items(RUSTED_KEY, 1);
                    ctx.spawn_near_npc(STRONG_WOODEN_CHEST, true);
                    ctx.set_cond(17, true);
                }
                Some(event.to_string())
            }
            "30628-01a.html" => {
                ctx.take_items(RUSTED_KEY, 1);
                ctx.give_items(GOLD_BAR, 20);
                ctx.set_cond(18, true);
                ctx.delete_npc();
                Some(event.to_string())
            }
            "30730-01d.html" => {
                if ctx.quest_items_count(OLD_ORDER) > 0 {
                    ctx.take_items(OLD_ORDER, 1);
                    ctx.give_items(JAXS_DIARY, 1);
                    ctx.set_cond(14, true);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs()
            && ctx.is_started()
            && ctx.npc_script_value() == 0
            && ctx.quest_items_count(LEIRYNNS_1ST_ORDER) > 0
        {
            ctx.set_npc_script_value(1);
            ctx.spawn_attacker(NEER_BODYGUARD, true);
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            HANGMAN_TREE => {
                if ctx.quest_items_count(JAXS_DIARY) > 0
                    && ctx.quest_items_count(MAKELS_MAP) == 0
                    && ctx.quest_items_count(TORN_MAP_PIECE_2ND) < 4
                {
                    if ctx.quest_items_count(TORN_MAP_PIECE_2ND) < 3 {
                        if ctx.roll(100) < 50 {
                            ctx.give_items(TORN_MAP_PIECE_2ND, 1);
                            ctx.play_sound(quest_sounds::ITEMGET);
                        }
                    } else if ctx.roll(100) < 50 {
                        ctx.take_items(TORN_MAP_PIECE_2ND, -1);
                        ctx.give_items(MAKELS_MAP, 1);
                        ctx.play_sound(quest_sounds::MIDDLE);
                        if ctx.quest_items_count(SOLTS_MAP) >= 1 {
                            ctx.set_cond(15, false);
                        }
                    }
                }
            }
            ROAD_SCAVENGER => {
                if ctx.quest_items_count(JAXS_DIARY) > 0
                    && ctx.quest_items_count(SOLTS_MAP) == 0
                    && ctx.quest_items_count(TORN_MAP_PIECE_1ST) < 4
                {
                    if ctx.quest_items_count(TORN_MAP_PIECE_1ST) < 3 {
                        ctx.give_items(TORN_MAP_PIECE_1ST, 1);
                        ctx.play_sound(quest_sounds::ITEMGET);
                    } else {
                        ctx.take_items(TORN_MAP_PIECE_1ST, -1);
                        ctx.give_items(SOLTS_MAP, 1);
                        ctx.play_sound(quest_sounds::MIDDLE);
                        if ctx.quest_items_count(MAKELS_MAP) >= 1 {
                            ctx.set_cond(15, false);
                        }
                    }
                }
            }
            GIANT_FUNGUS => {
                if ctx.quest_items_count(TYRAS_CONTRACT) > 0
                    && ctx.quest_items_count(RED_SPORE_DUST) < 10
                {
                    ctx.give_items(RED_SPORE_DUST, 1);
                    if ctx.quest_items_count(RED_SPORE_DUST) >= 10 {
                        ctx.set_cond(11, true);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            DELU_LIZARDMAN_SHAMAN => {
                if ctx.quest_items_count(LEIRYNNS_1ST_ORDER) > 0
                    && ctx.quest_items_count(DELU_TOTEM) < 10
                {
                    ctx.give_items(DELU_TOTEM, 1);
                    // NB: Java checks RED_SPORE_DUST here, not DELU_TOTEM — a
                    // copy-paste quirk kept faithfully. Red Spore Dust comes
                    // from a much later leg, so cond 4 never actually fires
                    // from this kill; the totem leg advances at Leirynn's
                    // turn-in (totem == 10 → cond 5) instead.
                    if ctx.quest_items_count(RED_SPORE_DUST) >= 10 {
                        ctx.set_cond(4, true);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            DELU_CHIEF_KALKIS => {
                if ctx.quest_items_count(LEIRYNNS_2ND_ORDER) > 0
                    && ctx.quest_items_count(CHIEF_KALKIS_FANG) == 0
                    && ctx.quest_items_count(STRINGE_MAP) == 0
                {
                    ctx.give_items(CHIEF_KALKIS_FANG, 1);
                    ctx.give_items(STRINGE_MAP, 1);
                    ctx.set_cond(6, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == MASTER_LUTHER {
                let class = ctx.player_class_id();
                if class == ROGUE || class == ELVEN_SCOUT || class == ASSASSIN || class == SCAVENGER
                {
                    if ctx.player_level() >= MIN_LEVEL {
                        return Some(if class == SCAVENGER {
                            "30690-04.htm".to_string()
                        } else {
                            "30690-03.htm".to_string()
                        });
                    }
                    return Some("30690-02.html".to_string());
                }
                return Some("30690-01.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == MASTER_LUTHER {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        // Started.
        match ctx.npc_id {
            MASTER_LUTHER => Some(luther_talk(ctx)),
            CAPTAIN_ALEX => Some(alex_talk(ctx)),
            TYRA => Some(tyra_talk(ctx)),
            TREE => Some(tree_talk(ctx)),
            STRONG_WOODEN_CHEST => Some(chest_talk(ctx)),
            MILITIAMAN_LEIRYNN => Some(leirynn_talk(ctx)),
            DRUNKARD_BORYS => Some(borys_talk(ctx)),
            BODYGUARD_JAX => Some(jax_talk(ctx)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

fn luther_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, LUTHERS_LETTER) && !has(ctx, ALEXS_RECOMMEND) {
        "30690-06.html".to_string()
    } else if !has(ctx, LUTHERS_LETTER) && !has(ctx, ALEXS_RECOMMEND) {
        "30690-07.html".to_string()
    } else if !has(ctx, LUTHERS_LETTER) && has(ctx, ALEXS_RECOMMEND) {
        ctx.give_adena(161806, true);
        ctx.give_items(MARK_OF_SEARCHER, 1);
        ctx.add_exp_and_sp(894888, 61408);
        ctx.exit_quest(false, true);
        ctx.social_action(3);
        "30690-08.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn alex_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, LUTHERS_LETTER) {
        ctx.take_items(LUTHERS_LETTER, 1);
        ctx.give_items(ALEXS_WARRANT, 1);
        ctx.set_cond(2, true);
        "30291-01.html".to_string()
    } else if has(ctx, ALEXS_WARRANT) {
        "30291-02.html".to_string()
    } else if has(ctx, LEIRYNNS_1ST_ORDER) || has(ctx, LEIRYNNS_2ND_ORDER) {
        "30291-03.html".to_string()
    } else if has(ctx, LEIRYNNS_REPORT) {
        "30291-04.html".to_string()
    } else if has(ctx, ALEXS_ORDER) {
        if has(ctx, ALEXS_LETTER) {
            "30291-08.html".to_string()
        } else if has(ctx, OLD_ORDER) || has(ctx, JAXS_DIARY) {
            "30291-09.html".to_string()
        } else if has(ctx, COMBINED_MAP) {
            if ctx.quest_items_count(GOLD_BAR) == 20 {
                ctx.take_items(ALEXS_ORDER, 1);
                ctx.take_items(COMBINED_MAP, 1);
                ctx.take_items(GOLD_BAR, -1);
                ctx.give_items(ALEXS_RECOMMEND, 1);
                ctx.set_cond(19, true);
                "30291-11.html".to_string()
            } else {
                "30291-10.html".to_string()
            }
        } else {
            ctx.no_quest_html()
        }
    } else if has(ctx, ALEXS_RECOMMEND) {
        "30291-12.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn tyra_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, WINE_CATALOG) {
        "30420-01.html".to_string()
    } else if has(ctx, TYRAS_CONTRACT) {
        if ctx.quest_items_count(RED_SPORE_DUST) < 10 {
            "30420-02.html".to_string()
        } else {
            ctx.take_items(TYRAS_CONTRACT, 1);
            ctx.take_items(RED_SPORE_DUST, -1);
            ctx.give_items(MALRUKIAN_WINE, 1);
            ctx.set_cond(12, true);
            "30420-03.html".to_string()
        }
    } else if has(ctx, JAXS_DIARY)
        || has(ctx, OLD_ORDER)
        || has(ctx, COMBINED_MAP)
        || has(ctx, ALEXS_RECOMMEND)
        || has(ctx, MALRUKIAN_WINE)
    {
        "30420-04.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn tree_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, COMBINED_MAP) {
        // Two different states land on the same page: nothing collected yet,
        // and the key plus a full 20 bars. Java writes them as separate
        // branches; kept as one `||` so the pair stays visible.
        if (!has(ctx, RUSTED_KEY) && !has(ctx, GOLD_BAR))
            || (has(ctx, RUSTED_KEY) && ctx.quest_items_count(GOLD_BAR) >= 20)
        {
            "30627-01.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn chest_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, RUSTED_KEY) {
        "30628-01.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn leirynn_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ALEXS_WARRANT) {
        ctx.take_items(ALEXS_WARRANT, 1);
        ctx.give_items(LEIRYNNS_1ST_ORDER, 1);
        ctx.set_cond(3, true);
        "30728-01.html".to_string()
    } else if has(ctx, LEIRYNNS_1ST_ORDER) {
        if ctx.quest_items_count(DELU_TOTEM) < 10 {
            "30728-02.html".to_string()
        } else {
            ctx.take_items(LEIRYNNS_1ST_ORDER, 1);
            ctx.take_items(DELU_TOTEM, -1);
            ctx.give_items(LEIRYNNS_2ND_ORDER, 1);
            ctx.set_cond(5, true);
            "30728-03.html".to_string()
        }
    } else if has(ctx, LEIRYNNS_2ND_ORDER) {
        if !has(ctx, CHIEF_KALKIS_FANG) {
            "30728-04.html".to_string()
        } else {
            ctx.take_items(LEIRYNNS_2ND_ORDER, 1);
            ctx.take_items(CHIEF_KALKIS_FANG, 1);
            ctx.give_items(LEIRYNNS_REPORT, 1);
            ctx.set_cond(7, true);
            "30728-05.html".to_string()
        }
    } else if has(ctx, LEIRYNNS_REPORT) {
        "30728-06.html".to_string()
    } else if has(ctx, ALEXS_RECOMMEND) || has(ctx, ALEXS_ORDER) {
        "30728-07.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn borys_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, ALEXS_LETTER) {
        ctx.take_items(ALEXS_LETTER, 1);
        ctx.give_items(WINE_CATALOG, 1);
        ctx.set_cond(9, true);
        "30729-01.html".to_string()
    } else if has(ctx, WINE_CATALOG) && !has(ctx, MALRUKIAN_WINE) {
        "30729-02.html".to_string()
    } else if has(ctx, MALRUKIAN_WINE) && !has(ctx, WINE_CATALOG) {
        ctx.take_items(MALRUKIAN_WINE, 1);
        ctx.give_items(OLD_ORDER, 1);
        ctx.set_cond(13, true);
        "30729-03.html".to_string()
    } else if has(ctx, OLD_ORDER) {
        "30729-04.html".to_string()
    } else if has(ctx, JAXS_DIARY) || has(ctx, COMBINED_MAP) || has(ctx, ALEXS_RECOMMEND) {
        "30729-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn jax_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, OLD_ORDER) {
        "30730-01.html".to_string()
    } else if has(ctx, JAXS_DIARY) {
        let maps = ctx.quest_items_count(SOLTS_MAP) + ctx.quest_items_count(MAKELS_MAP);
        if maps < 2 {
            "30730-02.html".to_string()
        } else {
            ctx.take_items(LAMBERTS_MAP, 1);
            ctx.take_items(JAXS_DIARY, 1);
            ctx.take_items(SOLTS_MAP, 1);
            ctx.take_items(MAKELS_MAP, -1);
            ctx.give_items(COMBINED_MAP, 1);
            ctx.set_cond(16, true);
            "30730-03.html".to_string()
        }
    } else if has(ctx, COMBINED_MAP) || has(ctx, ALEXS_RECOMMEND) {
        "30730-04.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}
