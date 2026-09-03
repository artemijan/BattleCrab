//! Olympiad Buffer (36402) — port of
//! `dist/game/data/scripts/ai/others/OlyBuffer/OlyBuffer.java`.
//!
//! Two of these stand in each Olympiad arena instance. A competitor waiting for
//! the match may take **five** buffs from a fixed list of nine, and after the
//! fifth the buffer says so and removes itself five seconds later — so the
//! allowance is per *NPC instance*, which is what makes it per arena-entry
//! rather than per player.
//!
//! The counter is `Npc.getScriptValue()`, incremented on each granted buff.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::game_loop::skills;

pub struct OlyBuffer;

/// `OlyBuffer.OLYMPIAD_BUFFER`.
const OLYMPIAD_BUFFER: i32 = 36402;
const NPCS: &[i32] = &[OLYMPIAD_BUFFER];

/// `ALLOWED_BUFFS`, in the order the htmls' `giveBuff;<n>` buttons index.
/// The levels are the ones Java declares, not the skills' maxima — the
/// `afterBuff` page's labels ("Haste Lv2") describe the *client's* icon, not
/// what is cast.
const ALLOWED_BUFFS: &[(i32, i32)] = &[
    (1086, 1), // Haste — Atk. Spd. +15%
    (1085, 1), // Acumen — Casting Spd. +15%
    (1204, 1), // Wind Walk — Speed +20
    (1068, 1), // Might — P. Atk. +8%
    (1040, 1), // Shield — P. Def. +8%
    (1036, 1), // Magic Barrier — M. Def. +23%
    (1045, 1), // Blessed Body — Max HP +10%
    (1048, 1), // Blessed Soul — Max MP +10%
    (1062, 1), // Berserker Spirit
];

/// `if (npc.getScriptValue() < 5)` — the per-buffer allowance.
const MAX_BUFFS: i32 = 5;

/// `getTimers().addTimer("DELETE_ME", 5000, …)`.
const DELETE_DELAY_MS: u64 = 5000;

impl QuestScript for OlyBuffer {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "OlyBuffer"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/OlyBuffer"
    }
    fn start_npcs(&self) -> &[i32] {
        NPCS
    }
    fn talk_npcs(&self) -> &[i32] {
        NPCS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        NPCS
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    /// `onFirstTalk`: the menu, but only while the buffer has buffs left.
    /// Java returns `null` past the fifth, which shows no window at all — the
    /// NPC is already on its way out.
    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.npc_script_value() < MAX_BUFFS).then(|| "OlyBuffer-index.html".to_string())
    }

    /// `onEvent`: `giveBuff;<index>` casts that buff on the talker and counts
    /// it. The fifth one also schedules the buffer's own removal.
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let index = event.strip_prefix("giveBuff;")?;
        if ctx.npc_script_value() >= MAX_BUFFS {
            return None;
        }
        // `Integer.parseInt` — Java throws on a forged tail and the exception
        // escapes the handler; here a bad index simply grants nothing.
        let &(skill_id, level) = index
            .trim()
            .parse::<usize>()
            .ok()
            .and_then(|i| ALLOWED_BUFFS.get(i))?;

        ctx.set_npc_script_value(ctx.npc_script_value() + 1);
        let (npc_oid, player_oid) = (ctx.npc, ctx.player);
        // `MagicSkillUse(npc, player, …)` then `applyEffects(npc, player)` —
        // Java shows the cast and lands the buff directly rather than routing
        // through a real cast, so there is no cast time and nothing to
        // interrupt.
        ctx.cast_visual_at(npc_oid, player_oid, skill_id, level, 0);
        if let Some(skill) = skills::skill_by_id(ctx.world, skill_id, level) {
            skills::effects::apply_skill_effects(ctx.world, npc_oid, player_oid, &skill);
        }

        let mut page = "OlyBuffer-afterBuff.html";
        if ctx.npc_script_value() >= MAX_BUFFS {
            page = "OlyBuffer-noMore.html";
            ctx.schedule_despawn(npc_oid, DELETE_DELAY_MS);
        }
        Some(page.to_string())
    }
}
