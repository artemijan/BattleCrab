//! The Valakas lair-entry NPC chain — port of
//! `ai/others/ValakasTeleporters`. Six NPCs, all reached through the bare
//! `Quest ValakasTeleporters` bypass in their `html/default/<id>.htm`
//! (→ [`on_talk`]), plus Klein's `31540` sub-event (→ [`on_event`]):
//!
//! - **Watcher Klein (31540)** — crowding status on talk; the antechamber
//!   teleport (Vacualite-gated) on the `31540` sub-event.
//! - **Heart of Volcano (31385)** — the lair door.
//! - **Teleport Cubic (31759)** — the exit.
//! - **Gatekeepers (31384 / 31686 / 31687)** — open the path doors.
//!
//! The interesting half lives in [`valakas`]; this is the
//! player-facing routing.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::game_loop::valakas;

const KLEIN: i32 = 31540;
const HEART: i32 = 31385;
const CUBE: i32 = 31759;
const GATE_A: i32 = 31384;
const GATE_B: i32 = 31686;
const GATE_C: i32 = 31687;

const TALK_NPCS: &[i32] = &[KLEIN, HEART, CUBE, GATE_A, GATE_B, GATE_C];

pub struct ValakasTeleporters;

impl QuestScript for ValakasTeleporters {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "ValakasTeleporters"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/ValakasTeleporters"
    }
    fn start_npcs(&self) -> &[i32] {
        TALK_NPCS
    }
    fn talk_npcs(&self) -> &[i32] {
        TALK_NPCS
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        match ctx.npc_id {
            HEART => valakas::heart_enter(ctx.world, ctx.player).map(str::to_string),
            KLEIN => Some(valakas::klein_status_html(ctx.world).to_string()),
            CUBE => {
                valakas::teleport_out(ctx.world, ctx.player);
                None
            }
            GATE_A => {
                crate::game_loop::npc::doors::open_door_by_id(ctx.world, 24210004);
                None
            }
            GATE_B => {
                crate::game_loop::npc::doors::open_door_by_id(ctx.world, 24210005);
                None
            }
            GATE_C => {
                crate::game_loop::npc::doors::open_door_by_id(ctx.world, 24210006);
                None
            }
            _ => None,
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        // Klein's `31540` button — the Vacualite-gated Hall of Flames teleport.
        // (Java's `onEvent` ignores the event string; the port keys on it so a
        // stray event doesn't teleport.)
        if event == "31540" {
            return valakas::enter_hall_of_flames(ctx.world, ctx.player).map(str::to_string);
        }
        None
    }
}
