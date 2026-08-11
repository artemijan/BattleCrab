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
use crate::game_loop::helpers::skill_by_id;
use std::collections::HashMap;
use std::sync::Arc;

use tracing::warn;

use crate::model::components::{LastFolkNpc, QuestTimerSeqs, Quests};
use crate::model::inventory::Inventory;
use crate::model::quest::{self, COND_VAR, FLAGS_VAR, QuestState, state};
use crate::network::enter_world as ew;
use crate::network::server_packets::{self, SmParam, quest_sounds, sm_ids};
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

/// Java `QuestManager` + the per-`NpcTemplate` listener containers, built
/// once at boot: name lookup plus npc-id → script indexes for the
/// start/talk/kill event routes.
pub struct QuestRegistry {
    scripts: Vec<Arc<dyn QuestScript>>,
    by_name: HashMap<&'static str, usize>,
    start: HashMap<i32, Vec<usize>>,
    talk: HashMap<i32, Vec<usize>>,
    kill: HashMap<i32, Vec<usize>>,
    attack: HashMap<i32, Vec<usize>>,
    spawn: HashMap<i32, Vec<usize>>,
    skill_see: HashMap<i32, Vec<usize>>,
    aggro_enter: HashMap<i32, Vec<usize>>,
    spell_finished: HashMap<i32, Vec<usize>>,
    creature_see: HashMap<i32, Vec<usize>>,
    first_talk: HashMap<i32, usize>,
    /// Scripts with `handles_global_events()` (login / tutorial-mark /
    /// item-pickup listeners).
    global_events: Vec<usize>,
}

impl QuestRegistry {
    pub fn new(scripts: Vec<Arc<dyn QuestScript>>) -> Self {
        let mut by_name = HashMap::new();
        let mut start: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut talk: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut kill: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut attack: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut spawn: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut skill_see: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut aggro_enter: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut spell_finished: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut creature_see: HashMap<i32, Vec<usize>> = HashMap::new();
        // One entry per NPC: the first-talk listener owns the whole chat
        // window, so two scripts claiming the same NPC is a bug, not a fan-out.
        let mut first_talk: HashMap<i32, usize> = HashMap::new();
        let mut global_events: Vec<usize> = Vec::new();
        for (idx, s) in scripts.iter().enumerate() {
            by_name.insert(s.name(), idx);
            if s.handles_global_events() {
                global_events.push(idx);
            }
            for &id in s.start_npcs() {
                start.entry(id).or_default().push(idx);
            }
            for &id in s.talk_npcs() {
                talk.entry(id).or_default().push(idx);
            }
            for &id in s.kill_npcs() {
                kill.entry(id).or_default().push(idx);
            }
            for &id in s.attack_npcs() {
                attack.entry(id).or_default().push(idx);
            }
            for &id in s.spawn_npcs() {
                spawn.entry(id).or_default().push(idx);
            }
            for &id in s.skill_see_npcs() {
                skill_see.entry(id).or_default().push(idx);
            }
            for &id in s.aggro_enter_npcs() {
                aggro_enter.entry(id).or_default().push(idx);
            }
            for &id in s.spell_finished_npcs() {
                spell_finished.entry(id).or_default().push(idx);
            }
            for &id in s.creature_see_npcs() {
                creature_see.entry(id).or_default().push(idx);
            }
            for &id in s.first_talk_npcs() {
                if let Some(&prev) = first_talk.get(&id) {
                    warn!(
                        "QuestRegistry: npc {id} first-talk claimed by both [{}] and [{}]; keeping the first.",
                        scripts[prev].name(),
                        s.name(),
                    );
                    continue;
                }
                first_talk.insert(id, idx);
            }
        }
        Self {
            scripts,
            by_name,
            start,
            talk,
            kill,
            attack,
            spawn,
            skill_see,
            aggro_enter,
            creature_see,
            spell_finished,
            first_talk,
            global_events,
        }
    }

    /// Scripts subscribed to the GLOBAL_PLAYERS event stream.
    pub fn global_event_quests(&self) -> Vec<Arc<dyn QuestScript>> {
        self.global_events
            .iter()
            .map(|&i| self.scripts[i].clone())
            .collect()
    }

    /// The script owning `npc_id`'s chat window, if any (Java
    /// `npc.hasListener(ON_NPC_FIRST_TALK)` + the listener itself).
    pub fn first_talk_quest(&self, npc_id: i32) -> Option<Arc<dyn QuestScript>> {
        self.first_talk
            .get(&npc_id)
            .map(|&i| self.scripts[i].clone())
    }

    pub fn by_name(&self, name: &str) -> Option<Arc<dyn QuestScript>> {
        self.by_name.get(name).map(|&i| self.scripts[i].clone())
    }

    /// The scripts that register any hook on `npc_id`, sorted and deduped —
    /// `//show_quests`' listing for an NPC target. Java gets the same set by
    /// walking every `EventType`'s listeners on the spawned NPC and collecting
    /// their owning quests into a `TreeSet` (alphabetical, one entry per
    /// quest however many hooks it registers).
    pub fn scripts_for_npc(&self, npc_id: i32) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = self
            .scripts
            .iter()
            .filter(|s| {
                s.start_npcs().contains(&npc_id)
                    || s.talk_npcs().contains(&npc_id)
                    || s.first_talk_npcs().contains(&npc_id)
                    || s.kill_npcs().contains(&npc_id)
                    || s.attack_npcs().contains(&npc_id)
            })
            .map(|s| s.name())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Every registered script's name, sorted — the `//quest_info` listing.
    pub fn names(&self) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = self.scripts.iter().map(|s| s.name()).collect();
        v.sort_unstable();
        v
    }

    pub fn by_id(&self, quest_id: i32) -> Option<Arc<dyn QuestScript>> {
        self.scripts.iter().find(|s| s.id() == quest_id).cloned()
    }

    pub fn quest_id(&self, name: &str) -> Option<i32> {
        self.by_name.get(name).map(|&i| self.scripts[i].id())
    }

    /// Scripts listing `npc_id` as a talk NPC (the quest-window set).
    pub fn talk_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.talk
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Scripts listing `npc_id` as a kill NPC.
    pub fn kill_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.kill
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Scripts listing `npc_id` as an attack NPC.
    pub fn attack_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.attack
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Scripts listing `npc_id` as a skill-see NPC.
    pub fn skill_see_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.skill_see
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Scripts listing `npc_id` as an aggro-range-enter NPC.
    pub fn aggro_enter_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.aggro_enter
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Scripts listing `npc_id` as a spell-finished NPC.
    pub fn spell_finished_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.spell_finished
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Scripts listing `npc_id` as a creature-see NPC.
    pub fn creature_see_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.creature_see
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Whether any script watches `npc_id` for creatures entering sight.
    pub fn has_creature_see(&self, npc_id: i32) -> bool {
        self.creature_see.contains_key(&npc_id)
    }

    /// Scripts listing `npc_id` as a spawn NPC.
    pub fn spawn_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.spawn
            .get(&npc_id)
            .map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Whether `npc_id` is a start NPC of the named script (the
    /// `ON_NPC_QUEST_START` owner check in `notifyTalk`).
    pub fn is_start_npc(&self, name: &str, npc_id: i32) -> bool {
        self.start
            .get(&npc_id)
            .is_some_and(|v| v.iter().any(|&i| self.scripts[i].name() == name))
    }

    pub fn len(&self) -> usize {
        self.scripts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty()
    }
}

/// Everything a script callback gets to work with: the world plus the
/// identities of the involved parties, with the `QuestState`/`AbstractScript`
/// primitive methods ported onto it.
pub struct QuestCtx<'w> {
    pub world: &'w mut World,
    pub client_id: u32,
    /// The player's object id (== char id).
    pub player: i32,
    /// Involved NPC's object id, 0 when there is none (some timers).
    pub npc: i32,
    /// Involved NPC's template id, 0 when there is none.
    pub npc_id: i32,
    script: Arc<dyn QuestScript>,
    /// `onAttack` only: the skill that struck (Java's `Skill skill`), `None`
    /// for a melee swing. `None` in every other callback.
    attack_skill_id: Option<i32>,
    /// `onAttack` only: whether the blow came from the player's servitor/pet
    /// (Java's `boolean isSummon`). `false` in every other callback.
    attack_is_summon: bool,
    /// Java's `QuestState.setSimulated` / `Player.setSimulatedTalking`: this
    /// context only *probes* what a callback would return — the quest-window
    /// filter runs `on_talk` purely to learn whether the quest has anything
    /// to say at this NPC — so nothing it does may be observable. See
    /// [`QuestCtx::new_simulated`].
    simulated: bool,
}

/// `AbstractScript.addExpAndSp` — quest XP/SP with the premium and server
/// reward rates, then the PA-point award the Java method closes with.
///
/// A free function so the rate arithmetic is reachable without a live script
/// `Arc`; [`QuestCtx::add_exp_and_sp`] is the thin wrapper quests call.
pub(crate) fn add_quest_exp_and_sp(world: &mut World, player: i32, exp: i64, sp: i64) {
    // `PremiumRateQuestXp`/`Sp` apply **first**, before the server rate — Java
    // multiplies `addExp` by the premium rate and only then by
    // `RATE_QUEST_REWARD_XP`. Both are 1 on this dist, so the ordering is
    // currently unobservable, but it is what decides the rounding.
    let (mut exp, mut sp) = (exp as f64, sp as f64);
    if world.cfg.premium.enabled && super::admin::premium::has_premium_status(world, player) {
        exp *= world.cfg.premium.rate_quest_xp;
        sp *= world.cfg.premium.rate_quest_sp;
    }
    let exp = (exp * world.cfg.rates.rate_quest_reward_xp) as i64;
    let sp = (sp * world.cfg.rates.rate_quest_reward_sp) as i64;
    // Java routes quest rewards through `Player.addExpAndSp(exp, sp)`, the
    // two-arg overload — `useBonuses = false`, so vitality neither boosts the
    // reward nor is spent on it.
    super::death::add_exp_and_sp(world, player, exp as f64, sp as f64, false);
    // `AbstractScript.addExpAndSp` closes with
    // `givePcCafePoint(player, addExp * RATE_QUEST_REWARD_XP)` — the premium-
    // and rate-multiplied value, which is exactly `exp` here.
    super::pc_cafe::give_point(world, player, exp as f64);
}

impl<'w> QuestCtx<'w> {
    fn new(
        world: &'w mut World,
        client_id: u32,
        player: i32,
        npc: i32,
        script: Arc<dyn QuestScript>,
    ) -> Self {
        let npc_id = npc_id_of(world, npc).unwrap_or(0);
        Self {
            world,
            client_id,
            player,
            npc,
            npc_id,
            script,
            attack_skill_id: None,
            attack_is_summon: false,
            simulated: false,
        }
    }

    /// `Quest.onTalk(npc, player, true)`: a context whose writes are all
    /// suppressed, used by the quest-window filter to see *which html* a
    /// script would answer with without actually talking to it.
    ///
    /// Java suppresses only the `QuestState` mutators (`_simulated` guards
    /// every setter, `exitQuest` included) and leaves `AbstractScript`'s
    /// `giveItems`/`takeItems`/`addExpAndSp` unguarded — which means merely
    /// *opening* Parina's quest window with all four elemental trinkets in
    /// hand strips them and awards the Bead of Season while `exitQuest` is
    /// swallowed, leaving the quest unfinishable. Deliberate deviation: the
    /// simulated context is inert here — items, XP, packets, spawns and
    /// teleports are all suppressed too, so a probe can never cost a player
    /// anything.
    fn new_simulated(
        world: &'w mut World,
        client_id: u32,
        player: i32,
        npc: i32,
        script: Arc<dyn QuestScript>,
    ) -> Self {
        Self {
            simulated: true,
            ..Self::new(world, client_id, player, npc, script)
        }
    }

    /// `onAttack` context: the skill that struck (Java's `Skill skill`) — `None`
    /// for a melee swing. Meaningful only inside [`QuestScript::on_attack`].
    pub fn attack_skill_id(&self) -> Option<i32> {
        self.attack_skill_id
    }

    /// `onAttack` context: whether the blow came from the player's servitor/pet
    /// (Java's `boolean isSummon`). Meaningful only inside
    /// [`QuestScript::on_attack`].
    pub fn attack_is_summon(&self) -> bool {
        self.attack_is_summon
    }

    pub fn quest_id(&self) -> i32 {
        self.script.id()
    }

    pub fn quest_name(&self) -> &'static str {
        self.script.name()
    }

    // --- QuestState reads -------------------------------------------------

    fn qs(&self) -> Option<&QuestState> {
        self.world
            .objects
            .get_component::<Quests>(&self.player)?
            .0
            .get(self.script.name())
    }

    /// `State.CREATED` when the player has no `QuestState` yet — Java
    /// creates one lazily (`getQuestState(player, true)`); we only
    /// materialize it on the first write.
    pub fn state(&self) -> u8 {
        self.qs().map(|qs| qs.state).unwrap_or(state::CREATED)
    }

    pub fn is_created(&self) -> bool {
        self.state() == state::CREATED
    }

    pub fn is_started(&self) -> bool {
        self.state() == state::STARTED
    }

    pub fn is_completed(&self) -> bool {
        self.state() == state::COMPLETED
    }

    pub fn cond(&self) -> i32 {
        self.qs().map(|qs| qs.cond()).unwrap_or(0)
    }

    /// The `cond` of a *different* quest for this player (0 if that quest is
    /// not started or absent). The Formal Wear sub-quests (33-36) gate on
    /// `Q037_MakeFormalWear` having reached cond 6/7.
    pub fn other_quest_cond(&self, quest_name: &str) -> i32 {
        self.world
            .objects
            .get_component::<Quests>(&self.player)
            .and_then(|q| q.0.get(quest_name))
            .map(|qs| qs.cond())
            .unwrap_or(0)
    }

    /// Whether a *different* quest is COMPLETED for this player. Q641 (Attack
    /// Sailren) only opens once `Q00126_TheNameOfEvil2` is finished.
    pub fn other_quest_completed(&self, quest_name: &str) -> bool {
        self.world
            .objects
            .get_component::<Quests>(&self.player)
            .and_then(|q| q.0.get(quest_name))
            .is_some_and(|qs| qs.state == state::COMPLETED)
    }

    pub fn is_cond(&self, cond: i32) -> bool {
        self.cond() == cond
    }

    pub fn get_var(&self, var: &str) -> Option<String> {
        self.qs()?.vars.get(var).cloned()
    }

    /// Whether a `QuestState` exists at all — Java's `getQuestState(player,
    /// false) != null` guard (scripts use it to reject events for quests
    /// never talked about).
    pub fn has_qs(&self) -> bool {
        self.qs().is_some()
    }

    /// Java `getQuestState(player, true)`: materialize a CREATED state
    /// without persisting anything (rows only appear once vars are set).
    pub fn ensure_qs(&mut self) {
        self.with_qs_mut(|_| ());
    }

    /// `Quest.getNoQuestMsg` — for `addCondMaxLevel`-style gates whose
    /// "can't take it" html is the generic no-quest message.
    pub fn no_quest_html(&self) -> String {
        no_quest_html(self.world)
    }

    pub fn get_int(&self, var: &str) -> i32 {
        self.qs().map(|qs| qs.get_int(var)).unwrap_or(0)
    }

    // --- QuestState writes (each mirrors to the DB like Java) -------------

    fn with_qs_mut<R>(&mut self, f: impl FnOnce(&mut QuestState) -> R) -> R {
        // Java's `_simulated` early-returns out of every `QuestState` setter,
        // so the write lands nowhere and later reads still see the old value.
        // Running `f` against a throwaway clone reproduces that (and keeps the
        // return value the callers destructure).
        if self.simulated {
            let mut scratch = self.qs().cloned().unwrap_or_default();
            return f(&mut scratch);
        }
        let quests = self
            .world
            .objects
            .get_component_mut::<Quests>(&self.player)
            .expect("in-game player always carries Quests");
        f(quests.0.entry(self.script.name().to_string()).or_default())
    }

    /// `QuestState.set`: store the var (memory-first — it persists on the next
    /// flush, not per write); a `cond` write additionally runs the skipped-step
    /// flag bookkeeping and pushes `QuestList` + `ExShowQuestMark` (Java's
    /// private `setCond(cond, old)`).
    pub fn set_var(&mut self, var: &str, value: impl Into<String>) {
        let value: String = value.into();
        let old = self.with_qs_mut(|qs| qs.vars.insert(var.to_string(), value.clone()));
        if var != COND_VAR {
            return;
        }
        let old_cond: i32 = old.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0);
        let new_cond: i32 = value.parse().unwrap_or(0);
        if new_cond == old_cond {
            return;
        }
        let stored = self
            .qs()
            .and_then(|qs| qs.vars.get(FLAGS_VAR).and_then(|v| v.parse::<i32>().ok()));
        let updated = quest::updated_cond_flags(new_cond, old_cond, stored);
        if updated != stored {
            match updated {
                Some(flags) => {
                    let s = flags.to_string();
                    self.with_qs_mut(|qs| qs.vars.insert(FLAGS_VAR.to_string(), s.clone()));
                }
                None => self.unset(FLAGS_VAR),
            }
        }
        self.send_quest_list();
        // Java: `!isCustomQuest() && cond > 0` — ExShowQuestMark carries
        // `getCond()`, which is still 0 while CREATED (the startQuest path).
        if self.script.id() > 0 && new_cond > 0 {
            let mark = server_packets::ex_show_quest_mark(self.script.id(), self.cond());
            self.send(mark);
        }
    }

    pub fn set_cond(&mut self, value: i32, play_quest_middle: bool) {
        if !self.is_started() {
            return;
        }
        self.set_var(COND_VAR, value.to_string());
        if play_quest_middle {
            self.play_sound(quest_sounds::MIDDLE);
        }
    }

    /// `QuestState.unset` — drop the var in memory (persists on the next flush).
    pub fn unset(&mut self, var: &str) {
        self.with_qs_mut(|qs| qs.vars.remove(var));
    }

    /// `QuestState.setState`: flip the state (memory-first; the `<state>` row is
    /// written on the next flush) + `QuestList`.
    pub fn set_state(&mut self, new_state: u8) {
        self.with_qs_mut(|qs| qs.state = new_state);
        self.send_quest_list();
    }

    /// `QuestState.startQuest`: cond 1, STARTED, accept sound. Only from
    /// CREATED, like Java.
    pub fn start_quest(&mut self) {
        if !self.is_created() {
            return;
        }
        self.set_var(COND_VAR, "1");
        self.set_state(state::STARTED);
        self.play_sound(quest_sounds::ACCEPT);
    }

    /// `QuestState.exitQuest(repeatable, playExitQuest)`: destroy the
    /// registered quest items, drop the DB rows (all of them when
    /// repeatable, else all but `<state>`), then either forget the quest or
    /// mark it COMPLETED.
    pub fn exit_quest(&mut self, repeatable: bool, play_exit_quest: bool) {
        // Java guards the whole of `QuestState.exitQuest` with `_simulated`,
        // registered quest items included.
        if self.simulated || !self.is_started() {
            return;
        }
        let quest_items: Vec<i32> = self.script.quest_items().to_vec();
        for item_id in quest_items {
            self.take_items(item_id, -1);
        }
        // Memory-first: forgetting the quest (repeatable) or clearing its vars +
        // marking it COMPLETED (below) is done in memory; the flush reconciles
        // the `character_quests` rows — dropping all of them, or all but the
        // `<state>` row, exactly as Java's `DeleteQuest` did.
        if repeatable {
            if let Some(quests) = self.world.objects.get_component_mut::<Quests>(&self.player) {
                quests.0.remove(self.script.name());
            }
            self.send_quest_list();
        } else {
            self.with_qs_mut(|qs| qs.vars.clear());
            self.set_state(state::COMPLETED);
        }
        if play_exit_quest {
            self.play_sound(quest_sounds::FINISH);
        }
    }

    // --- item / reward primitives (AbstractScript ports) ------------------

    /// `getQuestItemsCount`.
    pub fn quest_items_count(&self, item_id: i32) -> i64 {
        self.world
            .objects
            .get_component::<Inventory>(&self.player)
            .map(|inv| inv.count_of(item_id))
            .unwrap_or(0)
    }

    /// `AbstractScript.giveItems(player, id, count, playSound=false)`.
    pub fn give_items(&mut self, item_id: i32, count: i64) {
        if self.simulated || count <= 0 {
            return;
        }
        give_item_with_earned_message(self.world, self.client_id, self.player, item_id, count);
    }

    /// `AbstractScript.rewardItems` — the turn-in variant with the reward
    /// multipliers (`RateQuestRewardAdena` for adena, `RateQuestReward`
    /// otherwise; the per-EtcItem-type multiplier split is unported).
    pub fn reward_items(&mut self, item_id: i32, count: i64) {
        if self.simulated || count <= 0 {
            return;
        }
        let rate = if item_id == ADENA_ID {
            self.world.cfg.rates.rate_quest_reward_adena
        } else {
            self.world.cfg.rates.rate_quest_reward
        };
        let count = (count as f64 * rate) as i64;
        give_item_with_earned_message(self.world, self.client_id, self.player, item_id, count);
    }

    /// `Config.RATE_QUEST_DROP` — the drop-rate multiplier some quests fold
    /// into their own roll threshold (rather than through `give_item_randomly`).
    pub fn rate_quest_drop(&self) -> f64 {
        self.world.cfg.rates.rate_quest_drop
    }

    /// `AbstractScript.giveAdena`.
    pub fn give_adena(&mut self, count: i64, apply_rates: bool) {
        if apply_rates {
            self.reward_items(ADENA_ID, count);
        } else {
            self.give_items(ADENA_ID, count);
        }
    }

    /// The champion arm of `AbstractScript.giveItemRandomly` as a
    /// `(chance multiplier, amount multiplier)` pair — `(1.0, 1.0)` whenever
    /// the notifying NPC is absent, is not a champion, or the master gate is
    /// off. Adena and ancient adena take the `ADENAS_` pair, everything else
    /// the plain one; Java splits them because a 10× adena rate on a normal
    /// item would dwarf the intended reward.
    fn champion_quest_drop_mods(&self, item_id: i32) -> (f64, f64) {
        const ANCIENT_ADENA_ID: i32 = 5575;
        let cfg = &self.world.cfg.champion;
        // `npc != null` — `self.npc` is 0 for the script-driven calls that
        // have no NPC (timers, bypass handlers), and the component lookup also
        // fails once the corpse has decayed.
        let is_champion = cfg.enable
            && self
                .world
                .objects
                .get_component::<crate::model::npc::Npc>(&self.npc)
                .is_some_and(|n| n.champion);
        if !is_champion {
            return (1.0, 1.0);
        }
        if item_id == ADENA_ID || item_id == ANCIENT_ADENA_ID {
            (cfg.adenas_rewards_chance, cfg.adenas_rewards_amount)
        } else {
            (cfg.rewards_chance, cfg.rewards_amount)
        }
    }

    /// `AbstractScript.giveItemRandomly(player, npc, id, amount, limit,
    /// chance, playSound)`: chance and amount ×`RateQuestDrop`, capped at
    /// `limit`; returns true when the limit is (already) reached — the
    /// "collection finished" signal quests key `setCond` off.
    ///
    /// A champion kill multiplies both on top of the quest rate, exactly as
    /// the death-drop path does — Java repeats the whole champion arm here
    /// because quest items never pass through `NpcTemplate.calculateDrops`.
    /// Without it a champion was a pure penalty on a collection quest: ten
    /// times the HP for the same drop rate.
    pub fn give_item_randomly(
        &mut self,
        item_id: i32,
        amount: i64,
        limit: i64,
        chance: f64,
        play_sound: bool,
    ) -> bool {
        if self.simulated {
            return false;
        }
        let current = self.quest_items_count(item_id);
        if limit > 0 && current >= limit {
            return true;
        }
        let rate = self.world.cfg.rates.rate_quest_drop;
        // Java truncates to `long` *before* the champion multiply and again
        // after (`long *= double` is a narrowing compound assignment), so the
        // two casts below are both load-bearing for byte-parity on the amount.
        let mut amount_to_give = (amount as f64 * rate) as i64;
        let mut chance_with_bonus = chance * rate;
        // `(npc != null) && Config.CHAMPION_ENABLE && npc.isChampion()`.
        let (champ_chance, champ_amount) = self.champion_quest_drop_mods(item_id);
        chance_with_bonus *= champ_chance;
        amount_to_give = (amount_to_give as f64 * champ_amount) as i64;
        let random = self.world.roll_f64();
        if chance_with_bonus >= random && amount_to_give > 0 {
            if limit > 0 && current + amount_to_give > limit {
                amount_to_give = limit - current;
            }
            give_item_with_earned_message(
                self.world,
                self.client_id,
                self.player,
                item_id,
                amount_to_give,
            );
            if current + amount_to_give == limit {
                if play_sound {
                    self.play_sound(quest_sounds::MIDDLE);
                }
                return true;
            }
            if play_sound {
                self.play_sound(quest_sounds::ITEMGET);
            }
            return limit <= 0;
        }
        false
    }

    /// `AbstractScript.takeItems` (negative count = all). Returns whether
    /// anything was taken.
    pub fn take_items(&mut self, item_id: i32, count: i64) -> bool {
        if self.simulated {
            return false;
        }
        take_items(self.world, self.client_id, self.player, item_id, count)
    }

    /// `AbstractScript.addExpAndSp` — quest XP/SP with the
    /// `RateQuestRewardXP/SP` multipliers.
    pub fn add_exp_and_sp(&mut self, exp: i64, sp: i64) {
        if self.simulated {
            return;
        }
        add_quest_exp_and_sp(self.world, self.player, exp, sp);
    }

    // --- misc --------------------------------------------------------------

    pub fn play_sound(&mut self, sound: &str) {
        let pkt = server_packets::play_sound(sound);
        self.send(pkt);
    }

    /// `player.sendPacket(new SocialAction(player.getObjectId(), id))` — the
    /// victory animation the class-path quests play on completion. Java uses
    /// `sendPacket`, not a broadcast, so only the player sees it.
    pub fn social_action(&mut self, action_id: i32) {
        let pkt = server_packets::social_action(self.player, action_id);
        self.send(pkt);
    }

    /// `Rnd.get(bound)` through the world RNG (test-forceable).
    pub fn roll(&mut self, bound: i32) -> i32 {
        self.world.roll(bound)
    }

    pub fn player_level(&self) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .map(|p| p.level)
            .unwrap_or(0)
    }

    /// Hand over one `item_id` and advance to `next_cond` once `target` of them
    /// have been collected — the body of every "kill things until you have N"
    /// quest's `onKill`.
    ///
    /// The sound is the tell that this is one pattern rather than a dozen
    /// similar ones: `ITEMGET` plays on every drop **except** the last, because
    /// [`set_cond`](Self::set_cond) plays `MIDDLE` itself and Java never stacks
    /// the two. A hand-written copy that plays `ITEMGET` unconditionally is
    /// audibly wrong at exactly one moment per quest, which is precisely the
    /// kind of thing that survives review.
    ///
    /// The `==` is Java's, not a `>=`: an inventory already over the target
    /// (a second quest that shares the item, a GM `//give`) does not advance
    /// the condition here.
    pub fn collect_toward(&mut self, item_id: i32, target: i64, next_cond: i32) {
        self.give_items(item_id, 1);
        if self.quest_items_count(item_id) == target {
            self.set_cond(next_cond, true);
        } else {
            self.play_sound(quest_sounds::ITEMGET);
        }
    }

    /// [`collect_toward`](Self::collect_toward) with a cap: nothing is handed
    /// over once the player already holds `target`, and the condition advances
    /// on `>=` rather than `==`.
    ///
    /// The two differences travel together, which is why this is a separate
    /// method rather than a flag. Java writes these quests with the cap test in
    /// the `onKill` guard itself; that makes an exact `==` the wrong
    /// comparison, because a player who arrived over the target — a shared drop
    /// from another quest — would never see the count *land* on it.
    pub fn collect_capped(&mut self, item_id: i32, target: i64, next_cond: i32) {
        if self.quest_items_count(item_id) >= target {
            return;
        }
        self.give_items(item_id, 1);
        if self.quest_items_count(item_id) >= target {
            self.set_cond(next_cond, true);
        } else {
            self.play_sound(quest_sounds::ITEMGET);
        }
    }

    /// Give one of whatever a `(npc_id, item_id)` table yields for the NPC that
    /// just died, then play the pickup sound.
    ///
    /// `fallback` is Java's `else` branch: these quests register more kill
    /// targets than they tabulate drops for, and every untabled one yields the
    /// quest's staple item.
    pub fn give_table_drop(&mut self, table: &[(i32, i32)], fallback: i32) {
        let item = table
            .iter()
            .find(|(id, _)| *id == self.npc_id)
            .map_or(fallback, |(_, item)| *item);
        self.give_items(item, 1);
        self.play_sound(quest_sounds::ITEMGET);
    }

    /// Java `addCondLevel(min, max, html)` — a two-sided level gate, shaped as
    /// the `Some(html)` a `start_condition_html` returns when the gate refuses.
    ///
    /// Both bounds are inclusive, matching Java.
    pub fn cond_level(&self, min: i32, max: i32, html: &str) -> Option<String> {
        let level = self.player_level();
        (level < min || level > max).then(|| html.to_string())
    }

    /// Java `isOwningClan` — the player's clan is `owner_id`.
    ///
    /// `owner_id == 0` means *unowned*, and nobody's clan owns an unowned
    /// residence, so that case is `false` before the player is even looked at.
    pub fn is_owning_clan(&self, owner_id: i32) -> bool {
        owner_id != 0
            && self
                .world
                .objects
                .get_component::<crate::model::Player>(&self.player)
                .is_some_and(|p| p.clan_id == owner_id)
    }

    /// Java `player.hasClanPrivilege(...)`: the leader holds every privilege,
    /// otherwise the member's rank privilege mask must carry the bit.
    ///
    /// `false` for a clanless player, and for one whose clan id points at
    /// nothing — the residence scripts gate every paid action on this, so an
    /// unresolvable clan must not read as "allowed".
    pub fn has_clan_privilege(&self, privilege: i32) -> bool {
        let Some(p) = self
            .world
            .objects
            .get_component::<crate::model::Player>(&self.player)
        else {
            return false;
        };
        self.world
            .clans
            .get(&p.clan_id)
            .is_some_and(|c| c.has_privilege(self.player, p.clan_privs, privilege))
    }

    /// `npc.getLevel()` — the in-context NPC's template level (regular mobs do
    /// not level up, so the template value is authoritative). 0 when unknown.
    pub fn npc_level(&self) -> i32 {
        self.world
            .data
            .npc_data
            .get(self.npc_id)
            .map(|t| t.level)
            .unwrap_or(0)
    }

    /// The `Race` ordinal (`characters.race` — 0 Human, 1 Elf, 2 Dark Elf,
    /// 3 Orc, 4 Dwarf).
    pub fn player_race(&self) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .map(|p| p.race)
            .unwrap_or(0)
    }

    /// `Npc.getRace()` — the talked-to NPC's `<race>` as the same ordinal
    /// [`player_race`](QuestCtx::player_race) returns, `None` when the
    /// template declares a non-player race (or none).
    pub fn npc_race(&self) -> Option<i32> {
        self.world
            .objects
            .get_component::<crate::model::npc::Npc>(&self.npc)
            .and_then(|n| n.template(self.world))
            .and_then(|t| t.race)
    }

    /// `AbstractScript.addRadar` → `Radar.addMarker` — drop a radar marker on
    /// the player's map. Type 1 is the plain red flag the "find an NPC"
    /// services use; for a quest objective prefer [`add_quest_radar`], which
    /// the client draws as the quest pin.
    ///
    /// Java sends **two** packets here, not one: `RadarControl(2, 2, x, y, z)`
    /// clears any marker already standing at that spot, then
    /// `RadarControl(0, 1, x, y, z)` shows the new one. Dropping the leading
    /// clear — as this helper did until 2026-08-05 — leaves the client
    /// stacking duplicate flags when the same location is re-pinged, which is
    /// exactly what the "find an NPC" services and Q255's tutorial do on every
    /// repeat ask. `community_board::npc_trace` already sent the pair, so the
    /// two radar paths in this port disagreed with each other.
    ///
    /// [`add_quest_radar`]: QuestCtx::add_quest_radar
    pub fn add_radar(&mut self, x: i32, y: i32, z: i32) {
        let clear = server_packets::radar_control(2, 2, x, y, z);
        self.send(clear);
        let pkt = server_packets::radar_control(0, 1, x, y, z);
        self.send(pkt);
    }

    /// `player.sendPacket(new ExShowScreenMessage(npcString, position, time))`
    /// — an on-screen banner whose text is a client-side string id.
    ///
    /// Simulated probes are suppressed, as for every other send here.
    pub fn send_screen_message_npc_string(&self, npc_string_id: i32, position: i32, time: i32) {
        self.send(server_packets::ex_show_screen_message_npc_string(
            npc_string_id,
            position,
            time,
            &[],
        ));
    }

    /// `player.sendPacket(SystemMessageId.X)` — a parameterless system message.
    ///
    /// Prefer this over reaching into `world.clients` from a script: it routes
    /// through [`QuestCtx::send`], which suppresses output during a simulated
    /// probe exactly as Java's `isSimulatingTalking()` guards do. A direct
    /// client send skips that and leaks packets to a player who is only being
    /// *asked* whether a dialogue would proceed.
    pub fn send_sm(&self, message_id: i16) {
        self.send(server_packets::system_message_with(message_id, &[]));
    }

    /// `RadarControl(0, 2, x, y, z)` — the *quest* marker, as Q211 sends it
    /// raw in Java. Same packet as [`add_radar`] but radar type 2, which the
    /// client renders as the quest pin rather than the red flag.
    ///
    /// [`add_radar`]: QuestCtx::add_radar
    pub fn add_quest_radar(&mut self, x: i32, y: i32, z: i32) {
        let pkt = server_packets::radar_control(0, 2, x, y, z);
        self.send(pkt);
    }

    /// `RadarControl(2, 2, 0, 0, 0)` — drop every marker on the player's map.
    /// This is how Q348 retires its marker once the objective is reached; the
    /// client has no "remove this one type-2 marker" form, so reaching an
    /// objective clears the board.
    pub fn clear_radar(&mut self) {
        let pkt = server_packets::radar_control(2, 2, 0, 0, 0);
        self.send(pkt);
    }

    /// `SpawnTable.getAnySpawn(npcId)` — the spawn point of any live instance
    /// of `npc_id` (the `spawn_loc` anchor, not its wandered-to position, so
    /// the marker matches Java's `Spawn.getX/Y/Z`). Java reads its spawn
    /// *table* — every registered point, spawned or not; the Rust world holds
    /// spawned objects, so this scans those. The two agree for the
    /// always-spawned town NPCs this serves; a despawned NPC yields `None`
    /// where Java would still answer.
    pub fn any_spawn_location(&mut self, npc_id: i32) -> Option<(i32, i32, i32)> {
        let mut loc = None;
        self.world
            .objects
            .for_each_mut::<&crate::model::npc::Npc>(|npc| {
                if loc.is_none() && npc.npc_id == npc_id {
                    loc = Some(npc.spawn_loc);
                }
            });
        loc
    }

    /// `Player.getClan() != null` (AllianceMaster's clan gate). Clan id 0 is
    /// the no-clan sentinel.
    pub fn has_clan(&self) -> bool {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .is_some_and(|p| p.clan_id != 0)
    }

    /// `Player.isClanLeader` (ClanMaster's LEADER_REQUIRED gate).
    pub fn is_clan_leader(&self) -> bool {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .is_some_and(|p| p.clan_leader)
    }

    pub fn player_class_id(&self) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .map(|p| p.class_id)
            .unwrap_or(-1)
    }

    /// Java `Player.isSubClassActive()` — true while a subclass slot is the
    /// active one (G17). Several village-master scripts refuse to talk at all
    /// in that state.
    pub fn is_subclass_active(&self) -> bool {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .is_some_and(|p| p.class_index != 0)
    }

    /// `Player.isInCategory(CategoryType.X)` against `CategoryData.xml`.
    pub fn is_in_category(&self, category: &str) -> bool {
        self.world
            .data
            .categories
            .contains(category, self.player_class_id())
    }

    /// The village-master class transfer — routed through the G17 mechanic
    /// (`game_loop::subclass::set_class_id`), so it moves the *active* slot:
    /// the base class only when the player is on it. Persisted immediately
    /// through the regular `StorePlayer` snapshot.
    pub fn set_class_id(&mut self, class_id: i32) {
        if self.simulated {
            return;
        }
        // Was an unconditional `base_class_id = class_id`, which since G17
        // would rewrite the character's *base* class if a quest transfer ran
        // while a subclass was active. The shared mechanic moves the active
        // slot only, and also does `rewardSkills` + the stat/UserInfo refresh.
        super::subclass::set_class_id(self.world, self.player, class_id);
        super::net::store_player_now(self.world, self.player);
    }

    /// `player.teleToLocation(loc)` (TeleportWithCharm and friends).
    pub fn teleport_to(&mut self, x: i32, y: i32, z: i32) {
        if self.simulated {
            return;
        }
        super::death::teleport_player(self.world, self.player, x, y, z);
    }

    /// `player.getVariables().getInt(key, default)` — the *character*
    /// key/value store (`character_variables`), not the per-quest
    /// `QuestState` vars: it outlives the script's quest state and is what
    /// the `ai/others` behaviors use to remember something about a player
    /// (TeleportToRaceTrack's return point).
    pub fn player_var_int(&self, key: &str, default: i32) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::components::PlayerVariables>(&self.player)
            .map(|v| v.get_int(key, default))
            .unwrap_or(default)
    }

    /// `player.getVariables().getString(key, null)` — the raw value, so a
    /// caller can tell **absent** from a stored zero.
    ///
    /// [`player_var_int`] cannot: it folds both into its default. Java leans on
    /// the difference wherever a variable's *first* write is special —
    /// `giveNewbieReward` seeds `GUIDE_MISSION` to 100000 when unset but adds a
    /// digit when it exists, and those two branches disagree for a stored 0.
    ///
    /// [`player_var_int`]: QuestCtx::player_var_int
    pub fn player_var(&self, key: &str) -> Option<String> {
        self.world
            .objects
            .get_component::<crate::model::components::PlayerVariables>(&self.player)
            .and_then(|v| v.0.get(key).cloned())
    }

    /// `player.getVariables().set(key, value)` (memory-first — flushed with
    /// the character like every other persisted field).
    pub fn set_player_var_int(&mut self, key: &str, value: i32) {
        if self.simulated {
            return;
        }
        if let Some(v) = self
            .world
            .objects
            .get_component_mut::<crate::model::components::PlayerVariables>(&self.player)
        {
            v.set_int(key, value);
        }
    }

    /// `player.getVariables().remove(key)`.
    pub fn unset_player_var(&mut self, key: &str) {
        if self.simulated {
            return;
        }
        if let Some(v) = self
            .world
            .objects
            .get_component_mut::<crate::model::components::PlayerVariables>(&self.player)
        {
            v.0.remove(key);
        }
    }

    /// The involved NPC's per-instance scratch value (Java
    /// `Npc.isScriptValue`/`setScriptValue` — reset on respawn because the
    /// respawned NPC is a fresh instance).
    pub fn npc_script_value(&self) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::npc::Npc>(&self.npc)
            .map(|n| n.script_value)
            .unwrap_or(0)
    }

    pub fn set_npc_script_value(&mut self, value: i32) {
        if self.simulated {
            return;
        }
        if let Some(n) = self
            .world
            .objects
            .get_component_mut::<crate::model::npc::Npc>(&self.npc)
        {
            n.script_value = value;
        }
    }

    /// `QuestState.getMemoState()` — Java stores it as the quest variable
    /// `memoState` (`QuestState.MEMO_VAR`), a second progress axis alongside
    /// `cond`: `cond` drives the client's quest window, `memoState` is the
    /// script's own bookkeeping and is never shown.
    pub fn memo_state(&self) -> i32 {
        self.get_int("memoState")
    }

    /// `QuestState.setMemoState(value)`.
    pub fn set_memo_state(&mut self, value: i32) {
        self.set_var("memoState", value.to_string());
    }

    /// `QuestState.getMemoStateEx(slot)` — a *second*, slotted memo axis
    /// (`QuestState.MEMO_EX_VAR + slot`), independent of `memoState`. Quest
    /// 417 packs two counters into one slot via tens/units arithmetic.
    pub fn memo_state_ex(&self, slot: i32) -> i32 {
        self.get_int(&format!("memoStateEx{slot}"))
    }

    /// `QuestState.setMemoStateEx(slot, value)`.
    pub fn set_memo_state_ex(&mut self, slot: i32, value: i32) {
        self.set_var(&format!("memoStateEx{slot}"), value.to_string());
    }

    /// `Attackable.isSpoiled()` — whether a Spoil landed on the involved NPC.
    /// Quest 417 (the Scavenger path) pays out only on spoiled corpses.
    pub fn npc_is_spoiled(&self) -> bool {
        self.world
            .objects
            .get_component::<crate::model::npc::Npc>(&self.npc)
            .is_some_and(|n| n.spoiler_object_id != 0)
    }

    /// `Attackable.getSpoilerObjectId()`.
    pub fn npc_spoiler_object_id(&self) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::npc::Npc>(&self.npc)
            .map(|n| n.spoiler_object_id)
            .unwrap_or(0)
    }

    /// `npc.deleteMe()` — remove the involved NPC from the world.
    pub fn delete_npc(&mut self) {
        if self.simulated {
            return;
        }
        let Some(region) = self
            .world
            .objects
            .get_component::<crate::model::components::RegionCell>(&self.npc)
            .map(|r| r.0)
        else {
            return;
        };
        super::death::despawn_npc(self.world, self.npc, region);
    }

    /// `addSpawn(npcId, npc, randomOffset, 0, …)` followed by
    /// `addAttackPlayerDesire(spawned, player)` — the quest-ambush primitive.
    /// Spawns beside the NPC the player is talking to and sets the newcomer on
    /// them.
    ///
    /// `random_offset` reproduces Java's `Rnd.get(50, 100)` per axis with an
    /// independent sign, so a group of ambushers doesn't stack on one point.
    pub fn spawn_attacker(&mut self, npc_id: i32, random_offset: bool) -> Option<i32> {
        let target = self.player;
        self.spawn_attacker_on(npc_id, random_offset, target)
    }

    /// As [`spawn_attacker`](Self::spawn_attacker) but aims the newcomer at a
    /// specific playable — `addAttackPlayerDesire(spawned, attacker)` where
    /// Java picked the summon over its owner.
    pub fn spawn_attacker_on(
        &mut self,
        npc_id: i32,
        random_offset: bool,
        target_oid: i32,
    ) -> Option<i32> {
        let spawned = self.spawn_near_npc(npc_id, random_offset)?;
        super::ai::seed_attack(self.world, spawned, target_oid);
        Some(spawned)
    }

    /// `player.getAnyServitor()` — the acting player's summoned servitor, `None`
    /// if none is out. The counterpart to [`attack_is_summon`], for the
    /// servitor-duel quests (230) that pit a summon against a rival NPC.
    ///
    /// [`attack_is_summon`]: Self::attack_is_summon
    pub fn owner_servitor(&self) -> Option<i32> {
        super::servitor::servitor_of(self.world, self.player)
    }

    /// `player.getPet()` → its `getControlObjectId()`: the object id of the
    /// collar (a Dragonflute, in quest 421) that summoned the pet, or `None`
    /// when no *pet* is out. A servitor is not a pet — this is the
    /// item-summoned companion only, whose identity quest 421 binds its
    /// hatchling to (`summon.getControlObjectId() == fluteObjectId`).
    pub fn pet_control_object_id(&self) -> Option<i32> {
        let pet = super::servitor::pet_of(self.world, self.player)?;
        self.world
            .objects
            .get_component::<crate::model::components::PetOf>(&pet)
            .map(|p| p.collar_object_id)
    }

    /// Java's `isSummon ? killer.getServitors()…orElse(getPet()) : killer` —
    /// the **Playable** that actually landed the blow, which is what the
    /// ambush scripts set their spawn on. Falls back to the player when the
    /// summon has already gone (Java's `orElse` chain can yield null; a null
    /// target there simply means no desire is set).
    pub fn killing_playable(&self) -> i32 {
        if !self.attack_is_summon {
            return self.player;
        }
        super::servitor::servitor_of(self.world, self.player)
            .or_else(|| super::servitor::pet_of(self.world, self.player))
            .unwrap_or(self.player)
    }

    /// `player.hasSummon()` — a pet or a servitor is out.
    pub fn has_summon(&self) -> bool {
        super::servitor::pet_of(self.world, self.player).is_some()
            || super::servitor::servitor_of(self.world, self.player).is_some()
    }

    /// `player.getInventory().getItemByItemId(id).getObjectId()` — the object id
    /// of the first inventory item of `item_id`, `None` if absent.
    pub fn item_object_id(&self, item_id: i32) -> Option<i32> {
        self.world
            .objects
            .get_component::<Inventory>(&self.player)
            .and_then(|inv| inv.first_of_item(item_id).map(|i| i.object_id))
    }

    /// `getItemByItemId(id).getEnchantLevel()` — the enchant level of the first
    /// inventory item of `item_id`, `None` if absent. Quest 421 reads a
    /// Dragonflute's enchant level as its hatchling's level.
    pub fn item_enchant_level(&self, item_id: i32) -> Option<i32> {
        self.world
            .objects
            .get_component::<Inventory>(&self.player)
            .and_then(|inv| inv.first_of_item(item_id).map(|i| i.enchant_level))
    }

    /// `startQuestTimer("DESPAWN…", delay, npc, null)` for a **spawned** NPC:
    /// schedule its deletion `delay_ms` from now. Unlike [`start_quest_timer`],
    /// which fires `on_timer` carrying the in-context NPC, this deletes an
    /// arbitrary spawned actor with no script round-trip — quest 421's Soul of
    /// Tree Guardian ambush arms one per guardian.
    ///
    /// [`start_quest_timer`]: Self::start_quest_timer
    /// Java `Util.checkIfInRange(range, npc, player, includeZAxis)` for the
    /// quest ctx's own pair — the guard most `onKill` bodies open with, so a
    /// party member farming on the other side of the map does not collect.
    ///
    /// Java measures **centre to centre plus both collision radii**; the port
    /// has the radii on the templates but a quest kill is always npc↔player,
    /// where the difference is a few units against a 1500-unit range. Plain
    /// centre distance is used, and `include_z` matches Java's flag rather
    /// than always being on: several callers pass false.
    pub fn in_range_of_npc(&self, other_oid: i32, range: i32, include_z: bool) -> bool {
        let pos = |oid: i32| {
            self.world
                .objects
                .get_component::<crate::model::components::Position>(&oid)
                .map(|p| (p.x as f64, p.y as f64, p.z as f64))
        };
        let (Some(a), Some(b)) = (pos(self.npc), pos(other_oid)) else {
            // A dead or despawned actor is not "in range" — refusing is the
            // safe answer for a reward gate.
            return false;
        };
        let (dx, dy) = (a.0 - b.0, a.1 - b.1);
        let d2 = dx * dx + dy * dy + if include_z { (a.2 - b.2).powi(2) } else { 0.0 };
        d2 <= (range as f64).powi(2)
    }

    pub fn schedule_despawn(&mut self, npc_oid: i32, delay_ms: u64) {
        if self.simulated {
            return;
        }
        let fire_at = self.world.tick + delay_ms.div_ceil(100);
        self.world
            .scheduler
            .schedule(fire_at, ScheduledTask::DespawnNpc { npc_oid });
    }

    /// `addAttackPlayerDesire(npc, target)` on the **in-context NPC** — send it
    /// after `target_oid` (a servitor, in quest 230's arcana duels), the mirror
    /// of [`spawn_attacker`](Self::spawn_attacker) which seeds a *spawned* NPC.
    pub fn make_npc_attack(&mut self, target_oid: i32) {
        if self.simulated {
            return;
        }
        super::ai::seed_attack(self.world, self.npc, target_oid);
    }

    /// `Attackable.addAbsorber(caster)`: record the acting player as an absorber
    /// of the in-context NPC, tagged with the NPC's HP **right now** (quest
    /// 350's Soul Crystal cast). A repeat cast overwrites, as in Java's map.
    pub fn add_absorber(&mut self) {
        if self.simulated {
            return;
        }
        let Some(hp) = self
            .world
            .objects
            .get_component::<crate::model::components::Vitals>(&self.npc)
            .map(|v| v.cur_hp)
        else {
            return;
        };
        let player = self.player;
        if let Some(a) = self
            .world
            .objects
            .get_component_mut::<crate::model::npc::Absorbers>(&self.npc)
        {
            a.0.insert(player, hp);
        } else {
            let mut a = crate::model::npc::Absorbers::default();
            a.0.insert(player, hp);
            self.world.objects.add_components(&self.npc, a);
        }
    }

    /// Java `levelSoulCrystals`' skill gate: the acting player is in the
    /// in-context NPC's absorber list **and** cast the crystal skill while the
    /// NPC was at ≤ half HP (`AbsorberInfo.getAbsorbedHp() <= maxHp/2`).
    pub fn killer_absorbed_below_half(&self) -> bool {
        let Some(max_hp) = self
            .world
            .objects
            .get_component::<crate::model::components::Vitals>(&self.npc)
            .map(|v| v.max_hp)
        else {
            return false;
        };
        self.world
            .objects
            .get_component::<crate::model::npc::Absorbers>(&self.npc)
            .and_then(|a| a.0.get(&self.player).copied())
            .is_some_and(|hp| hp <= max_hp as f64 / 2.0)
    }

    /// The Soul Crystal data table (`LevelUpCrystalData.xml`).
    pub fn soul_crystal_data(&self) -> &crate::data::SoulCrystalData {
        &self.world.data.soul_crystal_data
    }

    /// Java `getSCForPlayer`: the item id of the **single** Soul Crystal the
    /// acting player carries, or `None` if they hold none or more than one
    /// (an ambiguous inventory levels nothing).
    pub fn single_soul_crystal(&self) -> Option<i32> {
        let inv = self
            .world
            .objects
            .get_component::<Inventory>(&self.player)?;
        let scd = &self.world.data.soul_crystal_data;
        let mut found = None;
        for item in inv.items() {
            if scd.crystal(item.item_id).is_some() {
                if found.is_some() {
                    return None;
                }
                found = Some(item.item_id);
            }
        }
        found
    }

    /// `npc.broadcastSay(NPC_GENERAL, text)` — a literal-text chat bubble from
    /// the in-context NPC (e.g. the Saga finale boss's retreat cry).
    pub fn npc_say_text(&self, text: &str) {
        if self.simulated {
            return;
        }
        super::helpers::npc_say_text(self.world, self.npc, text);
    }

    /// The same literal-text bubble, but from an *arbitrary* npc — a spawned
    /// finale actor rather than the in-context one.
    pub fn broadcast_npc_text(&self, npc_oid: i32, text: &str) {
        if self.simulated {
            return;
        }
        super::helpers::npc_say_text(self.world, npc_oid, text);
    }

    /// Seed aggro from an arbitrary npc onto a target (npc-vs-npc), for the Saga
    /// finale where the companion and boss duel each other.
    pub fn seed_npc_attack(&mut self, npc_oid: i32, target_oid: i32) {
        if self.simulated {
            return;
        }
        super::ai::seed_attack(self.world, npc_oid, target_oid);
    }

    /// `npc.getCurrentHp() / npc.getMaxHp()` for the in-context NPC, as a
    /// fraction in `0.0..=1.0`. A missing or zero-max target reads as full
    /// health, so an HP *threshold* test fails closed rather than firing on
    /// every NPC the script cannot see.
    pub fn npc_hp_ratio(&self) -> f64 {
        hp_fraction(self.world, self.npc).unwrap_or(1.0)
    }

    /// `npc.setTarget(target); npc.doCast(skill)` — a **real** cast by an NPC,
    /// with the skill's effects, not just its animation.
    ///
    /// The counterpart to [`cast_visual_at`], and the difference is the whole
    /// point: `cast_visual_at` broadcasts a `MagicSkillUse` and nothing
    /// happens, while this routes through the same `npc_cast::start_cast` the
    /// boss AIs use, so the target really is rooted / poisoned / cursed. Reach
    /// for the visual one only where Java's call is a bare `broadcastPacket`.
    ///
    /// Returns whether the cast started. It will not when the skill id is
    /// absent from the datapack or `check_use_conditions` refuses — Java's
    /// `doCast` runs its own checks and quietly does nothing on failure, so a
    /// caller that ignores the result matches it.
    ///
    /// [`cast_visual_at`]: QuestCtx::cast_visual_at
    pub fn npc_cast(
        &mut self,
        caster_oid: i32,
        target_oid: i32,
        skill_id: i32,
        level: i32,
    ) -> bool {
        if self.simulated {
            return false;
        }
        let Some(skill) = skill_by_id(self.world, skill_id, level) else {
            return false;
        };
        if !cast::check_use_conditions_pub(self.world, caster_oid, &skill) {
            return false;
        }
        cast::start_cast(self.world, caster_oid, target_oid, &skill);
        true
    }

    /// `<caster>.broadcastPacket(new MagicSkillUse(caster, target, skillId,
    /// level, hitTime, reuse))` — one visual cast from `caster_oid` aimed at
    /// `target_oid`, shown to everyone nearby.
    ///
    /// Distinct from [`cast_visual`], which emits *two* self-casts (one on the
    /// player, one on the in-context NPC). Java's quest scripts mostly want
    /// this shape instead: quest 125's Ulu Kaimu casts at the player
    /// (`npc → player`), quest 235's mixing flash is the player on themselves
    /// (`player → player`). Passing the same oid twice gives the self-cast.
    ///
    /// `reuse` is not on the wire in this chronicle's packet, so it is not a
    /// parameter — Java's differing values (0 vs 1) make no observable
    /// difference here.
    ///
    /// [`cast_visual`]: QuestCtx::cast_visual
    pub fn cast_visual_at(
        &self,
        caster_oid: i32,
        target_oid: i32,
        skill_id: i32,
        level: i32,
        hit_time: i32,
    ) {
        if self.simulated {
            return;
        }
        let at = |oid: i32| {
            self.world
                .objects
                .get_component::<crate::model::components::Position>(&oid)
                .map(|p| (oid, p.x, p.y, p.z))
        };
        let (Some(caster), Some(target)) = (at(caster_oid), at(target_oid)) else {
            return;
        };
        let Some(region) = self
            .world
            .objects
            .get_component::<crate::model::components::RegionCell>(&caster_oid)
            .map(|r| r.0)
        else {
            return;
        };
        let pkt = server_packets::magic_skill_use_raw(caster, target, skill_id, level, hit_time);
        super::helpers::broadcast_near_region(self.world, region, &pkt);
    }

    /// L2J's `Cast(npc, player, skillId, level)` — a purely visual self-cast
    /// `MagicSkillUse` shown on **both** the in-context NPC and the player, to
    /// everyone nearby. The Saga rite uses it for the tablet progression glow
    /// (4546) and the final transform flash (4339).
    pub fn cast_visual(&self, skill_id: i32, level: i32) {
        if self.simulated {
            return;
        }
        for oid in [self.player, self.npc] {
            let pos = maybe_position(self.world, oid);
            let region = self
                .world
                .objects
                .get_component::<crate::model::components::RegionCell>(&oid)
                .map(|r| r.0);
            if let (Some(pos), Some(region)) = (pos, region) {
                let pkt = server_packets::magic_skill_use_raw(
                    (oid, pos.x, pos.y, pos.z),
                    (oid, pos.x, pos.y, pos.z),
                    skill_id,
                    level,
                    6000,
                );
                super::helpers::broadcast_near_region(self.world, region, &pkt);
            }
        }
    }

    /// Whether the object `oid` is dead (or gone). Used by the arcana-duel
    /// `KILLED_ATTACKER` timer to tell whether the challenger's servitor fell.
    pub fn is_oid_dead(&self, oid: i32) -> bool {
        self.world
            .objects
            .get_component::<crate::model::components::Vitals>(&oid)
            .is_none_or(|v| v.dead)
    }

    /// `addSpawn(npcId, x, y, z, …)` + `addAttackPlayerDesire` — a hostile spawn
    /// at a **fixed world position** rather than beside the in-context NPC (which
    /// is what [`spawn_attacker`](Self::spawn_attacker) does). Quest 231's
    /// teleport ambush conjures its King Bugbears at the arrival spot.
    pub fn spawn_attacker_at(&mut self, npc_id: i32, x: i32, y: i32, z: i32) -> Option<i32> {
        if self.simulated {
            return None;
        }
        let spawned = crate::model::npc::spawn_npc_at(self.world, npc_id, x, y, z, -1)?;
        super::death::introduce_npc(self.world, spawned);
        self.link_summoned(spawned);
        super::ai::seed_attack(self.world, spawned, self.player);
        Some(spawned)
    }

    /// `addSpawn(npcId, npc, randomOffset, 0, …)` **without**
    /// `addAttackPlayerDesire` — the newcomer appears and is left alone.
    /// Quest 416 spawns its Durka Spirit this way: Java conjures it beside the
    /// dead spider and does *not* set it on the player, unlike quest 414's
    /// Kuruka. Keep the two apart; aggroing here would be an invention.
    pub fn spawn_near_npc(&mut self, npc_id: i32, random_offset: bool) -> Option<i32> {
        if self.simulated {
            return None;
        }
        let (mut x, mut y, z) = pos_of(self.world, self.npc)?;
        if random_offset {
            for axis in [&mut x, &mut y] {
                let offset = self.world.roll(51) + 50; // Rnd.get(50, 100)
                let sign = if self.world.roll(2) == 0 { -1 } else { 1 };
                *axis += offset * sign;
            }
        }
        let spawned = crate::model::npc::spawn_npc_at(self.world, npc_id, x, y, z, -1)?;
        super::death::introduce_npc(self.world, spawned);
        self.link_summoned(spawned);
        Some(spawned)
    }

    /// Java `getRandomPartyMemberState(player, condition, playerChance, npc)`
    /// / `getRandomPartyMember(player, cond)` — re-target this ctx at a random
    /// qualifying party member and return whether one was found:
    /// `condition == -1` means "any STARTED state", otherwise the member must
    /// be exactly on that cond; the original player is weighted
    /// `player_chance`× (retail kill credit favours the killer 2-3×); and the
    /// pick must stand within `AltPartyRange` of the involved NPC. Solo, the
    /// player is the only candidate. On success `ctx.player`/`ctx.client_id`
    /// point at the pick, so every later give/set lands on them, like Java's
    /// returned `QuestState`.
    pub fn retarget_random_party_member(&mut self, condition: i32, player_chance: i32) -> bool {
        let name = self.script.name();
        let qualifies = |world: &World, oid: i32| {
            world
                .objects
                .get_component::<Quests>(&oid)
                .and_then(|q| q.0.get(name))
                .is_some_and(|qs| {
                    if condition == -1 {
                        qs.state == state::STARTED
                    } else {
                        qs.state == state::STARTED && qs.cond() == condition
                    }
                })
        };
        let killer = self.player;
        let members: Vec<i32> =
            crate::game_loop::party::party_members(self.world, killer).unwrap_or_default();
        let in_range = |world: &World, oid: i32, npc: i32| {
            if npc == 0 {
                return true;
            }
            crate::geo::distance::within_3d(
                world,
                oid,
                npc,
                f64::from(world.cfg.character.alt_party_range),
            )
        };
        if members.is_empty() {
            // Java's solo arm range-checks too.
            return qualifies(self.world, killer) && in_range(self.world, killer, self.npc);
        }
        let mut candidates: Vec<i32> = Vec::new();
        if qualifies(self.world, killer) {
            for _ in 0..player_chance.max(1) {
                candidates.push(killer);
            }
        }
        for &m in &members {
            if m != killer && qualifies(self.world, m) {
                candidates.push(m);
            }
        }
        if candidates.is_empty() {
            return false;
        }
        let pick = candidates[self.world.roll(candidates.len() as i32) as usize];
        // `checkDistanceToTarget`: within `AltPartyRange` (3D) of the NPC.
        if !in_range(self.world, pick, self.npc) {
            return false;
        }
        self.player = pick;
        self.client_id = client_for_player(self.world, pick).unwrap_or(0);
        true
    }

    /// `addSpawn(npcId, x, y, z, heading, false, 0)` **without** any attack
    /// desire — a fixed-spot bystander. Quest 227's staged duels spawn the
    /// decoy Ol Mahum Pilgrim (and its attacker) this way; the aggro is
    /// seeded separately, at the decoy, not the player.
    pub fn spawn_bystander_at(&mut self, npc_id: i32, x: i32, y: i32, z: i32) -> Option<i32> {
        if self.simulated {
            return None;
        }
        let spawned = crate::model::npc::spawn_npc_at(self.world, npc_id, x, y, z, 0)?;
        super::death::introduce_npc(self.world, spawned);
        self.link_summoned(spawned);
        Some(spawned)
    }

    /// `summoner.addSummonedNpc(npc)` — record that the *talking* NPC spawned
    /// this one, so [`summoned_npc_count`](Self::summoned_npc_count) can cap
    /// repeat spawns.
    ///
    /// [`Self::summoned_npc_count`]: Self::summoned_npc_count
    fn link_summoned(&mut self, spawned: i32) {
        use crate::model::components::SummonedNpcs;
        let npc = self.npc;
        match self.world.objects.get_component_mut::<SummonedNpcs>(&npc) {
            Some(list) => list.0.push(spawned),
            None => self
                .world
                .objects
                .add_components(&npc, SummonedNpcs(vec![spawned])),
        }
    }

    /// Java `npc.getSummonedNpcCount()` — how many of the NPCs this one spawned
    /// are still in the world. Scripts gate re-spawns on it (`< 1`, `< 5`,
    /// `< 10`, …) so a repeatedly-clicked dialog doesn't flood the map.
    ///
    /// Dead-but-undecayed children still count: Java unlinks at `onDecay`, and
    /// a corpse is still an object here too.
    pub fn summoned_npc_count(&self) -> usize {
        use crate::model::components::SummonedNpcs;
        self.world
            .objects
            .get_component::<SummonedNpcs>(&self.npc)
            .map(|l| {
                l.0.iter()
                    .filter(|oid| {
                        self.world
                            .objects
                            .has_component::<crate::model::npc::Npc>(oid)
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    /// `npc.getVariables().getInt(key)` — 0 when unset, as in Java.
    pub fn npc_var_int(&self, key: &str) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::npc::Npc>(&self.npc)
            .and_then(|n| n.vars.get(key).copied())
            .unwrap_or(0)
    }

    /// `npc.getVariables().set(key, value)`.
    pub fn set_npc_var_int(&mut self, key: &str, value: i32) {
        if self.simulated {
            return;
        }
        if let Some(n) = self
            .world
            .objects
            .get_component_mut::<crate::model::npc::Npc>(&self.npc)
        {
            n.vars.insert(key.to_string(), value);
        }
    }

    /// `player.getActiveWeaponInstance()`'s item id, or 0 when bare-handed.
    pub fn equipped_weapon_id(&self) -> i32 {
        self.world
            .objects
            .get_component::<Inventory>(&self.player)
            .map(|inv| inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand))
            .unwrap_or(0)
    }

    /// Quest 415's `checkWeapon`: **bare hands or a fist weapon**
    /// (`weapon == null || FIST || DUALFIST`). Note this is the *inverse*
    /// shape of quests 401/403, which demand one specific weapon id — an Orc
    /// Monk fights unarmed, so "no weapon" is the pass case here, not the
    /// fail case.
    pub fn is_bare_or_fist_handed(&self) -> bool {
        let weapon = self.equipped_weapon_id();
        if weapon == 0 {
            return true;
        }
        matches!(
            self.world.data.item_data.weapon_type(weapon),
            crate::data::item_data::WeaponType::Fist | crate::data::item_data::WeaponType::DualFist
        )
    }

    /// `npc.broadcastPacket(new NpcSay(npc, NPC_GENERAL, npcStringId))`.
    pub fn npc_say(&mut self, npc_string_id: i32) {
        if self.simulated {
            return;
        }
        super::helpers::npc_say(self.world, self.npc, npc_string_id);
    }

    /// `attacker.sendPacket(new NpcSay(npc, NPC_GENERAL, npcStringId))` — the
    /// same line as [`Self::npc_say`] but delivered to the acting player only.
    /// Quest 403's Cat's Eye Bandit taunts its attacker this way while its
    /// death line broadcasts, so the two are not interchangeable.
    pub fn npc_say_to_player(&mut self, npc_string_id: i32) {
        let Some(npc) = self
            .world
            .objects
            .get_component::<crate::model::npc::Npc>(&self.npc)
        else {
            return;
        };
        let pkt = server_packets::npc_say(self.npc, npc.npc_id, npc_string_id);
        self.send(pkt);
    }

    /// `Quest.getAlreadyCompletedMsg` (`data/html/alreadycompleted.htm`).
    pub fn already_completed_html(&self) -> String {
        crate::data::htm_cache::read_htm(format!(
            "{}data/html/alreadycompleted.htm",
            self.world.data.root
        ))
        .unwrap_or_else(|| {
            "<html><body>This quest has already been completed.</body></html>".to_string()
        })
    }

    /// `Quest.getHtm(player, filename)` — the raw html content of one of the
    /// script's files, for scripts that must substitute a placeholder before
    /// returning it (e.g. quest 234's `%weaponname%`). Returning the result to
    /// the framework sends it inline (`showResult`'s `<html>` branch). Empty
    /// string if the file is missing, matching Java's null collapsing to "".
    pub fn get_htm(&self, filename: &str) -> String {
        load_quest_html(self.world, &self.script, filename).unwrap_or_default()
    }

    /// `ItemData.getInstance().getTemplate(id).getName()` — an item's display
    /// name, empty if the id is unknown.
    pub fn item_name(&self, item_id: i32) -> String {
        self.world
            .data
            .item_data
            .get(item_id)
            .map(|t| t.name.clone())
            .unwrap_or_default()
    }

    /// `Quest.startQuestTimer(name, time, npc, player)` — non-repeating
    /// only. Starting a timer with a live same-key predecessor supersedes it
    /// (Java refuses duplicates instead; superseding is the safer default
    /// for the seq scheme and no shipped script relies on the refusal).
    pub fn start_quest_timer(&mut self, name: &str, delay_ms: u64) {
        if self.simulated {
            return;
        }
        let seq = self.world.next_request_seq();
        let quest = self.script.name();
        {
            let timers = self
                .world
                .objects
                .get_component_mut::<QuestTimerSeqs>(&self.player);
            match timers {
                Some(t) => {
                    t.0.insert((quest, name.to_string()), seq);
                }
                None => {
                    let mut map = QuestTimerSeqs::default();
                    map.0.insert((quest, name.to_string()), seq);
                    self.world.objects.add_components(&self.player, map);
                }
            }
        }
        let fire_at = self.world.tick + delay_ms.div_ceil(100);
        self.world.scheduler.schedule(
            fire_at,
            ScheduledTask::QuestTimer {
                quest,
                name: name.to_string(),
                player: self.player,
                npc: self.npc,
                seq,
            },
        );
    }

    /// `QuestTimer.cancel`: bump the stored seq so the scheduled task
    /// no-ops.
    pub fn cancel_quest_timer(&mut self, name: &str) {
        let seq = self.world.next_request_seq();
        if let Some(t) = self
            .world
            .objects
            .get_component_mut::<QuestTimerSeqs>(&self.player)
        {
            t.0.insert((self.script.name(), name.to_string()), seq);
        }
    }

    // --- Tutorial window / global-event helpers (Q255) ---------------------

    /// `QuestState.isMemoState`.
    pub fn is_memo_state(&self, value: i32) -> bool {
        self.memo_state() == value
    }

    /// Another quest's `getMemoState` (with Java's STARTED gate).
    pub fn other_quest_memo_state(&self, quest_name: &str) -> i32 {
        self.world
            .objects
            .get_component::<Quests>(&self.player)
            .and_then(|q| q.0.get(quest_name))
            .map(|qs| {
                if qs.is_started() {
                    qs.get_int(crate::model::quest::MEMO_VAR)
                } else {
                    0
                }
            })
            .unwrap_or(0)
    }

    /// Whether the player has a quest state for another quest at all (Java
    /// `player.getQuestState(name) != null`).
    pub fn has_other_quest_state(&self, quest_name: &str) -> bool {
        self.world
            .objects
            .get_component::<Quests>(&self.player)
            .is_some_and(|q| q.0.contains_key(quest_name))
    }

    /// Write a var on *another* quest's state (Java
    /// `getQuestState(name).set(...)` — the NewbieGuide advancing Q255's
    /// memoState). No-op when the player has no state for that quest.
    pub fn set_other_quest_var(&mut self, quest_name: &str, var: &str, value: impl Into<String>) {
        if let Some(qs) = self
            .world
            .objects
            .get_component_mut::<Quests>(&self.player)
            .and_then(|q| q.0.get_mut(quest_name))
        {
            qs.vars.insert(var.to_string(), value.into());
        }
    }

    /// `TutorialShowHtml` with the content of a file from the script's html
    /// dir (Java `showTutorialHtml(getHtm(player, file))`).
    pub fn tutorial_show_html_file(&mut self, filename: &str) {
        let html = load_quest_html(self.world, &self.script, filename)
            .unwrap_or_else(|| format!("<html><body>File {filename} not found.</body></html>"));
        self.send(server_packets::tutorial_show_html(&html));
    }

    pub fn tutorial_show_question_mark(&mut self, mark_id: i32) {
        self.send(server_packets::tutorial_show_question_mark(mark_id));
    }

    pub fn tutorial_close_html(&mut self) {
        self.send(server_packets::tutorial_close_html());
    }

    /// `playTutorialVoice` — a `PlaySound(2, voice, …)` anchored at the
    /// player's position.
    pub fn play_tutorial_voice(&mut self, voice: &str) {
        let Some(pos) = self
            .world
            .objects
            .get_component::<crate::model::components::Position>(&self.player)
            .copied()
        else {
            return;
        };
        self.send(server_packets::play_tutorial_voice(
            voice, pos.x, pos.y, pos.z,
        ));
    }

    /// `ExShowScreenMessage` (the tutorial uses TOP_CENTER = 2).
    pub fn show_screen_message(&mut self, text: &str, position: i32, time_ms: i32) {
        self.send(server_packets::ex_show_screen_message(
            text, position, time_ms,
        ));
    }

    /// The template id of whatever the player currently targets, 0 when
    /// nothing / not an NPC (Java `player.getTarget().getId()`).
    pub fn player_target_npc_id(&self) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::components::TargetRef>(&self.player)
            .and_then(|t| t.0)
            .and_then(|oid| {
                self.world
                    .objects
                    .get_component::<crate::model::npc::Npc>(&oid)
            })
            .map(|n| n.npc_id)
            .unwrap_or(0)
    }

    /// `Npc.dropItem(killer, …)`: toss an item on the ground at the involved
    /// NPC's feet (the tutorial gremlins' Blue Gemstone).
    pub fn drop_item_from_npc(&mut self, item_id: i32, count: i64) {
        let Some(pos) = self
            .world
            .objects
            .get_component::<crate::model::components::Position>(&self.npc)
            .copied()
        else {
            return;
        };
        let npc = self.npc;
        super::ground_items::spawn_ground_item(
            self.world,
            item_id,
            count,
            0,
            pos.x,
            pos.y,
            pos.z,
            npc,
            super::ground_items::DropSource::Npc,
        );
    }

    /// Ground items of `item_id` within `radius` (2D) of the involved NPC
    /// (Java's `World.getVisibleObjectsInRange` gem-count cap).
    pub fn count_ground_items_near_npc(&self, item_id: i32, radius: f64) -> usize {
        let Some(npos) = self
            .world
            .objects
            .get_component::<crate::model::components::Position>(&self.npc)
        else {
            return 0;
        };
        self.world
            .ground_item_regions
            .values()
            .flat_map(|v| v.iter())
            .filter(|oid| {
                self.world
                    .objects
                    .get_component::<crate::model::components::GroundItem>(oid)
                    .is_some_and(|g| g.item_id == item_id)
                    && self
                        .world
                        .objects
                        .get_component::<crate::model::components::Position>(oid)
                        .is_some_and(|p| npos.distance_2d(p) <= radius)
            })
            .count()
    }

    fn send(&self, pkt: Vec<u8>) {
        // A simulated probe must stay invisible to the client: Java's
        // `_simulated` guards sit in front of the `QuestList` /
        // `ExShowQuestMark` / quest-sound sends for the same reason.
        if self.simulated {
            return;
        }
        send_to_client(self.world, self.client_id, pkt);
    }

    /// Push a fresh `QuestList` (Java sends it after every state/cond
    /// change).
    pub fn send_quest_list(&self) {
        let Some(quests) = self.world.objects.get_component::<Quests>(&self.player) else {
            return;
        };
        let pkt = ew::quest_list(quests, &self.world.quests);
        self.send(pkt);
    }
}

// ---------------------------------------------------------------------------
// Shared item plumbing (outside the ctx so non-quest callers could reuse it)
// ---------------------------------------------------------------------------

/// `Player.addItem("Quest", …)` + `sendItemGetMessage`: SM 52/53/54 ("You
/// have earned …") + `InventoryUpdate`.
///
/// Deliberately **no** `ExQuestItemList` here, matching Java: that packet is
/// only ever sent by `EnterWorld` and by `Player.sendItemList`, which always
/// puts a full `ItemList` in front of it. The client treats it as a list to
/// append to the inventory it was just handed, not as a standalone refresh, so
/// firing it bare on every quest item gain appends the whole quest tab again —
/// one visible duplicate row per gain, surviving until the next relog rebuilds
/// the inventory from `ItemList`. The `InventoryUpdate` below is the entire
/// client-side refresh Java performs (`PlayerInventory.addItem`).
pub(crate) fn give_item_with_earned_message(
    world: &mut World,
    client_id: u32,
    player: i32,
    item_id: i32,
    count: i64,
) {
    give_item_with_earned_message_enchanted(world, client_id, player, item_id, count, 0);
}

/// As [`give_item_with_earned_message`], but stamping `enchant` on what it
/// creates.
///
/// **Java never needs this.** An enchanted item keeps its `+N` across a drop
/// and pickup there because both move the *same* `Item` instance between
/// containers; this port mints a fresh instance on the give path, so the level
/// has to be carried across explicitly. It must be stamped *before* the
/// `InventoryUpdate` below is built, or the client is told about a `+0` item
/// the server considers enchanted.
pub(crate) fn give_item_with_earned_message_enchanted(
    world: &mut World,
    client_id: u32,
    player: i32,
    item_id: i32,
    count: i64,
    enchant: i32,
) {
    let Some(added) = super::items::add_inventory_item_tracked(world, player, item_id, count)
    else {
        warn!("quest give_items: object-id pool exhausted, dropping {item_id}×{count}");
        return;
    };
    if enchant != 0
        && let Some(inv) = world.objects.get_component_mut::<Inventory>(&player)
    {
        // Enchantable items are never stackable, so this is exactly one
        // freshly-created instance.
        for &(oid, _) in &added {
            inv.set_enchant_level(oid, enchant);
        }
    }
    // Snapshot after the enchant stamp, so the packet carries the `+N`.
    let changes = super::helpers::added_changes(world, player, &added);
    let sm = if item_id == ADENA_ID {
        server_packets::system_message_with(
            sm_ids::YOU_HAVE_EARNED_S1_ADENA,
            &[SmParam::Long(count)],
        )
    } else if count > 1 {
        server_packets::system_message_with(
            sm_ids::YOU_HAVE_EARNED_S2_S1_S,
            &[SmParam::ItemName(item_id), SmParam::Long(count)],
        )
    } else {
        server_packets::system_message_with(
            sm_ids::YOU_HAVE_EARNED_S1,
            &[SmParam::ItemName(item_id)],
        )
    };
    send_to_client(world, client_id, sm);
    // `InventoryUpdate` + adena counter + weight bar (Java `sendInventoryUpdate`),
    // so the status-bar adena count refreshes on adena gains (`//create_coin`).
    super::helpers::send_inventory_update(world, player, changes);
}

/// The game-loop half of `takeItems`: `Inventory::remove_item` + DB deletes/
/// count updates + `InventoryUpdate` (with removed entries) + quest-tab
/// refresh.
pub(crate) fn take_items(
    world: &mut World,
    client_id: u32,
    player: i32,
    item_id: i32,
    count: i64,
) -> bool {
    let (changes, unequipped) = {
        let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) else {
            return false;
        };
        // Java's `Inventory.removeItem` unequips whatever it takes out of the
        // bag; here the paperdoll clearing is silent, so note which worn
        // instances the removal took. A quest item can be equipment — Q229
        // `Test of Witchcraft` registers the Sword of Seal (a weapon), and its
        // `exitQuest` sweep destroys it while it is still in the player's hand.
        let equipped_before = inv.equipped_object_ids();
        let changes = inv.remove_item(item_id, count);
        let unequipped = super::items::unequipped_by_removal(&equipped_before, &changes);
        (changes, unequipped)
    };
    if changes.is_empty() {
        return false;
    }
    // Memory-first: the count decrements / removals already applied to the
    // `Inventory` component; they persist on the next flush.
    //
    // Java unequips *before* the destroy's `InventoryUpdate` goes out (the
    // `ExUserInfoEquipSlot` comes from inside `setPaperdollItem`), so this
    // runs first — without it the client keeps rendering a destroyed weapon.
    super::items::finish_equipped_item_destroyed(world, client_id, player, &unequipped);
    // As in `give_item_with_earned_message`, no bare `ExQuestItemList` — Java's
    // `takeItems` → `destroyItemByItemId` sends only the `InventoryUpdate`, and
    // the change-type-3 entries below are what retire the client's rows.
    super::helpers::send_inventory_update(world, player, changes);
    true
}

// ---------------------------------------------------------------------------
// Entry points (bypass router / kill hook / scheduler / abort packet)
// ---------------------------------------------------------------------------

/// The `QuestLink` bypass handler: `Quest` (chooser), `Quest <Name>`
/// (talk), `Quest <Name> <event>` (html-button event). `command` is the
/// full bypass command starting with `Quest`.
pub(crate) fn quest_link(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    command: &str,
) {
    let rest = command
        .strip_prefix("Quest")
        .map(|r| r.trim())
        .unwrap_or("");
    if rest.is_empty() {
        show_quest_window_all(world, client_id, player, npc_oid);
    } else if let Some((name, event)) = rest.split_once(' ') {
        process_quest_event(world, client_id, player, npc_oid, name, event.trim());
    } else {
        show_quest_window(world, client_id, player, npc_oid, rest);
    }
}

/// `QuestLink.showQuestWindow(player, npc)`: gather the NPC's talk quests →
/// chooser when several, straight to the single one, `noquest.htm` when
/// none. Quests whose simulated `onTalk` would only show the no-quest
/// message are dropped first, exactly as Java does — see
/// [`talk_shows_no_quest`].
fn show_quest_window_all(world: &mut World, client_id: u32, player: i32, npc_oid: i32) {
    let npc_id = npc_id_of(world, npc_oid).unwrap_or(0);
    let registry = world.quests.clone();
    // Opted-in utility scripts (`bare_talk`, e.g. TeleportWithCharm) run
    // their `on_talk` from the bare quest-window route; a returned html
    // ends the interaction (see the trait method's deviation note).
    for script in registry.talk_quests(npc_id) {
        if script.id() > 0 || !script.bare_talk() {
            continue;
        }
        let html = {
            let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, script.clone());
            script.on_talk(&mut ctx)
        };
        if let Some(html) = html {
            show_result(world, client_id, npc_oid, &script, Some(html));
            return;
        }
    }
    let candidates: Vec<_> = registry
        .talk_quests(npc_id)
        .into_iter()
        .filter(|q| q.id() > 0 && q.id() < 20000 && q.id() != 255)
        .collect();
    let mut quests = Vec::with_capacity(candidates.len());
    for q in candidates {
        if !talk_shows_no_quest(world, client_id, player, npc_oid, &q) {
            quests.push(q);
        }
    }
    match quests.len() {
        0 => send_no_quest_html(world, client_id, npc_oid),
        1 => show_quest_window(world, client_id, player, npc_oid, quests[0].name()),
        _ => show_quest_choose_window(world, client_id, player, npc_oid, &quests),
    }
}

/// Java's `Quest.getNoQuestMsg(player).equals(quest.onTalk(npc, player,
/// true))` probe: run the script's talk handler on a [simulated] context and
/// report whether all it would produce is `noquest.htm`. Both quest-window
/// routes drop such quests — a quest that has nothing to say at this NPC is
/// not listed at all.
///
/// This is not cosmetic. The chooser labels its buttons with the client
/// strings `<questId>01/02/03`, and the one-time class-change quests only
/// ship `01` ("Path of the Human Wizard") and `02` ("… (In Progress)") —
/// there is no `40403` for the completed state. Listing a finished Q404 at
/// Parina therefore rendered a *blank* grey button that answered
/// `noquest.htm` when clicked. Java never reaches that button because this
/// filter removes the quest first.
///
/// A script returning `None` is kept, matching Java: `equals(null)` is false.
///
/// [simulated]: QuestCtx::new_simulated
fn talk_shows_no_quest(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    script: &Arc<dyn QuestScript>,
) -> bool {
    let res = {
        let mut ctx = QuestCtx::new_simulated(world, client_id, player, npc_oid, script.clone());
        script.on_talk(&mut ctx)
    };
    match res {
        Some(html) => html == no_quest_html(world),
        None => false,
    }
}

/// `QuestLink.showQuestChooseWindow`: one `<button>` per quest, colored and
/// labeled by state (`<fstring>{questId}01/02/03</fstring>` — client-side
/// strings). A single *available* quest short-circuits straight to it.
fn show_quest_choose_window(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    quests: &[Arc<dyn QuestScript>],
) {
    let npc_id = npc_id_of(world, npc_oid).unwrap_or(0);
    let registry = world.quests.clone();
    let mut started = String::new();
    let mut can_start = String::new();
    let mut cant_start = String::new();
    let mut completed = String::new();

    let mut start_count = 0;
    let mut start_quest: Option<&'static str> = None;
    for q in quests {
        let button = |sb: &mut String, color: &str, suffix: &str| {
            sb.push_str(&format!(
                "<font color=\"{color}\"><button icon=\"quest\" align=\"left\" \
                 action=\"bypass npc_{npc_oid}_Quest {}\"><fstring>{}{suffix}</fstring></button></font>",
                q.name(),
                q.id(),
            ));
        };
        let qstate = world
            .objects
            .get_component::<Quests>(&player)
            .and_then(|qs| qs.0.get(q.name()))
            .map(|qs| (qs.state, qs.is_started()));
        match qstate {
            None | Some((state::CREATED, _)) => {
                if !registry.is_start_npc(q.name(), npc_id) {
                    continue;
                }
                let eligible = {
                    let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, q.clone());
                    q.start_condition_html(&mut ctx).is_none()
                };
                if eligible {
                    start_count += 1;
                    start_quest = Some(q.name());
                    button(&mut can_start, "bbaa88", "01");
                } else {
                    button(&mut cant_start, "a62f31", "01");
                }
            }
            // Java's `else if (getNoQuestMsg(player).equals(quest.onTalk(npc,
            // player, true))) continue;` sits ahead of both remaining arms: a
            // quest with nothing to say at this NPC gets no button at all.
            _ if talk_shows_no_quest(world, client_id, player, npc_oid, q) => continue,
            Some((_, true)) => {
                start_count += 1;
                start_quest = Some(q.name());
                button(&mut started, "ffdd66", "02");
            }
            Some((state::COMPLETED, _)) => button(&mut completed, "787878", "03"),
            _ => {}
        }
    }

    if start_count == 1 {
        show_quest_window(
            world,
            client_id,
            player,
            npc_oid,
            start_quest.expect("count == 1"),
        );
        return;
    }

    let content = if started.is_empty()
        && can_start.is_empty()
        && cant_start.is_empty()
        && completed.is_empty()
    {
        no_quest_html(world)
    } else {
        format!("<html><body>{started}{can_start}{cant_start}{completed}</body></html>")
    };
    let content = content.replace("%objectId%", &npc_oid.to_string());
    send_to_client(
        world,
        client_id,
        server_packets::npc_html_message(npc_oid, &content),
    );
    send_action_failed(world, client_id);
}

/// `QuestLink.showQuestWindow(player, npc, questId)` → `Quest.notifyTalk`:
/// the start-condition gate (only when this NPC starts the quest), else
/// `on_talk`. (The weight-penalty / 40-quest guards are unported — no
/// weight model.)
fn show_quest_window(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    quest_name: &str,
) {
    let registry = world.quests.clone();
    let Some(script) = registry.by_name(quest_name) else {
        send_no_quest_html(world, client_id, npc_oid);
        send_action_failed(world, client_id);
        return;
    };
    world.objects.add_components(&player, LastFolkNpc(npc_oid));
    let npc_id = npc_id_of(world, npc_oid).unwrap_or(0);
    let res = {
        let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, script.clone());
        let gate = if registry.is_start_npc(quest_name, npc_id) && ctx.is_created() {
            script.start_condition_html(&mut ctx)
        } else {
            None
        };
        match gate {
            Some(html) => Some(html),
            None => script.on_talk(&mut ctx),
        }
    };
    show_result(world, client_id, npc_oid, &script, res);
}

/// `Player.processQuestEvent` → `Quest.notifyEvent` → `onEvent`.
fn process_quest_event(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    name: &str,
    event: &str,
) {
    let registry = world.quests.clone();
    let Some(script) = registry.by_name(name) else {
        warn!("Quest event for unknown quest [{name}].");
        return;
    };
    let res = {
        let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, script.clone());
        script.on_event(&mut ctx, event)
    };
    show_result(world, client_id, npc_oid, &script, res);
}

/// `NpcAction`'s first-talk branch: if a script owns this NPC's chat
/// window, run its `onFirstTalk` and report `true` so the caller skips
/// `Npc.showChatWindow` entirely.
pub(crate) fn notify_first_talk(
    world: &mut World,
    client_id: u32,
    player: i32,
    npc_oid: i32,
    npc_id: i32,
) -> bool {
    let registry = world.quests.clone();
    let Some(script) = registry.first_talk_quest(npc_id) else {
        return false;
    };
    let res = {
        let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, script.clone());
        script.on_first_talk(&mut ctx)
    };
    show_result(world, client_id, npc_oid, &script, res);
    true
}

/// `onAggroRangeEnter`: the aggro scan just seeded first hate on a player
/// inside a registered monster's range.
pub(crate) fn notify_aggro_range_enter(
    world: &mut World,
    npc_oid: i32,
    npc_id: i32,
    player_oid: i32,
) {
    let registry = world.quests.clone();
    let scripts = registry.aggro_enter_quests(npc_id);
    if scripts.is_empty() {
        return;
    }
    let Some(client_id) = client_for_player(world, player_oid) else {
        return;
    };
    for script in scripts {
        let mut ctx = QuestCtx::new(world, client_id, player_oid, npc_oid, script.clone());
        script.on_aggro_range_enter(&mut ctx);
    }
}

/// `onSpellFinished`: a registered NPC's cast completed. The in-context
/// player is the cast's target when that target is a player (Java passes it
/// along); handlers that only touch the NPC work either way.
pub(crate) fn notify_spell_finished(
    world: &mut World,
    npc_oid: i32,
    npc_id: i32,
    skill_id: i32,
    target_oid: i32,
) {
    let registry = world.quests.clone();
    let scripts = registry.spell_finished_quests(npc_id);
    if scripts.is_empty() {
        return;
    }
    let is_player_target = world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
        .is_some();
    let (player, client_id) = if is_player_target {
        match client_for_player(world, target_oid) {
            Some(c) => (target_oid, c),
            None => (target_oid, 0),
        }
    } else {
        (0, 0)
    };
    for script in scripts {
        let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, script.clone());
        script.on_spell_finished(&mut ctx, skill_id);
    }
}

/// `Attackable` kill → registered kill quests' `onKill`. Called from
/// `death::npc_do_die` after combat rewards; killer-only (the
/// `getRandomPartyMemberState` party sharing is a documented deviation).
pub(crate) fn notify_kill(
    world: &mut World,
    killer_oid: i32,
    npc_oid: i32,
    npc_id: i32,
    is_summon: bool,
) {
    let registry = world.quests.clone();
    let scripts = registry.kill_quests(npc_id);
    if scripts.is_empty() {
        return;
    }
    let Some(client_id) = client_for_player(world, killer_oid) else {
        return;
    };
    for script in scripts {
        let mut ctx = QuestCtx::new(world, client_id, killer_oid, npc_oid, script.clone());
        // Java's `onKill(npc, killer, isSummon)` third argument — a handful of
        // scripts set the newly-spawned avenger on the *pet* that landed the
        // blow rather than on its owner.
        ctx.attack_is_summon = is_summon;
        script.on_kill(&mut ctx);
    }
}

/// The `onAttack` notification: a registered monster took damage from a
/// player (fired from `combat::npc_receive_damage`, killing blow included).
/// `player_oid` is the quest-acting player — the attacker itself, or a
/// servitor's owner. `skill_id` is the striking skill (`None` for melee) and
/// `is_summon` marks a servitor/pet blow, both surfaced to `on_attack` (Java's
/// `onAttack(npc, player, damage, isSummon, skill)`).
pub(crate) fn notify_attack(
    world: &mut World,
    player_oid: i32,
    npc_oid: i32,
    npc_id: i32,
    skill_id: Option<i32>,
    is_summon: bool,
) {
    let registry = world.quests.clone();
    let scripts = registry.attack_quests(npc_id);
    if scripts.is_empty() {
        return;
    }
    let Some(client_id) = client_for_player(world, player_oid) else {
        return;
    };
    for script in scripts {
        let mut ctx = QuestCtx::new(world, client_id, player_oid, npc_oid, script.clone());
        ctx.attack_skill_id = skill_id;
        ctx.attack_is_summon = is_summon;
        script.on_attack(&mut ctx);
    }
}

/// The `onSkillSee` notification: a registered NPC witnessed a skill cast by
/// `caster_oid`. Fired from the skill-finish path per affected NPC target
/// (quest 350's Soul Crystal absorb is a self-targeted read of the mob).
pub(crate) fn notify_skill_see(
    world: &mut World,
    caster_oid: i32,
    npc_oid: i32,
    npc_id: i32,
    skill_id: i32,
) {
    let registry = world.quests.clone();
    let scripts = registry.skill_see_quests(npc_id);
    if scripts.is_empty() {
        return;
    }
    let Some(client_id) = client_for_player(world, caster_oid) else {
        return;
    };
    for script in scripts {
        let mut ctx = QuestCtx::new(world, client_id, caster_oid, npc_oid, script.clone());
        script.on_skill_see(&mut ctx, skill_id);
    }
}

/// The `ON_PLAYER_LOGIN` notification (Java `Player.onPlayerEnter` →
/// `EventDispatcher`): fired at the end of the enter-world burst for every
/// global-event script. `npc` is 0.
pub(crate) fn notify_login(world: &mut World, client_id: u32, player: i32) {
    let registry = world.quests.clone();
    for script in registry.global_event_quests() {
        let mut ctx = QuestCtx::new(world, client_id, player, 0, script.clone());
        script.on_login(&mut ctx);
    }
}

/// The `ON_PLAYER_PRESS_TUTORIAL_MARK` notification
/// (`RequestTutorialQuestionMark` 0x87).
pub(crate) fn notify_tutorial_mark(world: &mut World, client_id: u32, player: i32, mark_id: i32) {
    let registry = world.quests.clone();
    for script in registry.global_event_quests() {
        let mut ctx = QuestCtx::new(world, client_id, player, 0, script.clone());
        script.on_tutorial_mark(&mut ctx, mark_id);
    }
}

/// The `ON_PLAYER_ITEM_PICKUP` notification (fired from
/// `ground_items::pickup_ground_item` after the give).
pub(crate) fn notify_item_pickup(world: &mut World, client_id: u32, player: i32, item_id: i32) {
    let registry = world.quests.clone();
    for script in registry.global_event_quests() {
        let mut ctx = QuestCtx::new(world, client_id, player, 0, script.clone());
        script.on_item_pickup(&mut ctx, item_id);
    }
}

/// The tutorial window's `bypass`/`link` press (`RequestTutorialPassCmdToServer`
/// 0x86 / `RequestTutorialLinkHtml` 0x85): `tutorial_close` closes the window
/// (Java's `TutorialClose` bypass handler), a `Quest <Name> <event>` command
/// fires the quest event with **no NPC** (this is Java's `OnPlayerBypass`
/// path — the tutorial window has no folk NPC behind it).
pub(crate) fn handle_tutorial_bypass(world: &mut World, client_id: u32, bypass: &str) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let bypass = bypass.trim();
    if bypass == "tutorial_close" {
        send_to_client(world, client_id, server_packets::tutorial_close_html());
        return;
    }
    if let Some(rest) = bypass.strip_prefix("Quest ") {
        let (name, event) = match rest.split_once(' ') {
            Some((n, e)) => (n, e.trim()),
            None => (rest, ""),
        };
        process_quest_event(world, client_id, player, 0, name, event);
    }
}

/// The `onSpawn` notification: a registered NPC just (re)spawned. No player
/// is involved — the ctx carries player/client 0 (see `QuestScript::on_spawn`).
/// Java `CreatureSeeTaskManager.run` — the once-per-second sweep behind
/// `addCreatureSeeId`. Every live watcher NPC scans the 3×3 region block
/// around it for creatures (players and NPCs) within its sight range — the
/// template's aggro range, or `AltPartyRange` when the template has none
/// (Java `initSeenCreatures`) — and fires `on_creature_see` once per newly
/// seen creature. The seen-set persists until the watcher despawns (a fresh
/// spawn starts blank), exactly like Java's per-creature `_seenCreatures`.
pub(crate) fn handle_creature_see_sweep(world: &mut World) {
    let registry = world.quests.clone();
    let mut watchers: Vec<(i32, i32, (i32, i32), crate::model::components::Position)> = Vec::new();
    world.objects.for_each_mut::<(
        &crate::model::npc::Npc,
        &crate::model::components::Position,
        &crate::model::components::Vitals,
        &crate::model::components::RegionCell,
    )>(|(n, p, v, r)| {
        if !v.dead && registry.has_creature_see(n.npc_id) {
            watchers.push((n.object_id, n.npc_id, r.0, *p));
        }
    });
    for (npc_oid, npc_id, region, pos) in watchers {
        let range = {
            let aggro = world.data.npc_data.get(npc_id).map_or(0, |t| t.aggro_range);
            f64::from(if aggro > 0 {
                aggro
            } else {
                world.cfg.character.alt_party_range
            })
        };
        let instance = crate::game_loop::helpers::instance_of(world, npc_oid);
        let in_sight = |world: &World, oid: i32| {
            if crate::game_loop::helpers::instance_of(world, oid) != instance {
                return false;
            }
            if is_dead(world, oid) {
                return false;
            }
            crate::geo::distance::within_3d_xyz(world, oid, pos.x, pos.y, pos.z, range)
        };
        let mut fresh: Vec<i32> = Vec::new();
        // Players in the surrounding block (Java skips invisible ones).
        for pid in world.players_visible_from(region).collect::<Vec<_>>() {
            let hidden = world
                .objects
                .get_component::<crate::model::components::AdminFlags>(&pid)
                .is_some_and(|f| f.hidden);
            if !hidden && in_sight(world, pid) {
                fresh.push(pid);
            }
        }
        // NPCs in the surrounding block.
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(list) = world.npc_regions.get(&(region.0 + dx, region.1 + dy)) {
                    for &noid in list {
                        if noid != npc_oid && in_sight(world, noid) {
                            fresh.push(noid);
                        }
                    }
                }
            }
        }
        if world
            .objects
            .get_component::<crate::model::components::SeenCreatures>(&npc_oid)
            .is_none()
        {
            world
                .objects
                .add_components(&npc_oid, crate::model::components::SeenCreatures::default());
        }
        let newly: Vec<i32> = {
            let Some(seen) = world
                .objects
                .get_component_mut::<crate::model::components::SeenCreatures>(&npc_oid)
            else {
                continue;
            };
            fresh.into_iter().filter(|&c| seen.0.insert(c)).collect()
        };
        for creature in newly {
            let is_player = world
                .objects
                .has_component::<crate::model::Player>(&creature);
            let (player, client_id) = if is_player {
                (creature, client_for_player(world, creature).unwrap_or(0))
            } else {
                (0, 0)
            };
            for script in registry.creature_see_quests(npc_id) {
                let mut ctx = QuestCtx::new(world, client_id, player, npc_oid, script.clone());
                script.on_creature_see(&mut ctx, creature);
            }
        }
    }
    world.scheduler.schedule(
        world.tick + 10,
        crate::scheduler::ScheduledTask::CreatureSeeSweep,
    );
}

pub(crate) fn notify_spawn(world: &mut World, npc_oid: i32, npc_id: i32) {
    let registry = world.quests.clone();
    let scripts = registry.spawn_quests(npc_id);
    for script in scripts {
        let mut ctx = QuestCtx::new(world, 0, 0, npc_oid, script.clone());
        script.on_spawn(&mut ctx);
    }
}

/// `ScheduledTask::QuestTimer` firing: seq-check against `QuestTimerSeqs`,
/// then `on_timer`.
pub(crate) fn handle_quest_timer(
    world: &mut World,
    quest: &'static str,
    name: &str,
    player: i32,
    npc: i32,
    seq: u64,
) {
    let live = world
        .objects
        .get_component::<QuestTimerSeqs>(&player)
        .and_then(|t| t.0.get(&(quest, name.to_string())).copied());
    if live != Some(seq) {
        return; // cancelled or superseded
    }
    if let Some(t) = world.objects.get_component_mut::<QuestTimerSeqs>(&player) {
        t.0.remove(&(quest, name.to_string()));
    }
    let Some(client_id) = client_for_player(world, player) else {
        return;
    };
    let registry = world.quests.clone();
    let Some(script) = registry.by_name(quest) else {
        return;
    };
    let mut ctx = QuestCtx::new(world, client_id, player, npc, script.clone());
    script.on_timer(&mut ctx, name);
}

/// `RequestQuestAbort` (0x63): the quest UI's Abandon button —
/// `qs.exitQuest(true)` + `QuestList`, no sound.
/// `RequestQuestList` (0x62, G33): the client opened its quest journal — resend
/// the `QuestList` (Java `player.sendPacket(new QuestList(player))`). Empty body.
pub(crate) fn handle_request_quest_list(world: &World, client_id: u32) {
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let Some(quests) = world.objects.get_component::<Quests>(&player) else {
        return;
    };
    let pkt = ew::quest_list(quests, &world.quests);
    send_to_client(world, client_id, pkt);
}

pub(crate) fn handle_request_quest_abort(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = crate::network::client_packets::RequestQuestAbort::read(body) else {
        return;
    };
    let Some(player) = world.player_oid(client_id) else {
        return;
    };
    let registry = world.quests.clone();
    let Some(script) = registry.by_id(pkt.quest_id) else {
        return;
    };
    let mut ctx = QuestCtx::new(world, client_id, player, 0, script);
    if ctx.is_started() {
        ctx.exit_quest(true, false);
    }
}

// ---------------------------------------------------------------------------
// Result rendering (Quest.showResult / showHtmlFile / getHtm)
// ---------------------------------------------------------------------------

/// `Quest.showResult`: `.htm`/`.html` → html file; inline `<html>` → plain
/// window; other non-empty strings are Java `sendMessage` (unported — none
/// of the shipped scripts return one; logged).
fn show_result(
    world: &mut World,
    client_id: u32,
    npc_oid: i32,
    script: &Arc<dyn QuestScript>,
    res: Option<String>,
) {
    let Some(res) = res else { return };
    if res.is_empty() {
        return;
    }
    if res.ends_with(".htm") || res.ends_with(".html") {
        show_html_file(world, client_id, npc_oid, script, &res);
    } else if res.starts_with("<html>") {
        let player_name = player_name_of_client(world, client_id);
        let content = res
            .replace("%objectId%", &npc_oid.to_string())
            .replace("%playername%", &player_name)
            .replace("%questname%", script.name());
        send_to_client(
            world,
            client_id,
            server_packets::npc_html_message(npc_oid, &content),
        );
        send_action_failed(world, client_id);
    } else {
        warn!(
            "Quest {}: plain-message result [{res}] (sendMessage unported).",
            script.name()
        );
    }
}

/// `Quest.showHtmlFile`: quest-window packet (`ExNpcQuestHtmlMessage`) for
/// `.htm` results of real quests (`0 < id < 20000`, id ≠ 999), plain
/// `NpcHtmlMessage` otherwise. Missing files send nothing, like Java's
/// null-content branch.
fn show_html_file(
    world: &mut World,
    client_id: u32,
    npc_oid: i32,
    script: &Arc<dyn QuestScript>,
    filename: &str,
) {
    let quest_window = !filename.ends_with(".html");
    let Some(content) = load_quest_html(world, script, filename) else {
        warn!("Quest {}: missing html [{filename}].", script.name());
        return;
    };
    let player_name = player_name_of_client(world, client_id);
    let content = content
        .replace("%objectId%", &npc_oid.to_string())
        .replace("%playername%", &player_name)
        // The shared Saga htmls are quest-agnostic; their bypass buttons carry
        // `%questname%` so one html set serves all 31 Sagas.
        .replace("%questname%", script.name());
    let id = script.id();
    if quest_window && id > 0 && id < 20000 && id != 999 {
        send_to_client(
            world,
            client_id,
            server_packets::ex_npc_quest_html_message(npc_oid, &content, id),
        );
    } else {
        send_to_client(
            world,
            client_id,
            server_packets::npc_html_message(npc_oid, &content),
        );
    }
    send_action_failed(world, client_id);
}

/// `Quest.getHtm`: the script's own folder, then the
/// `data/scripts/quests/<Name>/` fallback.
fn load_quest_html(world: &World, script: &Arc<dyn QuestScript>, filename: &str) -> Option<String> {
    let root = &world.data.root;
    crate::data::htm_cache::read_htm(format!(
        "{root}data/scripts/{}/{filename}",
        script.html_dir()
    ))
    .or_else(|| {
        crate::data::htm_cache::read_htm(format!(
            "{root}data/scripts/quests/{}/{filename}",
            script.name()
        ))
    })
}

/// `Quest.getNoQuestMsg` (`data/html/noquest.htm`, with Java's inline
/// default when the file is missing).
fn no_quest_html(world: &World) -> String {
    crate::data::htm_cache::read_htm(format!("{}data/html/noquest.htm", world.data.root))
        .unwrap_or_else(|| "<html><body>You are either not on a quest that involves this NPC, or you don't meet this NPC's minimum quest requirements.</body></html>".to_string())
}

fn send_no_quest_html(world: &mut World, client_id: u32, npc_oid: i32) {
    let content = no_quest_html(world).replace("%objectId%", &npc_oid.to_string());
    send_to_client(
        world,
        client_id,
        server_packets::npc_html_message(npc_oid, &content),
    );
}

fn player_name_of_client(world: &World, client_id: u32) -> String {
    if let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) {
        world
            .objects
            .get_component::<crate::model::Player>(&session.player_object_id())
            .map(|p| p.name.clone())
            .unwrap_or_default()
    } else {
        String::new()
    }
}
