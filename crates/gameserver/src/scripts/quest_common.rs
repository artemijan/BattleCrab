//! Mechanics shared by more than one quest script.

use crate::game_loop::quests::QuestCtx;

/// `lastAttacker` — Java's `npc.getVariables()` key, shared verbatim by the
/// class-path quests that use the weapon gate below.
const LAST_ATTACKER: &str = "lastAttacker";

/// Script value meaning "still qualifies": right weapon, one attacker.
pub const TAG_QUALIFIED: i32 = 1;
/// Script value meaning "disqualified" — terminal, never returns to 1.
pub const TAG_DISQUALIFIED: i32 = 2;

/// The "kill it solo with the quest weapon" gate used by the `Path of the *`
/// quests (401 Warrior, 403 Rogue, and others in the family).
///
/// Java's state machine, verbatim:
///
/// - **0** (untouched): record the attacker, then go to **1** if they are
///   holding one of `weapons`, else **2**.
/// - **1** (qualified): drop to **2** if the weapon is now wrong *or* a
///   different player joins in.
/// - **2**: terminal. Nothing re-qualifies the mob, so a mob someone else
///   tagged first is worthless to you even if you land the killing blow.
///
/// `onKill` then pays out only on `isScriptValue(1)`. Both halves are
/// required: without `on_attack` the value is never 1 and nothing ever drops.
///
/// Returns true when this call is the *first* hit and it qualified — the
/// moment Q00403 uses to fire the Cat's Eye Bandit's taunt.
pub fn tag_attacker_with_weapon(ctx: &mut QuestCtx, weapons: &[i32]) -> bool {
    if !ctx.has_qs() || !ctx.is_started() {
        return false;
    }
    let holding = weapons.contains(&ctx.equipped_weapon_id());
    match ctx.npc_script_value() {
        0 => {
            let attacker = ctx.player;
            ctx.set_npc_var_int(LAST_ATTACKER, attacker);
            if holding {
                ctx.set_npc_script_value(TAG_QUALIFIED);
                return true;
            }
            ctx.set_npc_script_value(TAG_DISQUALIFIED);
        }
        // Wrong weapon now, or a second player joined in.
        TAG_QUALIFIED if !holding || ctx.npc_var_int(LAST_ATTACKER) != ctx.player => {
            ctx.set_npc_script_value(TAG_DISQUALIFIED);
        }
        _ => {}
    }
    false
}

/// `npc.isScriptValue(1)` — the payout gate for the quests above.
pub fn is_tag_qualified(ctx: &QuestCtx) -> bool {
    ctx.npc_script_value() == TAG_QUALIFIED
}
