//! The script hooks the class-quest instances run on: `getTimers()` (one-shot
//! and repeating holders keyed by event, NPC and player), `onMoveFinished`,
//! `onEnterZone` / `onExitZone`, `ON_CREATURE_DEATH` whatever the killer, and
//! `ON_PLAYER_LEVEL_CHANGED`.

use super::*;
use crate::game_loop::quests;
use std::sync::Mutex;

const PLAYER: i32 = 3001;
const NPC: i32 = 900_001;
const NPC_ID: i32 = 30001;
/// Labyrinth of Belis' script zone (`custom_script.xml`, minZ -8000 / maxZ
/// -8700 — the bounds are written the wrong way round).
const BELIS_ZONE: i32 = 12014;

/// Records every hook call as a line in `log`.
struct Recorder {
    log: Arc<Mutex<Vec<String>>>,
}

impl Recorder {
    fn push(&self, line: String) {
        self.log.lock().unwrap().push(line);
    }
}

impl quests::QuestScript for Recorder {
    fn id(&self) -> i32 {
        -3
    }
    fn name(&self) -> &'static str {
        "HookTest"
    }
    fn html_dir(&self) -> &'static str {
        ""
    }
    fn start_npcs(&self) -> &[i32] {
        &[]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[NPC_ID]
    }
    fn move_finished_npcs(&self) -> &[i32] {
        &[NPC_ID]
    }
    fn creature_death_npcs(&self) -> &[i32] {
        &[NPC_ID]
    }
    fn enter_zone_ids(&self) -> &[i32] {
        &[BELIS_ZONE]
    }
    fn exit_zone_ids(&self) -> &[i32] {
        &[BELIS_ZONE]
    }
    fn handles_global_events(&self) -> bool {
        true
    }
    fn on_talk(&self, _ctx: &mut quests::QuestCtx) -> Option<String> {
        None
    }
    fn on_event(&self, ctx: &mut quests::QuestCtx, event: &str) -> Option<String> {
        let (npc, player) = (ctx.npc, ctx.player);
        match event {
            "once" => ctx.add_timer("ONCE", 500, npc, player),
            "repeat" => ctx.add_repeating_timer("REPEAT", 300, npc, 0),
            "stop" => ctx.cancel_timer("REPEAT", npc, 0),
            _ => {}
        }
        None
    }
    fn on_timer_event(&self, ctx: &mut quests::QuestCtx, event: &str) {
        self.push(format!(
            "timer {event} npc={} player={}",
            ctx.npc, ctx.player
        ));
    }
    fn on_move_finished(&self, ctx: &mut quests::QuestCtx) {
        self.push(format!("moved {}", ctx.npc));
    }
    fn on_enter_zone(&self, ctx: &mut quests::QuestCtx, creature: i32, zone_id: i32) {
        self.push(format!("enter {zone_id} {creature} player={}", ctx.player));
    }
    fn on_exit_zone(&self, _ctx: &mut quests::QuestCtx, creature: i32, zone_id: i32) {
        self.push(format!("exit {zone_id} {creature}"));
    }
    fn on_creature_death(&self, ctx: &mut quests::QuestCtx, killer: i32) {
        self.push(format!("death {} killer={killer}", ctx.npc));
    }
    fn on_level_changed(&self, _ctx: &mut quests::QuestCtx, old_level: i32, new_level: i32) {
        self.push(format!("level {old_level}->{new_level}"));
    }
}

fn hook_world(
    x: i32,
    y: i32,
    z: i32,
) -> (
    World,
    UnboundedReceiver<bytes::Bytes>,
    Arc<Mutex<Vec<String>>>,
) {
    let (mut world, _db_rx, _link_rx) = quest_test_world();
    let log = Arc::new(Mutex::new(Vec::new()));
    world.quests = Arc::new(quests::QuestRegistry::new(vec![Arc::new(Recorder {
        log: log.clone(),
    })]));
    add_test_npc(&mut world, NPC, NPC_ID, "Folk", 5, x, y, z);
    let rx = ingame_player(&mut world, 1, PLAYER, x, y, z);
    (world, rx, log)
}

fn event(world: &mut World, name: &str) {
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_{NPC}_Quest HookTest {name}")),
    );
}

fn take(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    std::mem::take(&mut *log.lock().unwrap())
}

/// `addTimer` fires once with its NPC and player; adding the same key again
/// replaces the pending holder instead of stacking a second one.
#[test]
fn a_one_shot_timer_fires_once_and_a_same_key_add_replaces_it() {
    let (mut world, _rx, log) = hook_world(0, 0, 0);
    event(&mut world, "once");
    advance_ticks(&mut world, 3);
    event(&mut world, "once");
    advance_ticks(&mut world, 4);
    assert!(take(&log).is_empty(), "the first holder was replaced");
    advance_ticks(&mut world, 1);
    assert_eq!(
        take(&log),
        [format!("timer ONCE npc={NPC} player={PLAYER}")]
    );
    advance_ticks(&mut world, 20);
    assert!(take(&log).is_empty(), "one-shot");
}

/// `addRepeatingTimer` keeps firing at its period until `cancelTimer`.
#[test]
fn a_repeating_timer_fires_at_its_period_until_cancelled() {
    let (mut world, _rx, log) = hook_world(0, 0, 0);
    event(&mut world, "repeat");
    advance_ticks(&mut world, 9);
    assert_eq!(take(&log).len(), 3, "at 300, 600 and 900 ms");
    event(&mut world, "stop");
    advance_ticks(&mut world, 10);
    assert!(take(&log).is_empty());
}

/// `Npc.onDecay` → `stopTimerHolders`: the task dies with the NPC, but the
/// holder stays in the executor, so `hasTimer` still answers for it.
#[test]
fn an_npc_despawn_stops_its_timers_but_keeps_the_holder() {
    let (mut world, _rx, log) = hook_world(0, 0, 0);
    event(&mut world, "repeat");
    crate::game_loop::npc::despawn_npc_by_oid(&mut world, NPC);
    advance_ticks(&mut world, 10);
    assert!(take(&log).is_empty());
    assert_eq!(world.script_timers.len(), 1);
}

/// `onActionArrived`'s `ON_NPC_MOVE_FINISHED` fires when the walk ends.
#[test]
fn an_npc_reaching_its_destination_fires_move_finished() {
    let (mut world, _rx, log) = hook_world(0, 0, 0);
    ai::move_npc_to(&mut world, NPC, 200, 0, 0);
    advance_world(&mut world, 1);
    assert!(take(&log).is_empty(), "still walking");
    advance_world(&mut world, 100);
    assert_eq!(take(&log), [format!("moved {NPC}")]);
}

/// Entry and exit fire on the edges only, and the zone's reversed Z bounds
/// still contain a player standing between them (`ZoneNPoly` swaps them).
#[test]
fn zone_enter_and_exit_fire_on_the_edges() {
    let (mut world, _rx, log) = hook_world(-118300, 214800, -8600);
    world.data = dist::game_data_owned();
    crate::game_loop::space::zones::revalidate_zone(&mut world, PLAYER, true);
    assert_eq!(
        take(&log),
        [format!("enter {BELIS_ZONE} {PLAYER} player={PLAYER}")]
    );
    crate::game_loop::space::zones::revalidate_zone(&mut world, PLAYER, true);
    assert!(take(&log).is_empty(), "still inside: no second enter");

    if let Some(p) = world.objects.get_component_mut::<Position>(&PLAYER) {
        p.x = -117000;
    }
    crate::game_loop::space::zones::revalidate_zone(&mut world, PLAYER, true);
    assert_eq!(take(&log), [format!("exit {BELIS_ZONE} {PLAYER}")]);
}

/// `ON_CREATURE_DEATH` runs whoever landed the blow — here another NPC.
#[test]
fn creature_death_fires_for_an_npc_killer() {
    let (mut world, _rx, log) = hook_world(0, 0, 0);
    add_test_npc(&mut world, NPC + 1, 20001, "Monster", 5, 50, 0, 0);
    crate::game_loop::npc::npc_do_die(&mut world, NPC, NPC + 1);
    assert_eq!(take(&log), [format!("death {NPC} killer={}", NPC + 1)]);
}

/// `PlayerStat.addLevel`: an exp gain that crosses levels reports the old
/// and new level once.
#[test]
fn an_exp_gain_across_levels_reports_the_level_change() {
    let (mut world, _rx, log) = hook_world(0, 0, 0);
    let exp = world.data.experience.exp_for_level(5) as f64;
    crate::game_loop::death::add_exp_and_sp(&mut world, PLAYER, exp, 0.0, false);
    assert_eq!(take(&log), ["level 1->5"]);
}
