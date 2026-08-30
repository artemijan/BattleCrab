//! `AdminTransform` (`//transform`/`//untransform`) and the transform-based
//! `AdminRide` rides (`//ride_horse` = transform 106, `//ride_bike` = 20001).
//!
//! A transform is durable state on the `Player` (`transform_id` +
//! `transform_display_id`) that swaps the model (CharInfo display id + the
//! self-view abnormal-visual packet), overrides run/walk speed and collision
//! from the [`Transform`](crate::data::transform_data::Transform) template,
//! grants the template's transform skills, swaps the client's action bar for
//! the template's `<actions>` (`ExBasicActionList`, restored to the default
//! list on the way out), and applies the `<base>` combat overrides through
//! `Player::recalculate_stats`.
//!
//! Still not applied, deliberately: the `<stats>`/`<defense>`/`<magicDefense>`/
//! `<levels>` blocks and the additional-item inventory blocks. A reachability
//! census found no template carrying any of them is enterable on this dist —
//! the evidence lives in `data::transform_data`'s module header.

use super::mounts::ride_target;
use crate::game_loop::admin::refresh_skill_list;
use crate::game_loop::helpers::{nth_arg, send_message, send_sm_bare_to_client};
use crate::model::Player;
use crate::model::components::{Collision, SkillBook};
use crate::world::World;

/// `//transform <id>` — transform the ride target (target player or GM) into the
/// given transform id.
pub(super) fn admin_transform(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(transform_id) = nth_arg::<i32>(args, 0) else {
        send_message(world, client_id, "Usage: //transform <id>");
        return;
    };
    if world.data.transforms.get(transform_id).is_none() {
        send_message(
            world,
            client_id,
            &format!("Transform {transform_id} does not exist."),
        );
        return;
    }
    let target = ride_target(world, object_id);
    // Java's gate order, and the subjects are not all the same object:
    // `activeChar.isSitting()` is the **GM issuing the command**, while the
    // transformed / in-water / mounted checks are the **target**. The port
    // tested the target's posture, which is a different rule whenever a GM
    // transforms someone else — and the message order matters too, since a
    // player who is both seated and mounted gets whichever check runs first.
    if crate::game_loop::character::sit_stand::is_sitting(world, object_id) {
        send_sm_bare_to_client(
            world,
            client_id,
            crate::network::server_packets::sm_ids::YOU_CANNOT_TRANSFORM_WHILE_SITTING,
        );
        return;
    }
    if world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.transform_id != 0)
    {
        send_message(
            world,
            client_id,
            "You already polymorphed and cannot polymorph again.",
        );
        return;
    }
    // `player.isInWater()`, and the predicate matters: Java's `Player`
    // method is `_taskWater != null` — the **drowning task is running** —
    // not "standing in a WATER zone". `position::is_in_water` is the zone
    // test (used for swim speed and geodata) and says so in its own doc;
    // using it here would refuse the transform for anyone in a castle moat
    // or wading where no breath timer ever started.
    if crate::game_loop::space::water::is_drowning_task_active(world, target) {
        send_sm_bare_to_client(
            world,
            client_id,
            crate::network::server_packets::sm_ids::YOU_CANNOT_POLYMORPH_INTO_THE_DESIRED_FORM_IN_WATER,
        );
        return;
    }
    if world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(Player::is_mounted)
    {
        send_sm_bare_to_client(
            world,
            client_id,
            crate::network::server_packets::sm_ids::YOU_CANNOT_TRANSFORM_WHILE_RIDING_A_PET,
        );
        return;
    }
    apply_transform(world, target, transform_id);
}

/// `//untransform` — revert the ride target to their normal form.
pub(super) fn admin_untransform(world: &mut World, object_id: i32) {
    let target = ride_target(world, object_id);
    remove_transform(world, target);
}

/// `AdminRide`'s transform-based rides — `//ride_horse` (106) / `//ride_bike`
/// (20001). Refused if already mounted or with a summon out (Java's shared
/// `isMounted() || hasSummon()` gate runs before every `//ride_*` branch), or
/// if already transformed (Java sends the polymorph message).
pub(super) fn admin_ride_transform(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    transform_id: i32,
) {
    let target = ride_target(world, object_id);
    if super::mounts::has_mount_or_summon(world, target) {
        send_message(world, client_id, "Target already have a summon.");
        return;
    }
    if world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.transform_id != 0)
    {
        send_message(
            world,
            client_id,
            "You already polymorphed and cannot polymorph again.",
        );
        return;
    }
    if world.data.transforms.get(transform_id).is_none() {
        send_message(world, client_id, "Transform data missing.");
        return;
    }
    apply_transform(world, target, transform_id);
}

/// `AdminRide`'s `//unride*` — Java dismounts, or untransforms if on a
/// transform-based ride (horse/bike). Routes to whichever the target is in.
pub(super) fn admin_dismount_or_untransform(world: &mut World, object_id: i32) {
    let target = ride_target(world, object_id);
    if world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.transform_id != 0)
    {
        remove_transform(world, target);
    } else {
        super::mounts::dismount(world, target);
    }
}

/// Apply a transform: set the display state, override collision, grant the
/// template's transform skills, recompute speed, and broadcast. Used by the
/// admin `//transform`/`//ride_*` commands, which are instantaneous and want
/// the broadcast immediately; the `Transformation` skill effect
/// (`game_loop::skills::effects`) instead calls [`apply_transform_state`]
/// directly and lets the buff-landing path own the broadcast, since it's
/// already sending `UserInfo`/`CharInfo` for the buff that carries this.
/// `game_loop::items::cursed_weapon` also calls it for `CursedWeapon.doTransform`.
pub(crate) fn apply_transform(world: &mut World, target: i32, transform_id: i32) {
    apply_transform_state(world, target, transform_id);
    broadcast_transform(world, target);
    // Java `Transform.onTransform` ends with the delayed
    // `updateAbnormalVisualEffects` ("you need to broadcast this to trigger the
    // transformation client-side"): the inline packets above reach the actor
    // that is being replaced, so the visual list has to arrive a tick later.
    crate::game_loop::abnormal::schedule_visual_refresh(world, target);
}

/// The state half of [`apply_transform`], without the broadcast: set the
/// display state, override collision, grant the template's transform skills,
/// recompute speed.
pub(crate) fn apply_transform_state(world: &mut World, target: i32, transform_id: i32) {
    let is_female = world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.is_female);
    let Some(tf) = world.data.transforms.get(transform_id) else {
        return;
    };
    let display_id = tf.display_id;
    let tmpl = tf.template(is_female);
    let (radius, height) = (tmpl.collision_radius, tmpl.collision_height);
    let skills = tmpl.skills.clone();
    let actions = tmpl.actions.clone();

    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.transform_id = transform_id;
        p.transform_display_id = display_id;
    }
    if radius > 0.0 || height > 0.0 {
        world
            .objects
            .add_components(&target, Collision { radius, height });
    }
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&target) {
        for (id, level) in &skills {
            book.0.insert(*id, *level);
        }
    }
    // Java `Transform.onTransform`: `if (template.hasBasicActionList())` — swap
    // the client's action bar for the transform's own `<actions>` list. An
    // empty list means the template carried no block and Java sends nothing,
    // leaving the previous bar alone.
    if !actions.is_empty() {
        crate::game_loop::helpers::send_to_player(
            world,
            target,
            crate::network::enter_world::ex_basic_action_list_ids(&actions),
        );
    }
    crate::game_loop::helpers::recalculate_player_stats_and_vitals(world, target);
}

/// Remove a transform: clear the display state, restore the class collision,
/// drop the template's transform skills, recompute, and broadcast. Used by the
/// admin `//untransform`/dismount commands, which want the broadcast
/// immediately.
pub(crate) fn remove_transform(world: &mut World, target: i32) {
    if remove_transform_state(world, target) {
        broadcast_transform(world, target);
        // Untransforming swaps the model back, so the visual list needs the
        // same delayed resend — otherwise an invisible GM loses the STEALTH
        // glow reverting, exactly as on dismount.
        crate::game_loop::abnormal::schedule_visual_refresh(world, target);
    }
}

/// The state half of [`remove_transform`], without the broadcast: clear the
/// display state, restore the class collision, drop the template's transform
/// skills, recompute speed. Returns `false` (no-op) if the target wasn't
/// transformed. Used directly by the `Transformation` skill effect's
/// `BuffExpire`/dispel/death cleanup
/// (`game_loop::skills::effects::handle_buff_expire`), which folds the
/// broadcast into the generic buff-removal `UserInfo` it already sends rather
/// than sending a second one.
/// Put the player's own collision box back, from their class template — the
/// undo half of the transform/mount collision override.
///
/// Falls back to the base class's template when a subclass has none, and does
/// nothing at all when neither is loaded.
pub(super) fn restore_class_collision(world: &mut World, target: i32) {
    let (class_id, base_class_id) = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| (p.class_id, p.base_class_id))
        .unwrap_or((0, 0));
    if let Some(t) = world
        .data
        .player_templates
        .get_or_base(class_id, base_class_id)
    {
        world.objects.add_components(
            &target,
            Collision {
                radius: t.collision_radius,
                height: t.collision_height,
            },
        );
    }
}

pub(crate) fn remove_transform_state(world: &mut World, target: i32) -> bool {
    let transform_id = world
        .objects
        .get_component::<Player>(&target)
        .map_or(0, |p| p.transform_id);
    if transform_id == 0 {
        return false;
    }
    // Skills the transform granted (removed on revert; Java tracks these in
    // `_transformSkills` — here we re-derive them from the template).
    let is_female = world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.is_female);
    let skills: Vec<i32> = world
        .data
        .transforms
        .get(transform_id)
        .map(|tf| {
            tf.template(is_female)
                .skills
                .iter()
                .map(|(id, _)| *id)
                .collect()
        })
        .unwrap_or_default();
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&target) {
        for id in &skills {
            book.0.remove(id);
        }
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.transform_id = 0;
        p.transform_display_id = 0;
    }
    restore_class_collision(world, target);
    // Java `Transform.onUntransform`: `player.sendPacket(
    // ExBasicActionList.STATIC_PACKET)` — unconditional, unlike the transform
    // side's `hasBasicActionList()` guard. A form whose template carried no
    // `<actions>` still restores the default bar on the way out, so a player
    // can never be left holding a previous transform's action list.
    let default_bar = crate::network::enter_world::ex_basic_action_list(&world.data);
    crate::game_loop::helpers::send_to_player(world, target, default_bar);
    crate::game_loop::helpers::recalculate_player_stats_and_vitals(world, target);
    true
}

/// Broadcast the transform change: UserInfo to self + CharInfo to nearby (via
/// `broadcast_user_info`), then [`refresh_transform_visuals`].
fn broadcast_transform(world: &mut World, target: i32) {
    crate::game_loop::character::player_info::broadcast_user_info(world, target);
    refresh_transform_visuals(world, target);
}

/// The self-view abnormal-visual packet carrying the transform model, and a
/// refreshed SkillList (the transform's granted skills need to show up in the
/// client's skill window immediately). Split out from [`broadcast_transform`]
/// so the `Transformation` skill effect can send these on top of the
/// `UserInfo`/`CharInfo` its buff-landing path already broadcasts, without a
/// second full `UserInfo` send.
pub(crate) fn refresh_transform_visuals(world: &World, target: i32) {
    let display_id = world
        .objects
        .get_component::<Player>(&target)
        .map_or(0, |p| p.transform_display_id);
    let hidden = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&target)
        .is_some_and(|f| f.hidden);
    if let Some(cid) = super::helpers::client_for_player(world, target) {
        let visuals = crate::game_loop::abnormal::visual_effects(world, target);
        let ave = crate::network::user_info::ex_user_info_abnormal_visual_effect(
            target, hidden, display_id, &visuals,
        );
        crate::game_loop::helpers::send_to_client(world, cid, ave);
        refresh_skill_list(world, target);
    }
}
