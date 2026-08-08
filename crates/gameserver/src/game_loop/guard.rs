//! Guard clauses for command handlers.
//!
//! Nearly every admin/bypass handler opens with the same shape: resolve
//! something (the current target, the target's clan, a parsed argument), and if
//! it isn't there, send one system message and return. Written literally that is
//! four to six lines per step, so a handler with three preconditions spends more
//! lines refusing than doing.
//!
//! The duplication comes from failure having no *value*: with nothing to return,
//! each site has to spell out its own `else { send_sm(...); return; }`. This
//! module gives failure a value — [`Reject`] — so a handler can bail with `?`:
//!
//! ```ignore
//! let target  = guard::player_target(world, object_id).or_sm(sm_ids::INVALID_TARGET)?;
//! let clan_id = guard::clan_of(world, target).or_sm(sm_ids::THE_TARGET_MUST_BE_A_CLAN_MEMBER)?;
//! ```
//!
//! The resolvers deliberately answer `Option` and pick **no** message of their
//! own: which system message a failed precondition sends is Java-fidelity data
//! that differs per call site (the very same "no clan" check answers
//! `THE_TARGET_MUST_BE_A_CLAN_MEMBER` in `AdminSkill` and a plain "Target player
//! has no clan!" in `AdminPledge`). Baking a message into the resolver would let
//! the next ported command silently send the wrong one, so the message stays at
//! the call site where a reviewer diffs it against the Java.
//!
//! Handlers that adopt this return [`Guard<()>`] and get wrapped by a thin
//! public function that hands the rejection to [`finish`]. Keeping the wrapper
//! separate is what lets a command run its own tail — `AdminPledge` re-shows the
//! Game panel after *both* outcomes — without pushing that tail into the shared
//! helper.

use crate::model::Player;
use crate::model::components::{Position, TargetRef};
use crate::model::npc::Npc;
use crate::network::server_packets::{SmParam, sm_ids};
use crate::world::World;

use super::helpers;

/// Why a handler stopped early, and what the client should be told about it.
///
/// Everything here is a *refusal*: the command did nothing and the player gets
/// one line of feedback. Failures that need a packet richer than a system
/// message stay written out at their call site — this type is for the long tail
/// of one-message bails, not a general error channel.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Reject {
    /// A bare `SystemMessage(id)` — the overwhelmingly common case.
    Sm(i16),
    /// A parameterised system message, e.g. `S1_IS_NOT_A_CLAN_LEADER` + name.
    SmWith(i16, Vec<SmParam>),
    /// Java `Player.sendMessage(String)` — free text through `S1_TEXT`.
    Msg(String),
    /// An exception that escaped the handler into Java's `AdminCommandHandler`.
    ///
    /// Distinct from [`Reject::Msg`] because the dispatcher catching a throw
    /// also skips whatever the command would have run afterwards — a menu
    /// re-show, typically. Callers that have such a tail check for this variant.
    Abort(String),
    /// Java returns without telling the player anything.
    Silent,
}

/// A handler result: `Ok` on the paths that did something, `Err` on a refusal.
pub(crate) type Guard<T> = Result<T, Reject>;

/// Turns a resolver's `Option` into a [`Guard`] carrying the message this
/// particular call site should send. Implemented for `Option<T>`, so it also
/// covers plain boolean preconditions via `cond.then_some(())`.
pub(crate) trait OrReject<T> {
    /// Bail with a bare system message.
    fn or_sm(self, message_id: i16) -> Guard<T>;
    /// Bail with a parameterised system message.
    fn or_sm_with(self, message_id: i16, params: Vec<SmParam>) -> Guard<T>;
    /// Bail with free text (Java `sendMessage`).
    fn or_msg(self, text: impl Into<String>) -> Guard<T>;
    /// Bail the way a Java exception would: print, and skip the command's tail.
    fn or_abort(self, text: impl Into<String>) -> Guard<T>;
    /// Bail without telling the player anything.
    fn or_silent(self) -> Guard<T>;
}

impl<T> OrReject<T> for Option<T> {
    fn or_sm(self, message_id: i16) -> Guard<T> {
        self.ok_or(Reject::Sm(message_id))
    }

    fn or_sm_with(self, message_id: i16, params: Vec<SmParam>) -> Guard<T> {
        self.ok_or(Reject::SmWith(message_id, params))
    }

    fn or_msg(self, text: impl Into<String>) -> Guard<T> {
        self.ok_or_else(|| Reject::Msg(text.into()))
    }

    fn or_abort(self, text: impl Into<String>) -> Guard<T> {
        self.ok_or_else(|| Reject::Abort(text.into()))
    }

    fn or_silent(self) -> Guard<T> {
        self.ok_or(Reject::Silent)
    }
}

/// Send the one line a [`Reject`] asks for. `Silent` sends nothing.
pub(crate) fn report(world: &World, client_id: u32, reject: Reject) {
    match reject {
        Reject::Sm(id) => helpers::send_sm_to_client(world, client_id, id, &[]),
        Reject::SmWith(id, params) => helpers::send_sm_to_client(world, client_id, id, &params),
        Reject::Msg(text) | Reject::Abort(text) => {
            helpers::send_sm_to_client(world, client_id, sm_ids::S1_TEXT, &[SmParam::Text(text)])
        }
        Reject::Silent => {}
    }
}

/// Close out a guarded handler: report a refusal, do nothing on success.
pub(crate) fn finish(world: &World, client_id: u32, result: Guard<()>) {
    if let Err(reject) = result {
        report(world, client_id, reject);
    }
}

// ---------------------------------------------------------------------------
// Resolvers — all answer `Option`, none picks a message
// ---------------------------------------------------------------------------

/// The object id this creature currently has selected, if any.
pub(crate) fn target(world: &World, object_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<TargetRef>(&object_id)
        .and_then(|t| t.0)
}

/// The current target, but only when it is a player — Java's
/// `target == null || !target.isPlayer()` guard in one call.
pub(crate) fn player_target(world: &World, object_id: i32) -> Option<i32> {
    target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
}

/// The current target, but only when it is an NPC.
pub(crate) fn npc_target(world: &World, object_id: i32) -> Option<i32> {
    target(world, object_id).filter(|oid| world.objects.has_component::<Npc>(oid))
}

/// The player's clan id, or `None` when they are clanless — Java
/// `player.getClan() == null`, which the port spells as the sentinel `0`.
pub(crate) fn clan_of(world: &World, player_object_id: i32) -> Option<i32> {
    world
        .objects
        .get_component::<Player>(&player_object_id)
        .map(|p| p.clan_id)
        .filter(|&clan_id| clan_id != 0)
}

/// An object's position.
pub(crate) fn position(world: &World, object_id: i32) -> Option<Position> {
    world.objects.get_component::<Position>(&object_id).copied()
}
