//! Chamber of Prophecies (instance 255) — port of
//! `dist/game/data/scripts/instances/ChamberOfProphecies/`, Q10753's solo
//! instance.
//!
//! Gereth's page enters it (Q10753 cond 16). Kain Van Halter's `teleport`
//! brings the helper Kain and Ferin into the first room, and a 7-second
//! `CHECK_STATUS` loop walks the world status through the rooms (0 → 5):
//! each cleared room opens the next door and spawns its wave while the two
//! helpers banter and fight. At the end the Grail's page gives the Atelia
//! and the Mysterious Wizard appears; his last page sets status 6, and
//! Kain's `exit` sets cond 17 and closes the instance.
//!
//! Timers keep Java's `QuestTimer` identity, (name, npc, player), and its
//! refusal of a duplicate. A cancel naming another NPC does nothing, so
//! `OPEN_DOOR1` / `OPEN_DOOR2` (run on Kain) never stop the helper's
//! `ATTACK` / `ATTACK1`, as in Java. `setCanReturnToSpawnPoint(false)` is not
//! modelled.

use super::q10751_winds_of_fate_encounters::TOP_CENTER;
use super::q10753_winds_of_fate_choices::{self as q10753, ATELIA, PROPHECY_MACHINE};
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::sm_ids;

// NPCs
const KAIN_VAN_HALTER: i32 = 31639;
const GRAIL: i32 = 33996;
const MYSTERIOUS_WIZARD: i32 = 33980;

// Helper NPCs
const HELPER_VAN_HALTER: i32 = 33999;
const HELPER_FERIN: i32 = 34001;

// Misc
const DOOR_2: i32 = 17230102;
const DOOR_3: i32 = 17230103;
const DOOR_4: i32 = 17230104;
const TEMPLATE_ID: i32 = 255;
const FIRST_ROOM_LOC: (i32, i32, i32) = (-88503, 184754, -10440);
const THIRD_ROOM_LOC: (i32, i32, i32) = (-88506, 177151, -10445);
/// `ExShowUsm.USM_Q015_E`.
const USM_Q015_E: i32 = 15;

// NpcStringIds
const TALK_TO_THE_MYSTERIOUS_WIZARD: i32 = 575214;
const THIS_CHOICE_CANNOT_BE_REVERSED: i32 = 1802668;
const THAT_GUY_KAIN_HAS_A_SMARMY_FACE: i32 = 1802636;
const YOU_CAN_T_DIE_HERE_I_DIDN_T_LEARN_RESURRECT_YET: i32 = 1802634;
const GISELLE_WAS_SUCH_A_SWEET_CHILD: i32 = 1802637;
const DO_YOU_THINK_I_LL_GROW_TALLER_IF_I_EAT_LOTS_AND_LOTS: i32 = 1802635;
const SUCH_MONSTERS_IN_A_PLACE_LIKE_THIS_UNBELIEVABLE: i32 = 1802612;
const THAT_S_THE_MONSTER_THAT_ATTACKED_FAERON: i32 = 1802642;
const GO_NOW_KAIN_CAN_HANDLE_THIS: i32 = 1802647;
const LEAVE_THIS_TO_ME_GO: i32 = 1802646;
const LEAVE_THIS_PLACE_TO_KAIN_NGO_TO_THE_NEXT_ROOM: i32 = 1802614;

pub struct ChamberOfProphecies;

/// `startQuestTimer(name, time, npc, player[, repeating])`: refused while a
/// same-key timer lives.
fn start_timer(ctx: &mut QuestCtx, name: &str, delay_ms: u64, npc: i32, repeating: bool) {
    let player = ctx.player;
    if ctx.has_timer(name, npc, player) {
        return;
    }
    if repeating {
        ctx.add_repeating_timer(name, delay_ms, npc, player);
    } else {
        ctx.add_timer(name, delay_ms, npc, player);
    }
}

impl ChamberOfProphecies {
    /// `player.getInstanceWorld()` when it is this template, else 0.
    fn world_of_player(ctx: &QuestCtx) -> i32 {
        let world = ctx.player_instance();
        if ctx.instance_template(world) == TEMPLATE_ID {
            world
        } else {
            0
        }
    }

    /// `world.getNpc(id)`, 0 for Java's `null`.
    fn npc_of(ctx: &QuestCtx, world: i32, npc_id: i32) -> i32 {
        ctx.instance_npc(world, npc_id).unwrap_or(0)
    }

    /// The helper's line and, when given, the voice the player hears.
    fn helper_say(ctx: &mut QuestCtx, helper: i32, line: i32, voice: Option<&str>) {
        if ctx.npc_id != helper {
            return;
        }
        let npc = ctx.npc;
        ctx.npc_say_of(npc, line);
        if let Some(voice) = voice {
            ctx.play_voice(voice);
        }
    }

    /// The `CHECK_STATUS` beat: room by room on the world status.
    fn check_status(ctx: &mut QuestCtx, world: i32) {
        let npc = ctx.npc;
        let ferin = Self::npc_of(ctx, world, HELPER_FERIN);
        let van_halter = Self::npc_of(ctx, world, HELPER_VAN_HALTER);
        let cleared = ctx.instance_alive_monsters(world).is_empty();
        match ctx.instance_status(world) {
            0 => {
                if cleared {
                    start_timer(ctx, "SEY2", 14000, ferin, false);
                    start_timer(ctx, "SEY_KAIN", 24000, van_halter, false);
                    start_timer(ctx, "OPEN_DOOR1", 5000, npc, false);
                }
                start_timer(ctx, "CHECK_STATUS", 7000, npc, false);
            }
            1 => {
                if cleared {
                    ctx.spawn_instance_group(world, "wof_room2_1");
                    ctx.set_instance_status(world, 2);
                }
                start_timer(ctx, "CHECK_STATUS", 7000, npc, false);
            }
            2 => {
                if cleared {
                    start_timer(ctx, "SEY3", 8000, ferin, false);
                    start_timer(ctx, "OPEN_DOOR2", 5000, npc, false);
                }
                start_timer(ctx, "CHECK_STATUS", 7000, npc, false);
            }
            3 => {
                if cleared {
                    ctx.set_instance_status(world, 4);
                    ctx.spawn_instance_group(world, "wof_room3_2");
                    ctx.open_close_instance_door(world, DOOR_3, false);
                    start_timer(ctx, "SEY_KAIN_1", 5000, van_halter, false);
                }
                start_timer(ctx, "CHECK_STATUS", 7000, npc, false);
            }
            4 => {
                if cleared {
                    ctx.set_instance_status(world, 5);
                    ctx.spawn_instance_group(world, "wof_room4");
                    start_timer(ctx, "SEY_KAIN_2", 3000, van_halter, false);
                    start_timer(ctx, "SEY4", 7000, ferin, false);
                } else {
                    start_timer(ctx, "CHECK_STATUS", 7000, npc, false);
                }
            }
            _ => {}
        }
    }

    /// `ATTACK` / `ATTACK1` / `ATTACK2` on a helper: follow the player
    /// once the rooms are clear (and the helper has not stood down), else
    /// go after every monster in sight.
    fn helper_attack(ctx: &mut QuestCtx, world: i32) {
        let (npc, player) = (ctx.npc, ctx.player);
        ctx.npc_set_running(npc);
        if ctx.npc_script_value() == 0 && ctx.instance_alive_monsters(world).is_empty() {
            ctx.npc_start_follow(npc, player);
        } else if !ctx.npc_is_attacking(npc) {
            let monsters: Vec<i32> = ctx
                .visible_npcs(npc, Some(3000))
                .into_iter()
                .filter(|&m| {
                    crate::game_loop::npc::npc_template(ctx.world, m)
                        .is_some_and(|t| t.is_monster())
                })
                .collect();
            for monster in monsters {
                ctx.npc_attack_desire(npc, monster);
            }
        }
    }

    fn teleport(ctx: &mut QuestCtx, world: i32) {
        for helper in [HELPER_VAN_HALTER, HELPER_FERIN] {
            if let Some(npc) = ctx.instance_npc(world, helper) {
                ctx.delete_npc_of(npc);
            }
        }
        if ctx.instance_status(world) == 0 && ctx.instance_alive_monsters(world).is_empty() {
            ctx.spawn_instance_group(world, "q10753_16_instance_halter_1_1");
            ctx.spawn_instance_group(world, "wof_room1");
        }
        let (x, y, z) = if ctx.instance_status(world) < 3 {
            FIRST_ROOM_LOC
        } else {
            THIRD_ROOM_LOC
        };
        ctx.teleport_to(x, y, z);
        let (npc, player) = (ctx.npc, ctx.player);
        ctx.cancel_timer("CHECK_STATUS", npc, player);
        let kain = Self::npc_of(ctx, world, KAIN_VAN_HALTER);
        start_timer(ctx, "CHECK_STATUS", 7000, kain, false);
    }

    /// `CLOSE`: once the player stands within 390 of the Grail, shut the
    /// last door behind them and clear the helpers and room 4.
    fn close(ctx: &mut QuestCtx, world: i32) {
        let player = ctx.player;
        let near_grail = ctx.instance_npc(world, GRAIL).is_some_and(|grail| {
            let pos = |oid| crate::game_loop::space::position::pos_of(ctx.world, oid);
            match (pos(grail), pos(player)) {
                (Some((gx, gy, _)), Some((px, py, _))) => {
                    let (dx, dy) = (f64::from(gx - px), f64::from(gy - py));
                    (dx * dx + dy * dy).sqrt() < 390.0
                }
                _ => false,
            }
        });
        if near_grail {
            ctx.open_close_instance_door(world, DOOR_4, false);
            ctx.despawn_instance_group(world, "q10753_16_instance_halter_1_1");
            ctx.despawn_instance_group(world, "wof_room4");
        } else {
            let npc = ctx.npc;
            start_timer(ctx, "CLOSE", 3000, npc, false);
        }
    }
}

impl QuestScript for ChamberOfProphecies {
    fn id(&self) -> i32 {
        // An `InstanceScript`, not a quest.
        -1
    }
    fn name(&self) -> &'static str {
        "ChamberOfProphecies"
    }
    fn html_dir(&self) -> &'static str {
        "instances/ChamberOfProphecies"
    }
    fn start_npcs(&self) -> &[i32] {
        &[KAIN_VAN_HALTER]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[KAIN_VAN_HALTER, GRAIL, MYSTERIOUS_WIZARD]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[KAIN_VAN_HALTER, GRAIL, MYSTERIOUS_WIZARD]
    }
    fn creature_see_npcs(&self) -> &[i32] {
        &[HELPER_FERIN, HELPER_VAN_HALTER]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let world = Self::world_of_player(ctx);
        let (npc, player) = (ctx.npc, ctx.player);
        match event {
            "31639-01.html" | "33996-01.html" | "33980-01.html" | "33980-02.html" => {
                return Some(event.to_string());
            }
            "33980-03.html" | "33980-04.html" => {
                ctx.send_screen_message_npc_string(
                    THIS_CHOICE_CANNOT_BE_REVERSED,
                    TOP_CENTER,
                    6000,
                );
                return Some(event.to_string());
            }
            "enterInstance" => {
                if ctx.has_other_quest_state(q10753::QUEST_NAME) {
                    ctx.enter_instance(TEMPLATE_ID);
                    if ctx.quest_items_count(PROPHECY_MACHINE) > 0 {
                        ctx.take_items(PROPHECY_MACHINE, 1);
                    }
                    ctx.with_other_quest(q10753::QUEST_NAME, |qs| qs.set_cond(16, true));
                }
                return None;
            }
            "BROADCAST_TEXT" => {
                ctx.npc_set_running(npc);
                ctx.npc_start_follow(npc, player);
                ctx.npc_say_of(npc, THAT_GUY_KAIN_HAS_A_SMARMY_FACE);
                ctx.play_voice("Npcdialog1.apple_quest_7");
                return None;
            }
            "SEY2" => Self::helper_say(
                ctx,
                HELPER_FERIN,
                YOU_CAN_T_DIE_HERE_I_DIDN_T_LEARN_RESURRECT_YET,
                Some("Npcdialog1.apple_quest_4"),
            ),
            "SEY_KAIN" => {
                Self::helper_say(
                    ctx,
                    HELPER_VAN_HALTER,
                    GISELLE_WAS_SUCH_A_SWEET_CHILD,
                    Some("Npcdialog1.holter_quest_1"),
                );
                start_timer(ctx, "ATTACK1", 200, npc, true);
            }
            "SEY3" => Self::helper_say(
                ctx,
                HELPER_FERIN,
                DO_YOU_THINK_I_LL_GROW_TALLER_IF_I_EAT_LOTS_AND_LOTS,
                Some("Npcdialog1.apple_quest_6"),
            ),
            "SEY_KAIN_1" => Self::helper_say(
                ctx,
                HELPER_VAN_HALTER,
                SUCH_MONSTERS_IN_A_PLACE_LIKE_THIS_UNBELIEVABLE,
                None,
            ),
            "SEY_KAIN_2" => {
                Self::helper_say(
                    ctx,
                    HELPER_VAN_HALTER,
                    THAT_S_THE_MONSTER_THAT_ATTACKED_FAERON,
                    Some("Npcdialog1.holter_quest_6"),
                );
                start_timer(ctx, "SEY_KAIN_3", 7000, npc, false);
            }
            "SEY4" => {
                if ctx.npc_id == HELPER_FERIN {
                    ctx.npc_say_of(npc, GO_NOW_KAIN_CAN_HANDLE_THIS);
                    ctx.set_npc_script_value(1);
                }
                start_timer(ctx, "REST", 5000, npc, false);
            }
            "SEY_KAIN_3" => {
                if ctx.npc_id == HELPER_VAN_HALTER {
                    ctx.npc_say_of(npc, LEAVE_THIS_TO_ME_GO);
                    ctx.set_npc_script_value(1);
                }
                start_timer(ctx, "SEY_KAIN_4", 1000, npc, false);
            }
            "REST" => {
                if ctx.npc_id == HELPER_FERIN {
                    ctx.npc_set_active(npc);
                }
                ctx.cancel_timer("BROADCAST_TEXT", npc, player);
            }
            "exit" => {
                start_timer(ctx, "finish", 3000, npc, false);
                ctx.send_sm_int(
                    sm_ids::THIS_DUNGEON_WILL_EXPIRE_IN_S1_MIN_YOU_WILL_BE_FORCED_OUT_OF_THE_DUNGEON_WHEN_THE_TIME_EXPIRES,
                    1,
                );
                ctx.with_other_quest(q10753::QUEST_NAME, |qs| {
                    if qs.has_qs() {
                        qs.set_cond(17, true);
                    }
                });
            }
            // Everything below needs the player inside this instance.
            _ if world == 0 => {}
            "33996-02.html" => {
                ctx.open_close_instance_door(world, DOOR_4, false);
                ctx.instance_show_usm(world, USM_Q015_E);
                ctx.despawn_instance_group(world, "q10753_16_instance_grail");
                ctx.spawn_instance_group(world, "q10753_16_instance_wizard");
                ctx.give_items(ATELIA, 1);
                ctx.send_screen_message_npc_string(TALK_TO_THE_MYSTERIOUS_WIZARD, TOP_CENTER, 6000);
                return Some(event.to_string());
            }
            "33980-05.html" => {
                ctx.spawn_instance_group(world, "q10753_16_instance_halter_2");
                ctx.set_instance_status(world, 6);
                start_timer(ctx, "DESPAWN_WIZARD", 2000, npc, false);
                return Some(event.to_string());
            }
            "teleport" => Self::teleport(ctx, world),
            "status" => {
                let html = if ctx.instance_status(world) < 5 {
                    "31639-01.html"
                } else {
                    "31639-02.html"
                };
                return Some(html.to_string());
            }
            "CHECK_STATUS" => Self::check_status(ctx, world),
            "ATTACK" | "ATTACK1" | "ATTACK2" if npc != 0 => Self::helper_attack(ctx, world),
            "OPEN_DOOR1" => {
                ctx.cancel_timer("ATTACK", npc, player);
                ctx.set_instance_status(world, 1);
                ctx.open_close_instance_door(world, DOOR_2, true);
                ctx.spawn_instance_group(world, "wof_room2");
            }
            "OPEN_DOOR2" => {
                ctx.cancel_timer("ATTACK1", npc, player);
                let van_halter = Self::npc_of(ctx, world, HELPER_VAN_HALTER);
                start_timer(ctx, "ATTACK2", 200, van_halter, true);
                ctx.set_instance_status(world, 3);
                ctx.spawn_instance_group(world, "wof_room3");
                ctx.open_close_instance_door(world, DOOR_3, true);
            }
            "SEY_KAIN_4" => {
                ctx.set_instance_status(world, 5);
                ctx.spawn_instance_group(world, "q10753_16_instance_grail");
                ctx.send_screen_message_npc_string(
                    LEAVE_THIS_PLACE_TO_KAIN_NGO_TO_THE_NEXT_ROOM,
                    TOP_CENTER,
                    6000,
                );
                ctx.open_close_instance_door(world, DOOR_4, true);
                ctx.cancel_timer("ATTACK2", npc, player);
                if npc != 0 {
                    ctx.npc_set_active(npc);
                }
                start_timer(ctx, "CLOSE", 15000, 0, false);
            }
            "CLOSE" => Self::close(ctx, world),
            "DESPAWN_WIZARD" => ctx.despawn_instance_group(world, "q10753_16_instance_wizard"),
            "finish" => ctx.finish_instance(world, 0),
            _ => {}
        }
        None
    }

    /// `startQuestTimer(...)` lands in `onEvent`.
    fn on_timer_event(&self, ctx: &mut QuestCtx, event: &str) {
        self.on_event(ctx, event);
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let on_cond_16 = ctx.has_other_quest_state(q10753::QUEST_NAME)
            && ctx.other_quest_cond(q10753::QUEST_NAME) == 16;
        match ctx.npc_id {
            KAIN_VAN_HALTER if on_cond_16 => Some("31639.html".to_string()),
            GRAIL => Some("33996.html".to_string()),
            MYSTERIOUS_WIZARD if on_cond_16 => Some("33980.html".to_string()),
            _ => None,
        }
    }

    fn on_creature_see(&self, ctx: &mut QuestCtx, creature: i32) {
        let world = ctx.npc_instance();
        let player = ctx.player;
        if world == 0 || player == 0 || player != creature || ctx.is_oid_dead(player) {
            return;
        }
        let npc = ctx.npc;
        match ctx.npc_id {
            HELPER_FERIN if ctx.npc_script_value() == 0 => {
                start_timer(ctx, "BROADCAST_TEXT", 12000, npc, false);
            }
            HELPER_VAN_HALTER if ctx.instance_status(world) == 0 => {
                start_timer(ctx, "ATTACK", 2000, npc, true);
            }
            _ => {}
        }
    }
}
