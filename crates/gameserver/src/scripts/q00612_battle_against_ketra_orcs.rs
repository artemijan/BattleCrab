//! Battle against Ketra Orcs (612) — `quests/Q00612_BattleAgainstKetraOrcs`.
//! Ashas Varka Durai (31377, level 74+) — a Varka Silenos quartermaster — buys
//! Ketra Orc Molars off the enemy camp: 100 Molars trade for 20 Ketra Orc Seeds
//! (the Varka-alliance token). The mirror of [`Q00606`](super::q00606_battle_against_varka_silenos).
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const ASHAS: i32 = 31377;
const SEED: i32 = 7187;
const MOLAR: i32 = 7234;
const MIN_LEVEL: i32 = 74;
const MOLAR_COUNT: i64 = 100;

/// `MOBS`: per-mob Molar drop chance out of 1000.
fn molar_chance(npc_id: i32) -> Option<i32> {
    let v = match npc_id {
        21324 => 500,
        21327 => 510,
        21328 => 522,
        21329 => 519,
        21331 => 529,
        21332 => 529,
        21334 => 539,
        21336 => 548,
        21338 => 558,
        21339 => 568,
        21340 => 568,
        21342 => 578,
        21343 => 664,
        21345 => 713,
        21347 => 738,
        _ => return None,
    };
    Some(v)
}

pub struct Q00612BattleAgainstKetraOrcs;

impl QuestScript for Q00612BattleAgainstKetraOrcs {
    fn id(&self) -> i32 {
        612
    }
    fn name(&self) -> &'static str {
        "Q00612_BattleAgainstKetraOrcs"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00612_BattleAgainstKetraOrcs"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ASHAS]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ASHAS]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            21324, 21327, 21328, 21329, 21331, 21332, 21334, 21336, 21338, 21339, 21340, 21342,
            21343, 21345, 21347,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[MOLAR]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "31377-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "31377-06.html" => Some(event.to_string()),
            "31377-07.html" => {
                if ctx.quest_items_count(MOLAR) < MOLAR_COUNT {
                    Some("31377-08.html".to_string())
                } else {
                    ctx.take_items(MOLAR, MOLAR_COUNT);
                    ctx.give_items(SEED, 20);
                    Some(event.to_string())
                }
            }
            "31377-09.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMember(killer, 1)` — a cond-1 member. Port is killer-only
        // (G11 party deviation).
        if !ctx.has_qs() || !ctx.is_cond(1) {
            return;
        }
        if let Some(chance) = molar_chance(ctx.npc_id) {
            if ctx.roll(1000) < chance {
                ctx.give_items(MOLAR, 1);
                ctx.play_sound(quest_sounds::ITEMGET);
            }
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL { "31377-01.htm" } else { "31377-02.htm" }
                    .to_string(),
            );
        }
        if ctx.is_started() {
            return Some(
                if ctx.quest_items_count(MOLAR) > 0 { "31377-04.html" } else { "31377-05.html" }
                    .to_string(),
            );
        }
        Some(ctx.no_quest_html())
    }
}
