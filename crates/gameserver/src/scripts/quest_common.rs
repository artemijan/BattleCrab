//! Mechanics shared by more than one quest script.

use crate::game_loop::quests::QuestCtx;

/// The Olympiad participation gate the Grand Olympiad Manager and the Monument
/// of Heroes both apply: 3rd/4th class group and level ≥ 55.
pub fn olympiad_eligible(ctx: &QuestCtx) -> bool {
    (ctx.is_in_category("THIRD_CLASS_GROUP") || ctx.is_in_category("FOURTH_CLASS_GROUP"))
        && ctx.player_level() >= 55
}
