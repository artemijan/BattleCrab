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

use std::collections::HashMap;
use std::sync::Arc;

use tracing::warn;

use crate::model::components::{LastFolkNpc, QuestTimerSeqs, Quests};
use crate::model::inventory::Inventory;
use crate::model::quest::{self, state, QuestState, COND_VAR, FLAGS_VAR};
use crate::network::enter_world as ew;
use crate::network::server_packets::{self, quest_sounds, sm_ids, SmParam};
use crate::scheduler::ScheduledTask;
use crate::world::World;

use super::death::ADENA_ID;
use super::helpers::client_for_player;

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
}

impl QuestRegistry {
    pub fn new(scripts: Vec<Arc<dyn QuestScript>>) -> Self {
        let mut by_name = HashMap::new();
        let mut start: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut talk: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut kill: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut attack: HashMap<i32, Vec<usize>> = HashMap::new();
        let mut spawn: HashMap<i32, Vec<usize>> = HashMap::new();
        for (idx, s) in scripts.iter().enumerate() {
            by_name.insert(s.name(), idx);
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
        }
        Self { scripts, by_name, start, talk, kill, attack, spawn }
    }

    pub fn by_name(&self, name: &str) -> Option<Arc<dyn QuestScript>> {
        self.by_name.get(name).map(|&i| self.scripts[i].clone())
    }

    pub fn by_id(&self, quest_id: i32) -> Option<Arc<dyn QuestScript>> {
        self.scripts.iter().find(|s| s.id() == quest_id).cloned()
    }

    pub fn quest_id(&self, name: &str) -> Option<i32> {
        self.by_name.get(name).map(|&i| self.scripts[i].id())
    }

    /// Scripts listing `npc_id` as a talk NPC (the quest-window set).
    pub fn talk_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.talk.get(&npc_id).map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect()).unwrap_or_default()
    }

    /// Scripts listing `npc_id` as a kill NPC.
    pub fn kill_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.kill.get(&npc_id).map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect()).unwrap_or_default()
    }

    /// Scripts listing `npc_id` as an attack NPC.
    pub fn attack_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.attack.get(&npc_id).map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect()).unwrap_or_default()
    }

    /// Scripts listing `npc_id` as a spawn NPC.
    pub fn spawn_quests(&self, npc_id: i32) -> Vec<Arc<dyn QuestScript>> {
        self.spawn.get(&npc_id).map(|v| v.iter().map(|&i| self.scripts[i].clone()).collect()).unwrap_or_default()
    }

    /// Whether `npc_id` is a start NPC of the named script (the
    /// `ON_NPC_QUEST_START` owner check in `notifyTalk`).
    pub fn is_start_npc(&self, name: &str, npc_id: i32) -> bool {
        self.start.get(&npc_id).is_some_and(|v| v.iter().any(|&i| self.scripts[i].name() == name))
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
}

impl<'w> QuestCtx<'w> {
    fn new(world: &'w mut World, client_id: u32, player: i32, npc: i32, script: Arc<dyn QuestScript>) -> Self {
        let npc_id = world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc)
            .map(|n| n.npc_id)
            .unwrap_or(0);
        Self { world, client_id, player, npc, npc_id, script }
    }

    pub fn quest_id(&self) -> i32 {
        self.script.id()
    }

    pub fn quest_name(&self) -> &'static str {
        self.script.name()
    }

    // --- QuestState reads -------------------------------------------------

    fn qs(&self) -> Option<&QuestState> {
        self.world.objects.get_component::<Quests>(&self.player)?.0.get(self.script.name())
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
        let stored = self.qs().and_then(|qs| qs.vars.get(FLAGS_VAR).and_then(|v| v.parse::<i32>().ok()));
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
        if !self.is_started() {
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
        if count <= 0 {
            return;
        }
        give_item_with_earned_message(self.world, self.client_id, self.player, item_id, count);
    }

    /// `AbstractScript.rewardItems` — the turn-in variant with the reward
    /// multipliers (`RateQuestRewardAdena` for adena, `RateQuestReward`
    /// otherwise; the per-EtcItem-type multiplier split is unported).
    pub fn reward_items(&mut self, item_id: i32, count: i64) {
        if count <= 0 {
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

    /// `AbstractScript.giveAdena`.
    pub fn give_adena(&mut self, count: i64, apply_rates: bool) {
        if apply_rates {
            self.reward_items(ADENA_ID, count);
        } else {
            self.give_items(ADENA_ID, count);
        }
    }

    /// `AbstractScript.giveItemRandomly(player, npc, id, amount, limit,
    /// chance, playSound)`: chance and amount ×`RateQuestDrop`, capped at
    /// `limit`; returns true when the limit is (already) reached — the
    /// "collection finished" signal quests key `setCond` off.
    pub fn give_item_randomly(&mut self, item_id: i32, amount: i64, limit: i64, chance: f64, play_sound: bool) -> bool {
        let current = self.quest_items_count(item_id);
        if limit > 0 && current >= limit {
            return true;
        }
        let rate = self.world.cfg.rates.rate_quest_drop;
        let mut amount_to_give = (amount as f64 * rate) as i64;
        let chance_with_bonus = chance * rate;
        let random = self.world.roll_f64();
        if chance_with_bonus >= random && amount_to_give > 0 {
            if limit > 0 && current + amount_to_give > limit {
                amount_to_give = limit - current;
            }
            give_item_with_earned_message(self.world, self.client_id, self.player, item_id, amount_to_give);
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
        take_items(self.world, self.client_id, self.player, item_id, count)
    }

    /// `AbstractScript.addExpAndSp` — quest XP/SP with the
    /// `RateQuestRewardXP/SP` multipliers.
    pub fn add_exp_and_sp(&mut self, exp: i64, sp: i64) {
        let exp = (exp as f64 * self.world.cfg.rates.rate_quest_reward_xp) as i64;
        let sp = (sp as f64 * self.world.cfg.rates.rate_quest_reward_sp) as i64;
        super::death::add_exp_and_sp(self.world, self.player, exp, sp);
    }

    // --- misc --------------------------------------------------------------

    pub fn play_sound(&mut self, sound: &str) {
        let pkt = server_packets::play_sound(sound);
        self.send(pkt);
    }

    /// `Rnd.get(bound)` through the world RNG (test-forceable).
    pub fn roll(&mut self, bound: i32) -> i32 {
        self.world.roll(bound)
    }

    pub fn player_level(&self) -> i32 {
        self.world.objects.get_component::<crate::model::Player>(&self.player).map(|p| p.level).unwrap_or(0)
    }

    /// The `Race` ordinal (`characters.race` — 0 Human, 1 Elf, 2 Dark Elf,
    /// 3 Orc, 4 Dwarf).
    pub fn player_race(&self) -> i32 {
        self.world.objects.get_component::<crate::model::Player>(&self.player).map(|p| p.race).unwrap_or(0)
    }

    /// `Player.isClanLeader` (ClanMaster's LEADER_REQUIRED gate).
    pub fn is_clan_leader(&self) -> bool {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .is_some_and(|p| p.clan_leader)
    }

    pub fn player_class_id(&self) -> i32 {
        self.world.objects.get_component::<crate::model::Player>(&self.player).map(|p| p.class_id).unwrap_or(-1)
    }

    /// `Player.isInCategory(CategoryType.X)` against `CategoryData.xml`.
    pub fn is_in_category(&self, category: &str) -> bool {
        self.world.data.categories.contains(category, self.player_class_id())
    }

    /// The village-master class transfer (`setClassId` + `setBaseClass` +
    /// `broadcastUserInfo`), narrowed to base-class changes — no subclasses
    /// exist, so both ids move together. Persisted immediately through the
    /// regular `StorePlayer` snapshot.
    pub fn set_class_id(&mut self, class_id: i32) {
        if let Some(p) = self.world.objects.get_component_mut::<crate::model::Player>(&self.player) {
            p.class_id = class_id;
            p.base_class_id = class_id;
        }
        super::net::store_player_now(self.world, self.player);
        super::party::broadcast_user_info(self.world, self.player);
    }

    /// `player.teleToLocation(loc)` (TeleportWithCharm and friends).
    pub fn teleport_to(&mut self, x: i32, y: i32, z: i32) {
        super::death::teleport_player(self.world, self.player, x, y, z);
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
        if let Some(n) = self.world.objects.get_component_mut::<crate::model::npc::Npc>(&self.npc) {
            n.script_value = value;
        }
    }

    /// `npc.broadcastPacket(new NpcSay(npc, NPC_GENERAL, npcStringId))`.
    pub fn npc_say(&mut self, npc_string_id: i32) {
        let Some(npc) = self.world.objects.get_component::<crate::model::npc::Npc>(&self.npc) else { return };
        let Some(region) =
            self.world.objects.get_component::<crate::model::components::RegionCell>(&self.npc).map(|r| r.0)
        else {
            return;
        };
        let pkt = server_packets::npc_say(self.npc, npc.npc_id, npc_string_id);
        super::helpers::broadcast_near_region(self.world, region, &pkt);
    }

    /// `Quest.getAlreadyCompletedMsg` (`data/html/alreadycompleted.htm`).
    pub fn already_completed_html(&self) -> String {
        std::fs::read_to_string(format!("{}data/html/alreadycompleted.htm", self.world.data.root))
            .unwrap_or_else(|_| "<html><body>This quest has already been completed.</body></html>".to_string())
    }

    /// `Quest.startQuestTimer(name, time, npc, player)` — non-repeating
    /// only. Starting a timer with a live same-key predecessor supersedes it
    /// (Java refuses duplicates instead; superseding is the safer default
    /// for the seq scheme and no shipped script relies on the refusal).
    pub fn start_quest_timer(&mut self, name: &str, delay_ms: u64) {
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
            ScheduledTask::QuestTimer { quest, name: name.to_string(), player: self.player, npc: self.npc, seq },
        );
    }

    /// `QuestTimer.cancel`: bump the stored seq so the scheduled task
    /// no-ops.
    pub fn cancel_quest_timer(&mut self, name: &str) {
        let seq = self.world.next_request_seq();
        if let Some(t) = self.world.objects.get_component_mut::<QuestTimerSeqs>(&self.player) {
            t.0.insert((self.script.name(), name.to_string()), seq);
        }
    }

    fn send(&self, pkt: Vec<u8>) {
        if let Some(cs) = self.world.clients.get(&self.client_id) {
            cs.send(pkt);
        }
    }

    /// Push a fresh `QuestList` (Java sends it after every state/cond
    /// change).
    pub fn send_quest_list(&self) {
        let Some(quests) = self.world.objects.get_component::<Quests>(&self.player) else { return };
        let pkt = ew::quest_list(quests, &self.world.quests);
        self.send(pkt);
    }
}

// ---------------------------------------------------------------------------
// Shared item plumbing (outside the ctx so non-quest callers could reuse it)
// ---------------------------------------------------------------------------

/// `Player.addItem("Quest", …)` + `sendItemGetMessage`: SM 52/53/54 ("You
/// have earned …") + `InventoryUpdate`, and an `ExQuestItemList` refresh
/// when the item lives in the quest tab.
pub(crate) fn give_item_with_earned_message(world: &mut World, client_id: u32, player: i32, item_id: i32, count: i64) {
    let Some(changed_oids) = super::items::add_inventory_item(world, player, item_id, count) else {
        warn!("quest give_items: object-id pool exhausted, dropping {item_id}×{count}");
        return;
    };
    let is_quest_item = world.data.item_data.get(item_id).is_some_and(|t| t.is_quest_item);
    let Some(inventory) = world.objects.get_component::<Inventory>(&player) else { return };
    let sm = if item_id == ADENA_ID {
        server_packets::system_message_with(sm_ids::YOU_HAVE_EARNED_S1_ADENA, &[SmParam::Long(count)])
    } else if count > 1 {
        server_packets::system_message_with(
            sm_ids::YOU_HAVE_EARNED_S2_S1_S,
            &[SmParam::ItemName(item_id), SmParam::Long(count)],
        )
    } else {
        server_packets::system_message_with(sm_ids::YOU_HAVE_EARNED_S1, &[SmParam::ItemName(item_id)])
    };
    let iu = ew::inventory_update(inventory, &world.data, &changed_oids);
    let quest_list = is_quest_item.then(|| ew::ex_quest_item_list(inventory, &world.data));
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(sm);
    }
    // `InventoryUpdate` + adena counter + weight bar (Java `sendInventoryUpdate`),
    // so the status-bar adena count refreshes on adena gains (`//create_coin`).
    super::helpers::send_inventory_update(world, client_id, player, iu);
    if let Some(q) = quest_list {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(q);
        }
    }
}

/// The game-loop half of `takeItems`: `Inventory::remove_item` + DB deletes/
/// count updates + `InventoryUpdate` (with removed entries) + quest-tab
/// refresh.
pub(crate) fn take_items(world: &mut World, client_id: u32, player: i32, item_id: i32, count: i64) -> bool {
    let changes = {
        let Some(inv) = world.objects.get_component_mut::<Inventory>(&player) else { return false };
        inv.remove_item(item_id, count)
    };
    if changes.is_empty() {
        return false;
    }
    // Memory-first: the count decrements / removals already applied to the
    // `Inventory` component; they persist on the next flush.
    let is_quest_item = world.data.item_data.get(item_id).is_some_and(|t| t.is_quest_item);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(ew::inventory_update_changes(&world.data, &changes));
        if is_quest_item {
            if let Some(inventory) = world.objects.get_component::<Inventory>(&player) {
                cs.send(ew::ex_quest_item_list(inventory, &world.data));
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Entry points (bypass router / kill hook / scheduler / abort packet)
// ---------------------------------------------------------------------------

/// The `QuestLink` bypass handler: `Quest` (chooser), `Quest <Name>`
/// (talk), `Quest <Name> <event>` (html-button event). `command` is the
/// full bypass command starting with `Quest`.
pub(crate) fn quest_link(world: &mut World, client_id: u32, player: i32, npc_oid: i32, command: &str) {
    let rest = command.strip_prefix("Quest").map(|r| r.trim()).unwrap_or("");
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
/// none. (Java pre-filters quests whose simulated `onTalk` would show the
/// no-quest message — the simulation machinery is unported; the chooser may
/// list a quest that then shows `noquest.htm`, which is harmless.)
fn show_quest_window_all(world: &mut World, client_id: u32, player: i32, npc_oid: i32) {
    let npc_id = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).map(|n| n.npc_id).unwrap_or(0);
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
    let quests: Vec<_> = registry
        .talk_quests(npc_id)
        .into_iter()
        .filter(|q| q.id() > 0 && q.id() < 20000 && q.id() != 255)
        .collect();
    match quests.len() {
        0 => send_no_quest_html(world, client_id, npc_oid),
        1 => show_quest_window(world, client_id, player, npc_oid, quests[0].name()),
        _ => show_quest_choose_window(world, client_id, player, npc_oid, &quests),
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
    let npc_id = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).map(|n| n.npc_id).unwrap_or(0);
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
        show_quest_window(world, client_id, player, npc_oid, start_quest.expect("count == 1"));
        return;
    }

    let content = if started.is_empty() && can_start.is_empty() && cant_start.is_empty() && completed.is_empty() {
        no_quest_html(world)
    } else {
        format!("<html><body>{started}{can_start}{cant_start}{completed}</body></html>")
    };
    let content = content.replace("%objectId%", &npc_oid.to_string());
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_oid, &content));
        cs.send(server_packets::action_failed());
    }
}

/// `QuestLink.showQuestWindow(player, npc, questId)` → `Quest.notifyTalk`:
/// the start-condition gate (only when this NPC starts the quest), else
/// `on_talk`. (The weight-penalty / 40-quest guards are unported — no
/// weight model.)
fn show_quest_window(world: &mut World, client_id: u32, player: i32, npc_oid: i32, quest_name: &str) {
    let registry = world.quests.clone();
    let Some(script) = registry.by_name(quest_name) else {
        send_no_quest_html(world, client_id, npc_oid);
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    };
    world.objects.add_components(&player, LastFolkNpc(npc_oid));
    let npc_id = world.objects.get_component::<crate::model::npc::Npc>(&npc_oid).map(|n| n.npc_id).unwrap_or(0);
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
fn process_quest_event(world: &mut World, client_id: u32, player: i32, npc_oid: i32, name: &str, event: &str) {
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

/// `Attackable` kill → registered kill quests' `onKill`. Called from
/// `death::npc_do_die` after combat rewards; killer-only (the
/// `getRandomPartyMemberState` party sharing is a documented deviation).
pub(crate) fn notify_kill(world: &mut World, killer_oid: i32, npc_oid: i32, npc_id: i32) {
    let registry = world.quests.clone();
    let scripts = registry.kill_quests(npc_id);
    if scripts.is_empty() {
        return;
    }
    let Some(client_id) = client_for_player(world, killer_oid) else { return };
    for script in scripts {
        let mut ctx = QuestCtx::new(world, client_id, killer_oid, npc_oid, script.clone());
        script.on_kill(&mut ctx);
    }
}

/// The `onAttack` notification: a registered monster took damage from a
/// player (fired from `combat::npc_receive_damage`, killing blow included).
pub(crate) fn notify_attack(world: &mut World, attacker_oid: i32, npc_oid: i32, npc_id: i32) {
    let registry = world.quests.clone();
    let scripts = registry.attack_quests(npc_id);
    if scripts.is_empty() {
        return;
    }
    let Some(client_id) = client_for_player(world, attacker_oid) else { return };
    for script in scripts {
        let mut ctx = QuestCtx::new(world, client_id, attacker_oid, npc_oid, script.clone());
        script.on_attack(&mut ctx);
    }
}

/// The `onSpawn` notification: a registered NPC just (re)spawned. No player
/// is involved — the ctx carries player/client 0 (see `QuestScript::on_spawn`).
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
pub(crate) fn handle_quest_timer(world: &mut World, quest: &'static str, name: &str, player: i32, npc: i32, seq: u64) {
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
    let Some(client_id) = client_for_player(world, player) else { return };
    let registry = world.quests.clone();
    let Some(script) = registry.by_name(quest) else { return };
    let mut ctx = QuestCtx::new(world, client_id, player, npc, script.clone());
    script.on_timer(&mut ctx, name);
}

/// `RequestQuestAbort` (0x63): the quest UI's Abandon button —
/// `qs.exitQuest(true)` + `QuestList`, no sound.
pub(crate) fn handle_request_quest_abort(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = crate::network::client_packets::RequestQuestAbort::read(body) else { return };
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let registry = world.quests.clone();
    let Some(script) = registry.by_id(pkt.quest_id) else { return };
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
fn show_result(world: &mut World, client_id: u32, npc_oid: i32, script: &Arc<dyn QuestScript>, res: Option<String>) {
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
            .replace("%playername%", &player_name);
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::npc_html_message(npc_oid, &content));
            cs.send(server_packets::action_failed());
        }
    } else {
        warn!("Quest {}: plain-message result [{res}] (sendMessage unported).", script.name());
    }
}

/// `Quest.showHtmlFile`: quest-window packet (`ExNpcQuestHtmlMessage`) for
/// `.htm` results of real quests (`0 < id < 20000`, id ≠ 999), plain
/// `NpcHtmlMessage` otherwise. Missing files send nothing, like Java's
/// null-content branch.
fn show_html_file(world: &mut World, client_id: u32, npc_oid: i32, script: &Arc<dyn QuestScript>, filename: &str) {
    let quest_window = !filename.ends_with(".html");
    let Some(content) = load_quest_html(world, script, filename) else {
        warn!("Quest {}: missing html [{filename}].", script.name());
        return;
    };
    let player_name = player_name_of_client(world, client_id);
    let content = content
        .replace("%objectId%", &npc_oid.to_string())
        .replace("%playername%", &player_name);
    let id = script.id();
    if let Some(cs) = world.clients.get(&client_id) {
        if quest_window && id > 0 && id < 20000 && id != 999 {
            cs.send(server_packets::ex_npc_quest_html_message(npc_oid, &content, id));
        } else {
            cs.send(server_packets::npc_html_message(npc_oid, &content));
        }
        cs.send(server_packets::action_failed());
    }
}

/// `Quest.getHtm`: the script's own folder, then the
/// `data/scripts/quests/<Name>/` fallback.
fn load_quest_html(world: &World, script: &Arc<dyn QuestScript>, filename: &str) -> Option<String> {
    let root = &world.data.root;
    std::fs::read_to_string(format!("{root}data/scripts/{}/{filename}", script.html_dir()))
        .or_else(|_| std::fs::read_to_string(format!("{root}data/scripts/quests/{}/{filename}", script.name())))
        .ok()
}

/// `Quest.getNoQuestMsg` (`data/html/noquest.htm`, with Java's inline
/// default when the file is missing).
fn no_quest_html(world: &World) -> String {
    std::fs::read_to_string(format!("{}data/html/noquest.htm", world.data.root))
        .unwrap_or_else(|_| "<html><body>You are either not on a quest that involves this NPC, or you don't meet this NPC's minimum quest requirements.</body></html>".to_string())
}

fn send_no_quest_html(world: &mut World, client_id: u32, npc_oid: i32) {
    let content = no_quest_html(world).replace("%objectId%", &npc_oid.to_string());
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_oid, &content));
    }
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
