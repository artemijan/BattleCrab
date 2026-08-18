//! The Clan Hall Manager (`ai/others/ClanHallManager`) — the owning clan's
//! console: door control and function-upgrade purchase/removal. The auction /
//! ownership / function state lives in [`crate::game_loop::clans::hall_auction`]
//! and [`crate::game_loop::clans::hall_function`].
//!
//! Wired here: `manageDoors`, `manageFunctions setFunction/removeFunction`, the
//! static function menus, `expel` (banishOthers), and all three `useFunctions`
//! benefits — `teleport` (the hall's TELEPORT-level `tel<n>` list), `buffs`
//! (the BUFF-function support-magic menu), and `items` (the ITEM-function
//! merchant buy window).

use crate::game_loop::clans::hall_auction::{banish_others, hall_ownership, open_close_hall_doors};
use crate::game_loop::clans::hall_function::{
    BuffCastOutcome, FunctionOutcome, buy_function, cast_hall_buff, function_level, remove_function,
};
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::clan::{CH_DISMISS, CH_OPEN_DOOR, CH_OTHER_RIGHTS, CH_SET_FUNCTIONS};
use crate::model::components::Vitals;

/// `CLANHALL_MANAGERS` — every clan-hall manager NPC.
const MANAGERS: &[i32] = &[
    35384, 35386, 35388, 35390, // Gludio
    35400, 35392, 35394, 35396, 35398, // Gludin
    35403, 35405, 35407, // Dion
    35439, 35441, 35443, 35445, 35447, 35449, // Aden
    35451, 35453, 35455, 35457, 35459, // Giran
    35461, 35463, 35465, 35467, // Goddard
    35566, 35568, 35570, 35572, 35574, 35576, 35578, // Rune
    35580, 35582, 35584, 35586, // Schuttgart
    36721, 36723, 36725, 36727, // Gludio Outskirts
    36729, 36731, 36733, 36735, // Dion Outskirts
    36737, 36739, // Floran Village
];

const NO_AUTHORITY: &str = "ClanHallManager-noAuthority.html";

pub struct ClanHallManager;

impl QuestScript for ClanHallManager {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "ClanHallManager"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/ClanHallManager"
    }
    fn start_npcs(&self) -> &[i32] {
        MANAGERS
    }
    fn talk_npcs(&self) -> &[i32] {
        MANAGERS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        MANAGERS
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let owner_id = hall_ownership(ctx.world, ctx.npc_id)
            .map(|(o, _)| o)
            .unwrap_or(0);
        // Your hall's console, or the "not the owner" page.
        Some(page(if ctx.is_owning_clan(owner_id) {
            "01"
        } else {
            "03"
        }))
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let (owner_id, hall_id) = hall_ownership(ctx.world, ctx.npc_id)?;
        // The whole console is owner-only (Java's outer `isOwningClan` gate).
        if !ctx.is_owning_clan(owner_id) {
            return Some(page("03"));
        }
        let mut parts = event.split_whitespace();
        match parts.next() {
            Some("index") => Some(page("01")),
            Some("manageDoors") => {
                if !ctx.has_clan_privilege(CH_OPEN_DOOR) {
                    return Some(NO_AUTHORITY.to_string());
                }
                match parts.next() {
                    Some(tok) => {
                        let open = tok == "1";
                        open_close_hall_doors(ctx.world, hall_id, open);
                        Some(page(if open { "05" } else { "06" }))
                    }
                    None => Some(page("04")),
                }
            }
            Some("manageFunctions") => {
                if !ctx.has_clan_privilege(CH_SET_FUNCTIONS) {
                    return Some(NO_AUTHORITY.to_string());
                }
                self.manage_functions(ctx, hall_id, &mut parts)
            }
            Some("expel") => {
                if !ctx.has_clan_privilege(CH_DISMISS) {
                    return Some(NO_AUTHORITY.to_string());
                }
                // A trailing token = confirmed (Java: `st.hasMoreTokens()`).
                if parts.next().is_some() {
                    banish_others(ctx.world, hall_id);
                    Some(page("08"))
                } else {
                    Some(page("07"))
                }
            }
            Some("useFunctions") => {
                if !ctx.has_clan_privilege(CH_OTHER_RIGHTS) {
                    return Some(NO_AUTHORITY.to_string());
                }
                self.use_functions(ctx, hall_id, &mut parts)
            }
            Some(e) if e.ends_with(".html") => Some(e.to_string()),
            _ => Some(page("01")),
        }
    }
}

impl ClanHallManager {
    /// `useFunctions` — the benefits the hall has bought. Only `teleport` is
    /// wired so far; the buff/item consoles are later slices.
    fn use_functions(
        &self,
        ctx: &mut QuestCtx,
        hall_id: i32,
        parts: &mut std::str::SplitWhitespace,
    ) -> Option<String> {
        match parts.next() {
            Some("teleport") => {
                // The hall's TELEPORT function level picks the `tel<level>` list.
                let func_id = ctx.world.data.residence_functions.id_of_type("TELEPORT")?;
                let level = function_level(ctx.world, hall_id, func_id);
                if level == 0 {
                    // No teleport function bought.
                    return Some("ClanHallManager-noFunction.html".to_string());
                }
                match (parts.next(), parts.next()) {
                    // A picked destination: `useFunctions teleport tel<n> <loc>`.
                    // Java guards the list token's level against the hall's own
                    // (`teleportLevel == funcLvl`) before teleporting.
                    (Some(list), Some(loc)) => {
                        let func_lvl = list.strip_prefix("tel").and_then(|n| n.parse().ok());
                        if func_lvl == Some(level) {
                            crate::game_loop::teleporter::do_teleport(
                                ctx.world,
                                ctx.client_id,
                                ctx.player,
                                ctx.npc,
                                list,
                                loc.parse().ok(),
                            );
                        }
                        None
                    }
                    // No destination yet: show the list, routed back through us.
                    _ => {
                        let list = format!("tel{level}");
                        let bypass = format!(
                            "npc_{}_Quest ClanHallManager useFunctions teleport",
                            ctx.npc
                        );
                        crate::game_loop::teleporter::show_teleport_list(
                            ctx.world,
                            ctx.client_id,
                            ctx.player,
                            ctx.npc,
                            &list,
                            &bypass,
                        );
                        None
                    }
                }
            }
            Some("buffs") => {
                let func_id = ctx.world.data.residence_functions.id_of_type("BUFF")?;
                let level = function_level(ctx.world, hall_id, func_id);
                if level == 0 {
                    return Some("ClanHallManager-noFunction.html".to_string());
                }
                match parts.next() {
                    // No skill picked: the buff menu for this function level.
                    None => Some(
                        self.buff_html(ctx, &format!("ClanHallManager-funcBuffs_{level}.html")),
                    ),
                    // `<skillId>_<skillLevel>` from a menu button.
                    Some(token) => {
                        let page = match cast_hall_buff(ctx.world, ctx.npc, ctx.player, token) {
                            // Java silently ignores an unlisted/bad skill.
                            BuffCastOutcome::NotAllowed => return Some(page("01")),
                            BuffCastOutcome::NotEnoughMp => "ClanHallManager-funcBuffsNoMp.html",
                            BuffCastOutcome::OnReuse => "ClanHallManager-funcBuffsNoReuse.html",
                            BuffCastOutcome::Cast => "ClanHallManager-funcBuffsDone.html",
                        };
                        Some(self.buff_html(ctx, page))
                    }
                }
            }
            Some("items") => {
                let func_id = ctx.world.data.residence_functions.id_of_type("ITEM")?;
                match function_level(ctx.world, hall_id, func_id) {
                    // Java `showBuyWindow(player, npcId·"0"·(level-1))` — the
                    // three item-function tiers map to buylists npcId*100 + 0/1/2.
                    level @ 1..=3 => {
                        let list_id = ctx.npc_id * 100 + (level - 1);
                        crate::game_loop::shop::show_buy_window(
                            ctx.world,
                            ctx.client_id,
                            ctx.player,
                            ctx.npc,
                            list_id,
                        );
                        None
                    }
                    _ => Some("ClanHallManager-noFunction.html".to_string()),
                }
            }
            _ => Some(page("01")),
        }
    }

    /// Read one of the buff pages and fill in `%manaLeft%` with the manager
    /// NPC's current MP (Java replaces it on every buff page).
    fn buff_html(&self, ctx: &QuestCtx, file: &str) -> String {
        let mana = ctx
            .world
            .objects
            .get_component::<Vitals>(&ctx.npc)
            .map(|v| v.cur_mp as i32)
            .unwrap_or(0);
        crate::data::htm_cache::read_htm_for(
            ctx.world,
            ctx.player,
            format!(
                "{}data/scripts/{}/{file}",
                ctx.world.data.root,
                self.html_dir()
            ),
        )
        .unwrap_or_default()
        .replace("%manaLeft%", &mana.to_string())
    }

    fn manage_functions(
        &self,
        ctx: &mut QuestCtx,
        hall_id: i32,
        parts: &mut std::str::SplitWhitespace,
    ) -> Option<String> {
        match parts.next() {
            // Buy a function level: `setFunction <funcId> <funcLv>`.
            Some("setFunction") => {
                let (Some(func_id), Some(level)) = (next_i32(parts), next_i32(parts)) else {
                    return Some(page("01"));
                };
                let now = commons::util::now_millis();
                let outcome = buy_function(ctx.world, hall_id, ctx.player, func_id, level, now);
                Some(match outcome {
                    FunctionOutcome::Bought => "ClanHallManager-manageFuncDone.html".to_string(),
                    FunctionOutcome::NotEnough => "ClanHallManager-noAdena.html".to_string(),
                    // AlreadyActive / NoSuchFunction → back to the console.
                    _ => page("01"),
                })
            }
            // `removeFunction confirm|remove <TYPE>`.
            Some("removeFunction") => {
                let act = parts.next().unwrap_or("");
                let type_name = parts.next().unwrap_or("");
                if act == "remove" {
                    if let Some(func_id) = ctx.world.data.residence_functions.id_of_type(type_name)
                        && remove_function(ctx.world, hall_id, func_id)
                    {
                        return Some("ClanHallManager-removeFunctionDone.html".to_string());
                    }
                    Some("ClanHallManager-removeFunctionFail.html".to_string())
                } else {
                    Some("ClanHallManager-removeFunctionConfirm.html".to_string())
                }
            }
            // The static sub-menus (recovery / other / decor / selectFunction).
            Some("recovery") => Some("ClanHallManager-manageFuncRecoveryBGrade.html".to_string()),
            Some("other") => Some("ClanHallManager-manageFuncOther.html".to_string()),
            Some("decor") => Some("ClanHallManager-manageFuncDecor.html".to_string()),
            Some("selectFunction") => {
                let func_id = next_i32(parts).unwrap_or(0);
                Some(format!("ClanHallManager-funcConfirm{func_id}.html"))
            }
            _ => Some(page("01")),
        }
    }
}

fn page(n: &str) -> String {
    format!("ClanHallManager-{n}.html")
}

fn next_i32(parts: &mut std::str::SplitWhitespace) -> Option<i32> {
    parts.next().and_then(|t| t.parse().ok())
}
