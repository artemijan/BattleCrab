//! Winds of Fate: Choices (10753) — port of
//! `dist/game/data/scripts/quests/Q10753_WindsOfFateChoices/`, the Ertheia
//! third class transfer.
//!
//! Katalin or Ayanthe hand over the Prophecy Machine. Arkenias wants three
//! each of the Nebulite drops (cond 2 → 3) for the restoration regeant
//! brewed at the alchemist's urn; Hardin and Icarus lead to Athrea, whose
//! 180-second treasure hunt among sixteen boxes must turn up four of her
//! belongings. Icarus then sends the player back to the side's master,
//! Gereth into the Chamber of Prophecies ([`super::chamber_of_prophecies`],
//! cond 16 → 17), and Queen Navari offers Eviscerator or Sayha's Seer.

use super::q10751_winds_of_fate_encounters::{
    ERTHEIA, TOP_CENTER, is_child_of_current, tutorial_bypass,
};
use super::q10752_winds_of_fate_a_promise::{show_side_letter, show_side_popup};
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

pub const QUEST_NAME: &str = "Q10753_WindsOfFateChoices";

// NPCs
const KATALIN: i32 = 33943;
const AYANTHE: i32 = 33942;
const ARKENIAS: i32 = 30174;
const ALCHEMISTS_MIXING_URN: i32 = 31149;
const HARDIN: i32 = 30832;
const ICARUS: i32 = 30835;
const ATHREA: i32 = 30758;
const ATHREAS_BOX: i32 = 33997;
const GERETH: i32 = 33932;
const QUEEN_NAVARI: i32 = 33931;

// Monsters
const QUEST_MONSTER_NEBULITE_EYE: i32 = 27544;
const QUEST_MONSTER_NEBULITE_WATCH: i32 = 27545;
const QUEST_MONSTER_NEBULITE_GOLEM: i32 = 27546;

// Items
pub(super) const PROPHECY_MACHINE: i32 = 39540;
const CRYSTAL_EYE: i32 = 39545;
const BROKEN_STONE_OF_PURITY: i32 = 39546;
const MIRACLE_DRUG_OF_ENCHANTMENT: i32 = 39547;
const CRUDE_PHILOPERS_STONE: i32 = 39544;
const EMPTY_REGEANT_FLASK: i32 = 39548;
const RESTORATION_REGEANT: i32 = 39549;
const WHITE_ROSE: i32 = 39551;
const ATHREAS_BELONGINGS: i32 = 39550;
const CRIMSON_ROSE: i32 = 39552;
pub(super) const ATELIA: i32 = 39542;

// Misc
const MIN_LEVEL: i32 = 85;
const CRYSTAL_EYE_VAR: &str = "EyeCount";
const BROKEN_STONE_OF_PURITY_VAR: &str = "PurityCount";
const EMPTY_REGEANT_FLASK_VAR: &str = "FlaskCount";
/// `PlayerClass.EVISCERATOR` / `SAYHA_SEER`.
const EVISCERATOR: i32 = 188;
const SAYHA_SEER: i32 = 189;

// Reward
const SAYHAS_BOX_W: i32 = 40268;
const SAYHAS_BOX_M: i32 = 40269;
const CHAOS_POMANDER: i32 = 37374;

// Locations
const TELEPORT_LOC: (i32, i32, i32) = (-81948, 249635, -3371);
const BOX_SPAWNS: [(i32, i32, i32, i32); 16] = [
    (102069, 103220, -3506, 0),
    (102009, 103220, -3506, 0),
    (101949, 103220, -3506, 0),
    (101889, 103220, -3506, 0),
    (102069, 103280, -3506, 0),
    (102009, 103280, -3506, 0),
    (101949, 103280, -3506, 0),
    (101889, 103280, -3506, 0),
    (102069, 103340, -3506, 0),
    (102009, 103340, -3506, 0),
    (101949, 103340, -3506, 0),
    (101889, 103340, -3506, 0),
    (102069, 103400, -3506, 0),
    (102009, 103400, -3506, 0),
    (101949, 103400, -3506, 0),
    (101889, 103400, -3506, 0),
];

// NpcStringIds
const REMAINING_TIME: i32 = 1811302;
const NEBULITE_EYE_CRYSTAL_EYE: i32 = 575311;
const NEBULITE_WATCH_BROKEN_STONE_OF_PURITY: i32 = 575312;
const NEBULITE_GOLEM_MIRACLE_DRUG_OF_ENCHANTMENT: i32 = 575313;
const CONGRATULATIONS_YOU_WILL_NOW_GRADUATE_FROM_THE_CLAN_ACADEMY: i32 = 17171749;

pub struct Q10753WindsOfFateChoices;

impl Q10753WindsOfFateChoices {
    /// The Eviscerator / Sayha's Seer transfer and its payout.
    fn change_class(ctx: &mut QuestCtx, new_class: i32, box_id: i32) {
        if !ctx.is_cond(18) || !is_child_of_current(ctx, new_class) {
            return;
        }
        ctx.set_class_id(new_class);
        ctx.send_skill_list();
        ctx.social_action(24);
        ctx.send_screen_message_npc_string(
            CONGRATULATIONS_YOU_WILL_NOW_GRADUATE_FROM_THE_CLAN_ACADEMY,
            TOP_CENTER,
            10000,
        );
        ctx.give_story_quest_reward();
        ctx.give_items(box_id, 1);
        ctx.give_items(CHAOS_POMANDER, 2);
        ctx.add_exp_and_sp(0, 500_000);
        ctx.give_adena(2_030_400, false);
        ctx.exit_quest(false, true);
    }

    /// `on cond` → `next`, sound on; the page is shown either way.
    fn step(ctx: &mut QuestCtx, cond: i32, next: i32) {
        if ctx.is_cond(cond) {
            ctx.set_cond(next, true);
        }
    }

    /// One Nebulite drop: below three, a better-than-even roll gives the
    /// item and bumps its counter. Returns the counter after the kill.
    fn nebulite_drop(ctx: &mut QuestCtx, var: &str, item_id: i32) -> i32 {
        let mut count = ctx.get_int(var);
        if count < 3 && ctx.roll(100) > 50 {
            ctx.give_items(item_id, 1);
            count += 1;
            ctx.set_var(var, count.to_string());
            ctx.play_sound(quest_sounds::ITEMGET);
        }
        count
    }

    /// Athrea's cond-10 talk: the timer hides, and every box she can see
    /// within 1000 is deleted.
    fn athrea_close_hunt(ctx: &mut QuestCtx) {
        ctx.send_ui_event(true, false, 1, 0, REMAINING_TIME);
        let athrea = ctx.npc;
        for npc in ctx.visible_npcs(athrea, Some(1000)) {
            if crate::game_loop::npc::npc_id_of(ctx.world, npc) == Some(ATHREAS_BOX)
                && ctx.can_see(athrea, npc)
            {
                ctx.delete_npc_of(npc);
            }
        }
        ctx.set_npc_script_value(0);
    }

    fn may_see_letter(ctx: &QuestCtx) -> bool {
        !ctx.has_qs() && ctx.player_race() == ERTHEIA && ctx.is_in_category("FOURTH_CLASS_GROUP")
    }
}

impl QuestScript for Q10753WindsOfFateChoices {
    fn id(&self) -> i32 {
        10753
    }
    fn name(&self) -> &'static str {
        QUEST_NAME
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q10753_WindsOfFateChoices"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KATALIN, AYANTHE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            KATALIN,
            AYANTHE,
            ARKENIAS,
            ALCHEMISTS_MIXING_URN,
            HARDIN,
            ICARUS,
            ATHREA,
            ATHREAS_BOX,
            GERETH,
            QUEEN_NAVARI,
        ]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[ATHREAS_BOX]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            QUEST_MONSTER_NEBULITE_EYE,
            QUEST_MONSTER_NEBULITE_WATCH,
            QUEST_MONSTER_NEBULITE_GOLEM,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            PROPHECY_MACHINE,
            CRYSTAL_EYE,
            BROKEN_STONE_OF_PURITY,
            MIRACLE_DRUG_OF_ENCHANTMENT,
            CRUDE_PHILOPERS_STONE,
            EMPTY_REGEANT_FLASK,
            RESTORATION_REGEANT,
            WHITE_ROSE,
            ATHREAS_BELONGINGS,
            CRIMSON_ROSE,
            ATELIA,
        ]
    }
    fn handles_global_events(&self) -> bool {
        true
    }

    /// `addCondRace(ERTHEIA, "")` and
    /// `addCondInCategory(FOURTH_CLASS_GROUP, "")`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.player_race() != ERTHEIA || !ctx.is_in_category("FOURTH_CLASS_GROUP") {
            return Some(String::new());
        }
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "33943-02.htm" | "33943-03.htm" | "33942-02.htm" | "33942-03.htm" | "30174-02.html"
            | "31149-02.html" | "30174-08.html" | "30174-09.html" | "30832-02.html"
            | "30835-02.html" | "30835-03.html" | "30835-04.html" | "30758-03.html"
            | "30758-08.html" | "33943-07.html" | "33943-08.html" | "33943-09.html"
            | "33942-07.html" | "33942-08.html" | "33942-09.html" | "33932-02.html"
            | "33932-03.html" | "33932-04.html" | "33932-05.html" | "33932-06.html"
            | "33931-02.html" => {}
            "33943-04.html" | "33942-04.html" => {
                ctx.give_items(PROPHECY_MACHINE, 1);
                ctx.start_quest();
            }
            "30174-03.html" => ctx.set_cond(2, true),
            "31149-03.html" => {
                if ctx.is_cond(4) {
                    ctx.give_items(RESTORATION_REGEANT, 1);
                    for item in [
                        CRUDE_PHILOPERS_STONE,
                        EMPTY_REGEANT_FLASK,
                        CRYSTAL_EYE,
                        BROKEN_STONE_OF_PURITY,
                        MIRACLE_DRUG_OF_ENCHANTMENT,
                    ] {
                        ctx.take_items(item, -1);
                    }
                    ctx.unset(CRYSTAL_EYE_VAR);
                    ctx.unset(EMPTY_REGEANT_FLASK_VAR);
                    ctx.unset(BROKEN_STONE_OF_PURITY_VAR);
                    ctx.set_cond(5, true);
                }
            }
            "30174-10.html" => {
                if ctx.is_cond(5) {
                    ctx.take_items(RESTORATION_REGEANT, -1);
                    ctx.set_cond(6, true);
                }
            }
            "30832-03.html" => Self::step(ctx, 6, 7),
            "30835-05.html" => {
                if ctx.is_cond(7) {
                    ctx.give_items(WHITE_ROSE, 1);
                    ctx.set_cond(8, true);
                }
            }
            "30758-02.html" => {
                if ctx.npc_script_value() != 0 || !ctx.is_cond(8) {
                    return Some("busy.html".to_string());
                }
            }
            "30758-04.html" => {
                if ctx.is_cond(8) {
                    ctx.set_npc_script_value(1);
                    ctx.send_ui_event(false, false, 180, 1, REMAINING_TIME);
                    for loc in BOX_SPAWNS {
                        ctx.add_spawn(ATHREAS_BOX, loc, false, 180_000, 0);
                    }
                    let (npc, player) = (ctx.npc, ctx.player);
                    ctx.add_timer("despawn", 180_000, npc, player);
                    ctx.set_cond(9, false);
                }
            }
            "despawn" => {
                ctx.set_npc_script_value(0);
                return None;
            }
            "open" => {
                let mut html = None;
                if ctx.npc_id == ATHREAS_BOX && ctx.is_cond(9) {
                    if ctx.roll(100) < 35 {
                        ctx.give_items(ATHREAS_BELONGINGS, 1);
                        // The `ON_PLAYER_ITEM_ADD` listener on the belongings,
                        // run where the item lands.
                        if ctx.is_cond(9) && ctx.quest_items_count(ATHREAS_BELONGINGS) >= 4 {
                            ctx.set_cond(10, true);
                        }
                        ctx.play_sound(quest_sounds::ITEMGET);
                        html = Some("33997-2.html".to_string());
                    } else {
                        html = Some("33997-3.html".to_string());
                    }
                }
                ctx.set_npc_script_value(1);
                return html;
            }
            "30758-09.html" => {
                if ctx.is_cond(10) {
                    ctx.take_items(WHITE_ROSE, -1);
                    ctx.take_items(ATHREAS_BELONGINGS, -1);
                    ctx.give_items(CRIMSON_ROSE, 1);
                    ctx.set_npc_script_value(0);
                    ctx.set_cond(11, true);
                }
            }
            "30835-08.html" => {
                if ctx.is_cond(11) {
                    let next = if ctx.is_mage_class() { 12 } else { 13 };
                    ctx.set_cond(next, true);
                    ctx.take_items(CRIMSON_ROSE, -1);
                }
            }
            "33943-10.html" => Self::step(ctx, 13, 14),
            "33942-10.html" => Self::step(ctx, 12, 14),
            "33932-09.html" => Self::step(ctx, 17, 18),
            "class" => {
                let html = if ctx.is_mage_class() {
                    "33931-03a.html"
                } else {
                    "33931-03.html"
                };
                return Some(html.to_string());
            }
            "33931-04.html" => Self::change_class(ctx, EVISCERATOR, SAYHAS_BOX_W),
            "33931-04a.html" => Self::change_class(ctx, SAYHA_SEER, SAYHAS_BOX_M),
            _ => return None,
        }
        Some(event.to_string())
    }

    /// `startQuestTimer("despawn", …)` lands in `onEvent`.
    fn on_timer_event(&self, ctx: &mut QuestCtx, event: &str) {
        self.on_event(ctx, event);
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.has_qs() && ctx.is_cond(9) && ctx.npc_script_value() == 0 {
            return Some("33997.html".to_string());
        }
        Some("33997-1.html".to_string())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        let mage = ctx.is_mage_class();
        if ctx.is_created() {
            let ready = ctx.player_level() >= MIN_LEVEL && ctx.is_in_category("FOURTH_CLASS_GROUP");
            let html = match ctx.npc_id {
                KATALIN if ready && !mage => "33943-01.html",
                KATALIN => "33943-00.html",
                AYANTHE if ready && mage => "33942-01.html",
                AYANTHE => "33942-00.html",
                _ => return Some(ctx.no_quest_html()),
            };
            return Some(html.to_string());
        }
        let cond = ctx.cond();
        let html = match ctx.npc_id {
            KATALIN => match cond {
                1 => "33943-05.html",
                13 => "33943-06.html",
                14 => "33943-11.html",
                _ => return Some(ctx.no_quest_html()),
            },
            AYANTHE => match cond {
                1 => "33942-05.html",
                12 => "33942-06.html",
                14 => "33942-11.html",
                _ => return Some(ctx.no_quest_html()),
            },
            ARKENIAS => match cond {
                1 => "30174-01.html",
                2 => "30174-04.html",
                3 => {
                    ctx.give_items(CRUDE_PHILOPERS_STONE, 1);
                    ctx.give_items(EMPTY_REGEANT_FLASK, 1);
                    ctx.set_cond(4, true);
                    "30174-05.html"
                }
                4 => "30174-06.html",
                5 if ctx.quest_items_count(RESTORATION_REGEANT) > 0 => "30174-07.html",
                6 => "30174-11.html",
                _ => return Some(ctx.no_quest_html()),
            },
            ALCHEMISTS_MIXING_URN => match cond {
                4 if ctx.quest_items_count(EMPTY_REGEANT_FLASK) > 0 => "31149-01.html",
                5 => "31149-04.html",
                _ => return Some(ctx.no_quest_html()),
            },
            HARDIN => match cond {
                6 => "30832-01.html",
                7 => "30832-04.html",
                _ => return Some(ctx.no_quest_html()),
            },
            ICARUS => match cond {
                7 => "30835-01.html",
                8 => "30835-06.html",
                11 if ctx.quest_items_count(CRIMSON_ROSE) > 0 => "30835-07.html",
                12 | 13 => "30835-09.html",
                _ => return Some(ctx.no_quest_html()),
            },
            ATHREA => {
                let belongings = ctx.quest_items_count(ATHREAS_BELONGINGS);
                match cond {
                    8 if ctx.quest_items_count(WHITE_ROSE) > 0 => "30758-01.html",
                    9 if belongings == 0 => "30758-05.html",
                    9 if belongings < 4 => "30758-06.html",
                    10 => {
                        Self::athrea_close_hunt(ctx);
                        "30758-07.html"
                    }
                    11 => "30758-10.html",
                    _ => return Some(ctx.no_quest_html()),
                }
            }
            GERETH => match cond {
                14 if ctx.quest_items_count(PROPHECY_MACHINE) > 0 => "33932-01.html",
                15 | 16 => "33932-07.html",
                17 => "33932-08.html",
                18 => "33932-10.html",
                _ => return Some(ctx.no_quest_html()),
            },
            QUEEN_NAVARI if cond == 18 && ctx.quest_items_count(ATELIA) > 0 => "33931-01.html",
            _ => return Some(ctx.no_quest_html()),
        };
        Some(html.to_string())
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_cond(2) {
            return;
        }
        let mut eye = ctx.get_int(CRYSTAL_EYE_VAR);
        let mut purity = ctx.get_int(BROKEN_STONE_OF_PURITY_VAR);
        let mut flask = ctx.get_int(EMPTY_REGEANT_FLASK_VAR);
        match ctx.npc_id {
            QUEST_MONSTER_NEBULITE_EYE => {
                eye = Self::nebulite_drop(ctx, CRYSTAL_EYE_VAR, CRYSTAL_EYE);
            }
            QUEST_MONSTER_NEBULITE_WATCH => {
                purity =
                    Self::nebulite_drop(ctx, BROKEN_STONE_OF_PURITY_VAR, BROKEN_STONE_OF_PURITY);
            }
            QUEST_MONSTER_NEBULITE_GOLEM => {
                flask =
                    Self::nebulite_drop(ctx, EMPTY_REGEANT_FLASK_VAR, MIRACLE_DRUG_OF_ENCHANTMENT);
            }
            _ => {}
        }
        if eye >= 3 && purity >= 3 && flask >= 3 {
            ctx.set_cond(0, false);
            ctx.set_cond(3, true);
        }
    }

    fn npc_log_list(&self, ctx: &mut QuestCtx) -> Vec<(i32, bool, i32)> {
        if !ctx.is_cond(2) {
            return Vec::new();
        }
        [
            (NEBULITE_EYE_CRYSTAL_EYE, CRYSTAL_EYE_VAR),
            (
                NEBULITE_WATCH_BROKEN_STONE_OF_PURITY,
                BROKEN_STONE_OF_PURITY_VAR,
            ),
            (
                NEBULITE_GOLEM_MIRACLE_DRUG_OF_ENCHANTMENT,
                EMPTY_REGEANT_FLASK_VAR,
            ),
        ]
        .into_iter()
        .map(|(id, var)| (id, true, ctx.get_int(var)))
        .filter(|&(_, _, count)| count > 0)
        .collect()
    }

    fn on_tutorial_mark(&self, ctx: &mut QuestCtx, mark_id: i32) {
        if mark_id == self.id() && !ctx.has_qs() {
            show_side_popup(ctx, "Npcdialog1.aris_quest_2", "Npcdialog1.katrina_quest_2");
        }
    }

    fn on_player_bypass(&self, ctx: &mut QuestCtx, command: &str) {
        tutorial_bypass(ctx, self.id(), command, TELEPORT_LOC);
    }

    fn on_level_changed(&self, ctx: &mut QuestCtx, old_level: i32, new_level: i32) {
        if ctx.world.cfg.character.disable_tutorial {
            return;
        }
        if old_level < new_level && new_level >= MIN_LEVEL && Self::may_see_letter(ctx) {
            show_side_letter(ctx, self.id());
            ctx.play_sound(quest_sounds::TUTORIAL);
        }
    }

    fn on_login(&self, ctx: &mut QuestCtx) {
        if ctx.world.cfg.character.disable_tutorial {
            return;
        }
        if ctx.player_level() >= MIN_LEVEL && Self::may_see_letter(ctx) {
            show_side_letter(ctx, self.id());
            ctx.play_sound(quest_sounds::TUTORIAL);
        }
    }
}
