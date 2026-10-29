//! Winds of Fate: Encounters (10751) — port of
//! `dist/game/data/scripts/quests/Q10751_WindsOfFateEncounters/`, the
//! Ertheia first class transfer.
//!
//! Queen Navari sends a fighter to Katalin and a mage to Ayanthe, who both
//! send the player on to Raymond in Gludio. After five skeletons fall,
//! Telesha appears beside the last one; her `SPAWN_WIZZARD` replaces her with
//! the Mysterious Wizard, whose page (served by
//! [`super::mysterious_wizard`]) gives the second relic. Raymond then sends
//! the player back to Katalin (Marauder) or Ayanthe (Cloud Breaker).
//!
//! From level 38 the login and the level-up show question mark 10751; its
//! popup's `Q10751_teleport` link goes through [`tutorial_teleport`], which
//! Q10752 and Q10753 share.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

pub const QUEST_NAME: &str = "Q10751_WindsOfFateEncounters";

// NPCs
const NAVARI: i32 = 33931;
const AYANTHE: i32 = 33942;
const KATALIN: i32 = 33943;
const RAYMOND: i32 = 30289;
const MYSTERIOUS_WIZARD: i32 = 33980;
const TELESHA: i32 = 33981;

// Monsters
const MONSTERS: [i32; 2] = [
    27528, // Skeleton Warrior
    27529, // Skeleton Archer
];

// Items
const WIND_SPIRIT_REALMS_RELIC: i32 = 39535;
const MAJOR_HEALING_POTION: i32 = 1061;
const SOULSHOT: i32 = 1464;
const SPIRITSHOT: i32 = 3949;
const PAULINA_EQUIPMENT_SET: i32 = 46850;

// Location
const TELEPORT_LOC: (i32, i32, i32) = (-80565, 251763, -3080);

// Misc
const MIN_LEVEL: i32 = 38;
const KILL_COUNT_VAR: &str = "KillCount";
/// `Race.ERTHEIA` ordinal.
pub(super) const ERTHEIA: i32 = 6;
/// `PlayerClass.MARAUDER` / `CLOUD_BREAKER`.
const MARAUDER: i32 = 184;
const CLOUD_BREAKER: i32 = 185;
/// `ExShowScreenMessage.TOP_CENTER`.
pub(super) const TOP_CENTER: i32 = 2;

// NpcStringIds
const TALK_TO_THE_MYSTERIOUS_WIZARD_2: i32 = 1802507;
const RETURN_TO_RAYMOND_OF_THE_TOWN_OF_GLUDIO: i32 = 1802623;
const CHECK_ON_TELESHA: i32 = 1802506;
const KILL_SKELETONS: i32 = 575113;
const QUEEN_NAVARI_HAS_SENT_A_LETTER: i32 = 575500;
const YOU_MAY_NOT_TELEPORT_IN_MIDDLE_OF_A_SIEGE: i32 = 1811237;
const YOU_CANNOT_TELEPORT_IN_PARTY_STATUS: i32 = 1811239;
const YOU_MAY_NOT_TELEPORT_WHILE_USING_INSTANCE_ZONE: i32 = 1811240;
const YOU_CANNOT_TELEPORT_IN_COMBAT: i32 = 1811241;
const YOU_CANNOT_TELEPORT_WHILE_IN_A_TRANSFORMED_STATE: i32 = 1802488;
const YOU_CANNOT_TELEPORT_WHILE_YOU_ARE_DEAD: i32 = 1811318;

/// The `Q1075x_teleport` / `Q1075x_close` popup links the three Winds of
/// Fate quests share (`onPlayerBypass`): only while the player has no state
/// for `quest_id`. `_teleport` refuses with a screen message during any
/// siege, in a party or an instance, in combat, transformed or dead.
pub(super) fn tutorial_bypass(
    ctx: &mut QuestCtx,
    quest_id: i32,
    command: &str,
    teleport_loc: (i32, i32, i32),
) {
    if ctx.has_qs() {
        return;
    }
    let Some(action) = command
        .strip_prefix('Q')
        .and_then(|rest| rest.strip_prefix(quest_id.to_string().as_str()))
    else {
        return;
    };
    match action {
        "_teleport" => {
            ctx.tutorial_close_html();
            tutorial_teleport(ctx, teleport_loc);
        }
        "_close" => {
            ctx.tutorial_close_html();
            ctx.tutorial_show_question_mark(quest_id, 1);
        }
        _ => {}
    }
}

fn tutorial_teleport(ctx: &mut QuestCtx, (x, y, z): (i32, i32, i32)) {
    let refusal = if ctx.any_siege_in_progress() {
        YOU_MAY_NOT_TELEPORT_IN_MIDDLE_OF_A_SIEGE
    } else if ctx.is_in_party() {
        YOU_CANNOT_TELEPORT_IN_PARTY_STATUS
    } else if ctx.player_instance() != 0 {
        YOU_MAY_NOT_TELEPORT_WHILE_USING_INSTANCE_ZONE
    } else if ctx.has_attack_stance() {
        YOU_CANNOT_TELEPORT_IN_COMBAT
    } else if ctx.is_transformed() {
        YOU_CANNOT_TELEPORT_WHILE_IN_A_TRANSFORMED_STATE
    } else if ctx.is_oid_dead(ctx.player) {
        YOU_CANNOT_TELEPORT_WHILE_YOU_ARE_DEAD
    } else {
        ctx.teleport_to(x, y, z);
        return;
    };
    ctx.send_screen_message_npc_string(refusal, TOP_CENTER, 5000);
}

/// `newClass.childOf(player.getPlayerClass())`: the current class is a
/// strict ancestor of `new_class`.
pub(super) fn is_child_of_current(ctx: &QuestCtx, new_class: i32) -> bool {
    let current = ctx.player_class_id();
    ctx.world
        .data
        .skill_trees
        .class_lineage(new_class)
        .iter()
        .skip(1)
        .any(|&c| c == current)
}

/// `npc.setSummoner(player)`, `setTitle(player's name)` and
/// `broadcastInfo()`: a quest NPC that wears its summoner's name.
pub(super) fn mark_as_summoned_by(ctx: &mut QuestCtx, npc: i32, player: i32) {
    ctx.set_npc_summoner(npc, player);
    let name = ctx.player_name();
    ctx.set_npc_title(npc, name);
    ctx.npc_broadcast_info(npc);
}

pub struct Q10751WindsOfFateEncounters;

impl Q10751WindsOfFateEncounters {
    /// The Marauder / Cloud Breaker transfer and its payout.
    fn change_class(ctx: &mut QuestCtx, cond: i32, new_class: i32) {
        if !ctx.is_cond(cond) || !is_child_of_current(ctx, new_class) {
            return;
        }
        ctx.set_class_id(new_class);
        ctx.send_skill_list();
        ctx.social_action(23);
        ctx.give_adena(461_880, true);
        ctx.give_items(MAJOR_HEALING_POTION, 50);
        ctx.give_items(SOULSHOT, 3000);
        ctx.give_items(SPIRITSHOT, 3000);
        ctx.give_items(PAULINA_EQUIPMENT_SET, 1);
        ctx.add_exp_and_sp(2_700_000, 648);
        ctx.exit_quest(false, true);
    }

    /// The login and level-up letter: the screen message and the mark.
    fn show_letter(&self, ctx: &mut QuestCtx) {
        ctx.send_screen_message_npc_string(QUEEN_NAVARI_HAS_SENT_A_LETTER, TOP_CENTER, 10000);
        ctx.tutorial_show_question_mark(self.id(), 1);
    }

    fn may_see_letter(ctx: &QuestCtx) -> bool {
        !ctx.has_qs() && ctx.player_race() == ERTHEIA && ctx.is_in_category("FIRST_CLASS_GROUP")
    }
}

impl QuestScript for Q10751WindsOfFateEncounters {
    fn id(&self) -> i32 {
        10751
    }
    fn name(&self) -> &'static str {
        QUEST_NAME
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q10751_WindsOfFateEncounters"
    }
    fn start_npcs(&self) -> &[i32] {
        &[NAVARI]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[TELESHA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            NAVARI,
            AYANTHE,
            KATALIN,
            RAYMOND,
            TELESHA,
            MYSTERIOUS_WIZARD,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn quest_items(&self) -> &[i32] {
        &[WIND_SPIRIT_REALMS_RELIC]
    }
    fn handles_global_events(&self) -> bool {
        true
    }

    /// `addCondRace(ERTHEIA, "")`, `addCondInCategory(FIRST_CLASS_GROUP, "")`
    /// and `addCondMinLevel(38, "33931-00.htm")`, in order.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.player_race() != ERTHEIA || !ctx.is_in_category("FIRST_CLASS_GROUP") {
            return Some(String::new());
        }
        if ctx.player_level() < MIN_LEVEL {
            return Some("33931-00.htm".to_string());
        }
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30289-02.html" | "30289-06.html" | "33942-05.html" | "33942-06.html"
            | "33942-07.html" | "33942-08.html" | "33942-09.html" | "33942-10.html"
            | "33943-05.html" | "33943-06.html" | "33943-07.html" | "33943-08.html"
            | "33943-09.html" | "33943-10.html" => {}
            "33931-02.htm" => {
                ctx.start_quest();
                if ctx.is_mage_class() {
                    ctx.set_cond(0, false);
                    ctx.set_cond(3, true);
                } else {
                    ctx.set_cond(2, true);
                    return Some("33931-03.htm".to_string());
                }
            }
            "33943-02.html" => {
                if ctx.is_cond(2) {
                    ctx.set_cond(4, true);
                }
            }
            "33942-02.html" => {
                if ctx.is_cond(3) {
                    ctx.set_cond(5, true);
                }
            }
            "30289-03.html" => {
                if ctx.is_cond(4) || ctx.is_cond(5) {
                    ctx.give_items(WIND_SPIRIT_REALMS_RELIC, 1);
                    ctx.set_cond(6, true);
                }
            }
            "SPAWN_WIZZARD" => {
                if ctx.is_cond(6) && ctx.npc_id == TELESHA {
                    let at = ctx.npc_location(ctx.npc)?;
                    if let Some(wizard) = ctx.add_spawn(MYSTERIOUS_WIZARD, at, true, 30000, 0) {
                        let player = ctx.player;
                        mark_as_summoned_by(ctx, wizard, player);
                    }
                    ctx.send_screen_message_npc_string(
                        TALK_TO_THE_MYSTERIOUS_WIZARD_2,
                        TOP_CENTER,
                        10000,
                    );
                    ctx.delete_npc();
                }
                return None;
            }
            "33980-06.html" => {
                if ctx.is_cond(6) {
                    ctx.give_items(WIND_SPIRIT_REALMS_RELIC, 1);
                    ctx.set_cond(7, true);
                    ctx.send_screen_message_npc_string(
                        RETURN_TO_RAYMOND_OF_THE_TOWN_OF_GLUDIO,
                        TOP_CENTER,
                        8000,
                    );
                }
            }
            "30289-07.html" => {
                if ctx.is_cond(7) {
                    if !ctx.is_mage_class() {
                        ctx.set_cond(8, true);
                    } else {
                        ctx.set_cond(9, true);
                        return Some("30289-08.html".to_string());
                    }
                }
            }
            "33942-11.html" => Self::change_class(ctx, 9, CLOUD_BREAKER),
            "33943-11.html" => Self::change_class(ctx, 8, MARAUDER),
            _ => return None,
        }
        Some(event.to_string())
    }

    /// Only Telesha is registered; Java's wizard branch here is never
    /// reached (the wizard's page comes from `MysteriousWizard`).
    fn on_first_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        Some("33981-01.html".to_string())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let mage = ctx.is_mage_class();
        let html = match ctx.npc_id {
            NAVARI if ctx.is_created() => Some("33931-01.htm"),
            NAVARI if ctx.is_started() => match ctx.cond() {
                2 => Some("33931-04.html"),
                3 => Some("33931-05.html"),
                _ => None,
            },
            NAVARI if ctx.is_completed() => return Some(ctx.already_completed_html()),
            KATALIN if !mage && ctx.is_started() => match ctx.cond() {
                2 => Some("33943-01.html"),
                4 => Some("33943-03.html"),
                8 => Some("33943-04.html"),
                _ => None,
            },
            AYANTHE if mage && ctx.is_started() => match ctx.cond() {
                3 => Some("33942-01.html"),
                5 => Some("33942-03.html"),
                9 => Some("33942-04.html"),
                _ => None,
            },
            KATALIN | AYANTHE if ctx.is_completed() && (mage == (ctx.npc_id == AYANTHE)) => {
                return Some(ctx.already_completed_html());
            }
            RAYMOND if ctx.is_started() => match ctx.cond() {
                4 | 5 => Some("30289-01.html"),
                6 => Some("30289-04.html"),
                7 => Some("30289-05.html"),
                8 => Some("30289-09.html"),
                9 => Some("30289-10.html"),
                _ => None,
            },
            _ => None,
        };
        Some(html.map_or_else(|| ctx.no_quest_html(), str::to_string))
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_cond(6) {
            return;
        }
        let mut kill_count = ctx.get_int(KILL_COUNT_VAR);
        if kill_count <= 5 {
            kill_count += 1;
            ctx.set_var(KILL_COUNT_VAR, kill_count.to_string());
            ctx.play_sound(quest_sounds::ITEMGET);
        }
        if kill_count < 5 {
            return;
        }
        let killer = ctx.player;
        let telesha_near = ctx.visible_npcs(ctx.npc, Some(1000)).into_iter().any(|n| {
            crate::game_loop::npc::npc_id_of(ctx.world, n) == Some(TELESHA)
                && ctx.npc_summoner(n) == killer
        });
        if telesha_near {
            return;
        }
        let Some(at) = ctx.npc_location(ctx.npc) else {
            return;
        };
        if let Some(telesha) = ctx.add_spawn(TELESHA, at, false, 30000, 0) {
            mark_as_summoned_by(ctx, telesha, killer);
        }
        ctx.send_screen_message_npc_string(CHECK_ON_TELESHA, TOP_CENTER, 10000);
    }

    fn npc_log_list(&self, ctx: &mut QuestCtx) -> Vec<(i32, bool, i32)> {
        let kill_count = ctx.get_int(KILL_COUNT_VAR);
        if ctx.is_cond(6) && kill_count > 0 {
            return vec![(KILL_SKELETONS, true, kill_count)];
        }
        Vec::new()
    }

    /// `ON_PLAYER_PRESS_TUTORIAL_MARK`: Navari's letter, until the quest is
    /// talked about.
    fn on_tutorial_mark(&self, ctx: &mut QuestCtx, mark_id: i32) {
        if mark_id == self.id() && !ctx.has_qs() {
            ctx.play_voice("Npcdialog1.serenia_quest_12");
            ctx.tutorial_show_html_file("popup.html");
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
            self.show_letter(ctx);
        }
    }

    fn on_login(&self, ctx: &mut QuestCtx) {
        if ctx.world.cfg.character.disable_tutorial {
            return;
        }
        if ctx.player_level() >= MIN_LEVEL && Self::may_see_letter(ctx) {
            self.show_letter(ctx);
        }
    }
}
