//! AllianceMaster — port of
//! `dist/game/data/scripts/village_master/AllianceMaster/AllianceMaster.java`,
//! the alliance dialog on every village master. 67 Java lines, the smallest
//! script in the group and the last one left.
//!
//! The whole script is one guard: **every page except the menu requires a
//! clan.** `onTalk` always opens `9001-01.htm`, and `onEvent` echoes the
//! requested page back unless the player is clanless, in which case it serves
//! `9001-04.htm` ("You must be in Clan"). Note the asymmetry — the menu itself
//! is *not* gated, so a clanless player sees the two buttons and only learns
//! they can't use them after clicking. That's Java's behaviour, kept.
//!
//! Like `ClanMaster`, the pages are numbered against a **virtual NPC id**
//! (`9001-NN.htm`; ClanMaster uses `9000`) which is not any of the 60 real
//! masters in the list — one page set serves all of them. The same 60 NPCs as
//! `ClanMaster`, deliberately: both scripts attach to every village master.
//!
//! Both buttons are live. `9001-02.htm` posts
//! `npc_%objectId%_create_ally $name` and `9001-03.htm` posts
//! `npc_%objectId%_dissolve_ally`; G18 landed the alliance system (ally
//! id/name/crest, the `PledgeShowInfoUpdate` ally fields, ally war) and routed
//! both verbs in `bypass.rs` to `clans::handle_create_ally` /
//! `handle_dissolve_ally`.

use crate::game_loop::quests::{QuestCtx, QuestScript};

use super::VILLAGE_MASTERS as NPCS;

const MENU: &str = "9001-01.htm";
const NEED_CLAN: &str = "9001-04.htm";

pub struct AllianceMaster;

impl QuestScript for AllianceMaster {
    fn id(&self) -> i32 {
        -1 // a "script", not a quest — never in QuestList/choosers
    }

    fn name(&self) -> &'static str {
        "AllianceMaster"
    }

    fn html_dir(&self) -> &'static str {
        "village_master/AllianceMaster"
    }

    fn start_npcs(&self) -> &[i32] {
        &NPCS
    }

    fn talk_npcs(&self) -> &[i32] {
        &NPCS
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        Some(MENU.to_string())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        // `if (!"9001-01.htm".equals(event) && (player.getClan() == null))`.
        // The menu is reachable without a clan; everything else is not.
        if event != MENU && !ctx.has_clan() {
            return Some(NEED_CLAN.to_string());
        }
        Some(event.to_string())
    }
}
