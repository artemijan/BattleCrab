//! The casting pipeline: `RequestMagicSkillUse` validation, target
//! resolution, and the three scheduled phases (launch → finish → cool-down
//! end), plus cast aborts.

use crate::game_loop::common::maybe_distance_too_far;
use crate::game_loop::guard::{maybe_position, target_is_chest};
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::npc_template;
use crate::game_loop::helpers::send_action_failed;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::helpers::skill_by_id;
use crate::game_loop::helpers::{
    broadcast_including_self, client_for_player, ms_to_ticks, run_queued_action,
    send_sm_and_action_failed, send_sm_to_client, send_to_player,
};
use crate::model::Player;
use crate::model::components::{
    AttackState, Casting, Collision, Intent, Position, QueuedAction, Vitals,
};
use crate::model::formulas;
use crate::model::skill::{OperateType, Skill, SkillEffect, TargetType};
use crate::network::client_packets as cp;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::effects::apply_skill_effects;
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::helpers::send_sm_bare_to_player;
use crate::game_loop::helpers::stat_add;
use crate::game_loop::helpers::stop_movement;

mod abort;
mod channeling;
mod consequences;
mod lifecycle;
mod request;
mod reuse;
mod start;
mod target;

pub(crate) use abort::*;
pub(crate) use channeling::*;
// A deliberate one-function test seam, not path plumbing: `consequences` is the
// only module here imported with a *private* glob (`use consequences::*` below),
// because the cast pipeline's consequence half is internal. The shim on the
// other end is `#[cfg(test)]` too, which is what forces the cfg here.
#[cfg(test)]
pub(crate) use consequences::apply_bad_skill_aggro_for_test;
use consequences::*;
pub(crate) use lifecycle::*;
pub(crate) use request::*;
pub(crate) use reuse::*;
pub(crate) use start::*;
pub(crate) use target::*;
