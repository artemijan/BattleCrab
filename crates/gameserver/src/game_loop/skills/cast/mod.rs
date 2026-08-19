//! The casting pipeline: `RequestMagicSkillUse` validation, target
//! resolution, and the three scheduled phases (launch → finish → cool-down
//! end), plus cast aborts.

mod abort;
mod channeling;
mod consequences;
mod lifecycle;
mod request;
mod reuse;
mod start;
mod target;

pub(crate) use abort::{
    abort_all_skill_casters, abort_cast, abort_cast_when_untargeted, break_cast, known_skill_level,
    set_cast_trigger_item,
};
#[cfg(test)]
pub(crate) use channeling::buff_level;
pub(crate) use channeling::{handle_channeling_tick, stop_channelizing};
#[cfg(test)]
pub(crate) use consequences::apply_bad_skill_aggro_for_test;
use consequences::{apply_cast_consequences, calc_buff_debuff_reflection, matchup_effects};
use lifecycle::live_cast_skill;
#[cfg(test)]
pub(crate) use lifecycle::resume_action_after_cast_for_test;
pub(crate) use lifecycle::{handle_cast_end, handle_skill_finish, handle_skill_launch};
pub(crate) use request::{
    handle_request_magic_skill_use, handle_request_magic_skill_use_ground, op_exist_npc_around,
    use_magic, use_magic_on,
};
pub(crate) use reuse::{check_skill_reuse, set_skill_reuse};
pub(crate) use start::{begin_cast, start_casting, stop_casting};
pub(crate) use target::{finalize_target, in_cast_range, resolve_cast_target, target_state};
// A deliberate one-function test seam, not path plumbing: `consequences` is the
// only module here imported with a *private* glob (`use consequences::*` below),
// because the cast pipeline's consequence half is internal. The shim on the
// other end is `#[cfg(test)]` too, which is what forces the cfg here.
