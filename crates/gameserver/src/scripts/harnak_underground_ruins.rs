//! Harnak Underground Ruins (instance template 195) — port of
//! `dist/game/data/scripts/instances/HarnakUndergroundRuins/`, the instance
//! of the awakening quest ([`super::q10338_seize_your_destiny`]).
//!
//! The world's status walks the rooms:
//!
//! - 1: the first room's eight guards step up one at a time. Hitting any
//!   but the one called up sets 0, and all of them attack; the room clears
//!   when they are all dead. Either way the first door opens.
//! - 2: the second room sends three waves of the player's category monster,
//!   the third with a Source of Power that casts Maximum Defense; the second
//!   door opens behind the last.
//! - 3: Harnak's Wraith, healed by a Source of Power. At half HP (4) two
//!   Seal Control Devices and their guards appear on a 60 s fail clock; the
//!   Wraith's death or both seals released frees Hermuncus (Q10338 cond 3).
//!
//! Java's timers are keyed by name, NPC and player. Here the NPC object id
//! is part of the timer name (`event#npc`), which keeps that key.
//!
//! Three Java quirks are kept. The script registers the Seal Control Device
//! for first talk but has no `onFirstTalk`, so a click shows nothing and
//! `33548.html`'s `activate_seal` is out of reach: only the Wraith's death
//! frees Hermuncus. `seal_say` never advances its counter, so a seal repeats
//! its 50-seconds line every 10 s. And `moveNpcRoom1`'s `timer` has no
//! handler, so it is not started.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::scripts::q10338_seize_your_destiny;

// NPCs
const HADEL: i32 = 33344;
const KRAKIA_BATHUS: i32 = 27437;
const KRAKIA_CARCASS: i32 = 27438;
const KRAKIA_LOTUS: i32 = 27439;
const RAKZAN: i32 = 27440;
const WEISS_KHAN: i32 = 27441;
const BAMONTI: i32 = 27442;
const SEKNUS: i32 = 27443;
const WEISS_ELE: i32 = 27454;
const HARNAKS_WRAITH: i32 = 27445;
const SEAL_CONTROL_DEVICE: i32 = 33548;
const POWER_SOURCE: i32 = 33501;
const POWER_SOURCES: [i32; 3] = [33501, 33556, 33557];
const MONSTERS: [i32; 9] = [
    KRAKIA_BATHUS,
    KRAKIA_CARCASS,
    KRAKIA_LOTUS,
    RAKZAN,
    WEISS_KHAN,
    BAMONTI,
    SEKNUS,
    WEISS_ELE,
    HARNAKS_WRAITH,
];

// Locations
const NPC_ROOM1_LOC: (i32, i32, i32) = (-107930, 206328, -10872);

// Skills
const RELEASE_OF_POWER: (i32, i32) = (14625, 1);
const MAXIMUM_DEFENSE: (i32, i32) = (14700, 1);
const LIGHT_HEAL: (i32, i32) = (14736, 1);
const ULTIMATE_BUFF: (i32, i32) = (4318, 1);

// Misc
const TEMPLATE_ID: i32 = 195;
const ZONE_ROOM_2: i32 = 200032;
const ZONE_ROOM_3: i32 = 200033;
const DOOR_ONE: i32 = 16240100;
const DOOR_TWO: i32 = 16240102;
/// `MOB_CATEGORY`, in `CategoryType` order (Java's `EnumMap`).
const MOB_CATEGORY: [(&str, i32); 8] = [
    ("TANKER_CATEGORY", RAKZAN),
    ("WARRIOR_CATEGORY", KRAKIA_BATHUS),
    ("ROGUE_CATEGORY", BAMONTI),
    ("ARCHER_CATEGORY", KRAKIA_CARCASS),
    ("WIZARD_CATEGORY", WEISS_KHAN),
    ("SUMMONER_CATEGORY", KRAKIA_LOTUS),
    ("HEALER_CATEGORY", WEISS_ELE),
    ("ENCHANTER_CATEGORY", SEKNUS),
];
/// `ExShowScreenMessage.TOP_CENTER`.
const TOP_CENTER: i32 = 2;
// `ChatType` client ids.
const WHISPER: i32 = 2;
const NPC_GENERAL: i32 = 22;
const NPC_SHOUT: i32 = 23;
// `Movie` ids.
const SC_AWAKENING_BOSS_OPENING: i32 = 46;
const SC_AWAKENING_BOSS_ENDING_A: i32 = 47;
const SC_AWAKENING_BOSS_ENDING_B: i32 = 48;

// NpcStringIds
const AN_INTRUDER_INTERESTING: i32 = 10338011;
const PROVE_YOUR_WORTH: i32 = 10338012;
const ONLY_THOSE_STRONG_ENOUGH_SHALL_PROCEED: i32 = 10338013;
const THOUGH_SMALL_THIS_POWER_WILL_HELP_YOU_GREATLY: i32 = 1811214;
const ARE_YOU_STRONG_OR_WEAK_OF_THE_LIGHT_OR_DARKNESS: i32 = 10338031;
const ONLY_THOSE_OF_LIGHT_MAY_PASS_OTHERS_MUST_PROVE_THEIR_STRENGTH: i32 = 10338032;
const COME_ATTACK_ME_IF_YOU_DARE: i32 = 10338015;
const IT_S_THE_END_FOR_YOU_TRAITOR: i32 = 10338017;
const I_WANT_TO_HEAR_YOU_CRY: i32 = 10338019;
const YOU_LL_HAVE_TO_KILL_US_FIRST: i32 = 10338023;
const LETS_SEE_WHAT_YOU_ARE_MADE_OF: i32 = 10338021;
const REPENT_AND_YOUR_DEATH_WILL_BE_QUICK: i32 = 10338025;
const DIE_TRAITOR: i32 = 10338027;
const ARE_YOU_AGAINST_THE_WILL_OF_LIGHT: i32 = 10338014;
const ARE_YOU_PLANNING_TO_BETRAY_THE_GODS_AND_FOLLOW_A_GIANT: i32 = 10338016;
const HAHA: i32 = 10338018;
const MORTAL: i32 = 10338020;
const YOU_WILL_NOT_FREE_HERMUNCUS: i32 = 10338022;
const TRYING_TO_FREE_HERMUNCUS: i32 = 10338024;
const YOU_WILL_NEVER_BREAK_THE_SEAL: i32 = 10338026;
const I_MUST_GO_HELP_SOME_MORE: i32 = 1811222;
const DISABLE_DEVICE_WILL_GO_OUT_OF_CONTROL_IN_1_MINUTE: i32 = 1811226;
const SEAL_COUNTDOWN: [i32; 10] = [
    1811227, // FIFTY_SECONDS_ARE_REMAINING
    1811228, // FOURTY_SECONDS_ARE_REMAINING_2
    1811229, // THIRTY_SECONDS_ARE_REMAINING_2
    1811230, // TWENTY_SECONDS_ARE_REMAINING_2
    1811231, // TEN_SECONDS_ARE_REMAINING_2
    1803409, // FIVE_SECONDS
    1803410, // FOUR_SECONDS
    1803391, // THREE_SECONDS
    1803392, // TWO_SECONDS
    1803393, // ONE_SECOND
];
const I_HERMUNCUS_GIVE_MY_POWER_TO_THOSE_WHO_FIGHT_FOR_ME: i32 = 1811213;
const RECEIVE_THIS_POWER_FORM_THE_ANCIENT_GIANT: i32 = 10338036;
const USE_THIS_NEW_POWER_WHEN_THE_TIME_IS_RIGHT: i32 = 10338037;
const FREE_ME_FROM_THIS_BINDING_OF_LIGHT: i32 = 10338006;
const DESTROY_THE_GHOST_OF_HARNAK_THIS_CORRUPTED_CREATURE: i32 = 10338007;
const FREE_ME_AND_I_PROMISE_YOU_THE_POWER_OF_GIANTS: i32 = 10338008;
const REMAINING_TIME: i32 = 1811302;
const NO_THE_SEAL_CONTROLS_HAVE_BEEN_EXPOSED: i32 = 1811223;

pub struct HarnakUndergroundRuins;

/// `startQuestTimer(event, delay, npc, player)`, `npc` 0 for Java's `null`.
fn timer(ctx: &mut QuestCtx, event: &str, delay_ms: u64, npc: i32) {
    ctx.start_quest_timer_on(&format!("{event}#{npc}"), delay_ms, npc);
}

/// `cancelQuestTimer("fail_instance", null, player)`.
fn cancel_fail_timer(ctx: &mut QuestCtx) {
    ctx.cancel_quest_timer("fail_instance#0");
}

fn screen_message(ctx: &QuestCtx, npc_string_id: i32, time: i32) {
    ctx.send_screen_message_npc_string(npc_string_id, TOP_CENTER, time);
}

/// `addAttackPlayerDesire(npc, player)` on every NPC of the list.
fn attack_player(ctx: &mut QuestCtx, npcs: &[i32]) {
    let player = ctx.player;
    for &npc in npcs {
        ctx.seed_npc_attack(npc, player);
    }
}

/// `npc.calculateDistance3D(player) < skill.getCastRange()`.
fn in_cast_range(ctx: &QuestCtx, npc: i32, (skill_id, level): (i32, i32)) -> bool {
    let Some(range) = ctx
        .world
        .data
        .skill_data
        .get(skill_id, level)
        .map(|s| s.cast_range)
    else {
        return false;
    };
    crate::geo::distance::distance_3d(ctx.world, npc, ctx.player)
        .is_some_and(|d| d < f64::from(range))
}

fn hp_percent(ctx: &QuestCtx) -> f64 {
    ctx.npc_hp_ratio() * 100.0
}

/// `moveNpcRoom1`: the next guard walks up and calls out.
fn move_npc_room1(ctx: &mut QuestCtx, world: i32, npc_id: i32, message: i32) {
    let Some(npc) = ctx.instance_npc(world, npc_id) else {
        return;
    };
    ctx.npc_move_to(npc, NPC_ROOM1_LOC);
    ctx.npc_say_typed(npc, NPC_GENERAL, message);
    ctx.set_instance_param(world, "currentNpc", i64::from(npc_id));
}

/// The Wraith fell or both seals were released: Hermuncus comes.
fn free_hermuncus(ctx: &mut QuestCtx, world: i32) {
    cancel_fail_timer(ctx);
    ctx.remove_instance_npcs(world);
    let (npc, player) = (ctx.npc, ctx.player);
    ctx.play_movie(player, SC_AWAKENING_BOSS_ENDING_A);
    timer(ctx, "spawn_hermuncus", 25050, npc);
}

impl HarnakUndergroundRuins {
    /// `onEvent`, for bypasses and timers alike.
    fn event(ctx: &mut QuestCtx, event: &str) {
        let (npc, player) = (ctx.npc, ctx.player);
        let world = ctx.player_instance();
        match event {
            "enter_instance" => {
                let first_enter = ctx.world.instances.allowed_instance_of(player).is_none();
                if ctx.enter_instance(TEMPLATE_ID).is_some() && first_enter {
                    // `onEnter(player, instance, true)`.
                    timer(ctx, "fail_instance", 1_260_000, 0);
                    timer(ctx, "message1", 2500, 0);
                    timer(ctx, "message2", 5000, 0);
                    timer(ctx, "message3", 8500, 0);
                    timer(ctx, "spawn_npc1", 10000, 0);
                }
            }
            "message1" => screen_message(ctx, AN_INTRUDER_INTERESTING, 5000),
            "message2" => screen_message(ctx, PROVE_YOUR_WORTH, 5000),
            "message3" => screen_message(ctx, ONLY_THOSE_STRONG_ENOUGH_SHALL_PROCEED, 5000),
            "message4" => screen_message(ctx, THOUGH_SMALL_THIS_POWER_WILL_HELP_YOU_GREATLY, 5000),
            "message5" => {
                screen_message(ctx, ARE_YOU_STRONG_OR_WEAK_OF_THE_LIGHT_OR_DARKNESS, 5000);
            }
            "message6" => screen_message(
                ctx,
                ONLY_THOSE_OF_LIGHT_MAY_PASS_OTHERS_MUST_PROVE_THEIR_STRENGTH,
                5000,
            ),
            "razkan_say" => ctx.npc_say_typed(npc, NPC_GENERAL, COME_ATTACK_ME_IF_YOU_DARE),
            "bathus_say" => ctx.npc_say_typed(npc, NPC_GENERAL, IT_S_THE_END_FOR_YOU_TRAITOR),
            "bamonti_say" | "carcass_say" => {
                ctx.npc_say_typed(npc, NPC_GENERAL, I_WANT_TO_HEAR_YOU_CRY);
            }
            "khan_say" => ctx.npc_say_typed(npc, NPC_GENERAL, YOU_LL_HAVE_TO_KILL_US_FIRST),
            "seknus_say" => ctx.npc_say_typed(npc, NPC_GENERAL, LETS_SEE_WHAT_YOU_ARE_MADE_OF),
            "lotus_say" => {
                ctx.npc_say_typed(npc, NPC_GENERAL, REPENT_AND_YOUR_DEATH_WILL_BE_QUICK);
            }
            "ele_say" => ctx.npc_say_typed(npc, NPC_GENERAL, DIE_TRAITOR),
            "cast_release_power" => {
                let (skill, level) = RELEASE_OF_POWER;
                ctx.npc_cast(npc, player, skill, level);
            }
            "whisper_to_player" => Self::whisper_to_player(ctx, npc),
            "spawn_wave3" => {
                screen_message(ctx, I_MUST_GO_HELP_SOME_MORE, 5000);
                if world == 0 {
                    return;
                }
                let wave_npc = ctx.instance_param(world, "waveNpcId");
                let npcs =
                    ctx.spawn_instance_group(world, &format!("second_room_wave_3_{wave_npc}"));
                attack_player(ctx, &npcs);
                for source in ctx.spawn_instance_group(world, "power_source") {
                    timer(ctx, "cast_defense_maximum", 1, source);
                }
                ctx.set_instance_param(world, "wave", 3);
            }
            // Every other event works on the player's instance.
            _ if world == 0 => {}
            "spawn_npc1" => {
                ctx.set_instance_status(world, 1);
                ctx.spawn_instance_group(world, "first_room");
                move_npc_room1(ctx, world, RAKZAN, ARE_YOU_AGAINST_THE_WILL_OF_LIGHT);
            }
            "spawn_npc2" => {
                ctx.open_close_instance_door(world, DOOR_ONE, true);
                ctx.spawn_instance_group(world, "power_sources");
            }
            "spawn_npc3" => {
                let status = ctx.instance_status(world);
                ctx.set_instance_status(world, status + 1);
                let npcs = ctx.spawn_instance_group(world, "third_room");
                if let Some(&source) = npcs.iter().find(|&&n| {
                    crate::game_loop::npc::npc_id_of(ctx.world, n) == Some(POWER_SOURCE)
                }) {
                    timer(ctx, "cast_light_heal", 3000, source);
                }
            }
            "spawn_wave1" => {
                if let Some(&(_, wave_npc)) = MOB_CATEGORY
                    .iter()
                    .find(|(category, _)| ctx.is_in_category(category))
                {
                    ctx.set_instance_status(world, 2);
                    ctx.set_instance_param(world, "wave", 1);
                    ctx.set_instance_param(world, "waveNpcId", i64::from(wave_npc));
                    let npcs =
                        ctx.spawn_instance_group(world, &format!("second_room_wave_1_{wave_npc}"));
                    attack_player(ctx, &npcs);
                }
            }
            "spawn_wave2" => {
                let wave_npc = ctx.instance_param(world, "waveNpcId");
                let npcs =
                    ctx.spawn_instance_group(world, &format!("second_room_wave_2_{wave_npc}"));
                attack_player(ctx, &npcs);
                ctx.set_instance_param(world, "wave", 2);
            }
            "cast_defense_maximum" => {
                if in_cast_range(ctx, npc, MAXIMUM_DEFENSE) {
                    let (skill, level) = MAXIMUM_DEFENSE;
                    ctx.npc_cast(npc, player, skill, level);
                    let counter = ctx.instance_param(world, "defenseCounter") + 1;
                    ctx.set_instance_param(world, "defenseCounter", counter);
                    if counter < 3 {
                        timer(ctx, "cast_defense_maximum", 60000, npc);
                    } else {
                        ctx.delete_npc_of(npc);
                    }
                } else {
                    timer(ctx, "cast_defense_maximum", 1, npc);
                }
            }
            "cast_light_heal" => {
                if npc != 0 && matches!(ctx.instance_status(world), 3 | 4) {
                    if in_cast_range(ctx, npc, LIGHT_HEAL) {
                        let (skill, level) = LIGHT_HEAL;
                        ctx.npc_cast(npc, player, skill, level);
                    }
                    timer(ctx, "cast_light_heal", 3000, npc);
                }
            }
            "fail_instance" => {
                ctx.remove_instance_npcs(world);
                ctx.play_movie(player, SC_AWAKENING_BOSS_ENDING_B);
                timer(ctx, "exit", 13500, npc);
            }
            "exit" => ctx.finish_instance(world, 0),
            "spawn_npc4" => {
                let wave_npc = ctx.instance_param(world, "waveNpcId");
                let npcs = ctx.spawn_instance_group(world, &format!("third_room_{wave_npc}"));
                attack_player(ctx, &npcs);
                for seal in ctx.spawn_instance_group(world, "seal") {
                    ctx.npc_say_typed(
                        seal,
                        NPC_GENERAL,
                        DISABLE_DEVICE_WILL_GO_OUT_OF_CONTROL_IN_1_MINUTE,
                    );
                    timer(ctx, "seal_say", 10000, seal);
                }
            }
            "activate_seal" => {
                if ctx.npc_script_value() != 0 {
                    return;
                }
                ctx.set_npc_script_value(1);
                let enabled = ctx.instance_param(world, "enabledSeal") + 1;
                ctx.set_instance_param(world, "enabledSeal", enabled);
                if enabled == 2 {
                    ctx.with_player_quest(player, q10338_seize_your_destiny::QUEST_NAME, |qs| {
                        if qs.is_cond(2) {
                            qs.set_cond(3, true);
                        }
                    });
                    free_hermuncus(ctx, world);
                }
            }
            "seal_say" => {
                let count = ctx.instance_param(world, "timerCount");
                if let Some(&line) = usize::try_from(count)
                    .ok()
                    .and_then(|i| SEAL_COUNTDOWN.get(i))
                {
                    ctx.npc_say_typed(npc, NPC_SHOUT, line);
                }
                if count <= 4 {
                    timer(ctx, "seal_say", 10000, npc);
                } else if count <= 9 {
                    timer(ctx, "seal_say", 1000, npc);
                }
                ctx.set_instance_param(world, "timerCount", count);
            }
            "spawn_hermuncus" => {
                ctx.spawn_instance_group(world, "hermuncus");
            }
            _ => {}
        }
    }

    fn whisper_to_player(ctx: &mut QuestCtx, npc: i32) {
        screen_message(
            ctx,
            I_HERMUNCUS_GIVE_MY_POWER_TO_THOSE_WHO_FIGHT_FOR_ME,
            5000,
        );
        ctx.npc_say_typed(npc, WHISPER, RECEIVE_THIS_POWER_FORM_THE_ANCIENT_GIANT);
        ctx.npc_say_typed(npc, WHISPER, USE_THIS_NEW_POWER_WHEN_THE_TIME_IS_RIGHT);
        timer(ctx, "message4", 3000, npc);
    }
}

impl QuestScript for HarnakUndergroundRuins {
    fn id(&self) -> i32 {
        // An `InstanceScript`, not a quest.
        -1
    }
    fn name(&self) -> &'static str {
        "HarnakUndergroundRuins"
    }
    fn html_dir(&self) -> &'static str {
        "instances/HarnakUndergroundRuins"
    }
    fn start_npcs(&self) -> &[i32] {
        &[HADEL]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[HADEL]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[SEAL_CONTROL_DEVICE]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn attack_npcs(&self) -> &[i32] {
        &MONSTERS
    }
    fn creature_see_npcs(&self) -> &[i32] {
        &POWER_SOURCES
    }
    fn enter_zone_ids(&self) -> &[i32] {
        &[ZONE_ROOM_2, ZONE_ROOM_3]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        Self::event(ctx, event);
        None
    }

    fn on_timer(&self, ctx: &mut QuestCtx, name: &str) {
        let event = name.split_once('#').map_or(name, |(event, _)| event);
        Self::event(ctx, event);
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        let world = ctx.player_instance();
        if world == 0 {
            return;
        }
        let npc = ctx.npc;
        match ctx.instance_status(world) {
            0 => {
                if ctx.instance_all_alive_npcs(world).is_empty() {
                    timer(ctx, "spawn_npc2", 100, npc);
                    ctx.set_instance_status(world, 1);
                }
            }
            1 => {
                let next = match ctx.npc_id {
                    RAKZAN => Some((
                        KRAKIA_BATHUS,
                        ARE_YOU_PLANNING_TO_BETRAY_THE_GODS_AND_FOLLOW_A_GIANT,
                    )),
                    KRAKIA_BATHUS => Some((BAMONTI, HAHA)),
                    BAMONTI => Some((KRAKIA_CARCASS, HAHA)),
                    KRAKIA_CARCASS => Some((WEISS_KHAN, YOU_WILL_NOT_FREE_HERMUNCUS)),
                    WEISS_KHAN => Some((SEKNUS, MORTAL)),
                    SEKNUS => Some((KRAKIA_LOTUS, TRYING_TO_FREE_HERMUNCUS)),
                    KRAKIA_LOTUS => Some((WEISS_ELE, YOU_WILL_NEVER_BREAK_THE_SEAL)),
                    _ => None,
                };
                match next {
                    Some((next_npc, message)) => move_npc_room1(ctx, world, next_npc, message),
                    None if ctx.npc_id == WEISS_ELE => timer(ctx, "spawn_npc2", 100, npc),
                    None => {}
                }
            }
            2 => {
                let wave_npc = ctx.instance_param(world, "waveNpcId") as i32;
                if ctx.instance_alive_npcs(world, wave_npc).is_empty() {
                    match ctx.instance_param(world, "wave") {
                        1 => timer(ctx, "spawn_wave2", 100, npc),
                        2 => timer(ctx, "spawn_wave3", 100, npc),
                        3 => ctx.open_close_instance_door(world, DOOR_TWO, true),
                        _ => {}
                    }
                }
            }
            _ => {
                if ctx.npc_id == HARNAKS_WRAITH {
                    free_hermuncus(ctx, world);
                }
            }
        }
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        let world = ctx.player_instance();
        if world == 0 {
            return;
        }
        let (npc, player) = (ctx.npc, ctx.player);
        match ctx.instance_status(world) {
            1 => {
                if i64::from(ctx.npc_id) != ctx.instance_param(world, "currentNpc") {
                    ctx.set_instance_status(world, 0);
                    let npcs = ctx.instance_all_alive_npcs(world);
                    attack_player(ctx, &npcs);
                }
            }
            2 => {
                if hp_percent(ctx) < 80.0 {
                    let (skill, level) = ULTIMATE_BUFF;
                    ctx.npc_cast(npc, player, skill, level);
                }
            }
            3 if ctx.npc_id == HARNAKS_WRAITH => {
                let hp = hp_percent(ctx);
                if ctx.npc_var_int("message1") == 0 && hp > 80.0 {
                    screen_message(ctx, FREE_ME_FROM_THIS_BINDING_OF_LIGHT, 5000);
                    ctx.set_npc_var_int("message1", 1);
                } else if ctx.npc_var_int("message2") == 0 && hp <= 80.0 {
                    screen_message(
                        ctx,
                        DESTROY_THE_GHOST_OF_HARNAK_THIS_CORRUPTED_CREATURE,
                        5000,
                    );
                    ctx.set_npc_var_int("message2", 1);
                } else if ctx.npc_var_int("message3") == 0 && hp <= 60.0 {
                    screen_message(ctx, FREE_ME_AND_I_PROMISE_YOU_THE_POWER_OF_GIANTS, 5000);
                    ctx.set_npc_var_int("message3", 1);
                } else if hp <= 50.0 {
                    ctx.set_instance_status(world, 4);
                    ctx.send_ui_event(false, false, 60, 0, REMAINING_TIME);
                    screen_message(ctx, NO_THE_SEAL_CONTROLS_HAVE_BEEN_EXPOSED, 10000);
                    timer(ctx, "spawn_npc4", 1, npc);
                    cancel_fail_timer(ctx);
                    timer(ctx, "fail_instance", 60000, 0);
                }
            }
            _ => {}
        }
    }

    fn on_creature_see(&self, ctx: &mut QuestCtx, _creature: i32) {
        if ctx.player == 0 {
            return;
        }
        let npc = ctx.npc;
        timer(ctx, "cast_release_power", 2000, npc);
        if ctx.npc_id == POWER_SOURCE {
            timer(ctx, "whisper_to_player", 2000, npc);
        }
    }

    fn on_enter_zone(&self, ctx: &mut QuestCtx, _creature: i32, zone_id: i32) {
        if ctx.player == 0 {
            return;
        }
        let world = ctx.player_instance();
        if world == 0 {
            return;
        }
        match zone_id {
            ZONE_ROOM_2 if ctx.instance_status(world) == 1 => {
                ctx.set_instance_status(world, 2);
                timer(ctx, "message2", 100, 0);
                timer(ctx, "message5", 2600, 0);
                timer(ctx, "message6", 5100, 0);
                timer(ctx, "spawn_wave1", 5100, 0);
            }
            ZONE_ROOM_3 if ctx.instance_param(world, "openingPlayed") == 0 => {
                ctx.set_instance_param(world, "openingPlayed", 1);
                timer(ctx, "spawn_npc3", 29950, 0);
                let player = ctx.player;
                ctx.play_movie(player, SC_AWAKENING_BOSS_OPENING);
            }
            _ => {}
        }
    }
}
