//! What a creature is doing right now — attacking, casting, targeting — and
//! the markers that gate those actions.

use bevy_ecs::component::Component;

/// Swing/stance timing (Java `_attackEndTime` + `AttackStanceTaskManager`
/// membership). `stance_until_tick` is player-only in practice (the client
/// sword-drawn state); it stays 0 on NPCs.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AttackState {
    /// Busy-swinging until this tick; the next swing may start once past.
    pub attack_end_tick: u64,
    /// In combat stance until this tick — 15 s past the last swing/hit;
    /// 0 = not in stance.
    pub stance_until_tick: u64,
    /// Which swing the pending `AttackHit` tasks belong to.
    ///
    /// Java's `CreatureAttackTaskManager.abortAttack` holds a handle on the
    /// scheduled hit and cancels it. The port's scheduler is a plain heap with
    /// no cancel, so an abort bumps this counter instead: every `AttackHit`
    /// carries the value current when it was scheduled, and a hit whose value
    /// no longer matches is a swing that was aborted after it was queued and
    /// is dropped when it fires.
    ///
    /// This is equivalent to cancelling, not an approximation of it: the two
    /// disagree only if the counter could be bumped *back*, and it only ever
    /// increments.
    pub swing_seq: u64,
}

/// PvP flag state (Java `Player._pvpFlag` + `_pvpFlagLasts`), runtime-only —
/// never persisted, so it lives in its own component rather than on the stored
/// `Player`. `flag` is the broadcast value: 0 = clean, 1 = solid purple,
/// 2 = blinking (the last 20 s before it clears). `expires_tick` is when the
/// flag drops back to 0; `PvpFlagTaskManager`'s 1 s sweep drives the 1→2→0
/// transitions.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PvpState {
    pub flag: u8,
    pub expires_tick: u64,
}

/// Java `Playable._lockedTarget` — set by the `TargetMe` effect (Aggression
/// 28, Aggression Aura 18) and cleared when it expires. While it is present,
/// `Npc.onAction` refuses to let the bearer select **any other NPC**
/// ("Failed to change enmity"), which is what makes a taunt stick on a player
/// or summon rather than merely nudging their current target.
///
/// **Playables only.** Java's `TargetMe.onStart` is wrapped in
/// `if (effected.isPlayable())`, so taunting a *monster* does nothing here —
/// a monster's aggro comes from `AddHate`/`GetAgro` instead, which is why
/// Aggression carries those effects too.
#[derive(Component, Debug, Clone, Copy)]
pub struct LockedTarget(pub i32);

/// Currently targeted object id (Java `Creature._target`), player-only —
/// NPC targeting goes through the aggro list.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TargetRef(pub Option<i32>);

/// An in-flight cast — **present only mid-cast** (Java's single NORMAL
/// `SkillCaster` slot, `Player.cast` before stage 2). The generation counter
/// (`Player.cast_seq`) stays on the player: it must survive across casts for
/// the scheduler's stale-task no-op contract.
#[derive(Component, Debug, Clone)]
pub struct Casting(pub crate::model::CastState);

/// A persistent AI intention (the attack loop) — **present only while set**,
/// so the player combat tick sweeps intent-holders only.
#[derive(Component, Debug, Clone, Copy)]
pub struct Intent(pub crate::model::PlayerIntent);

/// The one action a busy actor holds back — Java's three queue slots
/// (`PlayerAI._nextIntention` MOVE_TO, `Player._queuedSkill`,
/// `AbstractAI._nextAction` equip) folded into a single presence-based slot.
/// Written while a cast or attack swing is in flight, consumed when it stops
/// (`stop_casting` / the `AttackFinish` task), dropped on death/teleport.
/// The slot is last-click-wins, matching Java's observable outcomes: a move
/// packet wipes `_queuedSkill` (`MoveBackwardToLocation.runImpl`), and a
/// queued skill's `stopCasting` launch supersedes the saved move. Known
/// narrowing: Java keeps the equip in its own slot and can fire it
/// *alongside* a queued move; this single slot keeps only the last click.
#[derive(Component, Debug, Clone, Copy)]
pub enum QueuedAction {
    /// A move click swallowed while busy (`onIntentionMoveTo` →
    /// `saveNextIntention`), or the move a good-skill cast interrupted
    /// (`PlayerAI.changeIntention`).
    Move { x: i32, y: i32, z: i32 },
    /// `Player._queuedSkill` (`SkillUseHolder`): skill + click modifiers, no
    /// target — the target is re-resolved from the player's *current* target
    /// at replay, which is what lets a mid-cast re-target redirect the
    /// queued skill.
    Skill {
        skill_id: i32,
        ctrl: bool,
        shift: bool,
    },
    /// An equipable `UseItem` click (Java defers it via
    /// `NextAction(EVT_FINISH_CASTING, …)` / a swing-end schedule).
    UseItem { item_object_id: i32 },
}

/// Java `Attackable`'s over-hit trio (`_overhitEnabled` / `_overhitDamage` /
/// `_overhitAttacker`): a killing blow from an `<overHit>` skill banks the
/// *excess* damage, which becomes bonus XP for whoever landed it.
///
/// Armed per damaging blow and disarmed by any blow that fails to kill, so it
/// only ever survives on a corpse — exactly Java's `setOverhitValues` contract.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Overhit {
    pub damage: f64,
    pub attacker: i32,
}

/// Java `Creature._disableRangedAttackEndTime` — the tick a bow/crossbow may
/// fire again. Present only after a shot; the reload delay is
/// `900000 / pAtkSpd` ms (`Formulas.calculateReuseTime`).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RangedReload {
    pub ready_at_tick: u64,
}

/// A **summon's** charged shots — Java's `Creature._chargedShots`.
///
/// Java keeps that field on `Creature`, so players and summons share it; this
/// port grew the player half first (`Player.is_charged_shot`) and only needs
/// the summon half now. Kept as a separate component **deliberately** rather
/// than unified: folding `Player`'s shot bits in would touch every player-shot
/// call site for no behavioural gain — revisit only if a third shot carrier
/// ever appears.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ChargedShots {
    pub soulshot: bool,
    pub spiritshot: bool,
}

/// Java `Creature.setLethalable(false)` — this NPC cannot be finished by a
/// lethal blow (`Lethal`/`HalfKill`). Raid bosses are immune by template; this
/// marker is for the ones a script exempts, i.e. `ai/others/NonLethalableNpcs`'
/// siege Headquarters (35062), which would otherwise be one Lethal Strike away
/// from deleting a clan's whole siege investment.
#[derive(Component, Debug, Clone, Copy)]
pub struct NotLethalable;

/// Java `Creature.setImmobilized(true)` — a **movement-only** lock. Unlike
/// `AdminFlags.paralyzed` (which also blocks actions), an immobilized creature
/// can still attack and cast; it simply can't move. Used by stationary bosses
/// like Core, which melee adjacent attackers but never chase.
#[derive(Component, Debug, Clone, Copy)]
pub struct Immobilized;

/// Java `Creature.disableAllSkills()` — the `_allSkillsDisabled` flag, set
/// directly by scripts rather than by any abnormal.
///
/// Distinct from `hasBlockActions()` (a stun/sleep/paralyze, derived from the
/// buff list) and from [`Immobilized`] (movement only): Java's
/// `isAllSkillsDisabled()` is `_allSkillsDisabled || hasBlockActions()`, so
/// this blocks **casting only** and leaves walking and swinging alone. The TvT
/// freeze sets all three at once, which is what makes them easy to conflate.
#[derive(Component, Debug, Clone, Copy)]
pub struct SkillsDisabled;

/// Java `Player.setBlockActions(true)` during the 2.5 s sit-down animation.
/// A marker rather than a flag on `Player`: it is presence-based state with a
/// scheduled clear, exactly like `Casting`/`Movement`.
#[derive(Component, Debug, Clone, Copy)]
pub struct SitBlock;
