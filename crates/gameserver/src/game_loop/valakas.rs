//! Valakas (`ai/bosses/Valakas`) — the attack-side rules.
//!
//! Valakas uses the **four-state** status ladder rather than the two-state one
//! the simpler bosses share:
//!
//! | status | meaning |
//! |---|---|
//! | 0 `DORMANT` | spawned, nobody has entered; entry unlocked |
//! | 1 `WAITING` | someone entered, 30-minute window for others; entry unlocked |
//! | 2 `FIGHTING` | engaged; entry **locked** |
//! | 3 `DEAD` | killed; entry locked |
//!
//! Only the `onAttack` half is ported here — the lair's entry/teleport flow and
//! the 30-minute window are their own slice.

use crate::model::components::Position;
use crate::world::World;

pub const VALAKAS: i32 = 29028;

/// `getZoneById(12010)` — "Valakas Boss", a `ScriptZone`.
const BOSS_ZONE_ID: i32 = 12010;

/// `ATTACKER_REMOVE` — where a player attacking outside the fight is dumped.
const ATTACKER_REMOVE: (i32, i32, i32) = (150_037, -57_255, -2_976);

pub const DORMANT: i32 = 0;
pub const WAITING: i32 = 1;
pub const FIGHTING: i32 = 2;
pub const DEAD: i32 = 3;

/// Strider riders are debuffed on sight (skill 4258), once.
const STRIDER_DEBUFF: i32 = 4258;
/// Java `MountType.STRIDER`.
const MOUNT_STRIDER: u8 = 1;

/// What `Valakas.onAttack` decided to do about an attacker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackVerdict {
    /// Attacking from **outside the lair** — Java calls `attacker.doDie()`.
    /// A hard anti-exploit: you cannot plink at Valakas from safety.
    KilledForAttackingFromOutside,
    /// Attacking before the fight has started — bounced to `ATTACKER_REMOVE`.
    RemovedNotFighting,
    /// A normal hit.
    Allowed,
}

/// `Valakas.onAttack`, minus the timer bookkeeping.
///
/// The order is Java's and is load-bearing: the **zone check comes first**, so
/// an out-of-zone attacker dies whatever the boss's status — including while
/// Valakas is dead, when the status check would otherwise have merely teleported
/// them.
pub(crate) fn on_valakas_attacked(world: &mut World, valakas_oid: i32, attacker_oid: i32) -> AttackVerdict {
    if world.objects.get_component::<crate::model::Player>(&attacker_oid).is_none() {
        return AttackVerdict::Allowed;
    }

    if !attacker_in_lair(world, attacker_oid) {
        // `attacker.doDie(attacker)` — self-inflicted, so it carries no PvP or
        // karma consequence for anyone.
        crate::game_loop::death::player_do_die(world, attacker_oid, attacker_oid);
        return AttackVerdict::KilledForAttackingFromOutside;
    }

    if crate::game_loop::grand_boss::status(world, VALAKAS) != Some(FIGHTING) {
        let (x, y, z) = ATTACKER_REMOVE;
        crate::game_loop::death::teleport_player(world, attacker_oid, x, y, z);
        return AttackVerdict::RemovedNotFighting;
    }

    // A strider-mounted attacker is debuffed, once — Java checks
    // `!isAffectedBySkill(4258)` so it isn't recast every swing.
    let on_strider = world
        .objects
        .get_component::<crate::model::Player>(&attacker_oid)
        .is_some_and(|p| p.mount_type == MOUNT_STRIDER);
    if on_strider && !already_debuffed(world, attacker_oid) {
        cast_debuff(world, valakas_oid, attacker_oid);
    }

    AttackVerdict::Allowed
}

fn attacker_in_lair(world: &World, attacker_oid: i32) -> bool {
    let Some(pos) = world.objects.get_component::<Position>(&attacker_oid) else { return false };
    world.data.zone_data.by_id(BOSS_ZONE_ID).is_some_and(|z| z.contains(pos.x, pos.y, pos.z))
}

fn already_debuffed(world: &World, oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Buffs>(&oid)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == STRIDER_DEBUFF))
}

fn cast_debuff(world: &mut World, caster_oid: i32, target_oid: i32) {
    let Some(skill) = world.data.skill_data.get(STRIDER_DEBUFF, 1).cloned() else { return };
    crate::game_loop::skills::effects::apply_continuous_effects(world, caster_oid, target_oid, &skill, None);
}

// ---------------------------------------------------------------------------
// The entry cinematic
// ---------------------------------------------------------------------------

/// `VALAKAS_LAIR` — where Valakas is teleported before the cinematic runs.
const VALAKAS_LAIR: (i32, i32, i32) = (212_852, -114_842, -1_632);

const TICKS_PER_SECOND: u64 = 10;

/// The ten cinematic beats: `(delay_ms_from_start, camera args)`.
///
/// Transcribed literally from Java's `startQuestTimer("spawn_N", …)` chain and
/// its `SpecialCamera` calls, in the same argument order the packet takes —
/// including `range`, which the wire drops. Keeping the tables aligned with the
/// source is the whole reason the packet kept that parameter.
///
/// The final beat carries no camera: it flips the status to `FIGHTING`, which
/// is what actually starts the fight and locks entry.
const CINEMATIC: [(u64, Option<[i32; 11]>); 10] = [
    (1_700, Some([1800, 180, -1, 1500, 15000, 10000, 0, 0, 1, 0, 0])),
    (3_200, Some([1300, 180, -5, 3000, 15000, 10000, 0, -5, 1, 0, 0])),
    (6_500, Some([500, 180, -8, 600, 15000, 10000, 0, 60, 1, 0, 0])),
    (9_400, Some([800, 180, -8, 2700, 15000, 10000, 0, 30, 1, 0, 0])),
    (12_100, Some([200, 250, 70, 0, 15000, 10000, 30, 80, 1, 0, 0])),
    (12_430, Some([1100, 250, 70, 2500, 15000, 10000, 30, 80, 1, 0, 0])),
    (15_430, Some([700, 150, 30, 0, 15000, 10000, -10, 60, 1, 0, 0])),
    (16_830, Some([1200, 150, 20, 2900, 15000, 10000, -10, 30, 1, 0, 0])),
    (23_530, Some([750, 170, -10, 3400, 15000, 4000, 10, -15, 1, 0, 0])),
    (26_000, None), // status → FIGHTING
];

/// `"beginning"` — teleport Valakas to his lair and arm the cinematic.
///
/// Every beat is scheduled up front from the **start** of the sequence, exactly
/// as Java does, rather than each step chaining the next. That matters: the
/// beats are not evenly spaced (330 ms between steps 5 and 6, 6.7 s between 8
/// and 9), and a chain of relative delays would be far easier to get subtly
/// wrong.
pub(crate) fn begin_cinematic(world: &mut World, valakas_oid: i32) {
    if let Some(p) = world.objects.get_component_mut::<Position>(&valakas_oid) {
        p.x = VALAKAS_LAIR.0;
        p.y = VALAKAS_LAIR.1;
        p.z = VALAKAS_LAIR.2;
    }
    for (i, (delay_ms, _)) in CINEMATIC.iter().enumerate() {
        world.scheduler.schedule(
            world.tick + (delay_ms / 1000 * TICKS_PER_SECOND).max(1),
            crate::scheduler::ScheduledTask::ValakasCinematic { valakas_oid, step: i as u8 },
        );
    }
}

/// One cinematic beat.
pub(crate) fn handle_cinematic_step(world: &mut World, valakas_oid: i32, step: u8) {
    let Some((_, camera)) = CINEMATIC.get(step as usize).copied() else { return };
    match camera {
        Some(a) => {
            let pkt = crate::network::server_packets::special_camera(
                valakas_oid, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10],
            );
            broadcast_to_lair(world, &pkt);
        }
        None => {
            // The last beat: the fight is on, and entry locks behind it.
            if let Some(b) = world.grand_bosses.get_mut(&VALAKAS) {
                b.status = FIGHTING;
            }
        }
    }
}

/// `BOSS_ZONE.broadcastPacket` — the cinematic plays for everyone **in the
/// lair**, not everyone nearby: a player outside the zone sees nothing, which
/// is the point of running it on the zone rather than the boss's region.
fn broadcast_to_lair(world: &World, pkt: &[u8]) {
    let Some(zone) = world.data.zone_data.by_id(BOSS_ZONE_ID) else { return };
    for cs in world.clients.values() {
        if let crate::session::ClientSession::InGame(s) = cs {
            let oid = s.player_object_id();
            let inside = world
                .objects
                .get_component::<Position>(&oid)
                .is_some_and(|p| zone.contains(p.x, p.y, p.z));
            if inside {
                cs.send(pkt.to_vec());
            }
        }
    }
}
