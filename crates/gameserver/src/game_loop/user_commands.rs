//! `BypassUserCmd` (0xB3) — the client `/command` bar — routed to ports of
//! `handlers/usercommandhandlers/*` (G15.5). Handled: `/loc` (id 0) and
//! `/unstuck` (id 52). Unknown ids answer the Java GM-only "not implemented"
//! message. The rest of the family (`/time` needs the game clock, `/mount`
//! needs rideable pets, `/partyinfo`'s loot strings, …) lands with its
//! blocking system.

use crate::model::components::{Casting, Position, Vitals};
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::session::ClientSession;
use crate::world::World;

const USER_CMD_LOC: i32 = 0;
const USER_CMD_UNSTUCK: i32 = 52;

/// The 5-minute escape (`SkillData.getSkill(2099, 1)`) and the GM 1-second
/// escape (2100) — both `SELF`-targeted static skills with the
/// `Escape TOWN` effect.
const ESCAPE_SKILL_ID: i32 = 2099;
const GM_ESCAPE_SKILL_ID: i32 = 2100;

pub(crate) fn handle_bypass_user_cmd(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(command_id) = cp::read_user_command(body) else { return };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let object_id = session.player_object_id();

    match command_id {
        USER_CMD_LOC => loc(world, client_id, object_id),
        USER_CMD_UNSTUCK => unstuck(world, client_id, object_id),
        _ => {
            // `BypassUserCmd.runImpl`'s missing-handler branch: GMs get told,
            // players get silence.
            let is_gm = world
                .objects
                .get_component::<crate::model::Player>(&object_id)
                .is_some_and(|p| p.is_gm(&world.data));
            if is_gm {
                super::admin::send_message(
                    world,
                    client_id,
                    &format!("User commandID {command_id} not implemented yet."),
                );
            }
        }
    }
}

/// Port of `usercommandhandlers/Loc.java`: the map region's `locId` is itself
/// the "Current location: $s1 / $s2 / $s3 (near …)" system-message id.
/// Simplifications: no `RespawnZone` redirect (zone type unported), and the
/// three coordinate params are always attached — every `locId` this dist's
/// mapregion files reference is a 3-param location message (Java checks
/// `getParamCount() == 3` against its message table, which isn't ported).
fn loc(world: &World, client_id: u32, object_id: i32) {
    let Some(pos) = world.objects.get_component::<Position>(&object_id) else { return };
    let loc_id = world.data.map_region.region_at(pos.x, pos.y).map(|r| r.loc_id).unwrap_or(0);
    let packet = if loc_id > 0 {
        server_packets::system_message_with(
            loc_id as i16,
            &[SmParam::Int(pos.x), SmParam::Int(pos.y), SmParam::Int(pos.z)],
        )
    } else {
        server_packets::system_message_with(
            sm_ids::CURRENT_LOCATION_S1,
            &[SmParam::Text(format!("{}, {}, {}", pos.x, pos.y, pos.z))],
        )
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(packet);
    }
}

/// Port of `usercommandhandlers/Unstuck.java`: cast the escape skill and let
/// its `Escape TOWN` effect do the teleport on landing. Skipped guards
/// (systems don't exist): jail, faction, olympiad, mute, observer, combat
/// flag, `isMovementDisabled` beyond death.
fn unstuck(world: &mut World, client_id: u32, object_id: i32) {
    // `isCastingNow || isAlikeDead` → refuse silently (Java returns false).
    if world.objects.has_component::<Casting>(&object_id)
        || world.objects.get_component::<Vitals>(&object_id).is_none_or(|v| v.dead)
    {
        return;
    }
    let (is_gm, interval) = {
        let Some(p) = world.objects.get_component::<crate::model::Player>(&object_id) else {
            return;
        };
        (p.is_gm(&world.data), world.cfg.character.unstuck_interval)
    };

    if is_gm {
        // GM: the stock 1-second escape.
        if let Some(skill) = world.data.skill_data.get(GM_ESCAPE_SKILL_ID, 1).cloned() {
            super::skills::cast::start_casting(world, client_id, object_id, &skill, object_id);
        } else {
            super::admin::send_message(world, client_id, "You use Escape: 1 second.");
        }
        return;
    }
    let Some(mut skill) = world.data.skill_data.get(ESCAPE_SKILL_ID, 1).cloned() else {
        return;
    };
    if interval == 300 {
        // The stock 5-minute skill matches the config — cast unmodified.
        super::skills::cast::start_casting(world, client_id, object_id, &skill, object_id);
        return;
    }
    // Custom interval (30 s on this dist): Java forces the cast time via
    // `SkillCaster.castSkill(..., unstuckTimer)`. The skill is static
    // (`isMagic=2`), so the overridden hitTime is used verbatim.
    let unstuck_ms = interval * 1000;
    skill.hit_time = unstuck_ms;
    if interval > 100 {
        super::admin::send_message(
            world,
            client_id,
            &format!("You use Escape: {} minutes.", unstuck_ms / 60000),
        );
    } else {
        super::admin::send_message(
            world,
            client_id,
            &format!("You use Escape: {} seconds.", unstuck_ms / 1000),
        );
    }
    super::skills::cast::start_casting(world, client_id, object_id, &skill, object_id);
}
