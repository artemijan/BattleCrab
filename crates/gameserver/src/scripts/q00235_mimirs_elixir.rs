//! Mimir's Elixir (235) — `quests/Q00235_MimirsElixir`. The A-grade enchant
//! capstone: Ladd (30721, level 75+, needs a Star of Destiny from Fate's
//! Whisper) walks the player through brewing Mimir's Elixir — collect Pure
//! Silver (the product of [`Q00373`](super::q00373_supplier_of_reagents)),
//! forge True Gold via Joan (30718) and a Sage Stone drop, gather Blood Fire,
//! mix the lot at the Magister's Mixing Urn (31149), and hand the elixir back
//! for a Scroll: Enchant Weapon (A-Grade). `cond` 1 → 8.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const JOAN: i32 = 30718;
const LADD: i32 = 30721;
const MIXING_URN: i32 = 31149;

const STAR_OF_DESTINY: i32 = 5011;
const PURE_SILVER: i32 = 6320;
const TRUE_GOLD: i32 = 6321;
const SAGE_STONE: i32 = 6322;
const BLOOD_FIRE: i32 = 6318;
const MIMIR_ELIXIR: i32 = 6319;
const MAGISTER_MIXING_STONE: i32 = 5905;
const SCROLL_ENCHANT_WEAPON_A: i32 = 729;
/// The cosmetic mixing flash Java broadcasts as the elixir is brewed.
const MIXING_FLASH: i32 = 4339;

/// `hasQuestItems(player, ids…)` — true only when **all** are held.
fn has_all(ctx: &QuestCtx, ids: &[i32]) -> bool {
    ids.iter().all(|&id| ctx.quest_items_count(id) > 0)
}

pub struct Q00235MimirsElixir;

impl QuestScript for Q00235MimirsElixir {
    fn id(&self) -> i32 {
        235
    }
    fn name(&self) -> &'static str {
        "Q00235_MimirsElixir"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00235_MimirsElixir"
    }
    fn start_npcs(&self) -> &[i32] {
        &[LADD]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[LADD, JOAN, MIXING_URN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[20965, 21090]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            PURE_SILVER,
            TRUE_GOLD,
            SAGE_STONE,
            BLOOD_FIRE,
            MAGISTER_MIXING_STONE,
            MIMIR_ELIXIR,
        ]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        // Java initialises `htmltext = event`, echoing anything it doesn't rewrite.
        if !ctx.has_qs() {
            return Some(event.to_string());
        }
        match event {
            "30721-06.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30721-12.htm" => {
                if ctx.quest_items_count(TRUE_GOLD) > 0 {
                    ctx.set_cond(6, true);
                    ctx.give_items(MAGISTER_MIXING_STONE, 1);
                }
                Some(event.to_string())
            }
            "30721-16.htm" => {
                if ctx.quest_items_count(MIMIR_ELIXIR) > 0 {
                    // `player.broadcastPacket(new MagicSkillUse(player, player,
                    // 4339, 1, 1, 1))` — the mixing flash, cast by the player
                    // on themselves.
                    ctx.cast_visual_at(ctx.player, ctx.player, MIXING_FLASH, 1, 1);
                    ctx.take_items(MAGISTER_MIXING_STONE, -1);
                    ctx.take_items(MIMIR_ELIXIR, -1);
                    ctx.take_items(STAR_OF_DESTINY, -1);
                    ctx.give_items(SCROLL_ENCHANT_WEAPON_A, 1);
                    ctx.social_action(3);
                    ctx.exit_quest(false, true);
                }
                Some(event.to_string())
            }
            "30718-03.htm" => {
                ctx.set_cond(3, true);
                Some(event.to_string())
            }
            "31149-02.htm" => {
                if !has_all(ctx, &[MAGISTER_MIXING_STONE]) {
                    Some("31149-havent.htm".to_string())
                } else {
                    Some(event.to_string())
                }
            }
            "31149-03.htm" => {
                if !has_all(ctx, &[MAGISTER_MIXING_STONE, PURE_SILVER]) {
                    Some("31149-havent.htm".to_string())
                } else {
                    Some(event.to_string())
                }
            }
            "31149-05.htm" => {
                if !has_all(ctx, &[MAGISTER_MIXING_STONE, PURE_SILVER, TRUE_GOLD]) {
                    Some("31149-havent.htm".to_string())
                } else {
                    Some(event.to_string())
                }
            }
            "31149-07.htm" => {
                if !has_all(
                    ctx,
                    &[MAGISTER_MIXING_STONE, PURE_SILVER, TRUE_GOLD, BLOOD_FIRE],
                ) {
                    Some("31149-havent.htm".to_string())
                } else {
                    Some(event.to_string())
                }
            }
            "31149-success.htm" => {
                if has_all(
                    ctx,
                    &[MAGISTER_MIXING_STONE, PURE_SILVER, TRUE_GOLD, BLOOD_FIRE],
                ) {
                    ctx.set_cond(8, true);
                    ctx.take_items(PURE_SILVER, -1);
                    ctx.take_items(TRUE_GOLD, -1);
                    ctx.take_items(BLOOD_FIRE, -1);
                    ctx.give_items(MIMIR_ELIXIR, 1);
                    Some(event.to_string())
                } else {
                    Some("31149-havent.htm".to_string())
                }
            }
            _ => Some(event.to_string()),
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            20965 => {
                if ctx.is_cond(3) && ctx.roll(10) < 2 {
                    ctx.give_items(SAGE_STONE, 1);
                    ctx.set_cond(4, true);
                }
            }
            21090 if ctx.is_cond(6) && ctx.roll(10) < 2 => {
                ctx.give_items(BLOOD_FIRE, 1);
                ctx.set_cond(7, true);
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        if ctx.is_created() {
            // Java shows Ladd's intro regardless of which NPC is talked to.
            return Some(
                if ctx.player_level() < 75 {
                    "30721-01b.htm"
                } else if ctx.quest_items_count(STAR_OF_DESTINY) == 0 {
                    "30721-01a.htm"
                } else {
                    "30721-01.htm"
                }
                .to_string(),
            );
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        let cond = ctx.cond();
        match ctx.npc_id {
            LADD => {
                if cond == 1 {
                    if ctx.quest_items_count(PURE_SILVER) > 0 {
                        ctx.set_cond(2, true);
                        Some("30721-08.htm".to_string())
                    } else {
                        Some("30721-07.htm".to_string())
                    }
                } else if cond < 5 {
                    Some("30721-10.htm".to_string())
                } else if cond == 5 && ctx.quest_items_count(TRUE_GOLD) > 0 {
                    Some("30721-11.htm".to_string())
                } else if cond == 6 || cond == 7 {
                    Some("30721-13.htm".to_string())
                } else if cond == 8 && ctx.quest_items_count(MIMIR_ELIXIR) > 0 {
                    Some("30721-14.htm".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            JOAN => {
                if cond == 2 {
                    Some("30718-01.htm".to_string())
                } else if cond == 3 {
                    Some("30718-04.htm".to_string())
                } else if cond == 4 && ctx.quest_items_count(SAGE_STONE) > 0 {
                    ctx.set_cond(5, true);
                    ctx.take_items(SAGE_STONE, -1);
                    ctx.give_items(TRUE_GOLD, 1);
                    Some("30718-05.htm".to_string())
                } else if cond > 4 {
                    Some("30718-06.htm".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            MIXING_URN => Some("31149-01.htm".to_string()),
            _ => Some(ctx.no_quest_html()),
        }
    }
}
