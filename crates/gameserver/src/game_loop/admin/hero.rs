//! `AdminAdmin`'s hero commands — the Game panel's "Set Hero" / "Give unclaimed
//! Hero" buttons. `//sethero` toggles hero status on the target (grant/remove
//! the hero skill tree + refresh the aura). `//givehero` is the Olympiad claim
//! path: with no unclaimed-hero list (Olympiad crowning is unported, G25) the
//! target can never claim — Java's own outcome on a server with no crowned hero.

use crate::game_loop::helpers::{send_message, send_sm_bare_to_client};
use crate::game_loop::target;
use crate::model::Player;
use crate::model::components::SkillBook;
use crate::network::server_packets::sm_ids;
use crate::world::World;

/// Target = the current target if it's a player, else the GM (Java falls back to
/// `activeChar`). `None` only when nothing is targeted at all (Java's
/// `getTarget() == null` → INVALID_TARGET).
fn hero_target(world: &World, gm_object_id: i32) -> Option<i32> {
    let target = target::current(world, gm_object_id)?;
    Some(if world.objects.has_component::<Player>(&target) {
        target
    } else {
        gm_object_id
    })
}

/// `//sethero` — toggle the target's hero status.
pub(super) fn admin_sethero(world: &mut World, client_id: u32, gm_object_id: i32) {
    let Some(target) = hero_target(world, gm_object_id) else {
        send_sm_bare_to_client(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let now = world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.is_hero);
    set_hero(world, target, !now);
}

/// `//settruehero` — toggle the target's `isTrueHero()` flag (Java
/// `AdminAdmin`: `target.setTrueHero(!target.isTrueHero()); broadcastUserInfo()`).
/// This is a *different* flag from `//sethero`: it grants no skills and does
/// not touch the hero glow byte — it drives its own `100 : 0` byte at the tail
/// of `CharInfo`/`UserInfo`. Transient, exactly as in Java.
pub(super) fn admin_settruehero(world: &mut World, client_id: u32, gm_object_id: i32) {
    let Some(target) = hero_target(world, gm_object_id) else {
        send_sm_bare_to_client(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let Some(p) = world.objects.get_component_mut::<Player>(&target) else {
        return;
    };
    p.true_hero = !p.true_hero;
    let now = p.true_hero;
    crate::game_loop::player_info::broadcast_user_info(world, target);
    send_message(
        world,
        client_id,
        if now {
            "True hero status is now on."
        } else {
            "True hero status is now off."
        },
    );
}

/// `//givehero` (confirmDlg) — the Olympiad hero-claim path.
pub(super) fn admin_givehero(world: &mut World, client_id: u32, gm_object_id: i32) {
    let Some(target) = hero_target(world, gm_object_id) else {
        send_sm_bare_to_client(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    // Java: `Hero.isHero(objectId)` → already claimed.
    if world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.is_hero)
    {
        send_message(
            world,
            client_id,
            "This player has already claimed the hero status.",
        );
        return;
    }
    // Java: `!Hero.isUnclaimedHero(objectId)` → cannot claim. Anyone off the
    // current Olympiad crown lands here.
    if !world.olympiad.is_unclaimed_hero(target) {
        send_message(
            world,
            client_id,
            "This player cannot claim the hero status.",
        );
        return;
    }
    crate::game_loop::olympiad::claim_hero(world, target);
}

/// Java `Player.setHero`: grant the hero skill tree when turning hero on while
/// on the base class (removed in every other case), set the flag, refresh the
/// hero aura, and resend the skill list + UserInfo.
pub(crate) fn set_hero(world: &mut World, target: i32, hero: bool) {
    let on_base = world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.base_class_id == p.class_id);
    let hero_skills: Vec<(i32, i32)> = world.data.skill_trees.hero_skills().to_vec();
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&target) {
        if hero && on_base {
            for (id, level) in &hero_skills {
                book.0.insert(*id, *level);
            }
        } else {
            for (id, _) in &hero_skills {
                book.0.remove(id);
            }
        }
    }
    // Hero glow = isHero || (isGM && GM_HERO_AURA).
    let gm_aura = world.data.gm.hero_aura;
    let is_gm = world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.is_gm(&world.data));
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.is_hero = hero;
        p.hero_aura = hero || (is_gm && gm_aura);
    }
    super::skills::refresh_skill_list(world, target);
    crate::game_loop::player_info::broadcast_user_info(world, target);
}

/// `//setnoble` — toggle the target's nobless (Java `AdminEditChar`'s
/// `admin_setnoble`, which flips `setNoble(!isNoble())` on the target or self).
pub(super) fn admin_setnoble(world: &mut World, client_id: u32, gm_object_id: i32) {
    // Java reuses the same target resolution as the hero commands.
    let Some(target) = hero_target(world, gm_object_id) else {
        send_sm_bare_to_client(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let now = world
        .objects
        .get_component::<Player>(&target)
        .is_some_and(|p| p.is_noble);
    set_noble(world, target, !now);
    let msg = if now {
        "You are no longer a noblesse."
    } else {
        "You are now a noblesse."
    };
    if let Some(cid) = crate::game_loop::helpers::client_for_player(world, target) {
        send_message(world, cid, msg);
    }
}

/// Java `Player.setNoble`: grant the noble skill tree when turning nobless on
/// (removed otherwise), set the flag, and resend the skill list + UserInfo.
///
/// Unlike `setHero`, Java does **not** gate this on being on the base class —
/// nobless is a property of the character, not of the active class, so a
/// subclass keeps it.
pub(crate) fn set_noble(world: &mut World, target: i32, noble: bool) {
    let noble_skills: Vec<(i32, i32)> = world.data.skill_trees.noble_skills().to_vec();
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&target) {
        if noble {
            for (id, level) in &noble_skills {
                book.0.insert(*id, *level);
            }
        } else {
            for (id, _) in &noble_skills {
                book.0.remove(id);
            }
        }
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&target) {
        p.is_noble = noble;
    }
    super::skills::refresh_skill_list(world, target);
    crate::game_loop::player_info::broadcast_user_info(world, target);
}
