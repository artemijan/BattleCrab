//! Brigands Sweep (292) — `quests/Q00292_BrigandsSweep`. Spiron (30532) sends a
//! Dwarf (level 5–18) to cull the goblins of Gludin: necklaces/pendants/lord
//! pendants pay 6/8/10 adena each (+1000 for 10+ turned in at once). A rarer
//! kill path yields a Suspicious Memo — three of them assemble into a
//! Suspicious Contract, worth 100 adena from Spiron and 620 from Balanki
//! (30533). `addCondMaxLevel(18)`.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const SPIRON: i32 = 30532;
const BALANKI: i32 = 30533;
const GOBLIN_NECKLACE: i32 = 1483;
const GOBLIN_PENDANT: i32 = 1484;
const GOBLIN_LORD_PENDANT: i32 = 1485;
const SUSPICIOUS_MEMO: i32 = 1486;
const SUSPICIOUS_CONTRACT: i32 = 1487;
const MIN_LEVEL: i32 = 5;
const RACE_DWARF: i32 = 4;

/// `MOB_ITEM_DROP` — which goblin drops which token.
fn mob_item(npc_id: i32) -> Option<i32> {
    match npc_id {
        20322 => Some(GOBLIN_NECKLACE),     // Goblin Brigand
        20323 => Some(GOBLIN_PENDANT),      // Goblin Brigand Leader
        20324 => Some(GOBLIN_NECKLACE),     // Goblin Brigand Lieutenant
        20327 => Some(GOBLIN_NECKLACE),     // Goblin Snooper
        20528 => Some(GOBLIN_LORD_PENDANT), // Goblin Lord
        _ => None,
    }
}

/// `hasAtLeastOneQuestItem(getRegisteredItemIds())`.
fn has_any_registered(ctx: &QuestCtx) -> bool {
    [GOBLIN_NECKLACE, GOBLIN_PENDANT, GOBLIN_LORD_PENDANT, SUSPICIOUS_MEMO, SUSPICIOUS_CONTRACT]
        .iter()
        .any(|&id| ctx.quest_items_count(id) > 0)
}

pub struct Q00292BrigandsSweep;

impl QuestScript for Q00292BrigandsSweep {
    fn id(&self) -> i32 {
        292
    }
    fn name(&self) -> &'static str {
        "Q00292_BrigandsSweep"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00292_BrigandsSweep"
    }
    fn start_npcs(&self) -> &[i32] {
        &[SPIRON]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[SPIRON, BALANKI]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[20322, 20323, 20324, 20327, 20528]
    }
    fn quest_items(&self) -> &[i32] {
        &[GOBLIN_NECKLACE, GOBLIN_PENDANT, GOBLIN_LORD_PENDANT, SUSPICIOUS_MEMO, SUSPICIOUS_CONTRACT]
    }

    /// `addCondMaxLevel(18, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 18).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30532-03.htm" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "30532-06.html" => {
                if ctx.is_started() {
                    ctx.exit_quest(true, true);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "30532-07.html" => {
                if ctx.is_started() {
                    Some(event.to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `Util.checkIfInRange(ALT_PARTY_RANGE, npc, killer, true)` is trivially
        // true for the killer; port is killer-only (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let Some(item) = mob_item(ctx.npc_id) else {
            return;
        };
        let chance = ctx.roll(10);
        if chance > 5 {
            ctx.give_item_randomly(item, 1, 0, 1.0, true);
        } else if ctx.is_cond(1)
            && chance > 4
            && ctx.quest_items_count(SUSPICIOUS_CONTRACT) == 0
            && ctx.quest_items_count(SUSPICIOUS_MEMO) < 3
        {
            // The third memo (limit 3) assembles the contract.
            if ctx.give_item_randomly(SUSPICIOUS_MEMO, 1, 3, 1.0, false) {
                ctx.play_sound(quest_sounds::ITEMGET);
                ctx.give_items(SUSPICIOUS_CONTRACT, 1);
                ctx.take_items(SUSPICIOUS_MEMO, -1);
                ctx.set_cond(2, true);
            } else {
                ctx.play_sound(quest_sounds::ITEMGET);
            }
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        match ctx.npc_id {
            SPIRON => {
                if ctx.is_created() {
                    return Some(
                        if ctx.player_race() != RACE_DWARF {
                            "30532-00.htm"
                        } else if ctx.player_level() >= MIN_LEVEL {
                            "30532-02.htm"
                        } else {
                            "30532-01.htm"
                        }
                        .to_string(),
                    );
                }
                if ctx.is_started() {
                    if !has_any_registered(ctx) {
                        return Some("30532-04.html".to_string());
                    }
                    let necklaces = ctx.quest_items_count(GOBLIN_NECKLACE);
                    let pendants = ctx.quest_items_count(GOBLIN_PENDANT);
                    let lord = ctx.quest_items_count(GOBLIN_LORD_PENDANT);
                    let sum = necklaces + pendants + lord;
                    if sum > 0 {
                        ctx.give_adena(
                            necklaces * 6 + pendants * 8 + lord * 10 + if sum >= 10 { 1000 } else { 0 },
                            true,
                        );
                        ctx.take_items(GOBLIN_NECKLACE, -1);
                        ctx.take_items(GOBLIN_PENDANT, -1);
                        ctx.take_items(GOBLIN_LORD_PENDANT, -1);
                    }
                    let has_memo_or_contract = ctx.quest_items_count(SUSPICIOUS_MEMO) > 0
                        || ctx.quest_items_count(SUSPICIOUS_CONTRACT) > 0;
                    if sum > 0 && !has_memo_or_contract {
                        return Some("30532-05.html".to_string());
                    }
                    let memos = ctx.quest_items_count(SUSPICIOUS_MEMO);
                    if memos == 0 && ctx.quest_items_count(SUSPICIOUS_CONTRACT) > 0 {
                        // Retail: the contract pays in two pieces (100 here, 620
                        // at Balanki) when both conditions are met.
                        ctx.give_adena(100, true);
                        ctx.take_items(SUSPICIOUS_CONTRACT, -1);
                        return Some("30532-10.html".to_string());
                    }
                    if memos == 1 {
                        return Some("30532-08.html".to_string());
                    }
                    if memos >= 2 {
                        return Some("30532-09.html".to_string());
                    }
                }
                Some(ctx.no_quest_html())
            }
            BALANKI => {
                if ctx.is_started() {
                    if ctx.quest_items_count(SUSPICIOUS_CONTRACT) > 0 {
                        ctx.give_adena(620, true);
                        ctx.take_items(SUSPICIOUS_CONTRACT, -1);
                        return Some("30533-02.html".to_string());
                    }
                    return Some("30533-01.html".to_string());
                }
                Some(ctx.no_quest_html())
            }
            _ => Some(ctx.no_quest_html()),
        }
    }
}
