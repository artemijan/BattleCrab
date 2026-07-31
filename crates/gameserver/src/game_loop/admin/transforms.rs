//! `AdminTransform` (`//transform`/`//untransform`) and the transform-based
//! `AdminRide` rides (`//ride_horse` = transform 106, `//ride_bike` = 20001).
//!
//! A transform is durable state on the `Player` (`transform_id` +
//! `transform_display_id`) that swaps the model (CharInfo display id + the
//! self-view abnormal-visual packet), overrides run/walk speed and collision
//! from the [`Transform`](crate::data::transform_data::Transform) template, and
//! grants the template's transform skills. The deeper Java integration (base
//! combat-stat overrides, the `ExBasicActionList` swap, additional-item
//! inventory blocks) is not applied yet — documented TODO.

use crate::model::Player;
use crate::model::components::{
    BaseStats, Collision, CombatStats, SkillBook, Speeds, StatModifiers,
};
use crate::model::inventory::Inventory;
use crate::world::World;

use super::{current_target, send_message, send_sm};

/// `//transform <id>` — transform the ride target (target player or GM) into the
/// given transform id.
pub(super) fn admin_transform(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(transform_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
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
    // Java `AdminTransform`: a mounted target can't polymorph — SM 2063 — and
    // neither can a seated one. TODO(G33): the in-water leg
    // (`…_TRANSFORM_IN_WATER`) still has no reader here.
    if world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(Player::is_mounted)
    {
        send_sm(
            world,
            client_id,
            crate::network::server_packets::sm_ids::YOU_CANNOT_TRANSFORM_WHILE_RIDING_A_PET,
        );
        return;
    }
    if crate::game_loop::sit_stand::is_sitting(world, target) {
        send_sm(
            world,
            client_id,
            crate::network::server_packets::sm_ids::YOU_CANNOT_TRANSFORM_WHILE_SITTING,
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

/// Java `AdminRide.getRideTarget` — the current target if it's a *different*
/// player, else the GM. (Mirrors [`mounts::ride_target`].)
fn ride_target(world: &World, object_id: i32) -> i32 {
    current_target(world, object_id)
        .filter(|&oid| oid != object_id && world.objects.has_component::<Player>(&oid))
        .unwrap_or(object_id)
}

/// Apply a transform: set the display state, override collision, grant the
/// template's transform skills, recompute speed, and broadcast. Used by the
/// admin `//transform`/`//ride_*` commands, which are instantaneous and want
/// the broadcast immediately; the `Transformation` skill effect
/// (`game_loop::skills::effects`) instead calls [`apply_transform_state`]
/// directly and lets the buff-landing path own the broadcast, since it's
/// already sending `UserInfo`/`CharInfo` for the buff that carries this.
/// `game_loop::cursed_weapon` also calls it for `CursedWeapon.doTransform`.
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
    recompute_speeds(world, target);
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
    // Restore the class-template collision.
    let (class_id, base_class_id) = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| (p.class_id, p.base_class_id))
        .unwrap_or((0, 0));
    if let Some(t) = world
        .data
        .player_templates
        .get(class_id)
        .or_else(|| world.data.player_templates.get(base_class_id))
    {
        world.objects.add_components(
            &target,
            Collision {
                radius: t.collision_radius,
                height: t.collision_height,
            },
        );
    }
    recompute_speeds(world, target);
    true
}

/// Gather the stat components and re-run `recalculate_stats` so the transform
/// or mount speed override (in `Player::recalculate_stats`) takes effect.
/// Shared with the mount module (`mounts.rs`).
pub(super) fn recompute_speeds(world: &mut World, target: i32) {
    let data = &world.data;
    if let Some((p, base, mods, inventory, mut speeds, mut combat)) = world.objects.get_many_mut::<(
        &Player,
        &BaseStats,
        &StatModifiers,
        &Inventory,
        &mut Speeds,
        &mut CombatStats,
    )>(&target)
    {
        p.recalculate_stats(data, base, mods, inventory, &mut speeds, &mut combat);
    }
    crate::game_loop::skills::effects::recompute_max_vitals(world, target);
}

/// Broadcast the transform change: UserInfo to self + CharInfo to nearby (via
/// `broadcast_user_info`), then [`refresh_transform_visuals`].
fn broadcast_transform(world: &World, target: i32) {
    super::party::broadcast_user_info(world, target);
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
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(ave);
            if let Some(pkt) = super::helpers::skill_list_packet(world, target) {
                cs.send(pkt);
            }
        }
    }
}
