//! `ai/areas/ImperialTomb/FourSepulchers` — the chat/kill/spawn hooks; the
//! run machinery lives in [`crate::game_loop::four_sepulchers`].

use crate::game_loop::four_sepulchers as fs;
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::components::{AdminFlags, Position, RegionCell, Vitals};

const ROOM_3_VICTIM: i32 = 18150;
const ROOM_3_CHEST_REWARDER: i32 = 18158;
const ROOM_4_CHARMS: [i32; 4] = [18196, 18197, 18198, 18199];
const ROOM_5_STATUE_GUARD: i32 = 18232;
const ROOM_6_REWARD_CHEST: i32 = 18256;
const BOSSES: [i32; 4] = [25346, 25342, 25339, 25349];

/// Petrification (4616) the statue guards wear until the timer lifts it.
const PETRIFY: i32 = 4616;

/// `CHARM_SKILLS` — the trap-zone skill each charm shuts off.
fn charm_skill(npc_id: i32) -> Option<i32> {
    Some(match npc_id {
        18196 => 4146,
        18197 => 4145,
        18198 => 4148,
        18199 => 4624,
        _ => return None,
    })
}

/// `CHARM_MSG` (Java carries two mismatched ids and flags them; ported as shipped).
fn charm_msg(npc_id: i32) -> i32 {
    match npc_id {
        18196 | 18197 => 1010480, // P. Atk reduction device destroyed
        _ => 1010479,             // poison device destroyed
    }
}

const VICTIM_MSG: [i32; 3] = [8058, 8059, 8060]; // Help me! / Don't miss! / Keep pushing!
const MONSTERS_HAVE_SPAWNED: i32 = 1000502;

const ALL_TALK: [i32; 31] = [
    fs::CONQUEROR_MANAGER,
    fs::EMPEROR_MANAGER,
    fs::GREAT_SAGES_MANAGER,
    fs::JUDGE_MANAGER,
    fs::MYSTERIOUS_CHEST,
    fs::KEY_CHEST,
    fs::TELEPORTER,
    31453,
    31454,
    31919,
    31920,
    31925,
    31926,
    31927,
    31928,
    31929,
    31930,
    31931,
    31932,
    31933,
    31934,
    31935,
    31936,
    31937,
    31938,
    31939,
    31940,
    31941,
    31942,
    31943,
    31944,
];

const KILL_NPCS: [i32; 10] = [
    18120,
    ROOM_3_CHEST_REWARDER,
    18177,
    18212, // chest rewarders
    ROOM_3_VICTIM,
    18196,
    18197,
    18198,
    18199,
    ROOM_6_REWARD_CHEST,
];

const SPAWN_NPCS: [i32; 6] = [
    ROOM_3_VICTIM,
    18196,
    18197,
    18198,
    18199,
    ROOM_5_STATUE_GUARD,
];

pub struct FourSepulchers;

impl QuestScript for FourSepulchers {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "FourSepulchers"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/ImperialTomb/FourSepulchers"
    }
    fn start_npcs(&self) -> &[i32] {
        &ALL_TALK
    }
    fn talk_npcs(&self) -> &[i32] {
        &ALL_TALK
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &ALL_TALK
    }
    fn kill_npcs(&self) -> &[i32] {
        // The bosses route through their own registration below (they are
        // raid bosses, so the kill notification arrives the same way).
        &KILL_NPCS
    }
    fn spawn_npcs(&self) -> &[i32] {
        &SPAWN_NPCS
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let npc_id = ctx.npc_id;
        if npc_id == fs::MYSTERIOUS_CHEST {
            if ctx.npc_script_value() == 0 {
                ctx.set_npc_script_value(1);
                ctx.delete_npc();
                let sep = fs::sepulcher_of(ctx.world, ctx.player);
                if sep > 0 {
                    fs::spawn_next_wave(ctx.world, sep);
                }
            }
            return None;
        }
        if npc_id == fs::KEY_CHEST {
            if ctx.npc_script_value() == 0 {
                ctx.set_npc_script_value(1);
                ctx.delete_npc();
                ctx.give_items(fs::CHAPEL_KEY, 1);
            }
            return None;
        }
        Some(format!("{npc_id}.html"))
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        match event {
            "Enter" => {
                if !crate::game_loop::four_sepulchers::quest_started_or_completed(
                    ctx.world,
                    ctx.player,
                    "Q00620_FourGoblets",
                ) {
                    return Some(ctx.no_quest_html());
                }
                let npc_id = ctx.npc_id;
                let npc = ctx.npc;
                let player = ctx.player;
                match fs::try_enter(ctx.world, npc, player) {
                    // Java asks for "-FULL.htm" but the datapack ships ".html" —
                    // the working file wins over the typo.
                    fs::EnterOutcome::Full => Some(format!("{npc_id}-FULL.html")),
                    fs::EnterOutcome::SmallParty => Some(format!("{npc_id}-SP.html")),
                    fs::EnterOutcome::NotLeader => Some(format!("{npc_id}-NL.html")),
                    fs::EnterOutcome::NoQuest(name) => {
                        Some(with_member(ctx, &format!("{npc_id}-NS.html"), &name))
                    }
                    fs::EnterOutcome::NoPass(name) => {
                        Some(with_member(ctx, &format!("{npc_id}-SE.html"), &name))
                    }
                    fs::EnterOutcome::NotTime => Some(format!("{npc_id}-NE.html")),
                    fs::EnterOutcome::Ok => Some(format!("{npc_id}-OK.html")),
                }
            }
            "OpenGate" => {
                if !crate::game_loop::four_sepulchers::quest_started_or_completed(
                    ctx.world,
                    ctx.player,
                    "Q00620_FourGoblets",
                ) {
                    return Some(ctx.no_quest_html());
                }
                if ctx.npc_script_value() != 0 {
                    return None;
                }
                if ctx.item_object_id(fs::CHAPEL_KEY).is_some() {
                    ctx.set_npc_script_value(1);
                    ctx.take_items(fs::CHAPEL_KEY, -1);
                    let sep = fs::sepulcher_of(ctx.world, ctx.player);
                    if sep > 0 {
                        fs::open_gate(ctx.world, sep);
                    }
                    let npc = ctx.npc;
                    say(ctx, npc, MONSTERS_HAVE_SPAWNED);
                    None
                } else {
                    Some("Gatekeeper-no.html".into())
                }
            }
            _ => None,
        }
    }

    fn on_spawn(&self, ctx: &mut QuestCtx) {
        let npc = ctx.npc;
        match ctx.npc_id {
            ROOM_3_VICTIM => {
                ctx.world.scheduler.schedule(
                    ctx.world.tick + 10,
                    crate::scheduler::ScheduledTask::FsVictimFlee { npc_oid: npc },
                );
            }
            ROOM_5_STATUE_GUARD => {
                // Petrified for the first five minutes: untouchable and, for
                // the look of it, wearing Petrification.
                ctx.world.objects.add_components(
                    &npc,
                    AdminFlags {
                        invul: true,
                        untargetable: true,
                        ..Default::default()
                    },
                );
                if let Some(skill) = ctx.world.data.skill_data.get(PETRIFY, 1).cloned() {
                    crate::game_loop::npc_cast::start_cast(ctx.world, npc, npc, &skill);
                }
                ctx.world.scheduler.schedule(
                    ctx.world.tick + 5 * 60 * 10,
                    crate::scheduler::ScheduledTask::FsRemovePetrify { npc_oid: npc },
                );
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        let npc_id = ctx.npc_id;
        match npc_id {
            ROOM_3_VICTIM => {
                ctx.spawn_near_npc(ROOM_3_CHEST_REWARDER, false);
            }
            _ if ROOM_4_CHARMS.contains(&npc_id) => {
                if let Some(skill) = charm_skill(npc_id) {
                    let killer = ctx.player;
                    fs::disable_charm_zone(ctx.world, killer, skill);
                }
                let npc = ctx.npc;
                let msg = charm_msg(npc_id);
                say(ctx, npc, msg);
            }
            ROOM_6_REWARD_CHEST => {
                drop_adena_reward(ctx);
            }
            _ => {
                // Chest rewarders: a key chest appears on the corpse.
                let pos = ctx
                    .world
                    .objects
                    .get_component::<Position>(&ctx.npc)
                    .copied();
                if let Some(p) = pos {
                    crate::model::npc::spawn_npc_at(ctx.world, fs::KEY_CHEST, p.x, p.y, p.z, 0);
                }
            }
        }
    }
}

/// The four hall bosses are raid bosses with their own kill routing — a thin
/// second script keyed on just them, so the fan-out stays one-per-npc.
pub struct FourSepulchersBosses;

impl QuestScript for FourSepulchersBosses {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "FourSepulchersBosses"
    }
    fn html_dir(&self) -> &'static str {
        "ai/areas/ImperialTomb/FourSepulchers"
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[]
    }
    fn kill_npcs(&self) -> &[i32] {
        &BOSSES
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        let (npc, player) = (ctx.npc, ctx.player);
        fs::on_boss_killed(ctx.world, npc, player);
    }
}

/// `ROOM_6_REWARD_CHEST` pays 300–1300 adena on the ground.
fn drop_adena_reward(ctx: &mut QuestCtx) {
    let Some(p) = ctx
        .world
        .objects
        .get_component::<Position>(&ctx.npc)
        .copied()
    else {
        return;
    };
    let count = 300 + ctx.roll(1001) as i64;
    let (npc, player) = (ctx.npc, ctx.player);
    let ground_oid = crate::game_loop::ground_items::spawn_ground_item(
        ctx.world,
        57,
        count,
        0,
        p.x,
        p.y,
        p.z,
        npc,
        crate::game_loop::ground_items::DropSource::Npc,
    );
    if let Some(g) = ctx
        .world
        .objects
        .get_component_mut::<crate::model::components::GroundItem>(&ground_oid)
    {
        g.owner_id = player;
        g.owner_until_tick = ctx.world.tick + 150;
    }
}

/// `VICTIM_FLEE` — the room-3 victim scrambles around its room crying for
/// help every three seconds. (Java wanders around the spawn point; the port
/// wanders around the current spot — the room walls bound both.)
pub(crate) fn handle_victim_flee(world: &mut crate::world::World, npc_oid: i32) {
    let alive = world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .is_some_and(|v| !v.dead);
    if !alive {
        return;
    }
    let Some(pos) = world.objects.get_component::<Position>(&npc_oid).copied() else {
        return;
    };
    let dx = world.roll(801) - 400;
    let dy = world.roll(801) - 400;
    crate::game_loop::npc_ai::move_npc_to(world, npc_oid, pos.x + dx, pos.y + dy, pos.z);
    let msg = VICTIM_MSG[world.roll(3) as usize];
    if let Some(npc_id) = npc_id_of(world, npc_oid) {
        let pkt = crate::network::server_packets::npc_say(npc_oid, npc_id, msg);
        if let Some(region) = world
            .objects
            .get_component::<RegionCell>(&npc_oid)
            .map(|r| r.0)
        {
            crate::game_loop::helpers::broadcast_near_region(world, region, &pkt);
        }
    }
    world.scheduler.schedule(
        world.tick + 30,
        crate::scheduler::ScheduledTask::FsVictimFlee { npc_oid },
    );
}

/// `REMOVE_PETRIFY` — five minutes up: the statue comes alive.
pub(crate) fn handle_remove_petrify(world: &mut crate::world::World, npc_oid: i32) {
    if let Some(flags) = world.objects.get_component_mut::<AdminFlags>(&npc_oid) {
        flags.invul = false;
        flags.untargetable = false;
    }
}

fn say(ctx: &mut QuestCtx, npc_oid: i32, npc_string_id: i32) {
    let Some(npc_id) = npc_id_of(ctx.world, npc_oid) else {
        return;
    };
    let pkt = crate::network::server_packets::npc_say(npc_oid, npc_id, npc_string_id);
    if let Some(region) = ctx
        .world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    {
        crate::game_loop::helpers::broadcast_near_region(ctx.world, region, &pkt);
    }
}

/// Load a script html and fill Java's `%member%` placeholder.
fn with_member(ctx: &mut QuestCtx, file: &str, member: &str) -> String {
    ctx.get_htm(file).replace("%member%", member)
}
