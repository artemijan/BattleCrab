//! `ai/areas/PaganTemple/PaganTeleporters` — the mark-gated doors into the
//! Pagan Temple and the Triol's Mirrors that teleport deeper in.
//!
//! Outside gatekeepers (32034/32036) demand a Visitor's/Pagan's Mark; the
//! inside pair (32035/32037) always open the way back out. Every open closes
//! itself 10 s later. The mirrors (32039/32040) teleport on first talk.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const GATEKEEPER_OUTER_IN: i32 = 32034;
const GATEKEEPER_OUTER_OUT: i32 = 32035;
const GATEKEEPER_INNER_IN: i32 = 32036;
const GATEKEEPER_INNER_OUT: i32 = 32037;
const TRIOLS_MIRROR_1: i32 = 32039;
const TRIOLS_MIRROR_2: i32 = 32040;

const VISITORS_MARK: i32 = 8064;
const FADED_VISITORS_MARK: i32 = 8065;
const PAGANS_MARK: i32 = 8067;

const OUTER_DOOR: i32 = 19_160_001;
const INNER_DOORS: [i32; 2] = [19_160_010, 19_160_011];

/// Java's `Close_Door*` timers: 10 s.
const CLOSE_TICKS: u64 = 100;

/// Every NPC this script owns — Java registers the same set through both
/// `addStartNpc` and `addTalkId`.
const NPCS: [i32; 6] = [
    GATEKEEPER_OUTER_IN,
    GATEKEEPER_OUTER_OUT,
    GATEKEEPER_INNER_IN,
    GATEKEEPER_INNER_OUT,
    TRIOLS_MIRROR_1,
    TRIOLS_MIRROR_2,
];

pub struct PaganTeleporters;

impl QuestScript for PaganTeleporters {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "PaganTeleporters"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/PaganTemple/PaganTeleporters"
    }
    fn start_npcs(&self) -> &[i32] {
        &NPCS
    }
    fn talk_npcs(&self) -> &[i32] {
        &NPCS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[TRIOLS_MIRROR_1, TRIOLS_MIRROR_2]
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        // The mirrors teleport on the first click — no chat window at all.
        match ctx.npc_id {
            TRIOLS_MIRROR_1 => ctx.teleport_to(-12766, -35840, -10856),
            TRIOLS_MIRROR_2 => ctx.teleport_to(36640, -51218, 718),
            _ => {}
        }
        None
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        match ctx.npc_id {
            GATEKEEPER_OUTER_IN => {
                if ctx.item_object_id(VISITORS_MARK).is_none()
                    && ctx.item_object_id(FADED_VISITORS_MARK).is_none()
                    && ctx.item_object_id(PAGANS_MARK).is_none()
                {
                    return Some("noItem.htm".into());
                }
                crate::game_loop::doors::open_door_timed(ctx.world, OUTER_DOOR, CLOSE_TICKS);
                Some("FadedMark.htm".into())
            }
            GATEKEEPER_OUTER_OUT => {
                crate::game_loop::doors::open_door_timed(ctx.world, OUTER_DOOR, CLOSE_TICKS);
                Some("FadedMark.htm".into())
            }
            GATEKEEPER_INNER_IN => {
                if ctx.item_object_id(PAGANS_MARK).is_none() {
                    return Some("noMark.htm".into());
                }
                for door in INNER_DOORS {
                    crate::game_loop::doors::open_door_timed(ctx.world, door, CLOSE_TICKS);
                }
                Some("openDoor.htm".into())
            }
            GATEKEEPER_INNER_OUT => {
                for door in INNER_DOORS {
                    crate::game_loop::doors::open_door_timed(ctx.world, door, CLOSE_TICKS);
                }
                Some("FadedMark.htm".into())
            }
            _ => None,
        }
    }
}
