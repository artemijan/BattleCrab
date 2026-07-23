//! NPC location info — port of
//! `dist/game/data/scripts/custom/NpcLocationInfo/NpcLocationInfo.java`. The
//! Newbie Guide's "NPC Location Information" menu: pick a profession, pick a
//! person, and their spawn is marked on the radar.
//!
//! Reached through the bare `Quest NpcLocationInfo` bypass (`on_talk`), whose
//! pages then post `Quest NpcLocationInfo <arg>` — a page name to navigate,
//! or a bare npc id to place the marker.

use crate::game_loop::quests::{QuestCtx, QuestScript};

/// The five Newbie Guides whose menu hosts this script.
const NPC: &[i32] = &[30598, 30599, 30600, 30601, 30602];

/// Java `NPCRADAR` — the whitelist of NPCs this script will point at, five
/// starter villages' worth. Kept in the Java file's order (village by
/// village) so the two lists diff cleanly.
const NPC_RADAR: &[i32] = &[
    // Talking Island
    30006, 30039, 30040, 30041, 30042, 30043, 30044, 30045, 30046, 30283, 30003, 30004, 30001,
    30002, 30031, 30033, 30035, 30032, 30036, 30026, 30027, 30029, 30028, 30054, 30055, 30005,
    30048, 30312, 30368, 30049, 30047, 30497, 30050, 30311, 30051, //
    // Dark Elf Village
    30134, 30224, 30348, 30355, 30347, 30432, 30356, 30349, 30346, 30433, 30357, 30431, 30430,
    30307, 30138, 30137, 30135, 30136, 30143, 30360, 30145, 30144, 30358, 30359, 30141, 30139,
    30140, 30350, 30421, 30419, 30130, 30351, 30353, 30354, //
    // Elven Village
    30146, 30285, 30284, 30221, 30217, 30219, 30220, 30218, 30216, 30363, 30149, 30150, 30148,
    30147, 30155, 30156, 30157, 30158, 30154, 30153, 30152, 30151, 30423, 30414, 31853, 30223,
    30362, 30222, 30371, 31852, //
    // Dwarven Village
    30540, 30541, 30542, 30543, 30544, 30545, 30546, 30547, 30548, 30531, 30532, 30533, 30534,
    30535, 30536, 30525, 30526, 30527, 30518, 30519, 30516, 30517, 30520, 30521, 30522, 30523,
    30524, 30537, 30650, 30538, 30539, 30671, 30651, 30550, 30554, 30553, //
    // Orc Village
    30576, 30577, 30578, 30579, 30580, 30581, 30582, 30583, 30584, 30569, 30570, 30571, 30572,
    30564, 30560, 30561, 30558, 30559, 30562, 30563, 30565, 30566, 30567, 30568, 30585, 30587,
];

pub struct NpcLocationInfo;

impl QuestScript for NpcLocationInfo {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "NpcLocationInfo"
    }
    fn html_dir(&self) -> &'static str {
        "custom/NpcLocationInfo"
    }
    fn start_npcs(&self) -> &[i32] {
        NPC
    }
    fn talk_npcs(&self) -> &[i32] {
        NPC
    }
    /// The Newbie Guide's menu button posts the bare `Quest NpcLocationInfo`
    /// bypass, so this script has to run from the quest-window route.
    fn bare_talk(&self) -> bool {
        true
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        NPC.contains(&ctx.npc_id)
            .then(|| format!("{}.htm", ctx.npc_id))
    }

    /// `onEvent`: a numeric event is an npc id to mark on the radar, anything
    /// else is a page name to serve verbatim (`30598-1.htm`, …).
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let Ok(npc_id) = event.parse::<i32>() else {
            return Some(event.to_string());
        };
        if !NPC_RADAR.contains(&npc_id) {
            // Java returns null for an off-whitelist id: nothing is sent.
            return None;
        }
        // Java marks (0, 0, 0) when the NPC has no spawn — the arrow points
        // at the map origin rather than the page being suppressed.
        let (x, y, z) = ctx.any_spawn_location(npc_id).unwrap_or((0, 0, 0));
        ctx.add_radar(x, y, z);
        Some("MoveToLoc.htm".to_string())
    }
}
