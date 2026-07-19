//! Monster Derby Track teleport AI — port of
//! `dist/game/data/scripts/ai/others/TeleportToRaceTrack/`. Twelve
//! gatekeepers carry a free "Teleport to the Monster Arena and the Monster
//! Race Track" button; the Race Track Manager (30995) at the other end
//! sends the player back where they came from.
//!
//! The return point is remembered in the *character* variable store
//! (`MONSTER_RETURN` → the gatekeeper's npc id), not in quest state, which
//! is why this script needs `QuestCtx::{player_var_int, set_player_var_int,
//! unset_player_var}`.
//!
//! All twelve htmls (`html/teleporter/<id>.htm`, plus `html/default/`
//! 30995 and 31210) point their button at the *named* `Quest
//! TeleportToRaceTrack` bypass, so `bare_talk` stays false like Java: the
//! script is reached by name, never through the quest-window chooser
//! (which filters `id() > 0` on both sides).

use crate::game_loop::quests::{QuestCtx, QuestScript};

/// Race Track Manager — the NPC on the arena side, who sends players back.
const RACE_MANAGER: i32 = 30995;
/// Where every gatekeeper drops the player off.
const RACE_TRACK_TELEPORT: (i32, i32, i32) = (12661, 181687, -3540);
/// Java's `MONSTER_RETURN` player variable.
const MONSTER_RETURN: &str = "MONSTER_RETURN";
/// Fallback return NPC when the player has no stored origin — Trisha
/// (Dion), matching Java's `TELEPORTER_LOCATIONS.get(30059)`.
const DEFAULT_RETURN: i32 = 30059;

/// npc id → where talking to that gatekeeper returns you (Java
/// `TELEPORTER_LOCATIONS`). These are the gatekeepers' own towns, not the
/// gatekeeper's coordinates.
const TELEPORTER_LOCATIONS: &[(i32, (i32, i32, i32))] = &[
    (30320, (-80826, 149775, -3043)),  // Richlin
    (30256, (-12672, 122776, -3116)),  // Bella
    (30059, (15670, 142983, -2705)),   // Trisha
    (30080, (83400, 147943, -3404)),   // Clarissa
    (30899, (111409, 219364, -3545)),  // Flauen
    (30177, (82956, 53162, -1495)),    // Valentina
    (30848, (146331, 25762, -2018)),   // Elisa
    (30233, (116819, 76994, -2714)),   // Esmeralda
    (31320, (43835, -47749, -792)),    // Ilyana
    (31275, (147930, -55281, -2728)),  // Tatiana
    (31964, (87386, -143246, -1293)),  // Bilia
    (31210, (12882, 181053, -3560)),   // Race Track Gatekeeper
];

/// The talk/start npc list: every gatekeeper plus the manager.
const TALK_NPCS: &[i32] =
    &[RACE_MANAGER, 30320, 30256, 30059, 30080, 30899, 30177, 30848, 30233, 31320, 31275, 31964, 31210];

fn return_location(npc_id: i32) -> Option<(i32, i32, i32)> {
    TELEPORTER_LOCATIONS.iter().find(|(id, _)| *id == npc_id).map(|(_, loc)| *loc)
}

pub struct TeleportToRaceTrack;

impl QuestScript for TeleportToRaceTrack {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "TeleportToRaceTrack"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/TeleportToRaceTrack"
    }
    fn start_npcs(&self) -> &[i32] {
        TALK_NPCS
    }
    fn talk_npcs(&self) -> &[i32] {
        TALK_NPCS
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.npc_id == RACE_MANAGER {
            // Java guards with `> 30000` for "old script compatibility" —
            // the variable used to hold something else. Keep the guard so a
            // legacy value falls through to the Dion default instead of
            // being looked up.
            let return_id = ctx.player_var_int(MONSTER_RETURN, -1);
            if return_id > 30000 {
                // Java would NPE on an id outside the map; we fall back to
                // the default rather than drop the teleport on the floor.
                let loc = return_location(return_id).unwrap_or_else(|| {
                    return_location(DEFAULT_RETURN).expect("Trisha is in TELEPORTER_LOCATIONS")
                });
                ctx.teleport_to(loc.0, loc.1, loc.2);
                ctx.unset_player_var(MONSTER_RETURN);
            } else if let Some(loc) = return_location(DEFAULT_RETURN) {
                ctx.teleport_to(loc.0, loc.1, loc.2);
            }
        } else {
            ctx.teleport_to(RACE_TRACK_TELEPORT.0, RACE_TRACK_TELEPORT.1, RACE_TRACK_TELEPORT.2);
            ctx.set_player_var_int(MONSTER_RETURN, ctx.npc_id);
        }
        // Java returns `super.onTalk(...)` — null, i.e. no html: the
        // teleport closes the window on its own.
        None
    }
}
