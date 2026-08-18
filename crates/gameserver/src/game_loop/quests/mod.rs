//! The quest engine — the Rust counterpart of Java's script framework
//! (`Quest`/`AbstractScript` dispatch, the `QuestLink` bypass handler,
//! `QuestManager`). Scripts are compiled-in trait objects (see
//! `crate::scripts`) registered once at boot in a [`QuestRegistry`]; the
//! per-player progress they mutate is the `Quests` component
//! (`model/quest.rs`), mirrored row-per-var to `character_quests`.
//!
//! Borrow shape: the registry lives behind `World.quests: Arc<…>` (the
//! `World.geo` pattern) — entry points clone the script handle out of it,
//! then build a [`QuestCtx`] around `&mut World` and hand that to the
//! script. Scripts are stateless; all state flows through the ctx.

use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::hp_fraction;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::pos_of;
use crate::game_loop::helpers::send_action_failed;
use crate::game_loop::helpers::send_to_client;
use std::collections::HashMap;
use std::sync::Arc;

use tracing::warn;

use crate::model::components::{LastFolkNpc, QuestTimerSeqs, Quests};
use crate::model::inventory::Inventory;
use crate::model::quest::{self, COND_VAR, FLAGS_VAR, QuestState, state};
use crate::network::enter_world as ew;
use crate::network::server_packets::{self, quest_sounds};
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::death::ADENA_ID;
use super::helpers::client_for_player;
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::npc::cast;

/// One compiled-in script (Java: a `Quest` subclass). Implementations are
/// stateless — everything they touch goes through the [`QuestCtx`]. `id() >
/// 0` = a real quest (QuestList/quest window/DB rows); `id() <= 0` = a
/// utility script (dialog-only, e.g. `ClanMaster`).
pub trait QuestScript: Send + Sync {
    fn id(&self) -> i32;
    /// Registry/DB/bypass key (Java: the class simple name).
    fn name(&self) -> &'static str;
    /// Directory under `data/scripts/` holding this script's htmls
    /// (Java `Quest.getPath()`).
    fn html_dir(&self) -> &'static str;
    /// NPCs this quest can be started at (`addStartNpc`).
    fn start_npcs(&self) -> &[i32];
    /// NPCs whose quest window lists this quest (`addTalkId`).
    fn talk_npcs(&self) -> &[i32];
    /// Monsters whose death notifies this quest (`addKillId`).
    fn kill_npcs(&self) -> &[i32] {
        &[]
    }
    /// Monsters whose taking damage notifies this quest (`addAttackId`).
    fn attack_npcs(&self) -> &[i32] {
        &[]
    }
    /// NPCs whose (re)spawn notifies this quest (`addSpawnId`).
    fn spawn_npcs(&self) -> &[i32] {
        &[]
    }
    /// NPCs that notify this quest when they *witness* a skill (`addSkillSeeId`)
    /// — quest 350's Soul Crystal absorb.
    fn aggro_enter_npcs(&self) -> &[i32] {
        &[]
    }

    fn spell_finished_npcs(&self) -> &[i32] {
        &[]
    }

    fn skill_see_npcs(&self) -> &[i32] {
        &[]
    }

    /// NPCs that watch for creatures entering their sight (`addCreatureSeeId`;
    /// Java's `CreatureSeeTaskManager` scans once per second over the
    /// template's aggro range and fires once per newly-seen creature).
    fn creature_see_npcs(&self) -> &[i32] {
        &[]
    }
    /// NPCs whose chat window this script *replaces* (`addFirstTalkId`).
    /// Java's `NpcAction`: when an NPC carries an `ON_NPC_FIRST_TALK`
    /// listener, clicking it fires [`QuestScript::on_first_talk`] **instead
    /// of** `Npc.showChatWindow` — the default `data/html/default/<id>.htm`
    /// is never consulted for that NPC.
    fn first_talk_npcs(&self) -> &[i32] {
        &[]
    }
    /// Utility scripts (id ≤ 0) opting in to run `on_talk` from the bare
    /// `Quest` (quest-window) bypass — the `ai/others` behaviors whose talk
    /// *is* the behavior (TeleportWithCharm). Deliberate deviation: this
    /// Mobius build's chooser short-circuits utility scripts out
    /// (`getId() > 0 && … && onTalk(...)`), leaving them unreachable even
    /// though the dist htmls point their buttons at the bare `Quest`
    /// bypass.
    fn bare_talk(&self) -> bool {
        false
    }
    /// Items removed from the inventory when the quest exits
    /// (`registerQuestItems`).
    fn quest_items(&self) -> &[i32] {
        &[]
    }
    /// The `addCondMinLevel`-family gate: the html shown instead of
    /// `on_talk` while the player can't take the quest, `None` when
    /// eligible (Java `getStartConditionHtml`).
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        let _ = ctx;
        None
    }
    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String>;
    /// `onFirstTalk` — the whole chat window for a [`first_talk_npcs`] NPC.
    /// Returning `None` sends nothing (Java's null return, used by scripts
    /// that already pushed their own packet).
    ///
    /// [`first_talk_npcs`]: QuestScript::first_talk_npcs
    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let _ = ctx;
        None
    }
    /// HTML-button events (`Quest <Name> <event>` bypasses) and quest-timer
    /// names (Java routes both through `onEvent`; timers arrive via
    /// [`QuestScript::on_timer`] here for the trait's clarity).
    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let _ = (ctx, event);
        None
    }
    fn on_kill(&self, ctx: &mut QuestCtx) {
        let _ = ctx;
    }
    /// A registered monster took damage from a player (`onAttack`). Fired
    /// per damage application, including the killing blow (before
    /// `on_kill`).
    fn on_attack(&self, ctx: &mut QuestCtx) {
        let _ = ctx;
    }
    /// A registered NPC (re)spawned (`onSpawn`). **No player is involved**:
    /// `ctx.player`/`ctx.client_id` are 0 and the player-touching ctx
    /// methods must not be called.
    fn on_spawn(&self, ctx: &mut QuestCtx) {
        let _ = ctx;
    }
    fn on_timer(&self, ctx: &mut QuestCtx, name: &str) {
        let _ = (ctx, name);
    }
    /// A registered NPC witnessed `skill_id` being cast (`onSkillSee`).
    /// `ctx.npc` is the witnessing NPC and `ctx.player` the caster.
    /// `onAggroRangeEnter` — a player walked into this monster's aggro
    /// range and the scan just noticed them (first hate seeded).
    fn on_aggro_range_enter(&self, ctx: &mut QuestCtx) {
        let _ = ctx;
    }

    /// `onSpellFinished` — this NPC finished casting `skill_id` (fires at
    /// Java's `EVT_FINISH_CASTING`, after the cast bar completes).
    fn on_spell_finished(&self, ctx: &mut QuestCtx, skill_id: i32) {
        let _ = (ctx, skill_id);
    }

    fn on_skill_see(&self, ctx: &mut QuestCtx, skill_id: i32) {
        let _ = (ctx, skill_id);
    }

    /// `onCreatureSee` — `creature` (a player or another NPC) entered this
    /// NPC's sight for the first time since spawn. `ctx.player` carries the
    /// creature when it is a player, else 0.
    fn on_creature_see(&self, ctx: &mut QuestCtx, creature: i32) {
        let _ = (ctx, creature);
    }
    /// Whether this script subscribes to the GLOBAL_PLAYERS event stream
    /// (Java's `@RegisterEvent` login / tutorial-mark / item-pickup
    /// listeners). Opt-in so the enter-world path only builds a ctx for
    /// scripts that care.
    fn handles_global_events(&self) -> bool {
        false
    }
    /// `ON_PLAYER_LOGIN` — fired at the end of the enter-world burst.
    /// `ctx.npc` is 0.
    fn on_login(&self, ctx: &mut QuestCtx) {
        let _ = ctx;
    }
    /// `ON_PLAYER_PRESS_TUTORIAL_MARK` — the player clicked a shown tutorial
    /// question mark. The mark-id namespace is global across scripts.
    fn on_tutorial_mark(&self, ctx: &mut QuestCtx, mark_id: i32) {
        let _ = (ctx, mark_id);
    }
    /// `ON_PLAYER_ITEM_PICKUP` — the player picked `item_id` up off the
    /// ground. `ctx.npc` is 0.
    fn on_item_pickup(&self, ctx: &mut QuestCtx, item_id: i32) {
        let _ = (ctx, item_id);
    }
}

mod ctx;
mod ctx_items;
mod ctx_npc;
mod ctx_player;
mod ctx_ui;
pub(crate) mod dispatch;
mod registry;
mod render;

pub use ctx::*;
pub(crate) use dispatch::*;
pub use registry::*;
use render::*;

// The quest-style item primitives moved to `items`; re-exported here so the
// ~20 existing `quests::give_item_*`/`quests::take_items` callers are stable.
pub(crate) use super::items::{
    give_item_with_earned_message, give_item_with_earned_message_enchanted, take_items,
};
