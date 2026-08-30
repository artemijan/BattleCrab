//! [`QuestCtx`] itself: construction, the `QuestState` reads and writes
//! (cond/var/state, start/exit), and the send primitives that respect the
//! simulated-probe rule.

use super::QuestScript;
use super::no_quest_html;
use crate::game_loop::helpers::npc_id_of;
use crate::game_loop::helpers::send_to_client;
use crate::model::components::Quests;
use crate::model::quest;
use crate::model::quest::COND_VAR;
use crate::model::quest::FLAGS_VAR;
use crate::model::quest::QuestState;
use crate::model::quest::state;
use crate::network::enter_world as ew;
use crate::network::server_packets;
use crate::network::server_packets::quest_sounds;
use crate::world::World;
use std::sync::Arc;
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
    pub(super) script: Arc<dyn QuestScript>,
    /// `onAttack` only: the skill that struck (Java's `Skill skill`), `None`
    /// for a melee swing. `None` in every other callback.
    pub(super) attack_skill_id: Option<i32>,
    /// `onAttack` only: whether the blow came from the player's servitor/pet
    /// (Java's `boolean isSummon`). `false` in every other callback.
    pub(super) attack_is_summon: bool,
    /// Java's `QuestState.setSimulated` / `Player.setSimulatedTalking`: this
    /// context only *probes* what a callback would return — the quest-window
    /// filter runs `on_talk` purely to learn whether the quest has anything
    /// to say at this NPC — so nothing it does may be observable. See
    /// [`QuestCtx::new_simulated`].
    pub(super) simulated: bool,
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
    if world.cfg.premium.enabled
        && crate::game_loop::admin::premium::has_premium_status(world, player)
    {
        exp *= world.cfg.premium.rate_quest_xp;
        sp *= world.cfg.premium.rate_quest_sp;
    }
    let exp = (exp * world.cfg.rates.rate_quest_reward_xp) as i64;
    let sp = (sp * world.cfg.rates.rate_quest_reward_sp) as i64;
    // Java routes quest rewards through `Player.addExpAndSp(exp, sp)`, the
    // two-arg overload — `useBonuses = false`, so vitality neither boosts the
    // reward nor is spent on it.
    crate::game_loop::death::add_exp_and_sp(world, player, exp as f64, sp as f64, false);
    // `AbstractScript.addExpAndSp` closes with
    // `givePcCafePoint(player, addExp * RATE_QUEST_REWARD_XP)` — the premium-
    // and rate-multiplied value, which is exactly `exp` here.
    crate::game_loop::character::pc_cafe::give_point(world, player, exp as f64);
}

impl<'w> QuestCtx<'w> {
    pub(super) fn new(
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
    pub(super) fn new_simulated(
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
        no_quest_html(self.world, self.player)
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

    pub(super) fn send(&self, pkt: Vec<u8>) {
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
