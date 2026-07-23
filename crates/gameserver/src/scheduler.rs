//! One-shot timer scheduler — the Rust replacement for the ~300
//! `ThreadPool.schedule(runnable, delay)` call sites (CONCURRENCY_MODEL §2.2).
//!
//! Entries are keyed by the tick they fire on and **capture object IDs, never
//! references**. When a timer fires and its target is already gone, the task is
//! a no-op — exactly the effect Java gets from the `isDead()`/null checks inside
//! each runnable.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// A scheduled unit of work. Grows one variant per Java `schedule(...)` site as
/// milestones land; for now it only carries the id-capturing shape and a test
/// hook, so the loop and heap can be exercised before any real tasks exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduledTask {
    /// Placeholder used by tests and as the template for real tasks.
    Noop { object_id: i32 },
    /// Java `Servitor.run()` — the 5-second summon-upkeep tick: lifetime
    /// countdown, periodic item consumption, the remain-time packet and the
    /// far-from-owner leash. Reschedules itself while the servitor lives.
    ServitorLifeTick { servitor_oid: i32 },
    /// A grand boss's respawn window elapsing (Java's `*_unlock` quest timer).
    GrandBossRespawn { boss_id: i32 },
    /// Queen Ant's 1 s nurse-heal beat.
    QueenAntHeal { queen_oid: i32 },
    /// One beat of Valakas's entry cinematic.
    ValakasCinematic { valakas_oid: i32, step: u8 },
    /// `"beginning"` — Valakas's wait window elapsed after the first entry;
    /// the boss takes the lair and the entry cinematic runs (G23 slice 21).
    ValakasBeginning,
    /// One beat of Valakas's death cinematic (`die_1`..`die_8`); the last beat
    /// spawns the exit cubes and arms `ValakasRemovePlayers`.
    ValakasDeathCinematic { valakas_oid: i32, step: u8 },
    /// `remove_players` — 15 min after the death cubes appear, the lair empties.
    ValakasRemovePlayers,
    /// Dr. Chaos's paranoia tick (1 s while NORMAL) — drains the timer by the
    /// nearby-player count and transforms at ≤0 (G23 slice 22).
    DrChaosParanoia { dr_chaos_oid: i32 },
    /// One beat of Dr. Chaos's transformation cinematic (beat 5 spawns golem).
    DrChaosTransform { dr_chaos_oid: i32, step: u8 },
    /// The golem's 60 s idle check — reverts to Dr. Chaos after 30 idle min.
    DrChaosGolemDespawn { golem_oid: i32 },
    /// `reset_drchaos` — the golem's death window elapsed; Dr. Chaos returns.
    DrChaosReset,
    /// One of Antharas's five-minute minion waves.
    AntharasMinionWave { antharas_oid: i32 },
    /// `SPAWN_ANTHARAS` — the Heart of Warding's wait window elapsed; the
    /// boss takes the platform and the fight starts (G23 slice 20).
    AntharasSpawn,
    /// One beat of Antharas's entry cinematic.
    AntharasCinematic { antharas_oid: i32, step: u8 },
    /// The second social action `CAMERA_3` forks.
    AntharasSocial { antharas_oid: i32 },
    /// `CLEAR_ZONE` — 15 min after Antharas dies, everything left in the lair
    /// leaves: NPCs (the exit cube, straggler minions) despawn, players are
    /// teleported to the exit.
    AntharasClearZone,
    /// A Core minion's 60 s respawn.
    CoreMinionRespawn { npc_id: i32 },
    /// Clearing Core's minions 20 s after it dies.
    CoreDespawnMinions,
    /// Java `Cubic._skillUseTask` — one action attempt by a player's cubic.
    CubicAction { owner_oid: i32, cubic_id: i32 },
    /// Java `Pet.FeedTask` — a fixed 10 s period that burns food and
    /// auto-eats from the pet's own inventory when it gets hungry.
    PetFeedTick { pet_oid: i32 },
    /// `SkillCaster.run` phase 1 (`launchSkill`), fires `_hitTime` ms after
    /// `startCasting`. The skill/target live in the player's `CastState`;
    /// `cast_seq` must match it or the task is stale (aborted/replaced cast)
    /// and no-ops — the heap-free abort mechanism, same dead-id contract as
    /// everything else here.
    SkillLaunch { player_object_id: i32, cast_seq: u64 },
    /// `SkillCaster.run` phase 2 (`finishSkill`), fires `_cancelTime` ms
    /// after the launch: consume MP/HP, apply effects.
    SkillFinish { player_object_id: i32, cast_seq: u64 },
    /// `SkillCaster.run` end (`stopCasting(false)`), fires `_coolTime` ms
    /// after the finish: the cast slot frees up.
    CastEnd { player_object_id: i32, cast_seq: u64 },
    /// One `SkillChannelizer.run()` tick of a `CA1` cast (G19,
    /// PLAN_G19_GROUND_CHANNELING.md): MP upkeep, re-sweep, apply the
    /// CHANNELING effect scope. Re-schedules itself while the cast (matched
    /// by `cast_seq`, same stale contract as the other cast phases) lives —
    /// `stop_casting` removing the `Casting` component is Java's
    /// `stopChanneling`.
    ChannelingTick { player_object_id: i32, cast_seq: u64 },
    /// An `EffectPoint` totem's fixed-rate `union_skill` cast (G19,
    /// PLAN_G19_SYMBOLS.md). Re-schedules itself while the totem lives; a
    /// dead/despawned totem ends the series (Java cancels the task in
    /// `deleteMe`).
    EffectPointCast { npc_oid: i32 },
    /// `Npc.scheduleDespawn` for an `EffectPoint` totem — the seal's 15 s
    /// lifetime running out.
    EffectPointDespawn { npc_oid: i32 },
    /// `BuffFinishTask`: an active buff's `abnormalTime` has elapsed.
    BuffExpire { player_object_id: i32, skill_id: i32 },
    /// A `DamOverTime` effect's periodic tick (Java `EffectTickTask`, armed by
    /// `BuffInfo` via `scheduleAtFixedRate` at `ticks * EFFECT_TICK_RATIO` ms).
    /// Deals the per-tick poison/bleed damage from `caster` to `target` and
    /// reschedules itself; a no-op that stops the chain once the buff has
    /// expired/been removed (its `BuffExpire` drops it at `abnormalTime`) or the
    /// target is dead — the same dead-id/`isDead` contract as everything here.
    DamOverTimeTick { caster: i32, target: i32, skill_id: i32, skill_level: i32 },
    /// `CreatureAttackTaskManager.onHitTimeNotDual` — one auto-attack swing
    /// landing. The hit was rolled at swing start (Java
    /// `generateAttackTargetData`) and rides along; attacker/target death or
    /// disappearance before it fires makes it a no-op (Java's `isDead` /
    /// dead-ref checks inside the task).
    AttackHit { attacker: i32, target: i32, damage: i32, miss: bool, crit: bool },
    /// The Rust `EVT_READY_TO_ACT`: a player's swing period ended
    /// (`attack_end_tick`), releasing whatever action the swing held back
    /// (`run_queued_action`); a no-op when nothing is queued.
    AttackFinish { object_id: i32 },
    /// An NPC's swing period ended (`CreatureAttackTaskManager` re-firing at the
    /// weapon's attack rate): re-run the AI think so it swings again without
    /// waiting for the coarse 1 s `AttackableAI` tick. A no-op if the NPC has
    /// died, lost the target, or left attack reach.
    NpcAttackReady { npc_oid: i32 },
    /// `DecayTaskManager` firing for a dead NPC: the corpse disappears.
    NpcDecay { npc_object_id: i32 },
    /// A `dbSave` raid boss whose persisted respawn time came due — see
    /// [`crate::game_loop::boss_respawn`]. Carries the spawn definition index
    /// rather than an object id: the boss isn't in the world yet.
    BossRespawn { spawn_ref: (usize, usize, usize) },
    /// A leader's killed minion is due back — see [`crate::game_loop::minions`].
    MinionRespawn { master_object_id: i32, minion_npc_id: i32 },
    /// `ItemsOnGroundManager` cleanup: a dropped ground item auto-destroys after
    /// its lifetime elapses.
    GroundItemDecay { item_object_id: i32 },
    /// `RespawnTaskManager` → `Spawn.respawnNpc`: re-run the spawn line the
    /// dead NPC came from (indices into `GameData.spawn_data`).
    NpcRespawn { spawn_idx: usize, group_idx: usize, npc_idx: usize },
    /// A leader-requested clan dissolution came due (`ClanTable.
    /// scheduleRemoveClan`): destroy the clan if `dissolving_expiry_time` is
    /// still set (a `recover_clan` zeroes it → no-op). Re-armed at boot from
    /// the persisted stamp.
    ClanDissolve { clan_id: i32 },
    /// A non-mutual clan war's 7-day answer window elapsed (`ClanWar.
    /// clanWarTimeout`): if still BLOOD_DECLARATION the war becomes TIE and is
    /// torn down. Re-armed at boot; a war gone MUTUAL makes this a no-op.
    ClanWarTimeout { attacker: i32, attacked: i32 },
    /// The post-end deletion of a finished war's row/memory (Java schedules it
    /// seconds after cancel/timeout — the 5/21-day retention in the constants
    /// is dead code in the live Java path, mirrored as-is).
    ClanWarDelete { clan1: i32, clan2: i32 },
    /// A party/friend invite went unanswered (Java `PartyRequest.
    /// scheduleTimeout` / `_requestExpireTime`): clear the player's
    /// `PendingRequest` if `seq` still matches.
    RequestTimeout { object_id: i32, seq: u64 },
    /// The 12 s `PartyMemberPosition` broadcast (Java's per-party
    /// `_positionBroadcastTask`); reschedules itself while the party lives
    /// and `seq` matches.
    PartyPositionBroadcast { party_id: u32, seq: u64 },
    /// One second of a duel's pre-fight countdown.
    DuelCountdown { duel_id: u32 },
    /// The per-second `checkEndDuelCondition` sweep of a running duel.
    DuelTick { duel_id: u32 },
    /// The 15 s loot-rule-change window elapsed without unanimous approval
    /// (`Party.PARTY_DISTRIBUTION_TYPE_REQUEST_TIMEOUT`).
    PartyLootChangeTimeout { party_id: u32, seq: u64 },
    /// A `Quest.startQuestTimer` firing → `quest.notifyEvent(name, …)` →
    /// `on_timer`. `seq` is checked against the player's `QuestTimerSeqs`
    /// entry for `(quest, name)` — cancelling a timer is bumping that seq
    /// (the cast_seq pattern). `npc` is 0 when the timer has no NPC.
    QuestTimer { quest: &'static str, name: String, player: i32, npc: i32, seq: u64 },
    /// `Door.AutoClose`: a script-opened door's `closeTime` elapsed. Stale
    /// (superseded by a newer open/close → `auto_close_seq` mismatch) = no-op.
    DoorAutoClose { door_object_id: i32, seq: u64 },
    /// `Door.TimerOpen`: a BY_TIME door's cycle toggle; reschedules itself.
    DoorTimerToggle { door_object_id: i32 },
    /// `RecoGiveTask` (Java's per-player `scheduleAtFixedRate`): hand out
    /// recommendations-to-give over time — 10 after the first 2 h online, then 1
    /// every hour. Reschedules itself while the player is online; `seq` must
    /// match the player's `reco_give_seq` or the firing is stale (logout /
    /// relogin) and no-ops.
    RecoGive { player_object_id: i32, seq: u64 },
    /// Java `DailyTaskManager.resetRecommends`, fired daily at 06:30: zero every
    /// player's rec_left and decay rec_have. Reschedules itself 24 h out.
    DailyRecoReset,
    /// `Siege.ScheduleEndSiegeTask`: a castle siege's timed window elapsed —
    /// auto-end the siege.
    SiegeEnd { castle_id: i32 },
    /// A castle's scheduled weekly siege start (`SiegeSchedule.xml`) — begins
    /// the siege and re-arms next week (G24 slice 1).
    SiegeStart { castle_id: i32 },
    /// A cursed weapon's expiry — the wielder's duration ran out, or a
    /// monster-dropped weapon lay un-grabbed past its disappear deadline
    /// (G28). Keyed by item id; a stale timer no-ops via the `end_time`
    /// guard.
    CursedWeaponExpiry { item_id: i32 },
}

struct Entry {
    fire_at: u64,
    seq: u64,
    task: ScheduledTask,
}

// Min-heap on `fire_at`, FIFO within the same tick via `seq`. `BinaryHeap` is a
// max-heap, so all comparisons are reversed.
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.fire_at == other.fire_at && self.seq == other.seq
    }
}
impl Eq for Entry {}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other.fire_at.cmp(&self.fire_at).then_with(|| other.seq.cmp(&self.seq))
    }
}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Default)]
pub struct Scheduler {
    heap: BinaryHeap<Entry>,
    next_seq: u64,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Java: `ThreadPool.schedule(task, delayMs)`. `fire_at` is an absolute tick.
    pub fn schedule(&mut self, fire_at: u64, task: ScheduledTask) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.heap.push(Entry { fire_at, seq, task });
    }

    /// Pop every task due at or before `now`, in (fire_at, insertion) order.
    /// The caller runs each against `&mut World`.
    pub fn drain_due(&mut self, now: u64) -> Vec<ScheduledTask> {
        let mut due = Vec::new();
        while let Some(entry) = self.heap.peek() {
            if entry.fire_at > now {
                break;
            }
            due.push(self.heap.pop().unwrap().task);
        }
        due
    }

    /// Ticks that currently have work queued — a test hook for asserting a
    /// scheduled sequence's *shape* rather than its individual entries.
    #[cfg(test)]
    pub fn pending_ticks_for_test(&self) -> Vec<u64> {
        self.heap.iter().map(|e| e.fire_at).collect()
    }

    /// The task variants currently queued — a test hook for asserting *which*
    /// tasks are pending (not just how many), e.g. counting `SiegeStart`s.
    #[cfg(test)]
    pub fn pending_tasks_for_test(&self) -> Vec<ScheduledTask> {
        self.heap.iter().map(|e| e.task.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_in_tick_then_insertion_order() {
        let mut s = Scheduler::new();
        s.schedule(5, ScheduledTask::Noop { object_id: 2 });
        s.schedule(3, ScheduledTask::Noop { object_id: 1 });
        s.schedule(5, ScheduledTask::Noop { object_id: 3 });

        assert!(s.drain_due(2).is_empty());
        assert_eq!(s.drain_due(3), vec![ScheduledTask::Noop { object_id: 1 }]);
        assert_eq!(
            s.drain_due(10),
            vec![ScheduledTask::Noop { object_id: 2 }, ScheduledTask::Noop { object_id: 3 }]
        );
        assert!(s.is_empty());
    }
}
