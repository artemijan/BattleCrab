//! `Player._spawnProtectEndTime` and `Player.onActionRequest` — the grace
//! period a character gets against aggressive monsters on entering the world.
//!
//! **This is not invulnerability.** Java's `isSpawnProtected()` has exactly
//! four readers, and only two of them matter here:
//!
//! * `Attackable.getHating` drops a protected player out of the aggro list, so
//!   monsters neither aggro nor keep hating them;
//! * `Summon.isInvul()` is `super.isInvul() || _owner.isSpawnProtected()`, so
//!   the *pet* really is invulnerable while its owner is protected — an
//!   asymmetry with the owner, who is not.
//!
//! The window is `Character.ini`'s `PlayerSpawnProtection` (600 s here), but it
//! ends at the player's first deliberate action, which is what
//! `Player.onActionRequest` does. Java calls that from five client packets —
//! `Action`, `AttackRequest`, `MoveBackwardToLocation`, `RequestMagicSkillUse`
//! and `UseItem` — so 600 is a ceiling on an AFK login, not ten minutes of
//! safety. (`AutoPlayTaskManager` is Java's sixth caller and is post-Interlude.)
//!
//! The sibling key `PlayerTeleportProtection` is a different rule despite the
//! matching name: it *is* real invulnerability (`Player.isInvul()` ORs it in).
//! It ships at **0**, so it never arms, and the port parses it without wiring
//! the invulnerability — see the field doc in
//! [`crate::config::character::CharacterConfig::player_teleport_protection`].

use crate::game_loop::helpers::send_sm_bare_to_client;
use crate::model::Player;
use crate::network::server_packets::sm_ids;
use crate::world::World;

/// `EnterWorld`: `if (Config.PLAYER_SPAWN_PROTECTION > 0) setSpawnProtection(true)`.
pub(crate) fn arm(world: &mut World, player_oid: i32) {
    let secs = world.cfg.character.player_spawn_protection;
    if secs <= 0 {
        return;
    }
    let until = world.tick + secs as u64 * super::time::TICKS_PER_SECOND;
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.spawn_protect_end_tick = until;
    }
}

/// `Player.isSpawnProtected()` — set, and not yet expired.
pub(crate) fn is_protected(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.spawn_protect_end_tick > world.tick)
}

/// `Player.onActionRequest()` — the first deliberate action ends the window.
///
/// Java's servitor/pet `RESTORE_*_ON_RECONNECT` restores hang off this same
/// method; both are `False` on this dist, so nothing else belongs here.
pub(crate) fn on_action_request(world: &mut World, client_id: u32, player_oid: i32) {
    if !is_protected(world, player_oid) {
        return;
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.spawn_protect_end_tick = 0;
    }
    // `if (!isInsideZone(ZoneId.PEACE))` — no point telling someone standing
    // in town that the monsters can see them again.
    let in_peace = world
        .objects
        .get_component::<crate::model::components::ZoneFlags>(&player_oid)
        .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Peace));
    if !in_peace {
        send_sm_bare_to_client(
            world,
            client_id,
            sm_ids::YOU_ARE_NO_LONGER_PROTECTED_FROM_AGGRESSIVE_MONSTERS,
        );
    }
}
