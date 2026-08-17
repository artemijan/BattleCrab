//! Test of the Duelist (222) — `quests/Q00222_TestOfTheDuelist`. The Warrior
//! 2nd-class proof: Duelist Kaien (30623, level 39+, fighter classes) sends the
//! player across all five territories with an Order per region to collect **10
//! each of ten trophies** (stage 1), assembles them into a Final Order, then a
//! second hunt for **3 each of five tougher trophies** (stage 2). Completing
//! both yields the **Mark of Duelist** (2762), which the village-master
//! Gladiator/Warlord transfer consumes.
//!
//! Each stage uses `memoStateEx(1)` as an anti-shortcut **kill counter**: the
//! stage only completes on a kill where enough monsters have actually been
//! slain (`>= 9` in stage 1, `>= 5` in stage 2) *and* every trophy is capped —
//! so stockpiling items some other way can't finish it.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const DUELIST_KAIEN: i32 = 30623;

const ORDER_GLUDIO: i32 = 2763;
const ORDER_DION: i32 = 2764;
const ORDER_GIRAN: i32 = 2765;
const ORDER_OREN: i32 = 2766;
const ORDER_ADEN: i32 = 2767;
const FINAL_ORDER: i32 = 2778;
const MARK_OF_DUELIST: i32 = 2762;

// Stage-1 trophies (cap 10 each; 100 total).
const PUNCHERS_SHARD: i32 = 2768;
const NOBLE_ANTS_FEELER: i32 = 2769;
const DRONES_CHITIN: i32 = 2770;
const DEAD_SEEKER_FANG: i32 = 2771;
const OVERLORD_NECKLACE: i32 = 2772;
const FETTERED_SOULS_CHAIN: i32 = 2773;
const CHIEDS_AMULET: i32 = 2774;
const ENCHANTED_EYE_MEAT: i32 = 2775;
const TAMRIN_ORCS_RING: i32 = 2776;
const TAMRIN_ORCS_ARROW: i32 = 2777;
const STAGE1_TROPHIES: [i32; 10] = [
    PUNCHERS_SHARD,
    NOBLE_ANTS_FEELER,
    DRONES_CHITIN,
    DEAD_SEEKER_FANG,
    OVERLORD_NECKLACE,
    FETTERED_SOULS_CHAIN,
    CHIEDS_AMULET,
    ENCHANTED_EYE_MEAT,
    TAMRIN_ORCS_RING,
    TAMRIN_ORCS_ARROW,
];

// Stage-2 trophies (cap 3 each; 15 total).
const EXCUROS_SKIN: i32 = 2779;
const KRATORS_SHARD: i32 = 2780;
const GRANDIS_SKIN: i32 = 2781;
const TIMAK_ORCS_BELT: i32 = 2782;
const LAKINS_MACE: i32 = 2783;
const STAGE2_TROPHIES: [i32; 5] = [
    EXCUROS_SKIN,
    KRATORS_SHARD,
    GRANDIS_SKIN,
    TIMAK_ORCS_BELT,
    LAKINS_MACE,
];

const MIN_LEVEL: i32 = 39;
// Eligible starting classes: Warrior, Elven Knight, Palus Knight, Orc Monk.
const ELIGIBLE_CLASSES: [i32; 4] = [1, 19, 32, 47];

/// Stage-1 kill: npc → (trophy, the region Order it needs).
fn stage1_mob(npc_id: i32) -> Option<(i32, i32)> {
    Some(match npc_id {
        20085 => (PUNCHERS_SHARD, ORDER_GLUDIO),      // Puncher
        20090 => (NOBLE_ANTS_FEELER, ORDER_GLUDIO),   // Noble Ant Leader
        20202 => (DEAD_SEEKER_FANG, ORDER_DION),      // Dead Seeker
        20234 => (DRONES_CHITIN, ORDER_DION),         // Marsh Stakato Drone
        20270 => (OVERLORD_NECKLACE, ORDER_GIRAN),    // Breka Orc Overlord
        20552 => (FETTERED_SOULS_CHAIN, ORDER_GIRAN), // Fettered Soul
        20564 => (ENCHANTED_EYE_MEAT, ORDER_OREN),    // Enchanted Monster Eye
        20582 => (CHIEDS_AMULET, ORDER_OREN),         // Leto Lizardman Overlord
        20601 => (TAMRIN_ORCS_RING, ORDER_ADEN),      // Tamlin Orc
        20602 => (TAMRIN_ORCS_ARROW, ORDER_ADEN),     // Tamlin Orc Archer
        _ => return None,
    })
}

/// Stage-2 kill: npc → trophy (all gated on holding the Final Order).
fn stage2_mob(npc_id: i32) -> Option<i32> {
    Some(match npc_id {
        20214 => EXCUROS_SKIN,    // Excuro
        20217 => KRATORS_SHARD,   // Krator
        20554 => GRANDIS_SKIN,    // Grandis
        20588 => TIMAK_ORCS_BELT, // Timak Orc Overlord
        20604 => LAKINS_MACE,     // Lakin
        _ => return None,
    })
}

fn total(ctx: &QuestCtx, ids: &[i32]) -> i64 {
    ids.iter().map(|&id| ctx.quest_items_count(id)).sum()
}

pub struct Q00222TestOfTheDuelist;

impl QuestScript for Q00222TestOfTheDuelist {
    fn id(&self) -> i32 {
        222
    }
    fn name(&self) -> &'static str {
        "Q00222_TestOfTheDuelist"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00222_TestOfTheDuelist"
    }
    fn start_npcs(&self) -> &[i32] {
        &[DUELIST_KAIEN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[DUELIST_KAIEN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            20085, 20090, 20202, 20214, 20217, 20234, 20270, 20552, 20554, 20564, 20582, 20588,
            20601, 20602, 20604,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            ORDER_GLUDIO,
            ORDER_DION,
            ORDER_GIRAN,
            ORDER_OREN,
            ORDER_ADEN,
            PUNCHERS_SHARD,
            NOBLE_ANTS_FEELER,
            DRONES_CHITIN,
            DEAD_SEEKER_FANG,
            OVERLORD_NECKLACE,
            FETTERED_SOULS_CHAIN,
            CHIEDS_AMULET,
            ENCHANTED_EYE_MEAT,
            TAMRIN_ORCS_RING,
            TAMRIN_ORCS_ARROW,
            FINAL_ORDER,
            EXCUROS_SKIN,
            KRATORS_SHARD,
            GRANDIS_SKIN,
            TIMAK_ORCS_BELT,
            LAKINS_MACE,
        ]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ELIGIBLE_CLASSES.contains(&ctx.player_class_id()) {
                return Some(
                    if ctx.player_level() >= MIN_LEVEL {
                        "30623-03.htm"
                    } else {
                        "30623-01.html"
                    }
                    .to_string(),
                );
            }
            return Some("30623-02.html".to_string());
        }
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        // Stage 1: still holding the five region Orders.
        let has_orders = [
            ORDER_GLUDIO,
            ORDER_DION,
            ORDER_GIRAN,
            ORDER_OREN,
            ORDER_ADEN,
        ]
        .iter()
        .all(|&id| ctx.quest_items_count(id) > 0);
        if has_orders {
            return Some(
                if total(ctx, &STAGE1_TROPHIES) == 100 {
                    "30623-13.html"
                } else {
                    "30623-14.html"
                }
                .to_string(),
            );
        }
        // Stage 2: the Final Order.
        if ctx.quest_items_count(FINAL_ORDER) > 0 {
            if total(ctx, &STAGE2_TROPHIES) == 15 {
                ctx.give_adena(161806, true);
                ctx.give_items(MARK_OF_DUELIST, 1);
                ctx.add_exp_and_sp(894888, 61408);
                ctx.exit_quest(false, true);
                ctx.social_action(3);
                return Some("30623-18.html".to_string());
            }
            return Some("30623-17.html".to_string());
        }
        Some(ctx.no_quest_html())
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
                    for order in [
                        ORDER_GLUDIO,
                        ORDER_DION,
                        ORDER_GIRAN,
                        ORDER_OREN,
                        ORDER_ADEN,
                    ] {
                        ctx.give_items(order, 1);
                    }
                    ctx.play_sound(quest_sounds::MIDDLE);
                }
                None
            }
            "30623-04.htm" => {
                // Orcs are steered elsewhere (Orc Monk aside).
                Some(
                    if ctx.player_race() != 3 {
                        "30623-04.htm"
                    } else {
                        "30623-05.htm"
                    }
                    .to_string(),
                )
            }
            "30623-06.htm" | "30623-07.html" | "30623-09.html" | "30623-10.html"
            | "30623-11.html" | "30623-12.html" | "30623-15.html" => Some(event.to_string()),
            "30623-08.html" => {
                ctx.set_cond(2, true);
                Some(event.to_string())
            }
            "30623-16.html" => {
                for id in STAGE1_TROPHIES {
                    ctx.take_items(id, -1);
                }
                for order in [
                    ORDER_GLUDIO,
                    ORDER_DION,
                    ORDER_GIRAN,
                    ORDER_OREN,
                    ORDER_ADEN,
                ] {
                    ctx.take_items(order, 1);
                }
                ctx.give_items(FINAL_ORDER, 1);
                ctx.set_memo_state(2);
                ctx.set_cond(4, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        if let Some((trophy, order)) = stage1_mob(ctx.npc_id) {
            if ctx.memo_state() == 1 && ctx.quest_items_count(order) > 0 {
                let i0 = ctx.memo_state_ex(1);
                ctx.set_memo_state_ex(1, i0 + 1);
                if ctx.give_item_randomly(trophy, 1, 10, 1.0, true)
                    && total(ctx, &STAGE1_TROPHIES) == 100
                {
                    if i0 >= 9 {
                        ctx.set_cond(3, false);
                    }
                    ctx.set_memo_state_ex(1, 0);
                }
            }
        } else if let Some(trophy) = stage2_mob(ctx.npc_id)
            && ctx.memo_state() == 2
            && ctx.quest_items_count(FINAL_ORDER) > 0
        {
            let i0 = ctx.memo_state_ex(1);
            ctx.set_memo_state_ex(1, i0 + 1);
            if ctx.give_item_randomly(trophy, 1, 3, 1.0, true) && total(ctx, &STAGE2_TROPHIES) == 15
            {
                if i0 >= 5 {
                    ctx.set_cond(5, false);
                }
                ctx.set_memo_state_ex(1, 0);
            }
        }
    }
}
