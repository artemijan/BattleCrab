//! Skirmish with the Werewolves (274) — `quests/Q00274_SkirmishWithTheWerewolves`.
//! Orc-only and **gated on holding a Necklace of Valor or Courage** (from
//! Q00271): Brukurse (30569) wants 40 Werewolf Heads (+ rare Totems) for 200
//! adena. Level 9–18.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;
const BRUKURSE: i32 = 30569;
const MONSTERS: [i32; 2] = [20363, 20364];
const NECKLACE_OF_COURAGE: i32 = 1506;
const NECKLACE_OF_VALOR: i32 = 1507;
const WEREWOLF_HEAD: i32 = 1477;
const WEREWOLF_TOTEM: i32 = 1501;
const RACE_ORC: i32 = 3;
const MIN_LEVEL: i32 = 9;
const REQUIRED: i64 = 40;
pub struct Q00274SkirmishWithTheWerewolves;
impl QuestScript for Q00274SkirmishWithTheWerewolves {
    fn id(&self) -> i32 {
        274
    }
    fn name(&self) -> &'static str {
        "Q00274_SkirmishWithTheWerewolves"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00274_SkirmishWithTheWerewolves"
    }
    fn start_npcs(&self) -> &[i32] {
        &[BRUKURSE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[BRUKURSE]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn quest_items(&self) -> &[i32] {
        &[WEREWOLF_HEAD, WEREWOLF_TOTEM]
    }
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 18).then(|| ctx.no_quest_html())
    }
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            let has_necklace = ctx.quest_items_count(NECKLACE_OF_VALOR) > 0
                || ctx.quest_items_count(NECKLACE_OF_COURAGE) > 0;
            if !has_necklace {
                return Some("30569-08.html".to_string());
            }
            return Some(
                if ctx.player_race() != RACE_ORC {
                    "30569-01.html"
                } else if ctx.player_level() >= MIN_LEVEL {
                    "30569-03.htm"
                } else {
                    "30569-02.html"
                }
                .to_string(),
            );
        }
        if ctx.is_started() {
            match ctx.cond() {
                1 => return Some("30569-05.html".to_string()),
                2 if ctx.quest_items_count(WEREWOLF_HEAD) >= REQUIRED => {
                    let totems = ctx.quest_items_count(WEREWOLF_TOTEM);
                    ctx.give_adena(200, true);
                    ctx.exit_quest(true, true);
                    return Some(
                        if totems > 0 {
                            "30569-07.html"
                        } else {
                            "30569-06.html"
                        }
                        .to_string(),
                    );
                }
                _ => {}
            }
        }
        Some(ctx.no_quest_html())
    }
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if ctx.has_qs() && event.eq_ignore_ascii_case("30569-04.htm") {
            ctx.start_quest();
            return Some(event.to_string());
        }
        None
    }
    fn on_kill(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_cond(1) {
            ctx.give_items(WEREWOLF_HEAD, 1);
            if ctx.roll(100) <= 5 {
                ctx.give_items(WEREWOLF_TOTEM, 1);
            }
            if ctx.quest_items_count(WEREWOLF_HEAD) >= REQUIRED {
                ctx.set_cond(2, true);
            } else {
                ctx.play_sound(quest_sounds::ITEMGET);
            }
        }
    }
}
