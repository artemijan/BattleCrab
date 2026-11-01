//! Labyrinth of Belis (instance template 178) — port of
//! `dist/game/data/scripts/instances/LabyrinthOfBelis/`, the instance of the
//! first class transfer ([`super::q10331_start_of_fate`]).
//!
//! The Infiltration Officer (a `FriendlyNpc`) walks the player through four
//! rooms; the instance status counts the steps:
//!
//! - 0 → 1: room 1 opens and six Operatives rush in. 2: all dead.
//! - 3 → 4: room 2. Handymen drop Marks of Belis; three marks fed to the
//!   Verification System make 5.
//! - 5 → 6: room 3. The officer smashes the Electricity Generator while six
//!   waves of attackers come for him; the zone shocks the player meanwhile.
//!   7: the generator is destroyed.
//! - 7 → 8: room 4. The opening movie, then Nemertess. Her death moves
//!   Q10331 to cond 2 and plays the ending movie. 9: the officer walks to her
//!   spot.
//!
//! The officer's death fails the instance.
//!
//! Gaps against Java: `generator.disableCoreAI(true)` and
//! `npc.setTarget(generator)` have no counterpart (the generator is a Folk
//! with no AI, and an NPC's own target is not modelled), and the attackers
//! spawn without the summon animation (`isSummonSpawn`), which no NPC spawn
//! shows yet.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::scripts::q10331_start_of_fate;

// NPCs
/// Pantheon; Java names the constant `SEBION`.
const SEBION: i32 = 32972;
const INFILTRATION_OFFICER: i32 = 19155;
const BELIS_VERITIFICATION_SYSTEM: i32 = 33215;
const OPERATIVE: i32 = 22998;
const HANDYMAN: i32 = 22997;
const ELECTRICITY_GENERATOR: i32 = 33216;
const NEMERTESS: i32 = 22984;

// Items
const BELIS_MARK: i32 = 17615;

// Skills
const CURRENT_SHOCK: (i32, i32) = (14698, 1);

// Locations
const INFILTRATION_OFFICER_ROOM_2: (i32, i32, i32) = (-117040, 212502, -8592);
const INFILTRATION_OFFICER_ROOM_3: (i32, i32, i32) = (-117843, 214230, -8592);
const INFILTRATION_OFFICER_ROOM_3_INSIDE: (i32, i32, i32) = (-118248, 214676, -8590);
const INFILTRATION_OFFICER_ROOM_4: (i32, i32, i32) = (-119217, 213743, -8600);
const SPAWN_ATTACKERS: (i32, i32, i32) = (-116809, 213275, -8606);
const GENERATOR_SPAWN: (i32, i32, i32) = (-118333, 214791, -8557);
const ATTACKER_SPOT: (i32, i32, i32) = (-117927, 214391, -8600);
const NEMERTESS_SPAWN: (i32, i32, i32) = (-118336, 212973, -8680);

// Misc
const TEMPLATE_ID: i32 = 178;
const DOOR_ID_ROOM_1_2: i32 = 16240002;
const DOOR_ID_ROOM_2_1: i32 = 16240003;
const DOOR_ID_ROOM_2_2: i32 = 16240004;
const DOOR_ID_ROOM_3_1: i32 = 16240005;
const DOOR_ID_ROOM_3_2: i32 = 16240006;
const DOOR_ID_ROOM_4_1: i32 = 16240007;
const DOOR_ID_ROOM_4_2: i32 = 16240008;
const DAMAGE_ZONE: i32 = 12014;

/// `Movie.SC_TALKING_ISLAND_BOSS_OPENING` / `_ENDING`.
const MOVIE_BOSS_OPENING: i32 = 43;
const MOVIE_BOSS_ENDING: i32 = 44;

/// `ExShowScreenMessage.TOP_CENTER`.
const TOP_CENTER: i32 = 2;

// NpcStringIds
const MARK_OF_BELIS_CAN_BE_ACQUIRED: i32 = 1811199;
const DON_T_COME_BACK_HERE: i32 = 1811217;
const HEY_YOU_RE_NOT_ALL_BAD: i32 = 1802335;
const READY_LET_ME_KNOW: i32 = 1802336;
const SOMETHING_OMINOUS_IN_THERE: i32 = 1802337;
const LET_ME_KNOW_WHEN_YOU_RE_ALL_READY: i32 = 1802331;
const IF_TERAIN_DIES_THE_MISSION_WILL_FAIL: i32 = 1811197;
const BEHIND_YOU_THE_ENEMY_IS_AMBUSHING_YOU: i32 = 1811194;
const KILL_THE_GUY_MESSING_WITH_THE_ELECTRIC_DEVICE: i32 = 1811195;
const FOCUS_ON_ATTACKING_THE_GUY_IN_THE_ROOM: i32 = 1811196;
const ALL_RIGHT_LET_S_MOVE_OUT: i32 = 1802332;
const COME_ON_ONTO_THE_NEXT_PLACE: i32 = 1802333;
const ELECTRONIC_DEVICE_HAS_BEEN_DESTROYED: i32 = 1811200;
const DEVICE_DESTROYED_LET_S_GO_ONTO_THE_NEXT: i32 = 1802334;

const KILL_NPCS: [i32; 4] = [OPERATIVE, HANDYMAN, INFILTRATION_OFFICER, NEMERTESS];
const START_NPCS: [i32; 3] = [SEBION, INFILTRATION_OFFICER, BELIS_VERITIFICATION_SYSTEM];

pub struct LabyrinthOfBelis;

/// `Location` → `addSpawn`'s `(x, y, z, heading)`.
fn at((x, y, z): (i32, i32, i32)) -> (i32, i32, i32, i32) {
    (x, y, z, 0)
}

impl LabyrinthOfBelis {
    /// The script events the instance's htmls send.
    fn instance_event(ctx: &mut QuestCtx, world: i32, event: &str) -> Option<String> {
        let (npc, player) = (ctx.npc, ctx.player);
        match event {
            "room1" => {
                if ctx.instance_status(world) == 0 {
                    ctx.set_npc_script_value(1);
                    ctx.npc_start_follow(npc, player);
                    ctx.set_instance_status(world, 1);
                    ctx.spawn_instance_group(world, "operatives");
                    ctx.open_close_instance_door(world, DOOR_ID_ROOM_1_2, true);
                }
            }
            "room2" => {
                if ctx.instance_status(world) == 3 {
                    ctx.set_instance_status(world, 4);
                    ctx.open_close_instance_door(world, DOOR_ID_ROOM_2_2, true);
                    ctx.set_npc_script_value(1);
                    ctx.npc_start_follow(npc, player);
                    ctx.send_screen_message_npc_string(
                        MARK_OF_BELIS_CAN_BE_ACQUIRED,
                        TOP_CENTER,
                        4500,
                    );
                    ctx.add_repeating_timer("MESSAGE", 10000, npc, player);
                }
            }
            "room3" => {
                if ctx.instance_status(world) == 5 {
                    ctx.set_instance_status(world, 6);
                    ctx.open_close_instance_door(world, DOOR_ID_ROOM_3_2, true);
                    let generator =
                        ctx.spawn_in_instance(world, ELECTRICITY_GENERATOR, at(GENERATOR_SPAWN))?;
                    ctx.set_npc_script_value(1);
                    ctx.npc_say(DON_T_COME_BACK_HERE);
                    ctx.npc_move_to(npc, INFILTRATION_OFFICER_ROOM_3_INSIDE);
                    ctx.add_timer("GENERATOR_EFFECT", 300, generator, 0);
                    ctx.add_repeating_timer("MESSAGE", 7000, npc, 0);
                    ctx.add_repeating_timer("ATTACKERS", 12500, npc, player);
                    ctx.npc_add_damage_hate(npc, generator, 0.0, 9999.0);
                    ctx.npc_reduce_current_hp(npc, 1.0, generator);
                }
            }
            "room4" => {
                if ctx.instance_status(world) == 7 {
                    ctx.set_instance_status(world, 8);
                    ctx.open_close_instance_door(world, DOOR_ID_ROOM_4_2, true);
                    ctx.set_npc_script_value(1);
                    ctx.play_movie(player, MOVIE_BOSS_OPENING);
                    ctx.add_timer("SPAWN_NEMERTESS", 50000, npc, 0);
                }
            }
            "giveBelisMark" => {
                if ctx.instance_status(world) != 4 {
                    return Some("33215-05.html".to_string());
                }
                if ctx.quest_items_count(BELIS_MARK) > 0 {
                    ctx.take_items(BELIS_MARK, 1);
                    match ctx.npc_script_value() {
                        0 => {
                            ctx.set_npc_script_value(1);
                            return Some("33215-01.html".to_string());
                        }
                        1 => {
                            ctx.set_npc_script_value(2);
                            return Some("33215-02.html".to_string());
                        }
                        2 => {
                            ctx.set_instance_status(world, 5);
                            ctx.add_timer("ROOM_2_DONE", 500, npc, 0);
                            return Some("33215-03.html".to_string());
                        }
                        _ => {}
                    }
                }
                return Some("33215-04.html".to_string());
            }
            "finish" => ctx.finish_instance(world, 0),
            _ => {}
        }
        None
    }

    /// `ROOM_3_DONE` once the sixth wave is out and one of its attackers
    /// falls.
    fn attacker_down(ctx: &mut QuestCtx, world: i32) {
        if ctx.instance_status(world) == 6
            && ctx.npc_script_value() == 1
            && ctx.instance_param(world, "counter") == 6
        {
            let (npc, player) = (ctx.npc, ctx.player);
            ctx.add_timer("ROOM_3_DONE", 2000, npc, player);
        }
    }

    /// `world.setStatus(-1); world.finishInstance(1)`.
    fn officer_lost(ctx: &mut QuestCtx) {
        let world = ctx.npc_instance();
        if world != 0 {
            ctx.set_instance_status(world, -1);
            ctx.finish_instance(world, 1);
        }
    }

    /// The `ATTACKERS` wave: one Operative or Handyman runs at the officer.
    fn spawn_attacker(ctx: &mut QuestCtx, world: i32) {
        let (npc, player) = (ctx.npc, ctx.player);
        let counter = ctx.instance_param(world, "counter") + 1;
        if counter == 6 {
            ctx.cancel_timer("ATTACKERS", npc, player);
        }
        ctx.set_instance_param(world, "counter", counter);
        let warning = if ctx.roll(2) == 0 {
            IF_TERAIN_DIES_THE_MISSION_WILL_FAIL
        } else {
            BEHIND_YOU_THE_ENEMY_IS_AMBUSHING_YOU
        };
        ctx.send_screen_message_npc_string(warning, TOP_CENTER, 4500);
        let mob_id = if ctx.roll(2) == 0 {
            OPERATIVE
        } else {
            HANDYMAN
        };
        let Some(mob) = ctx.spawn_in_instance(world, mob_id, at(SPAWN_ATTACKERS)) else {
            return;
        };
        ctx.npc_set_running(mob);
        ctx.npc_move_to(mob, ATTACKER_SPOT);
        let shout = if ctx.roll(2) == 0 {
            KILL_THE_GUY_MESSING_WITH_THE_ELECTRIC_DEVICE
        } else {
            FOCUS_ON_ATTACKING_THE_GUY_IN_THE_ROOM
        };
        ctx.npc_say_of(mob, shout);
        ctx.npc_add_damage_hate(mob, npc, 0.0, 9999.0);
        ctx.npc_reduce_current_hp(mob, 1.0, npc);
        ctx.set_npc_script_value_of(mob, 1);
    }
}

impl QuestScript for LabyrinthOfBelis {
    fn id(&self) -> i32 {
        // An `InstanceScript`, not a quest.
        -1
    }
    fn name(&self) -> &'static str {
        "LabyrinthOfBelis"
    }
    fn html_dir(&self) -> &'static str {
        "instances/LabyrinthOfBelis"
    }
    fn start_npcs(&self) -> &[i32] {
        &START_NPCS
    }
    fn talk_npcs(&self) -> &[i32] {
        &START_NPCS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[
            INFILTRATION_OFFICER,
            ELECTRICITY_GENERATOR,
            BELIS_VERITIFICATION_SYSTEM,
        ]
    }
    fn spawn_npcs(&self) -> &[i32] {
        &[INFILTRATION_OFFICER]
    }
    fn move_finished_npcs(&self) -> &[i32] {
        &[INFILTRATION_OFFICER]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn creature_death_npcs(&self) -> &[i32] {
        &[INFILTRATION_OFFICER]
    }
    fn enter_zone_ids(&self) -> &[i32] {
        &[DAMAGE_ZONE]
    }
    fn exit_zone_ids(&self) -> &[i32] {
        &[DAMAGE_ZONE]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if event == "enter_instance" {
            ctx.enter_instance(TEMPLATE_ID);
            return None;
        }
        let world = ctx.npc_instance();
        if world == 0 {
            return None;
        }
        Self::instance_event(ctx, world, event)
    }

    // "Replace me with effect zone when support for instances are done."
    fn on_enter_zone(&self, ctx: &mut QuestCtx, creature: i32, _zone_id: i32) {
        if ctx.player == 0 || creature != ctx.player {
            return;
        }
        let world = ctx.player_instance();
        if world != 0 && ctx.instance_status(world) == 6 {
            let generator = ctx.instance_npc(world, ELECTRICITY_GENERATOR).unwrap_or(0);
            ctx.add_repeating_timer("DEBUFF", 1500, generator, creature);
        }
    }

    fn on_exit_zone(&self, ctx: &mut QuestCtx, creature: i32, _zone_id: i32) {
        if ctx.player == 0 || creature != ctx.player {
            return;
        }
        let world = ctx.player_instance();
        if world != 0 && matches!(ctx.instance_status(world), 6 | 7) {
            let generator = ctx.instance_npc(world, ELECTRICITY_GENERATOR).unwrap_or(0);
            ctx.cancel_timer("DEBUFF", generator, creature);
        }
    }

    fn on_move_finished(&self, ctx: &mut QuestCtx) {
        let world = ctx.npc_instance();
        if world == 0 {
            return;
        }
        let npc = ctx.npc;
        let line = match ctx.instance_status(world) {
            3 => Some(HEY_YOU_RE_NOT_ALL_BAD),
            5 => Some(READY_LET_ME_KNOW),
            7 => {
                ctx.npc_abort_attack(npc);
                Some(SOMETHING_OMINOUS_IN_THERE)
            }
            9 => None,
            _ => return,
        };
        ctx.set_npc_script_value(0);
        if let Some(line) = line {
            ctx.npc_broadcast_info(npc);
            ctx.npc_say(line);
        }
        let heading = ctx.npc_heading(npc);
        ctx.npc_set_heading(npc, heading + 32500);
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let world = ctx.npc_instance();
        if world == 0 {
            return None;
        }
        let html = match ctx.npc_id {
            INFILTRATION_OFFICER => {
                if ctx.npc_script_value() != 0 {
                    "19155-02.html"
                } else {
                    match ctx.instance_status(world) {
                        0 => "19155-01.html",
                        3 => "19155-03.html",
                        5 => "19155-04.html",
                        7 => "19155-05.html",
                        9 => "19155-06.html",
                        _ => return None,
                    }
                }
            }
            BELIS_VERITIFICATION_SYSTEM => "33215.html",
            ELECTRICITY_GENERATOR => "33216.html",
            _ => return None,
        };
        Some(html.to_string())
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        let world = ctx.npc_instance();
        if world == 0 {
            return;
        }
        let (npc, player) = (ctx.npc, ctx.player);
        match ctx.npc_id {
            OPERATIVE => {
                if ctx.instance_status(world) == 1 {
                    if ctx.instance_alive_npcs(world, OPERATIVE).is_empty() {
                        ctx.set_instance_status(world, 2);
                        ctx.add_timer("ROOM_1_DONE", 500, npc, 0);
                    }
                } else {
                    Self::attacker_down(ctx, world);
                }
            }
            HANDYMAN => {
                if ctx.instance_status(world) == 4 {
                    if ctx.roll(100) > 10 {
                        ctx.drop_item_from_npc(BELIS_MARK, 1);
                    }
                } else {
                    Self::attacker_down(ctx, world);
                }
            }
            INFILTRATION_OFFICER => Self::officer_lost(ctx),
            NEMERTESS => {
                // `qs.isCond(1)` on a missing state throws, which ends the
                // listener before anything below runs.
                let has_state = ctx.with_other_quest(q10331_start_of_fate::QUEST_NAME, |qs| {
                    if !qs.has_qs() {
                        return false;
                    }
                    if qs.is_cond(1) {
                        qs.set_cond(2, true);
                    }
                    true
                });
                if has_state != Some(true) {
                    return;
                }
                ctx.delete_npc();
                ctx.play_movie(player, MOVIE_BOSS_ENDING);
                ctx.add_timer("ROOM_4_DONE", 30000, npc, 0);
            }
            _ => {}
        }
    }

    fn on_creature_death(&self, ctx: &mut QuestCtx, _killer: i32) {
        Self::officer_lost(ctx);
    }

    fn on_spawn(&self, ctx: &mut QuestCtx) {
        // `setCanReturnToSpawnPoint(false)`: the FriendlyNpc AI never walks
        // home anyway.
        let npc = ctx.npc;
        ctx.npc_set_running(npc);
        ctx.add_repeating_timer("MESSAGE", 6000, npc, 0);
    }

    fn on_timer_event(&self, ctx: &mut QuestCtx, event: &str) {
        let world = ctx.npc_instance();
        if world == 0 {
            return;
        }
        let (npc, player) = (ctx.npc, ctx.player);
        let officer = ctx.instance_npc(world, INFILTRATION_OFFICER);
        match event {
            "MESSAGE" => match ctx.instance_status(world) {
                0 => ctx.npc_say(LET_ME_KNOW_WHEN_YOU_RE_ALL_READY),
                4 => ctx.send_screen_message_npc_string(
                    MARK_OF_BELIS_CAN_BE_ACQUIRED,
                    TOP_CENTER,
                    4500,
                ),
                6 => ctx.npc_say(DON_T_COME_BACK_HERE),
                _ => ctx.cancel_timer("MESSAGE", npc, 0),
            },
            "ATTACKERS" => {
                if ctx.instance_status(world) == 6 {
                    Self::spawn_attacker(ctx, world);
                } else {
                    ctx.cancel_timer("ATTACKERS", npc, player);
                }
            }
            "ROOM_1_DONE" => {
                if ctx.instance_status(world) == 2 {
                    ctx.set_instance_status(world, 3);
                    ctx.open_close_instance_door(world, DOOR_ID_ROOM_2_1, true);
                    let Some(officer) = officer else { return };
                    ctx.npc_move_to(officer, INFILTRATION_OFFICER_ROOM_2);
                    ctx.npc_say_of(officer, ALL_RIGHT_LET_S_MOVE_OUT);
                }
            }
            "ROOM_2_DONE" => {
                ctx.open_close_instance_door(world, DOOR_ID_ROOM_3_1, true);
                let Some(officer) = officer else { return };
                ctx.npc_move_to(officer, INFILTRATION_OFFICER_ROOM_3);
                ctx.npc_say_of(officer, COME_ON_ONTO_THE_NEXT_PLACE);
            }
            "ROOM_3_DONE" => {
                if ctx.instance_status(world) == 6 {
                    ctx.set_instance_status(world, 7);
                    ctx.open_close_instance_door(world, DOOR_ID_ROOM_4_1, true);
                    ctx.send_screen_message_npc_string(
                        ELECTRONIC_DEVICE_HAS_BEEN_DESTROYED,
                        TOP_CENTER,
                        4500,
                    );
                    let generator = ctx.instance_npc(world, ELECTRICITY_GENERATOR);
                    let (Some(generator), Some(officer)) = (generator, officer) else {
                        return;
                    };
                    ctx.npc_do_die(generator, officer);
                    ctx.delete_npc_of(generator);
                    ctx.add_timer("MOVE_TO_ROOM_4", 3000, officer, 0);
                }
            }
            "MOVE_TO_ROOM_4" => {
                ctx.npc_set_running(npc);
                ctx.npc_move_to(npc, INFILTRATION_OFFICER_ROOM_4);
                ctx.npc_say(DEVICE_DESTROYED_LET_S_GO_ONTO_THE_NEXT);
            }
            "ROOM_4_DONE" => {
                if ctx.instance_status(world) == 8 {
                    ctx.set_instance_status(world, 9);
                    let Some(officer) = officer else { return };
                    ctx.npc_move_to(officer, NEMERTESS_SPAWN);
                }
            }
            "GENERATOR_EFFECT" => ctx.npc_set_display_effect(npc, 1),
            "SPAWN_NEMERTESS" => {
                ctx.spawn_in_instance(world, NEMERTESS, at(NEMERTESS_SPAWN));
            }
            "DEBUFF" => ctx.apply_skill_effects(CURRENT_SHOCK.0, CURRENT_SHOCK.1, npc, player),
            _ => {}
        }
    }
}
