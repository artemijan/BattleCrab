//! Specialty Liquor Delivery (622) — `quests/Q00622_SpecialtyLiquorDelivery`.
//! Jeremy (31521, level 68+) hands the player 5 Special Drinks to deliver to
//! five bartenders in order — Boelin (31547), then Kuber/Crocus/Naff/Pulin —
//! each swapping a drink for a payment slip. Five slips back to Jeremy (cond 7),
//! then Lietta (31267) pays a weighted 1000-slot reward. A pure delivery chain,
//! `cond` 1 → 7, no kills.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const JEREMY: i32 = 31521;
const PULIN: i32 = 31543;
const NAFF: i32 = 31544;
const CROCUS: i32 = 31545;
const KUBER: i32 = 31546;
const BOELIN: i32 = 31547;
const LIETTA: i32 = 31267;
const SPECIAL_DRINK: i32 = 7197;
const SPECIAL_DRINK_PRICE: i32 = 7198;
// Rewards.
const QUICK_STEP_POTION: i32 = 734;
const SEALED_RING_OF_AURAKYRA: i32 = 6849;
const SEALED_SANDDRAGONS_EARING: i32 = 6847;
const SEALED_DRAGON_NECKLACE: i32 = 6851;
const MIN_LEVEL: i32 = 68;

/// `TALKERS.indexOf(npcId) + 2` — the cond at which each bartender accepts a
/// drink (they are visited in this order after Boelin).
fn talker_cond(npc_id: i32) -> Option<i32> {
    match npc_id {
        KUBER => Some(2),
        CROCUS => Some(3),
        NAFF => Some(4),
        PULIN => Some(5),
        _ => None,
    }
}

pub struct Q00622SpecialtyLiquorDelivery;

impl QuestScript for Q00622SpecialtyLiquorDelivery {
    fn id(&self) -> i32 {
        622
    }
    fn name(&self) -> &'static str {
        "Q00622_SpecialtyLiquorDelivery"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00622_SpecialtyLiquorDelivery"
    }
    fn start_npcs(&self) -> &[i32] {
        &[JEREMY]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[JEREMY, BOELIN, LIETTA, KUBER, CROCUS, NAFF, PULIN]
    }
    fn quest_items(&self) -> &[i32] {
        &[SPECIAL_DRINK, SPECIAL_DRINK_PRICE]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "31521-03.htm" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    ctx.give_items(SPECIAL_DRINK, 5);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "31521-06.html" => {
                if !ctx.is_cond(6) {
                    return None;
                }
                if ctx.quest_items_count(SPECIAL_DRINK_PRICE) >= 5 {
                    ctx.set_cond(7, true);
                    ctx.take_items(SPECIAL_DRINK_PRICE, -1);
                    Some(event.to_string())
                } else {
                    Some("31521-07.html".to_string())
                }
            }
            "31547-02.html" => {
                if !ctx.is_cond(1) {
                    return None;
                }
                if ctx.quest_items_count(SPECIAL_DRINK) > 0 {
                    ctx.set_cond(2, true);
                    ctx.take_items(SPECIAL_DRINK, 1);
                    ctx.give_items(SPECIAL_DRINK_PRICE, 1);
                    Some(event.to_string())
                } else {
                    Some("31547-03.html".to_string())
                }
            }
            "31543-02.html" | "31544-02.html" | "31545-02.html" | "31546-02.html" => {
                let cond = talker_cond(ctx.npc_id)?;
                if !ctx.is_cond(cond) {
                    return None;
                }
                if ctx.quest_items_count(SPECIAL_DRINK) > 0 {
                    ctx.set_cond(cond + 1, true);
                    ctx.take_items(SPECIAL_DRINK, 1);
                    ctx.give_items(SPECIAL_DRINK_PRICE, 1);
                    Some(event.to_string())
                } else {
                    Some(format!("{}-03.html", ctx.npc_id))
                }
            }
            "31267-02.html" => {
                if !ctx.is_cond(7) {
                    return None;
                }
                let rnd = ctx.roll(1000);
                if rnd < 800 {
                    ctx.reward_items(QUICK_STEP_POTION, 1);
                    ctx.give_adena(18800, true);
                } else if rnd < 880 {
                    ctx.reward_items(SEALED_RING_OF_AURAKYRA, 1);
                } else if rnd < 960 {
                    ctx.reward_items(SEALED_SANDDRAGONS_EARING, 1);
                } else {
                    ctx.reward_items(SEALED_DRAGON_NECKLACE, 1);
                }
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        match ctx.npc_id {
            JEREMY => {
                if ctx.is_created() {
                    return Some(
                        if ctx.player_level() >= MIN_LEVEL {
                            "31521-01.htm"
                        } else {
                            "31521-02.htm"
                        }
                        .to_string(),
                    );
                }
                if ctx.is_started() {
                    match ctx.cond() {
                        1 => return Some("31521-04.html".to_string()),
                        6 => {
                            if ctx.quest_items_count(SPECIAL_DRINK_PRICE) > 0 {
                                return Some("31521-05.html".to_string());
                            }
                        }
                        7 if ctx.quest_items_count(SPECIAL_DRINK) == 0 => {
                            return Some("31521-08.html".to_string());
                        }
                        _ => {}
                    }
                    return Some(ctx.no_quest_html());
                }
                if ctx.is_completed() {
                    return Some(ctx.already_completed_html());
                }
                Some(ctx.no_quest_html())
            }
            BOELIN => {
                if ctx.is_started() {
                    match ctx.cond() {
                        1 => {
                            if ctx.quest_items_count(SPECIAL_DRINK) >= 5 {
                                return Some("31547-01.html".to_string());
                            }
                        }
                        2 => return Some("31547-04.html".to_string()),
                        _ => {}
                    }
                }
                Some(ctx.no_quest_html())
            }
            KUBER | CROCUS | NAFF | PULIN => {
                if ctx.is_started() {
                    let cond = talker_cond(ctx.npc_id).unwrap();
                    if ctx.is_cond(cond) && ctx.quest_items_count(SPECIAL_DRINK_PRICE) > 0 {
                        return Some(format!("{}-01.html", ctx.npc_id));
                    } else if ctx.is_cond(cond + 1) {
                        return Some(format!("{}-04.html", ctx.npc_id));
                    }
                }
                Some(ctx.no_quest_html())
            }
            LIETTA => {
                if ctx.is_started() && ctx.is_cond(7) {
                    return Some("31267-01.html".to_string());
                }
                Some(ctx.no_quest_html())
            }
            _ => Some(ctx.no_quest_html()),
        }
    }
}
