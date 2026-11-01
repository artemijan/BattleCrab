//! Fortress of the Dead (instance 254) — port of
//! `dist/game/data/scripts/instances/FortressOfTheDead/`, Q10752's solo
//! instance.
//!
//! The Broken Bookshelf's page enters it. Once the last Vampiric Soldier
//! falls, Von Hellman appears; his death plays `ERT_QUEST_A` and brings Kain
//! Van Halter, whose talk spawns the Mysterious Wizard. The wizard's page
//! (from [`super::mysterious_wizard`]) fires `endCinematic`: Q10752 cond 8 →
//! 9 with Kain's fragment, `ERT_QUEST_B`, and the instance closes 25 seconds
//! later.

use super::q10751_winds_of_fate_encounters::{TOP_CENTER, mark_as_summoned_by};
use super::q10752_winds_of_fate_a_promise::{self as q10752, KAIN_PROPHECY_MACHINE_FRAGMENT};
use crate::game_loop::quests::{QuestCtx, QuestScript};

// NPCs
const BROKEN_BOOKSHELF: i32 = 31526;
const VAMPIRIC_SOLDIER: i32 = 19567;
const VON_HELLMAN: i32 = 19566;
const MYSTERIOUS_WIZARD: i32 = 33980;
const KAIN_VAN_HALTER: i32 = 33979;

// Locations
const VON_HELLMAN_LOC: (i32, i32, i32, i32) = (57963, -28676, 568, 49980);
const MYSTERIOUS_WIZARD_LOC: (i32, i32, i32, i32) = (57982, -28645, 568, 0);
const KAIN_VAN_HALTER_LOC: (i32, i32, i32, i32) = (57963, -28676, 568, 49980);

// Misc
const TEMPLATE_ID: i32 = 254;
/// `Movie.ERT_QUEST_A` / `ERT_QUEST_B`.
const ERT_QUEST_A: i32 = 110;
const ERT_QUEST_B: i32 = 111;
/// `NpcStringId.TALK_TO_THE_MYSTERIOUS_WIZARD`.
const TALK_TO_THE_MYSTERIOUS_WIZARD: i32 = 575214;

pub struct FortressOfTheDead;

impl FortressOfTheDead {
    /// `npc.getInstanceWorld()` when it is this template, else 0.
    fn world_of_npc(ctx: &QuestCtx) -> i32 {
        let world = ctx.npc_instance();
        if ctx.instance_template(world) == TEMPLATE_ID {
            world
        } else {
            0
        }
    }

    /// `endCinematic`. Java deletes `world.getNpc(...)` unguarded: a missing
    /// Kain or wizard ends the event there.
    fn end_cinematic(ctx: &mut QuestCtx, world: i32) {
        ctx.with_other_quest(q10752::QUEST_NAME, |qs| {
            if qs.has_qs() && qs.is_cond(8) {
                qs.set_cond(9, true);
                qs.give_items(KAIN_PROPHECY_MACHINE_FRAGMENT, 1);
            }
        });
        let Some(kain) = ctx.instance_npc(world, KAIN_VAN_HALTER) else {
            return;
        };
        ctx.delete_npc_of(kain);
        let Some(wizard) = ctx.instance_npc(world, MYSTERIOUS_WIZARD) else {
            return;
        };
        ctx.delete_npc_of(wizard);
        let player = ctx.player;
        ctx.play_movie(player, ERT_QUEST_B);
        let npc = ctx.npc;
        ctx.add_timer("exitInstance", 25000, npc, player);
    }
}

impl QuestScript for FortressOfTheDead {
    fn id(&self) -> i32 {
        // An `InstanceScript`, not a quest.
        -1
    }
    fn name(&self) -> &'static str {
        "FortressOfTheDead"
    }
    fn html_dir(&self) -> &'static str {
        "instances/FortressOfTheDead"
    }
    fn start_npcs(&self) -> &[i32] {
        &[BROKEN_BOOKSHELF]
    }
    fn first_talk_npcs(&self) -> &[i32] {
        &[KAIN_VAN_HALTER]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[BROKEN_BOOKSHELF, KAIN_VAN_HALTER, MYSTERIOUS_WIZARD]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[VAMPIRIC_SOLDIER, VON_HELLMAN]
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if event == "enterInstance" {
            ctx.enter_instance(TEMPLATE_ID);
            return None;
        }
        let world = Self::world_of_npc(ctx);
        if world == 0 {
            return None;
        }
        match event {
            "33979-01.html" | "33979-02.html" | "33979-03.html" | "33979-04.html"
            | "33979-05.html" | "33979-06.html" | "33979-07.html" | "33979-08.html"
            | "33979-09.html" | "33979-10.html" => return Some(event.to_string()),
            "exitInstance" => ctx.finish_instance(world, 0),
            "vampire_dead" => {
                ctx.add_spawn(VON_HELLMAN, VON_HELLMAN_LOC, false, 0, world);
            }
            "hellman_dead" => {
                ctx.add_spawn(KAIN_VAN_HALTER, KAIN_VAN_HALTER_LOC, false, 0, world);
            }
            "spawnWizard" => {
                ctx.send_screen_message_npc_string(TALK_TO_THE_MYSTERIOUS_WIZARD, TOP_CENTER, 5000);
                if let Some(wizard) =
                    ctx.add_spawn(MYSTERIOUS_WIZARD, MYSTERIOUS_WIZARD_LOC, true, 0, world)
                {
                    let player = ctx.player;
                    mark_as_summoned_by(ctx, wizard, player);
                }
                return Some("33979-11.html".to_string());
            }
            "endCinematic" => Self::end_cinematic(ctx, world),
            _ => {}
        }
        None
    }

    /// `startQuestTimer(...)` lands in `onEvent`.
    fn on_timer_event(&self, ctx: &mut QuestCtx, event: &str) {
        self.on_event(ctx, event);
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        let world = Self::world_of_npc(ctx);
        if world == 0 {
            return;
        }
        let (npc, player) = (ctx.npc, ctx.player);
        match ctx.npc_id {
            VAMPIRIC_SOLDIER => {
                if ctx.instance_alive_npcs(world, VAMPIRIC_SOLDIER).is_empty() {
                    ctx.add_timer("vampire_dead", 180, npc, player);
                }
            }
            VON_HELLMAN => {
                ctx.delete_npc();
                ctx.play_movie(player, ERT_QUEST_A);
                ctx.add_timer("hellman_dead", 180, npc, player);
            }
            _ => {}
        }
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let quest = q10752::QUEST_NAME;
        (ctx.has_other_quest_state(quest) && ctx.other_quest_cond(quest) == 8)
            .then(|| "33979.html".to_string())
    }
}
