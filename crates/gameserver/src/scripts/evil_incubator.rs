//! Evil Incubator (instance template 185) — port of
//! `dist/game/data/scripts/instances/EvilIncubator/`, the instance of the
//! third class transfer ([`super::third_class_transfer_quest`]).
//!
//! The script works on the player's race quest (`RACE_QUESTS`), whose cond
//! it moves from 5 to 13:
//!
//! - 5 → 6: Adolph's briefing. 6 → 7: the player picks two of Adolph's
//!   officers, who move to the battle line; the officers not picked leave.
//! - 7 → 8: Adolph and the player take the line, and the waves begin. A wave
//!   follows 5 s after the last of its monsters falls; after wave 6 the
//!   attack stops (9).
//! - 9 → 10: Adolph gives the class's Cry of Destiny pendant. 10 → 11: the
//!   fight resumes with waves 8–15 every 20 s, the last bringing the class's
//!   Death Wound. Its death makes 12, and Adolph's Scroll of Escape 13.
//!
//! Every wave and kill sends the friendly NPCs around the player at the
//! monsters around them.
//!
//! Java's wave timers carry no NPC or player, so every Evil Incubator world
//! shares one `SPAWN_WAVE` and one `WORLD_ATTACK` key, and a second world's
//! timer cancels the first's. Here the world id is part of the event, so each
//! world keeps its own.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::scripts::third_class_transfer_quest;

// NPCs
const VANGUARD_MEMBER: i32 = 33165;
const ADOLPH: i32 = 33170;
const ALICE: i32 = 33171;
const BARTON: i32 = 33172;
const HAYUK: i32 = 33173;
const ELIYAH: i32 = 33174;
const MONSTERS: [i32; 5] = [
    27431, // Slayer
    27432, // Tracker
    27433, // Priest of Darkness
    27434, // Guardian of Darkness
    27430, // Screaming Shaman
];
const BOSSES: [i32; 5] = [
    27425, // Death Wound
    27426, // Death Wound
    27427, // Death Wound
    27428, // Death Wound
    27429, // Death Wound
];
const HELPERS: [i32; 6] = [
    33170, // Adolph
    // 33171, // Priest Alice
    33172, // Officer Barton
    33173, // Sniper Hayuk
    33174, // Magus Eliyah
    33414, // Van Archer
    33415, // Van Infantry
];
const DEATH_NPCS: [i32; 10] = [
    27431, 27432, 27433, 27434, 27430, 27425, 27426, 27427, 27428, 27429,
];

// Items
const SOE: i32 = 736; // Scroll of Escape

// Locations
const PLAYER_BATTLE_LOC: (i32, i32, i32) = (56167, -175615, -7944);
const BATTLE_LOC_2: (i32, i32, i32) = (56005, -175536, -7952);
const BATTLE_LOC_3: (i32, i32, i32) = (56325, -175536, -7952);
const BOSS_LOC: (i32, i32, i32, i32) = (56165, -177550, -7944, 0);

// Misc
const TEMPLATE_ID: i32 = 185;
/// `ExShowScreenMessage.TOP_CENTER`.
const TOP_CENTER: i32 = 2;

// NpcStringIds
const CREATURES_RESURRECTED_DEFEND_YOURSELF: i32 = 1034105;
const CREATURES_HAVE_STOPPED_THEIR_ATTACK: i32 = 1034113;
const I_DEATH_WOUND_CHAMPION_OF_SHILEN: i32 = 1034116;
const THE_CRY_OF_DESTINY_PENDANT_WILL_BE_HELPFUL: i32 = 1034124;

/// `CLASS_BOSS`: the Death Wound each class fights.
const CLASS_BOSS: [(i32, i32); 35] = [
    (2, 27426),   // GLADIATOR
    (3, 27426),   // WARLORD
    (5, 27425),   // PALADIN
    (6, 27425),   // DARK_AVENGER
    (8, 27426),   // TREASURE_HUNTER
    (9, 27428),   // HAWKEYE
    (12, 27428),  // SORCERER
    (13, 27428),  // NECROMANCER
    (14, 27428),  // WARLOCK
    (16, 27427),  // BISHOP
    (17, 27429),  // PROPHET
    (20, 27425),  // TEMPLE_KNIGHT
    (21, 27429),  // SWORDSINGER
    (23, 27426),  // PLAINS_WALKER
    (24, 27428),  // SILVER_RANGER
    (27, 27428),  // SPELLSINGER
    (28, 27428),  // ELEMENTAL_SUMMONER
    (30, 27427),  // ELDER
    (33, 27425),  // SHILLIEN_KNIGHT
    (34, 27429),  // BLADEDANCER
    (36, 27426),  // ABYSS_WALKER
    (37, 27428),  // PHANTOM_RANGER
    (40, 27428),  // SPELLHOWLER
    (41, 27428),  // PHANTOM_SUMMONER
    (43, 27427),  // SHILLIEN_ELDER
    (46, 27426),  // DESTROYER
    (48, 27426),  // TYRANT
    (51, 27429),  // OVERLORD
    (52, 27429),  // WARCRYER
    (55, 27426),  // BOUNTY_HUNTER
    (57, 27426),  // WARSMITH
    (127, 27426), // BERSERKER
    (128, 27426), // MALE_SOULBREAKER
    (129, 27426), // FEMALE_SOULBREAKER
    (130, 27428), // ARBALESTER
];

/// `CLASS_ITEMS`: each class's Cry of Destiny pendant.
const CLASS_ITEMS: [(i32, i32); 35] = [
    (2, 17484),   // GLADIATOR
    (3, 17485),   // WARLORD
    (5, 17486),   // PALADIN
    (6, 17487),   // DARK_AVENGER
    (8, 17488),   // TREASURE_HUNTER
    (9, 17489),   // HAWKEYE
    (12, 17490),  // SORCERER
    (13, 17491),  // NECROMANCER
    (14, 17492),  // WARLOCK
    (16, 17493),  // BISHOP
    (17, 17494),  // PROPHET
    (20, 17495),  // TEMPLE_KNIGHT
    (21, 17496),  // SWORDSINGER
    (23, 17497),  // PLAINS_WALKER
    (24, 17498),  // SILVER_RANGER
    (27, 17499),  // SPELLSINGER
    (28, 17500),  // ELEMENTAL_SUMMONER
    (30, 17501),  // ELDER
    (33, 17502),  // SHILLIEN_KNIGHT
    (34, 17503),  // BLADEDANCER
    (36, 17504),  // ABYSS_WALKER
    (37, 17505),  // PHANTOM_RANGER
    (40, 17506),  // SPELLHOWLER
    (41, 17507),  // PHANTOM_SUMMONER
    (43, 17508),  // SHILLIEN_ELDER
    (46, 17509),  // DESTROYER
    (48, 17510),  // TYRANT
    (51, 17511),  // OVERLORD
    (52, 17512),  // WARCRYER
    (55, 17513),  // BOUNTY_HUNTER
    (57, 17514),  // WARSMITH
    (127, 17515), // BERSERKER
    (128, 17516), // MALE_SOULBREAKER
    (129, 17516), // FEMALE_SOULBREAKER
    (130, 17517), // ARBALESTER
];

const SPAWN_WAVE: &str = "SPAWN_WAVE";
const WORLD_ATTACK: &str = "WORLD_ATTACK";

pub struct EvilIncubator;

fn lookup(table: &[(i32, i32)], class_id: i32) -> Option<i32> {
    table.iter().find(|&&(c, _)| c == class_id).map(|&(_, v)| v)
}

/// `getQuestState(player)`: the player's race quest, when they have a
/// state for it.
fn race_quest(ctx: &QuestCtx, player: i32) -> Option<&'static str> {
    let race = if player == ctx.player {
        ctx.player_race()
    } else {
        ctx.world
            .objects
            .get_component::<crate::model::Player>(&player)?
            .race
    };
    let name = third_class_transfer_quest::quest_name_for_race(race)?;
    ctx.world
        .objects
        .get_component::<crate::model::components::social::Quests>(&player)
        .is_some_and(|q| q.0.contains_key(name))
        .then_some(name)
}

/// The race quest's cond while STARTED, `None` otherwise.
fn started_cond(ctx: &mut QuestCtx, player: i32) -> Option<(&'static str, i32)> {
    let name = race_quest(ctx, player)?;
    let cond = ctx.with_player_quest(player, name, |qs| qs.is_started().then(|| qs.cond()))??;
    Some((name, cond))
}

fn set_cond(ctx: &mut QuestCtx, player: i32, name: &str, cond: i32, play_middle: bool) {
    ctx.with_player_quest(player, name, |qs| qs.set_cond(cond, play_middle));
}

/// A wave timer's event: the name and the world it belongs to.
fn timer_event(name: &str, world: i32) -> String {
    format!("{name} {world}")
}

/// `WORLD_WAVE`, which Java reads with a default of 1.
fn wave(ctx: &QuestCtx, world: i32) -> i64 {
    match ctx.instance_param(world, "WORLD_WAVE") {
        0 => 1,
        w => w,
    }
}

impl EvilIncubator {
    fn instance_event(
        ctx: &mut QuestCtx,
        world: i32,
        quest: &'static str,
        cond: i32,
        event: &str,
    ) -> Option<String> {
        let (npc, player) = (ctx.npc, ctx.player);
        match event {
            "33170-02.html" if cond == 5 => {
                set_cond(ctx, player, quest, 6, true);
                return Some(event.to_string());
            }
            "selectHelper" if cond == 6 => {
                let mut helpers = ctx.instance_param(world, "HELPER_COUNT");
                if helpers < 2 {
                    helpers += 1;
                    ctx.set_instance_param(world, "HELPER_COUNT", helpers);
                    let loc = if helpers == 1 {
                        BATTLE_LOC_2
                    } else {
                        BATTLE_LOC_3
                    };
                    ctx.npc_teleport(npc, loc);
                    if helpers == 2 {
                        set_cond(ctx, player, quest, 7, true);
                        let adolph = ctx.instance_npc(world, ADOLPH)?;
                        for other in ctx.visible_npcs(adolph, Some(1000)) {
                            if ctx.npc_is_friendly(other) {
                                ctx.delete_npc_of(other);
                            }
                        }
                    }
                }
            }
            "teamSelected" if cond == 7 => {
                let helpers = ctx.instance_param(world, "HELPER_COUNT");
                if helpers == 2 {
                    ctx.set_instance_param(world, "HELPER_COUNT", helpers + 1);
                    set_cond(ctx, player, quest, 8, true);
                    ctx.npc_teleport(npc, PLAYER_BATTLE_LOC);
                    let (x, y, z) = PLAYER_BATTLE_LOC;
                    ctx.teleport_to(x, y, z);
                    ctx.add_timer(&timer_event(SPAWN_WAVE, world), 5000, 0, 0);
                }
            }
            "giveItem" if cond == 9 => {
                set_cond(ctx, player, quest, 10, true);
                if let Some(item) = lookup(&CLASS_ITEMS, ctx.player_class_id())
                    && ctx.quest_items_count(item) == 0
                {
                    ctx.give_items(item, 1);
                }
                ctx.npc_say(THE_CRY_OF_DESTINY_PENDANT_WILL_BE_HELPFUL);
                return Some("33170-06.html".to_string());
            }
            "resumeFight" if cond == 10 => {
                set_cond(ctx, player, quest, 11, true);
                Self::manage_wave_spawn(ctx, world);
            }
            _ => {}
        }
        None
    }

    /// `manageWaveSpawn`.
    fn manage_wave_spawn(ctx: &mut QuestCtx, world: i32) {
        let Some(first) = ctx.instance_first_player(world) else {
            return;
        };
        let Some((quest, _)) = started_cond(ctx, first) else {
            return;
        };
        let wave_id = wave(ctx, world);
        let spawned = match wave_id {
            1..=6 => {
                let spawned = ctx.spawn_instance_group(world, &format!("wave_{wave_id}"));
                ctx.set_instance_param(world, "WORLD_WAVE", wave_id + 1);
                Self::screen_message(ctx, world, CREATURES_RESURRECTED_DEFEND_YOURSELF);
                spawned
            }
            7 => {
                ctx.set_instance_param(world, "WORLD_WAVE", wave_id + 1);
                Self::screen_message(ctx, world, CREATURES_HAVE_STOPPED_THEIR_ATTACK);
                set_cond(ctx, first, quest, 9, true);
                Vec::new()
            }
            8..=14 => {
                let spawned = ctx.spawn_instance_group(world, &format!("wave_{wave_id}"));
                ctx.set_instance_param(world, "WORLD_WAVE", wave_id + 1);
                Self::screen_message(ctx, world, CREATURES_RESURRECTED_DEFEND_YOURSELF);
                ctx.add_timer(&timer_event(SPAWN_WAVE, world), 20000, 0, 0);
                spawned
            }
            15 => {
                let spawned = ctx.spawn_instance_group(world, &format!("wave_{wave_id}"));
                ctx.set_instance_param(world, "WORLD_WAVE", wave_id + 1);
                Self::screen_message(ctx, world, I_DEATH_WOUND_CHAMPION_OF_SHILEN);
                ctx.add_timer(&timer_event(SPAWN_WAVE, world), 20000, 0, 0);
                let class_id = ctx
                    .world
                    .objects
                    .get_component::<crate::model::Player>(&first)
                    .map_or(-1, |p| p.class_id);
                // `CLASS_BOSS.get(...).intValue()` throws for a class off the
                // table, which ends the method here.
                let Some(boss_id) = lookup(&CLASS_BOSS, class_id) else {
                    return;
                };
                if let Some(boss) = ctx.spawn_in_instance(world, boss_id, BOSS_LOC) {
                    ctx.instance_earthquake(world, boss, 50, 10);
                }
                spawned
            }
            _ => Vec::new(),
        };
        Self::world_attack(ctx, world, &spawned);
    }

    /// `managerWorldAttack`: each new monster draws a random helper from the
    /// friendly NPCs around the player, and every helper left over goes for
    /// the monsters around it.
    fn world_attack(ctx: &mut QuestCtx, world: i32, spawned: &[i32]) {
        let mut helpers: Vec<i32> = match ctx.instance_first_player(world) {
            Some(first) => ctx
                .visible_npcs(first, None)
                .into_iter()
                .filter(|&n| ctx.npc_is_friendly(n))
                .collect(),
            None => Vec::new(),
        };
        for &npc in spawned {
            if helpers.is_empty() {
                continue;
            }
            let pick = ctx.roll(helpers.len() as i32) as usize;
            let helper = helpers[pick];
            let helper_id = crate::game_loop::npc::npc_id_of(ctx.world, helper).unwrap_or(0);
            if HELPERS.contains(&helper_id) {
                ctx.npc_reduce_current_hp(npc, 1.0, helper);
                ctx.npc_reduce_current_hp(helper, 1.0, npc);
                ctx.npc_attack_desire(helper, npc);
                helpers.remove(pick);
            }
        }
        for helper in helpers {
            for monster in ctx.visible_npcs(helper, None) {
                if ctx.npc_is_attackable_class(monster) && !ctx.npc_is_friendly(monster) {
                    ctx.npc_attack_desire(helper, monster);
                }
            }
        }
    }

    /// `showOnScreenMsg(world, id, TOP_CENTER, 5000, true)`.
    fn screen_message(ctx: &QuestCtx, world: i32, npc_string_id: i32) {
        ctx.instance_screen_message_npc_string(world, npc_string_id, TOP_CENTER, 5000, true);
    }
}

impl QuestScript for EvilIncubator {
    fn id(&self) -> i32 {
        // An `InstanceScript`, not a quest.
        -1
    }
    fn name(&self) -> &'static str {
        "EvilIncubator"
    }
    fn html_dir(&self) -> &'static str {
        "instances/EvilIncubator"
    }
    fn start_npcs(&self) -> &[i32] {
        &[VANGUARD_MEMBER]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[VANGUARD_MEMBER, ADOLPH, ALICE, BARTON, HAYUK, ELIYAH]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[ADOLPH, ALICE, BARTON, HAYUK, ELIYAH]
    }
    fn creature_death_npcs(&self) -> &[i32] {
        &DEATH_NPCS
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let player = ctx.player;
        let (quest, cond) = started_cond(ctx, player)?;
        if event == "enterInstance" {
            ctx.enter_instance(TEMPLATE_ID);
            set_cond(ctx, player, quest, 5, false);
            return Some("33165-01.html".to_string());
        }
        let world = ctx.npc_instance();
        if world == 0 {
            return None;
        }
        Self::instance_event(ctx, world, quest, cond, event)
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let player = ctx.player;
        let (quest, cond) = started_cond(ctx, player)?;
        let npc = ctx.npc_id;
        let html = match npc {
            ADOLPH => match cond {
                5 => "33170-01.html",
                6 => "33170-02.html",
                7 => "33170-03.html",
                9 => "33170-05.html",
                10 => "33170-07.html",
                12 => {
                    set_cond(ctx, player, quest, 13, true);
                    ctx.give_items(SOE, 1);
                    "33170-08.html"
                }
                13 => "33170-08.html",
                _ => return Some(ctx.no_quest_html()),
            },
            ALICE | BARTON | ELIYAH | HAYUK => match cond {
                5 => return Some(format!("{npc}-01.html")),
                6 => return Some(format!("{npc}-02.html")),
                _ => return Some(ctx.no_quest_html()),
            },
            _ => return Some(ctx.no_quest_html()),
        };
        Some(html.to_string())
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let player = ctx.player;
        let (_, cond) = started_cond(ctx, player)?;
        let npc = ctx.npc_id;
        match npc {
            ADOLPH => Some(
                if matches!(cond, 5 | 6 | 7 | 9 | 10 | 12 | 13) {
                    "33170.html"
                } else {
                    "33170-04.html"
                }
                .to_string(),
            ),
            ALICE | BARTON | ELIYAH | HAYUK => Some(if cond > 7 {
                format!("{npc}-03.html")
            } else {
                format!("{npc}.html")
            }),
            _ => None,
        }
    }

    /// `onCreatureKill` on a wave monster or a Death Wound.
    fn on_creature_death(&self, ctx: &mut QuestCtx, _killer: i32) {
        let world = ctx.npc_instance();
        if world == 0 {
            return;
        }
        let wave_id = wave(ctx, world);
        if BOSSES.contains(&ctx.npc_id) {
            let Some(first) = ctx.instance_first_player(world) else {
                return;
            };
            let Some((quest, _)) = started_cond(ctx, first) else {
                return;
            };
            set_cond(ctx, first, quest, 12, true);
        } else if wave_id < 8
            && MONSTERS
                .iter()
                .all(|&id| ctx.instance_alive_npcs(world, id).is_empty())
        {
            ctx.add_timer(&timer_event(SPAWN_WAVE, world), 5000, 0, 0);
        }
        ctx.add_timer(&timer_event(WORLD_ATTACK, world), 1000, 0, 0);
    }

    fn on_timer_event(&self, ctx: &mut QuestCtx, event: &str) {
        let Some((name, world)) = event.split_once(' ') else {
            return;
        };
        let Ok(world) = world.parse::<i32>() else {
            return;
        };
        if !ctx.world.instances.contains(world) {
            return;
        }
        match name {
            SPAWN_WAVE => Self::manage_wave_spawn(ctx, world),
            WORLD_ATTACK => Self::world_attack(ctx, world, &[]),
            _ => {}
        }
    }
}
