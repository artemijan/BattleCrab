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
    Noop {
        object_id: i32,
    },
    /// The `//server_shutdown|restart` countdown beat (announce marks + the
    /// final `Shutdown::request`).
    ServerShutdownTick,
    /// `ServerRestartManager`'s scheduled restart moment: begin the countdown
    /// and re-arm for the next slot.
    ServerRestartSchedule,
    /// `Player.dismount()`'s 1.5 s post-dismount `broadcastUserInfo()`, armed
    /// only when there is water below — the client needs the swim speeds once
    /// the rider has actually gone under.
    DismountWaterUserInfo {
        object_id: i32,
    },
    /// `//debug doors|geodata|movement` redraw beats (Java `AdminDebug`'s
    /// per-player fixed-rate tasks: 3000/1500/100 ms).
    DebugDoorTick {
        object_id: i32,
    },
    DebugGeoTick {
        object_id: i32,
    },
    DebugMoveTick {
        object_id: i32,
    },
    /// Java `Servitor.run()` — the 5-second summon-upkeep tick: lifetime
    /// countdown, periodic item consumption, the remain-time packet and the
    /// far-from-owner leash. Reschedules itself while the servitor lives.
    ServitorLifeTick {
        servitor_oid: i32,
    },
    /// `ItemManaTaskManager`'s 60-second beat for one worn shadow item: burn a
    /// point of mana, warn at 10/5/1, and unequip+destroy at 0. Java keys the
    /// manager by `Item` object and re-adds from `decreaseMana`; here the
    /// entry is (owner, item object id) and re-arms the same way.
    /// See [`crate::game_loop::item_mana`].
    ItemManaTick {
        player_object_id: i32,
        item_object_id: i32,
    },
    /// A grand boss's respawn window elapsing (Java's `*_unlock` quest timer).
    GrandBossRespawn {
        boss_id: i32,
    },
    /// Queen Ant's 1 s nurse-heal beat.
    QueenAntHeal {
        queen_oid: i32,
    },
    /// Queen Ant's 5 s leash check — drag her too far and she returns home.
    QueenAntDistanceCheck {
        queen_oid: i32,
    },
    /// Orfen's 5 s leash check — dragged past 10000 from her spawn, she resets.
    OrfenDistanceCheck {
        orfen_oid: i32,
    },
    /// Baium's archangels re-pick a target every 5 s (engage a nearby player, or
    /// regroup on Baium); they despawn once he falls.
    BaiumSelectTarget,
    /// Valakas's `skill_task` beat: pick a target and a breath/AoE/utility
    /// skill by his HP band and how surrounded he is, then cast or give chase.
    ValakasSkillTask {
        valakas_oid: i32,
    },
    /// One beat of Baium's wakeUp cinematic (stone → live boss): earthquake,
    /// the social-action poses, porting the waker in and the gift-skill kill.
    BaiumCinematic {
        step: u8,
    },
    /// Baium's 60 s CHECK_ATTACK beat: 30-min-idle reset (revert to stone +
    /// clear the zone) or a `<75%`-HP self-heal after 5 min without a hit.
    BaiumCheckAttack,
    /// 15 minutes after Baium dies: despawn the exit cube and oust any
    /// stragglers still in the lair (Java's post-kill `CLEAR_ZONE`).
    BaiumClearZone,
    /// The first Velociraptors enter, 60 s after the party is admitted.
    SailrenBeginFight,
    /// Sailren enters, 3 minutes after the Tyrannosaurus falls.
    SailrenSpawn,
    /// Sailren's intro invulnerability lifts — the fight begins.
    SailrenAttackEnable {
        sailren_oid: i32,
    },
    /// One beat of Valakas's entry cinematic.
    ValakasCinematic {
        valakas_oid: i32,
        step: u8,
    },
    /// `"beginning"` — Valakas's wait window elapsed after the first entry;
    /// the boss takes the lair and the entry cinematic runs (G23 slice 21).
    ValakasBeginning,
    /// One beat of Valakas's death cinematic (`die_1`..`die_8`); the last beat
    /// spawns the exit cubes and arms `ValakasRemovePlayers`.
    ValakasDeathCinematic {
        valakas_oid: i32,
        step: u8,
    },
    /// Valakas's 60 s regen tick: escalating self-heal, and a 15-min-idle reset
    /// that sends him home and empties the lair.
    ValakasRegen {
        valakas_oid: i32,
    },
    /// `remove_players` — 15 min after the death cubes appear, the lair empties.
    ValakasRemovePlayers,
    /// Dr. Chaos's paranoia tick (1 s while NORMAL) — drains the timer by the
    /// nearby-player count and transforms at ≤0 (G23 slice 22).
    DrChaosParanoia {
        dr_chaos_oid: i32,
    },
    /// One beat of Dr. Chaos's transformation cinematic (beat 5 spawns golem).
    DrChaosTransform {
        dr_chaos_oid: i32,
        step: u8,
    },
    /// The golem's 60 s idle check — reverts to Dr. Chaos after 30 idle min.
    DrChaosGolemDespawn {
        golem_oid: i32,
    },
    /// `reset_drchaos` — the golem's death window elapsed; Dr. Chaos returns.
    DrChaosReset,
    /// One of Antharas's five-minute minion waves.
    AntharasMinionWave {
        antharas_oid: i32,
    },
    /// `SPAWN_ANTHARAS` — the Heart of Warding's wait window elapsed; the
    /// boss takes the platform and the fight starts (G23 slice 20).
    AntharasSpawn,
    /// One beat of Antharas's entry cinematic.
    AntharasCinematic {
        antharas_oid: i32,
        step: u8,
    },
    /// The second social action `CAMERA_3` forks.
    AntharasSocial {
        antharas_oid: i32,
    },
    /// `CLEAR_ZONE` — 15 min after Antharas dies, everything left in the lair
    /// leaves: NPCs (the exit cube, straggler minions) despawn, players are
    /// teleported to the exit.
    AntharasClearZone,
    /// The weekly clan-hall auction close (`ClanHallAuctionManager.onEnd`):
    /// finalize every hall's auction, then re-arm for next week.
    ClanHallAuctionEnd,
    /// A clan hall's lease check (`ClanHall.CheckPaymentTask`): charge the weekly
    /// rent from the owner's warehouse, or revoke the hall if a week overdue.
    ClanHallLeaseCheck {
        hall_id: i32,
    },
    /// A hall function's rental expiry (`ResidenceFunction.onFunctionExpiration`):
    /// re-pay from the clan warehouse or drop the function.
    ClanHallFunctionExpire {
        hall_id: i32,
        func_id: i32,
    },
    /// Antharas's 60 s `SET_REGEN` beat: a self-heal that escalates as his HP
    /// falls (one of four regeneration skills, cast once per band).
    AntharasSetRegen {
        antharas_oid: i32,
    },
    /// Antharas's 60 s `CHECK_ATTACK` beat: a 15-min-idle reset (home + revert
    /// to ALIVE + empty the lair) or a threat re-evaluation.
    AntharasCheckAttack {
        antharas_oid: i32,
    },
    /// A Core minion's 60 s respawn.
    CoreMinionRespawn {
        npc_id: i32,
    },
    /// Clearing Core's minions 20 s after it dies.
    CoreDespawnMinions,
    /// A one-shot "delete this NPC after N ms" — Java's
    /// `startQuestTimer("DESPAWN…", delay, npc, null)` / `deleteMe` pattern for
    /// quest-spawned actors on a lifespan (quest 421's Soul of Tree Guardian
    /// ambush). A no-op if the NPC is already gone, like every dead-id task here.
    DespawnNpc {
        npc_oid: i32,
    },
    /// Boats (G24.5): the ferry reached its current waypoint — snap there,
    /// broadcast its position, and set sail for the next one.
    BoatArrive {
        boat_object_id: i32,
    },
    /// Boats: the dock anchor dwell elapsed — the ferry weighs anchor and sails
    /// on (players could board/disembark while it was anchored).
    BoatDepart {
        boat_object_id: i32,
    },
    /// Boats: run the next stage of a harbor dwell — broadcast its departure
    /// announcements and schedule the following stage (or depart after the last).
    BoatDwellStage {
        boat_object_id: i32,
        stage: usize,
    },
    /// Boats: an in-transit "the ferry will arrive in ~N minutes" shout, fired
    /// at a fixed offset after departure (only while the boat is still sailing).
    BoatVoyageShout {
        boat_object_id: i32,
        messages: &'static [u32],
    },
    /// Olympiad (G25): the daily competition window opens (18:00 on a
    /// competition day) — registration/matches allowed until it closes.
    OlympiadCompStart,
    /// Olympiad: the competition window closes (6 h later); schedule the next.
    OlympiadCompEnd,
    /// Olympiad: the weekly refresh — add weekly points + reset weekly matches.
    OlympiadWeeklyChange,
    /// Olympiad: the match-making sweep (Java `OlympiadGameManager`, every 30 s
    /// while the window is open) — pair waiting nobles into stadium matches.
    OlympiadGameManager,
    /// Olympiad: the pre-fight ceremony step (Java `OlympiadGameTask`'s teleport
    /// + battle countdowns) — announce, teleport, or start the fight.
    OlympiadCountdown {
        arena: usize,
        step: usize,
    },
    /// Olympiad: poll a running match (Java `OlympiadGameTask`) — resolve it on
    /// a death/disconnect or the battle timeout, otherwise keep watching.
    OlympiadMatchTick {
        arena: usize,
    },
    /// Olympiad: the monthly round ends (Java `OlympiadEndTask`) — enter the
    /// validation period and crown the new heroes.
    OlympiadEnd,
    /// Olympiad: the validation period ends (Java `ValidationEndTask`) — start a
    /// new cycle's competition period.
    OlympiadValidationEnd,
    /// Instances (G27): tear down an instance if it is still empty after its
    /// `<time empty>` grace period.
    InstanceEmptyCheck {
        instance_id: i32,
    },
    /// Frintezza (G27 content): one beat of the intro cinematic step machine
    /// (step 0 = the 10-min-later start; the chain spawns the boss ensemble and
    /// hands control back for the fight).
    FrintezzaIntro {
        instance_id: i32,
        step: u8,
    },
    /// Frintezza fight beats — Scarlet's 80%/20% morphs (the second turns
    /// Scarlet into its final form).
    FrintezzaFight {
        instance_id: i32,
        step: u8,
    },
    /// Frintezza plays a random song every 90 s while the fight is on.
    FrintezzaSong {
        instance_id: i32,
    },
    /// The portraits emit a demon every 20 s (up to 24) while the fight is on.
    FrintezzaDemons {
        instance_id: i32,
    },
    /// One beat of the finish cinematic after Scarlet's final form falls
    /// (Frintezza's death, then the doors reopen).
    FrintezzaFinish {
        instance_id: i32,
        step: u8,
    },
    /// Scarlet's combat skill AI tick (Java `ScarletVanHalisha` ATTACK timer):
    /// pick and cast a daemon skill at a random target while engaged.
    ScarletSkill {
        instance_id: i32,
    },
    /// Fishing (G32): the cast's line reels in — roll the bait's win chance,
    /// consume the bait, reward a fish, then schedule the next cast. `cast_seq`
    /// must match the player's `FishingSession` or the task is stale.
    FishingReel {
        player_object_id: i32,
        cast_seq: u64,
    },
    /// Fishing: the post-reel wait elapsed; cast the line again (auto-fish loop).
    FishingCast {
        player_object_id: i32,
        cast_seq: u64,
    },
    /// Java `Cubic._skillUseTask` — one action attempt by a player's cubic.
    CubicAction {
        owner_oid: i32,
        cubic_id: i32,
    },
    /// Java `Pet.FeedTask` — a fixed 10 s period that burns food and
    /// auto-eats from the pet's own inventory when it gets hungry.
    PetFeedTick {
        pet_oid: i32,
    },
    /// Java `PetFeedTask` — the *rider's* copy of the same 10 s clock: a mount
    /// burns feed while ridden and force-dismounts when the bar empties. Keyed
    /// by the rider, and self-cancelling once they are no longer mounted.
    MountFeedTick {
        player_oid: i32,
    },
    /// `SkillCaster.run` phase 1 (`launchSkill`), fires `_hitTime` ms after
    /// `startCasting`. The skill/target live in the player's `CastState`;
    /// `cast_seq` must match it or the task is stale (aborted/replaced cast)
    /// and no-ops — the heap-free abort mechanism, same dead-id contract as
    /// everything else here.
    SkillLaunch {
        player_object_id: i32,
        cast_seq: u64,
    },
    /// `SkillCaster.run` phase 2 (`finishSkill`), fires `_cancelTime` ms
    /// after the launch: consume MP/HP, apply effects.
    SkillFinish {
        player_object_id: i32,
        cast_seq: u64,
    },
    /// `SkillCaster.run` end (`stopCasting(false)`), fires `_coolTime` ms
    /// after the finish: the cast slot frees up.
    CastEnd {
        player_object_id: i32,
        cast_seq: u64,
    },
    /// One `SkillChannelizer.run()` tick of a `CA1` cast (G19,
    /// PLAN_G19_GROUND_CHANNELING.md): MP upkeep, re-sweep, apply the
    /// CHANNELING effect scope. Re-schedules itself while the cast (matched
    /// by `cast_seq`, same stale contract as the other cast phases) lives —
    /// `stop_casting` removing the `Casting` component is Java's
    /// `stopChanneling`.
    ChannelingTick {
        player_object_id: i32,
        cast_seq: u64,
    },
    /// An `EffectPoint` totem's fixed-rate `union_skill` cast (G19,
    /// PLAN_G19_SYMBOLS.md). Re-schedules itself while the totem lives; a
    /// dead/despawned totem ends the series (Java cancels the task in
    /// `deleteMe`).
    EffectPointCast {
        npc_oid: i32,
    },
    /// `Npc.scheduleDespawn` for an `EffectPoint` totem — the seal's 15 s
    /// lifetime running out.
    EffectPointDespawn {
        npc_oid: i32,
    },
    /// `BuffFinishTask`: an active buff's `abnormalTime` has elapsed.
    BuffExpire {
        player_object_id: i32,
        skill_id: i32,
    },
    /// Java `Player.updateAbnormalVisualEffects`'s `_abnormalVisualEffectTask`:
    /// resend the character's visual-effect list **50 ms later**, never inline.
    /// The delay is load-bearing — after a model swap (mount, dismount,
    /// transform) the client rebuilds the actor and drops its per-actor visual
    /// state, so a list arriving in the same batch as the swap is discarded
    /// along with the old actor. Java leans on the same delay: the call at the
    /// end of `Transform.onTransform` carries the comment "you need to
    /// broadcast this to trigger the transformation client-side".
    RefreshVisuals {
        object_id: i32,
    },
    /// A `DamOverTime` effect's periodic tick (Java `EffectTickTask`, armed by
    /// `BuffInfo` via `scheduleAtFixedRate` at `ticks * EFFECT_TICK_RATIO` ms).
    /// Deals the per-tick poison/bleed damage from `caster` to `target` and
    /// reschedules itself; a no-op that stops the chain once the buff has
    /// expired/been removed (its `BuffExpire` drops it at `abnormalTime`) or the
    /// target is dead — the same dead-id/`isDead` contract as everything here.
    DamOverTimeTick {
        caster: i32,
        target: i32,
        skill_id: i32,
        skill_level: i32,
    },
    /// `CreatureAttackTaskManager.onHitTimeNotDual` — one auto-attack swing
    /// landing. The hit was rolled at swing start (Java
    /// `generateAttackTargetData`) and rides along; attacker/target death or
    /// disappearance before it fires makes it a no-op (Java's `isDead` /
    /// dead-ref checks inside the task).
    AttackHit {
        attacker: i32,
        target: i32,
        damage: i32,
        miss: bool,
        crit: bool,
        /// The attacker's `AttackState::swing_seq` when this hit was queued.
        /// A mismatch at fire time means the swing was aborted (Java cancels
        /// the task outright; see the field's own doc comment).
        swing_seq: u64,
    },
    /// The Rust `EVT_READY_TO_ACT`: a player's swing period ended
    /// (`attack_end_tick`), releasing whatever action the swing held back
    /// (`run_queued_action`); a no-op when nothing is queued.
    AttackFinish {
        object_id: i32,
    },
    /// `ResetChargesTask` — the ten-minute Force decay. `seq` is the
    /// `Player::charges_seq` current when it was armed; a mismatch means a
    /// later gain or spend replaced it (see that field).
    ResetCharges {
        player_oid: i32,
        seq: u64,
    },
    /// A scripted `npc.doDie(null)` on a timer — Java's "suicide" event. Not
    /// `DespawnNpc`: dying plays the death animation and leaves a corpse that
    /// decays, where a despawn makes the NPC blink out.
    NpcSuicide {
        npc_oid: i32,
    },
    /// `Player._fameTask` — one payment of the siege-zone fame task, which
    /// re-arms itself while the player still qualifies (see
    /// `game_loop::siege::arm_fame_task`).
    SiegeFame {
        player_oid: i32,
    },
    /// An NPC's swing period ended (`CreatureAttackTaskManager` re-firing at the
    /// weapon's attack rate): re-run the AI think so it swings again without
    /// waiting for the coarse 1 s `AttackableAI` tick. A no-op if the NPC has
    /// died, lost the target, or left attack reach.
    NpcAttackReady {
        npc_oid: i32,
    },
    /// `DecayTaskManager` firing for a dead NPC: the corpse disappears.
    NpcDecay {
        npc_object_id: i32,
    },
    /// A `dbSave` raid boss whose persisted respawn time came due — see
    /// [`crate::game_loop::boss_respawn`]. Carries the spawn definition index
    /// rather than an object id: the boss isn't in the world yet.
    BossRespawn {
        spawn_ref: (usize, usize, usize),
    },
    /// A leader's killed minion is due back — see [`crate::game_loop::minions`].
    MinionRespawn {
        master_object_id: i32,
        minion_npc_id: i32,
    },
    /// `ItemsOnGroundManager` cleanup: a dropped ground item auto-destroys after
    /// its lifetime elapses.
    GroundItemDecay {
        item_object_id: i32,
    },
    /// `RespawnTaskManager` → `Spawn.respawnNpc`: re-run the spawn line the
    /// dead NPC came from (indices into `GameData.spawn_data`).
    NpcRespawn {
        spawn_idx: usize,
        group_idx: usize,
        npc_idx: usize,
    },
    /// A leader-requested clan dissolution came due (`ClanTable.
    /// scheduleRemoveClan`): destroy the clan if `dissolving_expiry_time` is
    /// still set (a `recover_clan` zeroes it → no-op). Re-armed at boot from
    /// the persisted stamp.
    ClanDissolve {
        clan_id: i32,
    },
    /// A non-mutual clan war's 7-day answer window elapsed (`ClanWar.
    /// clanWarTimeout`): if still BLOOD_DECLARATION the war becomes TIE and is
    /// torn down. Re-armed at boot; a war gone MUTUAL makes this a no-op.
    ClanWarTimeout {
        attacker: i32,
        attacked: i32,
    },
    /// The post-end deletion of a finished war's row/memory (Java schedules it
    /// seconds after cancel/timeout — the 5/21-day retention in the constants
    /// is dead code in the live Java path, mirrored as-is).
    ClanWarDelete {
        clan1: i32,
        clan2: i32,
    },
    /// A party/friend invite went unanswered (Java `PartyRequest.
    /// scheduleTimeout` / `_requestExpireTime`): clear the player's
    /// `PendingRequest` if `seq` still matches.
    RequestTimeout {
        object_id: i32,
        seq: u64,
    },
    /// The 12 s `PartyMemberPosition` broadcast (Java's per-party
    /// `_positionBroadcastTask`); reschedules itself while the party lives
    /// and `seq` matches.
    PartyPositionBroadcast {
        party_id: u32,
        seq: u64,
    },
    /// One second of a duel's pre-fight countdown.
    DuelCountdown {
        duel_id: u32,
    },
    /// The per-second `checkEndDuelCondition` sweep of a running duel.
    DuelTick {
        duel_id: u32,
    },
    /// The 15 s loot-rule-change window elapsed without unanimous approval
    /// (`Party.PARTY_DISTRIBUTION_TYPE_REQUEST_TIMEOUT`).
    PartyLootChangeTimeout {
        party_id: u32,
        seq: u64,
    },
    /// A `Quest.startQuestTimer` firing → `quest.notifyEvent(name, …)` →
    /// `on_timer`. `seq` is checked against the player's `QuestTimerSeqs`
    /// entry for `(quest, name)` — cancelling a timer is bumping that seq
    /// (the cast_seq pattern). `npc` is 0 when the timer has no NPC.
    QuestTimer {
        quest: &'static str,
        name: String,
        player: i32,
        npc: i32,
        seq: u64,
    },
    /// `ai/areas` Toma: the 30-minute beat that despawns Toma and respawns
    /// him at one of his three haunts (Java `RESPAWN_TOMA`); reschedules
    /// itself.
    TomaRelocate,
    /// `ai/others/Mammons`: the 30-minute `RESPAWN_MERCHANT` / `_BLACKSMITH` /
    /// `_PRIEST` beat — delete the script's copy of that Mammon and place it at
    /// another haunt. Reschedules itself; `npc_id` says which merchant.
    MammonRelocate {
        npc_id: i32,
    },
    /// `ai/others/CastleTeleporter`: the mass gatekeeper's armed
    /// `MASS_TELEPORT` — pull everyone in the castle's owner-restart territory
    /// back inside and re-arm the gatekeeper. One-shot per arming.
    CastleMassTeleport {
        npc_oid: i32,
    },
    /// `ai/others/RandomWalkingGuards`: a village guard's 15–45 s stroll
    /// beat. Reschedules itself for as long as the guard stands.
    GuardRandomWalk {
        npc_oid: i32,
    },
    /// `ai/others/Servitors/SinEater`: the pet's 60 s chatter beat.
    SinEaterTalk {
        pet_oid: i32,
    },
    /// The in-game day/night watch (Java `OnDayNightChange` listeners):
    /// fires the transition scripts when the flag flips. Reschedules itself,
    /// carrying the last-seen state.
    DayNightCheck {
        was_night: bool,
    },
    /// Eilhalder von Hellmann's daybreak despawn retry — he only vanishes
    /// once out of combat (Java's 30 s `"despawn"` timer).
    EilhalderDespawnRetry,
    /// Forge of the Gods: the 15 s `"refresh"` beat that resets the
    /// Lavasaurus escalation counter; reschedules itself.
    FogRefresh,
    /// A tamed beast's 60 s spice clock (Java `CheckDuration`): consume the
    /// owner's spice to stay, starve out otherwise. Reschedules itself.
    TamedBeastDuration {
        beast_oid: i32,
    },
    /// A tamed beast's 1 s follow beat — trot after the tamer, vanish if
    /// they are gone. Reschedules itself.
    TamedBeastFollow {
        beast_oid: i32,
    },
    /// Java `Player._broadcastCharInfoTask` firing: the 50 ms-coalesced
    /// `CharInfo` broadcast to onlookers.
    BroadcastCharInfo {
        object_id: i32,
    },
    /// The community board teleport's 3 s `disableAllSkills` window ending —
    /// Java's `ThreadPool.schedule(enableAllSkills, 3000)`.
    SkillsReenable {
        object_id: i32,
    },
    /// A tamed beast's 5 s `CheckOwnerBuffs` beat — when the tamer is
    /// missing more than a third of the beast's template buffs, sit-cast a
    /// random one on them. Reschedules itself.
    TamedBeastBuffCheck {
        beast_oid: i32,
    },
    /// Beast Farm: a "mad cow" reverts to the plain top-stage animal 10 s
    /// after emerging, keeping its grudge against the feeder.
    MadCowPolymorph {
        cow_oid: i32,
        feeder_oid: i32,
    },
    /// A Primeval Isle Sprigant's 15 s trap cast (Anesthesia / Deadly
    /// Poison); reschedules itself while the plant lives.
    SprigantTrap {
        npc_oid: i32,
    },
    /// The Tyrannosaurus finished sizing a wanderer up (Java `TREX_ATTACK`,
    /// 6 s after `onAggroRangeEnter`): still close → stun + charge.
    TrexAttack {
        trex_oid: i32,
        player_oid: i32,
    },
    /// Four Sepulchers: the 3-minute entry delay elapsed — the first
    /// mysterious chest appears in the hall.
    FsMysteriousChest {
        sepulcher: i32,
    },
    /// Four Sepulchers: the 5 s wave-defeated poll for waves 2 and 5.
    FsWaveCheck {
        sepulcher: i32,
    },
    /// Four Sepulchers: the 60-minute bell — oust everyone, sweep the hall.
    FsOust {
        sepulcher: i32,
    },
    /// Four Sepulchers: the room-3 victim's flee-and-cry beat (3 s).
    FsVictimFlee {
        npc_oid: i32,
    },
    /// Four Sepulchers: the room-5 statue guards shed their petrification
    /// five minutes after spawning.
    FsRemovePetrify {
        npc_oid: i32,
    },
    /// `Door.AutoClose`: a script-opened door's `closeTime` elapsed. Stale
    /// (superseded by a newer open/close → `auto_close_seq` mismatch) = no-op.
    DoorAutoClose {
        door_object_id: i32,
        seq: u64,
    },
    /// `Door.TimerOpen`: a BY_TIME door's cycle toggle; reschedules itself.
    DoorTimerToggle {
        door_object_id: i32,
    },
    /// `RecoGiveTask` (Java's per-player `scheduleAtFixedRate`): hand out
    /// recommendations-to-give over time — 10 after the first 2 h online, then 1
    /// every hour. Reschedules itself while the player is online; `seq` must
    /// match the player's `reco_give_seq` or the firing is stale (logout /
    /// relogin) and no-ops.
    RecoGive {
        player_object_id: i32,
        seq: u64,
    },
    /// `PcCafePointsManager.run`'s fixed-rate award (Java's per-player
    /// `scheduleAtFixedRate`): pay the flat retail-like PA points every
    /// `PcCafeRewardTime`. Reschedules itself; `seq` must match the player's
    /// `pc_cafe_seq` or the firing is stale and no-ops.
    PcCafeReward {
        player_object_id: i32,
        seq: u64,
    },
    /// Java `DailyTaskManager.onReset`, fired daily at 06:30: the recommends
    /// reset + the vitality daily/weekly refill (G33). Reschedules itself 24 h
    /// out.
    DailyReset,
    /// Java `BotReportTable.ResetPointTask` — every reporter's daily budget
    /// back to 7, at `BotReportPointsResetHour`. Separate from `DailyReset`:
    /// Java schedules it on its own clock (00:00 here, not 06:30).
    BotReportPointsReset,
    /// `Siege.ScheduleEndSiegeTask`: a castle siege's timed window elapsed —
    /// auto-end the siege.
    SiegeEnd {
        castle_id: i32,
    },
    /// A castle's scheduled weekly siege start (`SiegeSchedule.xml`) — begins
    /// the siege and re-arms next week (G24 slice 1).
    SiegeStart {
        castle_id: i32,
    },
    /// The castle-manor daily mode change (`CastleManorManager.changeMode`):
    /// advances APPROVED → MAINTENANCE → MODIFIABLE → APPROVED on the wall
    /// clock, rolls the production/procure period, and re-arms the next change.
    ManorModeChange,
    /// `CastleManorManager`'s autosave (`ThreadPool.scheduleAtFixedRate(this::
    /// storeMe, rate, rate)`), armed only when `AltManorSaveAllActions` is off
    /// — with it on, every action writes and the timer is never scheduled.
    ManorAutosave,
    /// `InventoryEnableTask` — 1500 ms after a shop/warehouse/wear window
    /// opened, inventory refreshes are answered again.
    InventoryEnable {
        object_id: i32,
    },
    /// `SitDownTask` — 2.5 s after sitting, the animation is over and actions
    /// unblock (Java `setBlockActions(false)`).
    SitDownFinish {
        object_id: i32,
    },
    /// `StandUpTask` — 2.5 s after the stand-up broadcast, the character is
    /// really on their feet (`setSitting(false)`).
    StandUpFinish {
        object_id: i32,
    },
    /// A cursed weapon's expiry — the wielder's duration ran out, or a
    /// monster-dropped weapon lay un-grabbed past its disappear deadline
    /// (G28). Keyed by item id; a stale timer no-ops via the `end_time`
    /// guard.
    CursedWeaponExpiry {
        item_id: i32,
    },
    /// A mail message reached its expiration (Java `MessageDeletionTaskManager`,
    /// G30): return any attachments to the sender's warehouse and drop the
    /// message. Keyed by message id; a stale timer no-ops because the message
    /// is gone or its expiration moved.
    MailExpire {
        message_id: i32,
    },
    /// TvT's registration window closed (`startQuestTimer("TeleportToArena",
    /// REGISTRATION_TIME)`, G28): prune offline registrants and either stand up
    /// the arena or cancel for too few players. A singleton event, so no key.
    TvtTeleportToArena,
    /// TvT's warm-up ended (`startQuestTimer("StartFight", WAIT_TIME)`, G28):
    /// open the arena doors and start the fight timer.
    TvtStartFight,
    /// TvT's fight ended (`startQuestTimer("EndFight", FIGHT_TIME)`, G28):
    /// resolve the winner, reward, and tear the arena down.
    TvtEndFight,
    /// A killed TvT participant's timed respawn (`startQuestTimer(
    /// "ResurrectPlayer", 10000, killedPlayer)`, G28): revive at the team spawn
    /// with the Ghost Walking invulnerability. Keyed by the victim's object id;
    /// a stale timer no-ops via the still-dead / still-on-event guard.
    TvtResurrect {
        player: i32,
    },
    /// A cron schedule slot fired (Java's `Schedule<n>` event timer): start the
    /// event and re-arm the slot for its next occurrence. The pattern rides the
    /// task so the re-arm needs no registry lookup.
    EventSchedule {
        index: usize,
        pattern: String,
    },
    /// A TvT participant's inactivity clock (Java's `KickPlayer<oid>` /
    /// `KickPlayerWarning<oid>` quest timers): half-way through, a screen
    /// warning; at the end, the player is ejected from the arena. Re-armed on
    /// every headquarters entry, cancelled on exit — `seq` is the player's
    /// generation, so a re-arm silences the previous pair.
    TvtInactivity {
        player: i32,
        warning: bool,
        seq: u64,
    },
    /// One tick of TvT's second-by-second countdown (Java's `"10"`…`"1"`
    /// quest timers): show the number on the arena's screen. `seq` is the
    /// chain's generation — a forfeit or early end bumps it and every pending
    /// tick from the old chain is dropped.
    TvtCountdown {
        seconds: i32,
        seq: u64,
    },
    /// TvT's end-of-match scoreboard (`startQuestTimer("ScoreBoard", 3500)`,
    /// G28): broadcast `ExPVPMatchCCRecord::FINISH`.
    TvtScoreBoard,
    /// TvT's teleport-out (`startQuestTimer("TeleportOut", 7000)`, G28): unfreeze
    /// participants and tear the arena down.
    TvtTeleportOut,
    /// Open a new Lucky Lottery round (Java `Lottery.startLottery`, G26.5): begin
    /// selling + arm the stop-selling and draw timers. Singleton, so no key.
    LotteryStart,
    /// Suspend ticket sales 10 min before the draw (Java `stopSellingTickets`).
    LotteryStopSelling,
    /// Draw the round and roll over to the next (Java `finishLottery`).
    LotteryFinish,
    /// The Monster Race's 1-second cycle beat (Java `MonsterRace.Announcement`
    /// at fixed 1 s rate, G26.5): advance the countdown timeline. Re-arms itself.
    MonsterRaceTick,
    /// An item auction's next state transition (Java `ItemAuctionInstance
    /// .ScheduleAuctionTask`, G30.5): CREATED→STARTED at its start time, then
    /// STARTED→FINISHED at its (possibly extended) end time. Keyed by auction id.
    ItemAuctionState {
        auction_id: i32,
    },
    /// A timed punishment's expiry (Java `PunishmentTask` scheduling itself at
    /// `expirationTime`, G31): drop the punishment and run its release effect
    /// (jail-out teleport). Keyed by the `punishments.id`; a stale timer (the
    /// row already removed by `//unjail`) no-ops.
    PunishmentExpire {
        punishment_id: i32,
    },
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
        other
            .fire_at
            .cmp(&self.fire_at)
            .then_with(|| other.seq.cmp(&self.seq))
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
            vec![
                ScheduledTask::Noop { object_id: 2 },
                ScheduledTask::Noop { object_id: 3 }
            ]
        );
        assert!(s.is_empty());
    }
}
