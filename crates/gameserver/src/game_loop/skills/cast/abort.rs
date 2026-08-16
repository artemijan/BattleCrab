//! Cast aborts: the manual/forced abort family and the packet emit.

use super::*;

/// Java `SkillCaster._item`: mark the running cast as started by this item
/// instance, so `finishSkill` can spend it if the cast lands. Called right
/// after [`start_casting`] by the item-skill path — nothing runs in between on
/// the game thread, so this is the same as Java setting it in the constructor.
pub(crate) fn set_cast_trigger_item(world: &mut World, object_id: i32, item_object_id: i32) {
    if let Some(cast) = world.objects.get_component_mut::<Casting>(&object_id) {
        cast.0.trigger_item_object_id = item_object_id;
    }
}

/// Port of `Creature.abortCast` → `stopCasting(aborted == true)`: only casts
/// that haven't launched can be aborted; broadcast `MagicSkillCanceled` (self
/// included, to stop the animation) + `ActionFailed` to the caster. The
/// already-scheduled phase tasks go stale via the seq mismatch.
pub(crate) fn abort_cast(world: &mut World, object_id: i32) {
    let abortable = world
        .objects
        .get_component::<Casting>(&object_id)
        .is_some_and(|c| !c.0.launched);
    if !abortable {
        return;
    }
    // Java `stopCasting(true)` also ends with `EVT_FINISH_CASTING`, so an
    // interrupted cast still releases the click it held back.
    emit_cast_abort(world, object_id);
}

/// `stopCasting(aborted == true)`'s payload, shared by every abort gate above
/// it: `MagicSkillCanceled` broadcast **including self** — it is the only
/// packet that stops the cast animation client-side, so leaving it out keeps
/// the caster visibly channelling for the rest of the client-side cast time —
/// then `ActionFailed` to the caster (`caster.sendPacket`, a no-op for an NPC,
/// which is why it isn't gated on the victim being a player), then the stop
/// itself. The gates differ; this tail never does.
fn emit_cast_abort(world: &mut World, object_id: i32) {
    broadcast_including_self(
        world,
        object_id,
        &server_packets::magic_skill_canceld(object_id),
    );
    send_to_player(world, object_id, server_packets::action_failed());
    stop_casting(world, object_id);
}

/// Port of `Creature.abortAllSkillCasters` → `stopCasting(true)` on *every*
/// running caster. Unlike [`abort_cast`] this takes **no** gate at all: Java
/// iterates `getSkillCasters()` raw, without the `canAbortCast` filter and
/// without any phase check, so a cast that already launched is killed too.
///
/// That distinction is the whole point of the call site — `BlockActions.onStart`
/// (stun / sleep / paralyze) uses this, not `abortCast`, so a stun landing in
/// the launched window still stops the skill instead of letting it resolve.
pub(crate) fn abort_all_skill_casters(world: &mut World, object_id: i32) {
    if !world.objects.has_component::<Casting>(&object_id) {
        return;
    }
    emit_cast_abort(world, object_id);
}

/// `Creature.abortCast()` with Java's *real* gate: `abortCast` resolves its
/// caster through `SkillCaster.canAbortCast` — and that is *not* the phase
/// check its comment claims. It is literally
/// `getCaster().getTarget() == null` (`SkillCaster.java:940`), so the cast is
/// cancelled exactly while the creature has nothing selected.
///
/// [`abort_cast`]'s `!launched` guard models the other abort paths and is
/// deliberately not reused: the effects that call this fire from the *finish*
/// phase, when `launched` is already true, so that guard would swallow the
/// `MagicSkillCanceled`. That packet is the only thing that stops the cast
/// animation client-side — without it the escape FX keeps playing at the
/// destination until the client's own skill duration elapses (5 minutes for
/// skill 2099), long after `/unstuck` already teleported the player.
///
/// Call sites: `Creature.teleToLocation`'s prologue (`Escape`, `Recall`) and
/// `CallPc`'s ENEMY branch, where a monster drags its victim (Porta 20213 /
/// skill 4161) — both are `abortCast()` in Java, so both take this gate.
pub(crate) fn abort_cast_when_untargeted(world: &mut World, object_id: i32) {
    if !world.objects.has_component::<Casting>(&object_id) {
        return;
    }
    let has_target = world
        .objects
        .get_component::<crate::model::components::TargetRef>(&object_id)
        .is_some_and(|t| t.0.is_some());
    if has_target {
        return;
    }
    emit_cast_abort(world, object_id);
}

/// Port of `Creature.breakCast`: a cast broken by *incoming damage* (as opposed
/// to a self-initiated `abortCast`). It performs the same abort — `MagicSkillCanceled`
/// + `ActionFailed`, only for a not-yet-launched cast — and then, if the victim
///   is a player, additionally sends the `YOUR_CASTING_HAS_BEEN_INTERRUPTED`
///   system message. That extra message is the sole difference from [`abort_cast`],
///   which is why the movement/self-abort call sites keep using `abort_cast`.
pub(crate) fn break_cast(world: &mut World, object_id: i32) {
    let breakable = world
        .objects
        .get_component::<Casting>(&object_id)
        .is_some_and(|c| !c.0.launched);
    if !breakable {
        return;
    }
    abort_cast(world, object_id);
    send_sm_bare_to_player(
        world,
        object_id,
        server_packets::sm_ids::YOUR_CASTING_HAS_BEEN_INTERRUPTED,
    );
}

/// Java `Creature.getKnownSkill(id)` — the level at which this player knows a
/// skill, from either the persisted book or a transient grant.
///
/// The book wins when both carry the id: that is Java's map-insertion order for
/// a learned skill re-granted by an option, and it keeps a player's own trained
/// level from being downgraded by an item.
pub(crate) fn known_skill_level(world: &World, object_id: i32, skill_id: i32) -> Option<i32> {
    if let Some(level) = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&object_id)
        .and_then(|book| book.0.get(&skill_id).copied())
    {
        return Some(level);
    }
    world
        .objects
        .get_component::<crate::model::components::OptionSkills>(&object_id)
        .and_then(|opts| opts.0.get(&skill_id).copied())
}
