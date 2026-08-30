//! Four Sepulchers (`ai/areas/ImperialTomb/FourSepulchers`) — the Imperial
//! Tomb party dungeon: a leader with a 4+ party, Entrance Passes and the
//! Four Goblets quest enters one of four sepulchers for a 60-minute time
//! attack; waves spawn behind mysterious chests, key chests open the next
//! chapel gate, and the hall boss pays each member a sepulcher goblet.
//!
//! World state is [`FsState`] on [`World`], with each hall's last entry time
//! persisted to `global_variables` exactly as Java keys it
//! (`"FourSepulchers" + managerNpcId`) and rehydrated at boot, so the
//! 60-minute re-entry gate survives a restart.

use crate::game_loop::helpers::maybe_position;
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::helpers::player_name_or_empty;
use crate::model::components::{Position, RegionCell, Vitals};
use crate::scheduler::ScheduledTask;
use crate::world::World;

pub(crate) const CONQUEROR_MANAGER: i32 = 31921;
pub(crate) const EMPEROR_MANAGER: i32 = 31922;
pub(crate) const GREAT_SAGES_MANAGER: i32 = 31923;
pub(crate) const JUDGE_MANAGER: i32 = 31924;
pub(crate) const MYSTERIOUS_CHEST: i32 = 31468;
pub(crate) const KEY_CHEST: i32 = 31467;
pub(crate) const TELEPORTER: i32 = 31452;

pub(crate) const ENTRANCE_PASS: i32 = 7075;
pub(crate) const USED_PASS: i32 = 7261;
pub(crate) const CHAPEL_KEY: i32 = 7260;
pub(crate) const ANTIQUE_BROOCH: i32 = 7262;

/// Goblet item = `7255 + sepulcherId` (7256–7259).
const GOBLET_BASE: i32 = 7255;

pub(crate) const PARTY_MEMBER_COUNT: usize = 4;
/// `ENTRY_DELAY` (3 min) until the first mysterious chest appears.
const ENTRY_DELAY_TICKS: u64 = 3 * 60 * 10;
/// `TIME_ATTACK` (60 min): the run length and the re-entry cooldown.
const TIME_ATTACK_MINUTES: i64 = 60;
const TIME_ATTACK_TICKS: u64 = (TIME_ATTACK_MINUTES as u64) * 60 * 10;

/// The four halls' script zones (`royal_rush_script_*`).
const ZONES: [i32; 4] = [200221, 200222, 200224, 200223]; // conqueror, emperor, sages, judge

pub(crate) fn manager_zone(manager_id: i32) -> Option<i32> {
    Some(match manager_id {
        CONQUEROR_MANAGER => ZONES[0],
        EMPEROR_MANAGER => ZONES[1],
        GREAT_SAGES_MANAGER => ZONES[2],
        JUDGE_MANAGER => ZONES[3],
        _ => return None,
    })
}

fn start_hall(manager_id: i32) -> (i32, i32, i32) {
    match manager_id {
        CONQUEROR_MANAGER => (181632, -85587, -7218),
        EMPEROR_MANAGER => (179963, -88978, -7218),
        GREAT_SAGES_MANAGER => (173217, -86132, -7218),
        _ => (175608, -82296, -7218),
    }
}

/// `DOORS` — (sepulcher, wave, doorId). Wave 7 is the boss hall gate.
const DOORS: [(i32, i32, i32); 20] = [
    (1, 2, 25150012),
    (1, 3, 25150013),
    (1, 4, 25150014),
    (1, 5, 25150015),
    (1, 7, 25150016),
    (2, 2, 25150002),
    (2, 3, 25150003),
    (2, 4, 25150004),
    (2, 5, 25150005),
    (2, 7, 25150006),
    (3, 2, 25150032),
    (3, 3, 25150033),
    (3, 4, 25150034),
    (3, 5, 25150035),
    (3, 7, 25150036),
    (4, 2, 25150022),
    (4, 3, 25150023),
    (4, 4, 25150024),
    (4, 5, 25150025),
    (4, 7, 25150026),
];

/// `CHEST_SPAWN_LOCATIONS` — (sepulcher, room, x, y, z, heading).
const CHEST_SPAWNS: [(i32, i32, i32, i32, i32, i32); 20] = [
    (1, 1, 182074, -85579, -7216, 32768),
    (1, 2, 183868, -85577, -7216, 32768),
    (1, 3, 185681, -85573, -7216, 32768),
    (1, 4, 187498, -85566, -7216, 32768),
    (1, 5, 189306, -85571, -7216, 32768),
    (2, 1, 180375, -88968, -7216, 32768),
    (2, 2, 182151, -88962, -7216, 32768),
    (2, 3, 183960, -88964, -7216, 32768),
    (2, 4, 185792, -88966, -7216, 32768),
    (2, 5, 187625, -88953, -7216, 32768),
    (3, 1, 173218, -85703, -7216, 49152),
    (3, 2, 173206, -83929, -7216, 49152),
    (3, 3, 173208, -82085, -7216, 49152),
    (3, 4, 173191, -80290, -7216, 49152),
    (3, 5, 173198, -78465, -7216, 49152),
    (4, 1, 175601, -81905, -7216, 49152),
    (4, 2, 175619, -80094, -7216, 49152),
    (4, 3, 175608, -78268, -7216, 49152),
    (4, 4, 175588, -76472, -7216, 49152),
    (4, 5, 175594, -74655, -7216, 49152),
];

/// Per-run state for the four halls (index = sepulcher - 1).
#[derive(Debug, Default)]
pub struct FsState {
    /// Last entry unix-millis per hall — the 60-minute re-entry gate
    /// (Java `GlobalVariablesManager "FourSepulchers<npcId>"`).
    pub last_entry_ms: [i64; 4],
    /// The current wave (Java `STORED_PROGRESS`), 1-based.
    pub progress: [i32; 4],
    /// The tracked wave spawns (`STORED_MONSTER_SPAWNS`) for the
    /// defeat-check waves (2 and 5).
    pub wave_spawns: [Vec<i32>; 4],
}

impl FsState {
    fn idx(sepulcher: i32) -> usize {
        (sepulcher.clamp(1, 4) - 1) as usize
    }
}

/// The manager NPC whose key holds a hall's entry stamp — Java keys the global
/// variable by *manager id*, not by hall index, so the mapping is part of the
/// storage format rather than an internal detail.
fn manager_npc_id_of(sepulcher: i32) -> i32 {
    match sepulcher {
        1 => CONQUEROR_MANAGER,
        2 => EMPEROR_MANAGER,
        3 => GREAT_SAGES_MANAGER,
        _ => JUDGE_MANAGER,
    }
}

/// Reload the per-hall entry stamps from `global_variables` at boot.
///
/// Without this the 60-minute re-entry gate reset on every restart. Java has no
/// explicit restore step because its check reads the variable directly; this
/// port keeps the stamps in [`FsState`] for the rest of the run, so they are
/// hydrated once here.
pub(crate) fn restore_entry_times(world: &mut World) {
    for sepulcher in 1..=4 {
        let key = super::global_vars::four_sepulchers_key(manager_npc_id_of(sepulcher));
        let stamp = super::global_vars::get_i64(world, &key, 0);
        world.four_sepulchers.last_entry_ms[FsState::idx(sepulcher)] = stamp;
    }
}

/// `getSepulcherId(player)` — which hall zone the player stands in, 0 = none.
pub(crate) fn sepulcher_of(world: &World, player: i32) -> i32 {
    let Some(pos) = world.objects.get_component::<Position>(&player) else {
        return 0;
    };
    for (i, &zone_id) in ZONES.iter().enumerate() {
        let inside = world
            .data
            .zone_data
            .zones
            .iter()
            .any(|z| z.id == zone_id && z.contains(pos.x, pos.y, pos.z));
        if inside {
            return (i + 1) as i32;
        }
    }
    0
}

fn any_player_inside(world: &mut World, zone_id: i32) -> bool {
    let mut found = false;
    let World { objects, data, .. } = world;
    objects.for_each_mut::<(&crate::model::Player, &Position)>(|(_, pos)| {
        if !found
            && data
                .zone_data
                .zones
                .iter()
                .any(|z| z.id == zone_id && z.contains(pos.x, pos.y, pos.z))
        {
            found = true;
        }
    });
    found
}

/// The outcome of `tryEnter`, so the chat window can answer (`<id>-XX.html`).
pub(crate) enum EnterOutcome {
    /// Hall occupied.
    Full,
    /// No party / too small.
    SmallParty,
    /// Not the leader.
    NotLeader,
    /// A member is missing the Four Goblets quest (name attached).
    NoQuest(String),
    /// A member has no Entrance Pass (name attached).
    NoPass(String),
    /// The hall's 60-minute window is still running.
    NotTime,
    /// In — the run started.
    Ok,
}

/// `tryEnter` — the whole admission ritual.
pub(crate) fn try_enter(world: &mut World, manager_oid: i32, player: i32) -> EnterOutcome {
    let manager_id = npc_id_of(world, manager_oid).unwrap_or(0);
    let Some(zone_id) = manager_zone(manager_id) else {
        return EnterOutcome::Full;
    };
    if any_player_inside(world, zone_id) {
        return EnterOutcome::Full;
    }
    let party = world
        .objects
        .get_component::<crate::model::components::PartyRef>(&player)
        .map(|r| r.0)
        .and_then(|pid| world.parties.get(&pid));
    let Some(party) = party else {
        return EnterOutcome::SmallParty;
    };
    if party.members.len() < PARTY_MEMBER_COUNT {
        return EnterOutcome::SmallParty;
    }
    if party.members.first() != Some(&player) {
        return EnterOutcome::NotLeader;
    }
    let members = party.members.clone();
    for &mem in &members {
        if !quest_started_or_completed(world, mem, "Q00620_FourGoblets") {
            return EnterOutcome::NoQuest(player_name_or_empty(world, mem));
        }
        if item_count(world, mem, ENTRANCE_PASS) < 1 {
            return EnterOutcome::NoPass(player_name_or_empty(world, mem));
        }
    }
    let sepulcher = manager_sepulcher(manager_id);
    let now = commons::util::now_millis();
    let ready_at =
        world.four_sepulchers.last_entry_ms[FsState::idx(sepulcher)] + TIME_ATTACK_MINUTES * 60_000;
    if ready_at > now {
        return EnterOutcome::NotTime;
    }

    // Sweep leftovers from the previous run: monsters and the service NPCs.
    clear_hall(world, zone_id, false);
    // Room-4 trap zones off until wave 4 turns them back on.
    set_room4_effect_zones(world, sepulcher, false);
    for (s, _, door) in DOORS {
        if s == sepulcher {
            crate::game_loop::doors::set_door_by_id(world, door, false);
        }
    }

    // Teleport the nearby members in, collect the toll.
    let leader_pos = maybe_position(world, player);
    let (hx, hy, hz) = start_hall(manager_id);
    for &mem in &members {
        let near = match (leader_pos, maybe_position(world, mem)) {
            (Some(a), Some(b)) => a.distance_2d(&b) <= 700.0,
            _ => false,
        };
        if !near {
            continue;
        }
        crate::game_loop::death::teleport_player(world, mem, hx, hy, hz);
        take_item(world, mem, ENTRANCE_PASS, 1);
        take_item(world, mem, CHAPEL_KEY, -1);
        if item_count(world, mem, ANTIQUE_BROOCH) < 1 {
            give_item(world, mem, USED_PASS, 1);
        }
    }

    let idx = FsState::idx(sepulcher);
    world.four_sepulchers.last_entry_ms[idx] = now;
    // Java `vars.set("FourSepulchers" + npcId, currentTimeMillis())` — the
    // 60-minute re-entry gate has to survive a restart, or a reboot hands
    // everyone a free re-entry.
    super::global_vars::set(
        world,
        &super::global_vars::four_sepulchers_key(manager_npc_id_of(sepulcher)),
        now,
    );
    world.four_sepulchers.progress[idx] = 1;
    world.four_sepulchers.wave_spawns[idx].clear();
    world.scheduler.schedule(
        world.tick + ENTRY_DELAY_TICKS,
        ScheduledTask::FsMysteriousChest { sepulcher },
    );
    world.scheduler.schedule(
        world.tick + TIME_ATTACK_TICKS,
        ScheduledTask::FsOust { sepulcher },
    );
    EnterOutcome::Ok
}

pub(crate) fn manager_sepulcher(manager_id: i32) -> i32 {
    match manager_id {
        CONQUEROR_MANAGER => 1,
        EMPEROR_MANAGER => 2,
        GREAT_SAGES_MANAGER => 3,
        JUDGE_MANAGER => 4,
        _ => 0,
    }
}

/// `spawnMysteriousChest` — the chest that starts the current room's wave.
pub(crate) fn handle_mysterious_chest(world: &mut World, sepulcher: i32) {
    let wave = world.four_sepulchers.progress[FsState::idx(sepulcher)];
    for (s, room, x, y, z, h) in CHEST_SPAWNS {
        if s == sepulcher && room == wave {
            crate::game_loop::npc::spawn_npc_at(world, MYSTERIOUS_CHEST, x, y, z, h);
            break;
        }
    }
}

/// `spawnNextWave` — pour the current wave out of the spawn table.
pub(crate) fn spawn_next_wave(world: &mut World, sepulcher: i32) {
    let idx = FsState::idx(sepulcher);
    let wave = world.four_sepulchers.progress[idx];
    let rows: Vec<_> = world
        .data
        .four_sepulchers
        .spawns
        .iter()
        .filter(|r| r.sepulcher == sepulcher && r.wave == wave)
        .copied()
        .collect();
    let mut spawned = Vec::new();
    for r in rows {
        if let Some(oid) =
            crate::game_loop::npc::spawn_npc_at(world, r.npc_id, r.x, r.y, r.z, r.heading)
        {
            spawned.push(oid);
        }
    }
    // Wave 4 re-arms the room's trap zones.
    if wave == 4 {
        set_room4_effect_zones(world, sepulcher, true);
    }
    // Waves 2 and 5 are "clear the room" waves — track them for the check.
    if wave == 2 || wave == 5 {
        world.four_sepulchers.wave_spawns[idx] = spawned;
        world
            .scheduler
            .schedule(world.tick + 50, ScheduledTask::FsWaveCheck { sepulcher });
    } else {
        world.four_sepulchers.wave_spawns[idx].clear();
    }
}

/// `WAVE_DEFEATED_CHECK` — all tracked spawns dead? Wave < 5 pays a key
/// chest at the last corpse; wave 5 rolls straight into wave 6.
pub(crate) fn handle_wave_check(world: &mut World, sepulcher: i32) {
    let idx = FsState::idx(sepulcher);
    let spawns = world.four_sepulchers.wave_spawns[idx].clone();
    if spawns.is_empty() {
        return;
    }
    let mut last_pos = None;
    let mut all_dead = true;
    for oid in &spawns {
        match world.objects.get_component::<Vitals>(oid) {
            Some(v) if !v.dead => {
                all_dead = false;
                break;
            }
            _ => {
                if let Some(p) = world.objects.get_component::<Position>(oid) {
                    last_pos = Some((p.x, p.y, p.z));
                }
            }
        }
    }
    if !all_dead {
        world
            .scheduler
            .schedule(world.tick + 50, ScheduledTask::FsWaveCheck { sepulcher });
        return;
    }
    world.four_sepulchers.wave_spawns[idx].clear();
    let wave = world.four_sepulchers.progress[idx];
    if wave < 5 {
        let (x, y, z) = last_pos.unwrap_or_else(|| {
            let (hx, hy, hz) = start_hall(sepulcher_manager(sepulcher));
            (hx, hy, hz)
        });
        crate::game_loop::npc::spawn_npc_at(world, KEY_CHEST, x, y, z, 0);
    } else if wave == 5 {
        world.four_sepulchers.progress[idx] = wave + 1;
        spawn_next_wave(world, sepulcher);
    }
}

pub(crate) fn sepulcher_manager(sepulcher: i32) -> i32 {
    match sepulcher {
        1 => CONQUEROR_MANAGER,
        2 => EMPEROR_MANAGER,
        3 => GREAT_SAGES_MANAGER,
        _ => JUDGE_MANAGER,
    }
}

/// The chapel gatekeeper accepted a key: advance the wave, open the gate for
/// 15 s, and stage what the next room needs.
pub(crate) fn open_gate(world: &mut World, sepulcher: i32) {
    let idx = FsState::idx(sepulcher);
    let wave = world.four_sepulchers.progress[idx] + 1;
    world.four_sepulchers.progress[idx] = wave;
    for (s, w, door) in DOORS {
        if s == sepulcher && w == wave {
            crate::game_loop::doors::open_door_timed(world, door, 150);
            break;
        }
    }
    if wave < 7 {
        handle_mysterious_chest(world, sepulcher);
    } else {
        spawn_next_wave(world, sepulcher);
    }
}

/// A hall boss fell: goblets for the party, the exit teleporter, and the
/// (vestigial) progress bump.
pub(crate) fn on_boss_killed(world: &mut World, boss_oid: i32, killer: i32) {
    let sepulcher = sepulcher_of(world, killer);
    if sepulcher == 0 {
        return;
    }
    let idx = FsState::idx(sepulcher);
    world.four_sepulchers.progress[idx] += 1;

    let members: Vec<i32> =
        crate::game_loop::party::party_members(world, killer).unwrap_or_default();
    let killer_pos = maybe_position(world, killer);
    for mem in members {
        let near = match (killer_pos, maybe_position(world, mem)) {
            (Some(a), Some(b)) => a.distance_2d(&b) <= 1500.0,
            _ => false,
        };
        // Java quirk ported as-is: the quest gate re-checks the *killer's*
        // quest state for every member.
        if near && quest_started_or_completed(world, killer, "Q00620_FourGoblets") {
            give_item(world, mem, GOBLET_BASE + sepulcher, 1);
        }
    }
    spawn_next_wave(world, sepulcher);
    if let Some(p) = maybe_position(world, boss_oid) {
        crate::game_loop::npc::spawn_npc_at(world, TELEPORTER, p.x, p.y, p.z, 0);
    }
}

/// The 60-minute bell: everyone still inside is walked out (Java
/// `oustAllPlayers`), and the hall is swept clean.
pub(crate) fn handle_oust(world: &mut World, sepulcher: i32) {
    let zone_id = ZONES[FsState::idx(sepulcher)];
    let mut inside: Vec<i32> = Vec::new();
    {
        let World { objects, data, .. } = world;
        objects.for_each_mut::<(&crate::model::Player, &Position)>(|(p, pos)| {
            if data
                .zone_data
                .zones
                .iter()
                .any(|z| z.id == zone_id && z.contains(pos.x, pos.y, pos.z))
            {
                inside.push(p.object_id);
            }
        });
    }
    let (hx, hy, hz) = manager_exit(sepulcher);
    for player in inside {
        crate::game_loop::death::teleport_player(world, player, hx, hy, hz);
    }
    clear_hall(world, zone_id, true);
}

/// Players ousted at time-up land back at their hall's manager.
fn manager_exit(sepulcher: i32) -> (i32, i32, i32) {
    match sepulcher {
        1 => (181589, -87910, -7200),
        2 => (177826, -88917, -7216),
        3 => (173195, -88257, -7200),
        _ => (175591, -87021, -7200),
    }
}

/// Despawn a hall's run content: monsters, plus the service NPCs
/// (mysterious/key chests and the exit teleporter).
fn clear_hall(world: &mut World, zone_id: i32, _include_players: bool) {
    let mut goners: Vec<(i32, (i32, i32))> = Vec::new();
    {
        let World { objects, data, .. } = world;
        objects.for_each_mut::<(&crate::model::npc::Npc, &Position, &RegionCell)>(|(n, pos, r)| {
            let inside = data
                .zone_data
                .zones
                .iter()
                .any(|z| z.id == zone_id && z.contains(pos.x, pos.y, pos.z));
            if !inside {
                return;
            }
            let is_service =
                n.npc_id == MYSTERIOUS_CHEST || n.npc_id == KEY_CHEST || n.npc_id == TELEPORTER;
            let is_monster = data
                .npc_data
                .get(n.npc_id)
                .is_some_and(|t| t.is_monster() || t.type_name == "RaidBoss");
            if is_service || is_monster {
                goners.push((n.object_id, r.0));
            }
        });
    }
    for (oid, region) in goners {
        crate::game_loop::death::despawn_npc(world, oid, region);
    }
}

/// Toggle the room-4 trap `EffectZone`s around the room's chest anchor.
fn set_room4_effect_zones(world: &mut World, sepulcher: i32, enabled: bool) {
    let Some(&(_, _, x, y, z, _)) = CHEST_SPAWNS
        .iter()
        .find(|(s, room, ..)| *s == sepulcher && *room == 4)
    else {
        return;
    };
    for zone in world.data.zone_data.zones.iter_mut() {
        if zone.effect.is_some()
            && zone.contains(x, y, z)
            && let Some(e) = zone.effect.as_mut()
        {
            e.enabled = enabled;
        }
    }
}

/// A room-4 charm was destroyed: kill its trap zone (matched by the charm's
/// zone skill) wherever the killer stands.
pub(crate) fn disable_charm_zone(world: &mut World, killer: i32, charm_skill: i32) {
    let Some(pos) = maybe_position(world, killer) else {
        return;
    };
    for zone in world.data.zone_data.zones.iter_mut() {
        if !zone.contains(pos.x, pos.y, pos.z) {
            continue;
        }
        if let Some(e) = zone.effect.as_mut()
            && e.skills.iter().any(|&(id, _)| id == charm_skill)
        {
            e.enabled = false;
            break;
        }
    }
}

fn item_count(world: &World, player: i32, item_id: i32) -> i64 {
    world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&player)
        .map(|inv| inv.count_of(item_id))
        .unwrap_or(0)
}

fn take_item(world: &mut World, player: i32, item_id: i32, count: i64) {
    let count = if count < 0 {
        item_count(world, player, item_id)
    } else {
        count
    };
    if count <= 0 {
        return;
    }
    if let Some(client_id) = crate::game_loop::helpers::client_for_player(world, player) {
        crate::game_loop::quests::take_items(world, client_id, player, item_id, count);
    }
}

fn give_item(world: &mut World, player: i32, item_id: i32, count: i64) {
    if let Some(client_id) = crate::game_loop::helpers::client_for_player(world, player) {
        crate::game_loop::quests::give_item_with_earned_message(
            world, client_id, player, item_id, count,
        );
    }
}

/// `qs.isStarted() || qs.isCompleted()` for an arbitrary player + quest name.
pub(crate) fn quest_started_or_completed(world: &World, player: i32, quest: &str) -> bool {
    world
        .objects
        .get_component::<crate::model::components::Quests>(&player)
        .and_then(|q| q.0.get(quest))
        .is_some_and(|qs| {
            qs.state == crate::model::quest::state::STARTED
                || qs.state == crate::model::quest::state::COMPLETED
        })
}
