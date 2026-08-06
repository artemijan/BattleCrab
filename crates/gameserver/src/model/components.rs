//! ECS components shared by players and NPCs — stage 2 of the `bevy_ecs`
//! adoption (`PLAN_ECS_STAGE2.md` §2). Data only: components are split
//! along *system access seams* (what a per-tick sweep reads/writes without
//! the rest of the object), not per field, and carry no game logic beyond
//! trivial accessors. Player-only / NPC-only state stays in the (shrinking)
//! fat structs in `model/mod.rs` / `model/npc.rs` until its own phase.

use std::collections::HashMap;

use bevy_ecs::component::Component;

/// World position + facing (from Java `WorldObject`'s x/y/z +
/// `Creature._heading`). On both players and NPCs.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

/// The instance (logical world partition) an object is in — Java
/// `WorldObject.getInstanceId()`. The component is only present on objects that
/// have left the overworld; its absence means instance 0 (the shared world).
/// Two objects interact only when their instance ids match (G27).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceId(pub i32);

/// Java `Player._observerMode` + `_lastLoc` — present while a player is watching
/// an Olympiad match. Holds the location to teleport back to on exit and the
/// arena being watched (`_olympiadGameId`).
#[derive(Component, Debug, Clone, Copy)]
pub struct OlympiadObserver {
    pub return_pos: (i32, i32, i32),
    pub arena: i32,
}

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

/// Java `Creature.setLethalable(false)` — this NPC cannot be finished by a
/// lethal blow (`Lethal`/`HalfKill`). Raid bosses are immune by template; this
/// marker is for the ones a script exempts, i.e. `ai/others/NonLethalableNpcs`'
/// siege Headquarters (35062), which would otherwise be one Lethal Strike away
/// from deleting a clan's whole siege investment.
#[derive(Component, Debug, Clone, Copy)]
pub struct NotLethalable;

/// Marks an NPC as part of the active Sailren wave encounter (its
/// velociraptors, pterosaur, trex, and Sailren himself). The wave mobs also
/// spawn in the open world, so the kill-chain only advances for tagged ones.
#[derive(Component, Debug, Clone, Copy)]
pub struct SailrenWaveMob;

/// Per-instance door open state (G27 Frintezza slice 2). Instance door copies
/// carry their own open/closed flag instead of the global `geo.doors` atomic —
/// concurrent instances of the same template toggle independently. Absent on
/// the overworld boot doors (they follow the shared collision grid).
#[derive(Component, Debug, Clone, Copy)]
pub struct InstanceDoorOpen(pub bool);

impl Position {
    /// 2D center-to-center distance (the shape every range/reach check uses).
    pub fn distance_2d(&self, other: &Position) -> f64 {
        (((other.x - self.x) as f64).powi(2) + ((other.y - self.y) as f64).powi(2)).sqrt()
    }
}

/// An item lying on the ground (Java `Item` in `ItemLocation.VOID`, tracked by
/// `ItemsOnGroundManager`). A world entity with [`Position`]/[`RegionCell`];
/// indexed in `World::ground_item_regions`. Dropped by players (`//` drop) or
/// monster death (auto-loot off), picked up via a click (`Action`).
#[derive(Component, Debug, Clone)]
pub struct GroundItem {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub enchant: i32,
    /// Loot protection (Java `Item._ownerId` + the `ResetOwner` schedule,
    /// set by `ItemData.createItem("loot")`): while `world.tick <
    /// owner_until_tick`, only `owner_id`, their party, or — for raid drops —
    /// their command channel may pick the item up. `0`/`0` = unprotected.
    /// Expiry is lazy (checked at pickup) instead of Java's scheduled task.
    pub owner_id: i32,
    pub owner_until_tick: u64,
}

/// Java `Attackable._firstCommandChannelAttacked` + `_commandChannelLastAttack`:
/// the command channel that earned raid looting rights on this boss, refreshed
/// on every hit from that channel. Expires `RaidLootRightsInterval` after the
/// last hit — lazily (checked on read) instead of Java's 10 s polling timer.
#[derive(Component, Debug, Clone, Copy)]
pub struct RaidLootRights {
    pub cc_id: u32,
    pub last_attack_tick: u64,
}

/// One line in a player's private sell store (Java `TradeItem`): the inventory
/// instance offered, how many, and the asking price per unit.
#[derive(Debug, Clone, Copy)]
pub struct StoreItem {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub price: i64,
    pub enchant: i32,
}

/// A player's active private sell store (Java `Player._sellList` + store title).
/// Present only while the store is open; the store *type* (the CharInfo byte)
/// lives on [`Player::store_type`](crate::model::Player::store_type).
#[derive(Component, Debug, Clone, Default)]
pub struct PrivateStore {
    pub items: Vec<StoreItem>,
    pub title: String,
    /// Java `TradeList.isPackaged()` — a **package** store (`/packagesale`,
    /// `PrivateStoreType.PACKAGE_SELL`): the whole list is sold as one lot, so
    /// a buyer must take every line at once.
    pub packaged: bool,
}

/// One line of a private **buy** store: what the owner wants, how many are
/// still wanted, and what they pay each. Keyed by item id — the owner doesn't
/// hold the item yet, which is what separates this from [`StoreItem`].
#[derive(Debug, Clone, Copy)]
pub struct WantedItem {
    pub item_id: i32,
    pub count: i64,
    pub price: i64,
    pub enchant: i32,
}

/// A player's active private *buy* store (Java `Player._buyList` + store
/// title). Present only while the store is open; the store *type* byte
/// (BUY / BUY_MANAGE) lives on
/// [`Player::store_type`](crate::model::Player::store_type).
#[derive(Component, Debug, Clone, Default)]
pub struct PrivateBuyStore {
    pub items: Vec<WantedItem>,
    pub title: String,
}

/// An in-progress player-to-player trade (Java `Player._activeTradeList`).
/// Present on both partners while the trade window is open; `items` are this
/// player's offered lines (`price` unused), `confirmed` is their "OK" press.
#[derive(Component, Debug, Clone, Default)]
pub struct Trade {
    pub partner: i32,
    pub items: Vec<StoreItem>,
    pub confirmed: bool,
}

/// A pending trade *request* on the target (Java `Player._activeRequester`):
/// `from` asked to trade; cleared on answer/timeout.
#[derive(Component, Debug, Clone, Copy)]
pub struct PendingTrade {
    pub from: i32,
}

/// Which warehouse the player currently has open (Java
/// `Player._activeWarehouse`), set by the warehouse-keeper bypass. The
/// deposit/withdraw client packets carry no warehouse type, so the handlers
/// read this to route items to the right container.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ActiveWarehouse {
    #[default]
    Private,
    Clan,
    Freight,
}

/// An open enchant window (Java `EnchantItemRequest`, held as a `Player`
/// request). Present from the `EnchantScrolls` handler's `ChooseInventoryItem`
/// until the enchant completes or is cancelled. Object ids are `0` (none) until
/// the client fills them via the Ex-packet handshake.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct EnchantRequest {
    /// The scroll's inventory object id (set when the window opens).
    pub scroll_oid: i32,
    /// The item being enchanted (set by `RequestExTryToPutEnchantTargetItem`).
    pub item_oid: i32,
    /// The support item, if any (`0` = none). Support items are not yet wired.
    pub support_oid: i32,
    /// `_isProcessing` — set once `RequestEnchantItem` starts, to reject
    /// re-entrant packets mid-roll.
    pub processing: bool,
}

/// The world-region cell this object is registered in (Java
/// `WorldObject._worldRegion`). Kept in sync with `Position` by the
/// visibility/movement systems (Java `updateWorldRegion`/`switchRegion`) —
/// visibility deltas and broadcast scoping compare this, never raw
/// coordinates. Separate from `Position` because it changes on a different
/// cadence (cell crossings) and has different readers (visibility, not math).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionCell(pub (i32, i32));

/// HP/MP + liveness (Java `CreatureStatus` + `Creature._isDead`). On both
/// players and NPCs; CP is player-only and lives in [`PlayerVitals`]. `dead`
/// rides here (not a marker component): every writer flips it in the same
/// breath as HP, and death is a branch inside systems rather than a sweep
/// filter — a field avoids an archetype move per death/revive.
#[derive(Component, Debug, Clone, Copy)]
pub struct Vitals {
    pub max_hp: i32,
    pub cur_hp: f64,
    pub max_mp: i32,
    pub cur_mp: f64,
    /// Java `Creature._isDead` — for NPCs: corpse until decay removes it.
    pub dead: bool,
}

impl Vitals {
    pub fn hp_full(max_hp: i32, max_mp: i32) -> Self {
        Self {
            max_hp,
            cur_hp: max_hp as f64,
            max_mp,
            cur_mp: max_mp as f64,
            dead: false,
        }
    }
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

/// Java `Creature._basicPropertyResists` — the mesmerizing-debuff resistance
/// chain, one slot per [`BasicProperty`] (`PHYSICAL`, `MAGIC`), each holding
/// `(level, tick the 15 s window ends)`.
///
/// A fixed pair rather than a map: Java's `EnumMap` has exactly these two live
/// keys (`NONE` never accrues), and the component is `Copy` so the read-modify
/// -write in `basic_property::increase_resist_level` stays a single ECS write.
/// Expiry is evaluated on read — there is no sweep, matching Java's
/// `isExpired()` check inside `getResistLevel`.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct BasicPropertyResists {
    physical: (i32, u64),
    magic: (i32, u64),
}

impl BasicPropertyResists {
    /// `(level, end tick)` for one property. `NONE` never accrues and reads as
    /// a permanently-expired zero.
    pub fn get(&self, property: crate::model::skill::BasicProperty) -> (i32, u64) {
        match property {
            crate::model::skill::BasicProperty::Physical => self.physical,
            crate::model::skill::BasicProperty::Magic => self.magic,
            crate::model::skill::BasicProperty::None => (0, 0),
        }
    }

    pub fn set(&mut self, property: crate::model::skill::BasicProperty, level: i32, end: u64) {
        match property {
            crate::model::skill::BasicProperty::Physical => self.physical = (level, end),
            crate::model::skill::BasicProperty::Magic => self.magic = (level, end),
            crate::model::skill::BasicProperty::None => {}
        }
    }
}

/// CP (`PcStatus`) — the player-only vitals extension, so NPC damage code
/// never sees a CP field it must ignore.
#[derive(Component, Debug, Clone, Copy)]
pub struct PlayerVitals {
    pub max_cp: i32,
    pub cur_cp: f64,
}

/// Movement speeds + run/walk mode. For players these are stat-finalizer
/// *outputs* (`recalculate_stats` writes them: template base × buff
/// modifiers); for NPCs they're memoized from the template at spawn (the
/// template never changes, so this is the same value the old code re-read
/// per use). f64 keeps NPC fractional speeds exact; player values are the
/// same rounded numbers as before, just stored as f64.
#[derive(Component, Debug, Clone, Copy)]
pub struct Speeds {
    /// `SwampZone.getMoveBonus()` for the zone the creature is standing in,
    /// or 1.0. Java re-reads the zone inside `SpeedFinalizer`; the port caches
    /// it here and refreshes it on zone enter/exit, so the stat recompute stays
    /// free of world lookups.
    pub swamp_multiplier: f64,
    pub run_spd: f64,
    pub walk_spd: f64,
    pub swim_run_spd: f64,
    pub swim_walk_spd: f64,
    pub move_multiplier: f64,
    /// Raw template base run speed (Java `getTemplate().getBaseValue(RUN_SPEED,
    /// 0)`) — the *unboosted, unbuffed* class/NPC template value. Constant for
    /// the object's lifetime; used only as the denominator of
    /// [`Speeds::client_move_multiplier`], never in the movement math.
    pub base_run_spd: f64,
    /// The other three raw template bases, for the same denominator. Java's
    /// `getMovementSpeedMultiplier` picks between all four by
    /// `isInsideZone(WATER)` and `isRunning()`, so a swimming (or walking)
    /// character needs its *own* base or the animation rate is scaled against
    /// the wrong yardstick — this is why entering water left the legs running
    /// at land cadence.
    pub base_walk_spd: f64,
    pub base_swim_run_spd: f64,
    pub base_swim_walk_spd: f64,
    /// `Creature._isRunning` — players spawn running; NPCs walk until AI
    /// flips to run on aggro.
    pub running: bool,
    /// In a `WaterZone` (`isInsideZone(ZoneId.WATER)`) — flipped by zone
    /// revalidation; `move_speed` switches to the swim speeds while set.
    pub swimming: bool,
}

impl Speeds {
    /// The ground speed movement math uses (`Creature.getMoveSpeed`, incl.
    /// its "in water → swim speeds" branch).
    pub fn move_speed(&self) -> f64 {
        let (run, walk) = if self.swimming {
            (self.swim_run_spd, self.swim_walk_spd)
        } else {
            (self.run_spd, self.walk_spd)
        };
        (if self.running { run } else { walk }) * self.move_multiplier
    }

    /// Java `CreatureStat.getMovementSpeedMultiplier`: current move speed ÷ the
    /// raw template base speed for the movement mode in effect — swim bases
    /// while `isInsideZone(WATER)`, walk bases while walking. This is the value
    /// the client uses to set the **leg-animation playback rate**, so it must be
    /// *derived* from the finalized speed — not a standalone field. Stat-based
    /// speed buffs (Super Haste, Wind Walk, …) raise `run_spd` without touching
    /// `move_multiplier`; sending a bare `move_multiplier` there made the
    /// character glide at the buffed speed while its legs animated at the base
    /// cadence. Falls back to `1.0` if the base is unknown (0), so a
    /// zero-template object (every NPC, whose swim bases are 0) is unchanged.
    pub fn client_move_multiplier(&self) -> f64 {
        let base = match (self.swimming, self.running) {
            (true, true) => self.base_swim_run_spd,
            (true, false) => self.base_swim_walk_spd,
            (false, true) => self.base_run_spd,
            (false, false) => self.base_walk_spd,
        };
        if base <= 0.0 {
            return 1.0;
        }
        self.move_speed() * (1.0 / base)
    }

    /// The four speed shorts `UserInfo`/`CharInfo` carry, in wire order. Java
    /// sends `Math.round(speed / moveMultiplier)` and the client multiplies
    /// [`Speeds::client_move_multiplier`] back in for display and movement —
    /// so the finalized speeds must be sent *divided*, or the buff scale is
    /// counted twice (Super Haste 4 showed ~3100 on the client while the
    /// server moved at ~630).
    ///
    /// The first two slots are **water-aware**: Java fills them from
    /// `getRunSpeed()`/`getWalkSpeed()`, and both of those return the *swim*
    /// stat while `isInsideZone(WATER)`. The client drives its own prediction
    /// and leg animation off the run slot, so sending the land speed there is
    /// what made entering water feel like no slowdown at all — the server
    /// swam at 50 while the client kept running at 120. Slots 3/4 stay the raw
    /// `getSwimRunSpeed()`/`getSwimWalkSpeed()`, which is why they duplicate
    /// slots 1/2 while submerged (Java does exactly this).
    pub fn client_speed_fields(&self) -> [i16; 4] {
        let mult = self.client_move_multiplier();
        let div = |v: f64| {
            if mult > 0.0 {
                (v / mult).round() as i16
            } else {
                v as i16
            }
        };
        let (run, walk) = if self.swimming {
            (self.swim_run_spd, self.swim_walk_spd)
        } else {
            (self.run_spd, self.walk_spd)
        };
        [
            div(run),
            div(walk),
            div(self.swim_run_spd),
            div(self.swim_walk_spd),
        ]
    }
}

/// Collision cylinder (template `collision_radius`/`collision_height`) —
/// reach/range gates and packet fields.
#[derive(Component, Debug, Clone, Copy)]
pub struct Collision {
    pub radius: f64,
    pub height: f64,
}

/// Combat-stat finalizer outputs (Java `CreatureStat`'s computed values).
/// Players: written by `recalculate_stats` (base × stat bonus × level mod ×
/// buff modifiers), same rounded values as before stored as f64. NPCs:
/// memoized once at spawn from the (immutable) template through the same
/// finalizer math the old `combatant()` ran per call — values identical.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct CombatStats {
    pub p_atk: f64,
    pub m_atk: f64,
    pub p_def: f64,
    pub m_def: f64,
    pub p_atk_spd: i32,
    pub m_atk_spd: i32,
    /// Per-mille (×10), like Java's `PCriticalRateFinalizer` output.
    pub crit_hit: f64,
    pub m_crit_hit: f64,
    pub evasion: i32,
    pub accuracy: i32,
    pub magic_evasion: i32,
    pub magic_accuracy: i32,
    pub atk_range: i32,
    /// Weapon `randomDamage` (class templates all declare `baseRndDam = 10`;
    /// NPC templates carry their own).
    pub random_dmg: i32,
}

impl CombatStats {
    /// Java `CreatureStat.getAttackSpeedMultiplier` (`Formulas.calcAtkSpdMultiplier`):
    /// the client uses this to set the **attack-animation playback rate**, the
    /// haste counterpart of [`Speeds::client_move_multiplier`]. Java's formula
    /// `dexBonus × (weaponBaseAtkSpd / 333) × mul + add / 333` reduces exactly to
    /// `pAtkSpd / 333` (the finalized `p_atk_spd` is `weaponBase × dexBonus × mul
    /// + add`) whenever `mul ≥ 0.7` and there is no move-type term — the case for
    /// every player here. Sending a bare `1.0` (the old value) left the swing
    /// animation at base cadence while Super Haste quadrupled the actual p_atk_spd.
    pub fn client_atk_speed_multiplier(&self) -> f64 {
        self.p_atk_spd as f64 / 333.0
    }
}

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

/// An in-flight move — **present only while moving** (the stage-2 shape of
/// Java's nullable `Creature._move`). Presence is the movement tick's sweep
/// filter: the interpolation query visits only entities that carry this,
/// instead of scanning 34.9k static NPCs' `None`s every 100 ms. Insert =
/// `moveToLocation`, remove = arrival/stop/teleport/death.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Movement(pub crate::model::movement::MoveData);

/// A move deferred on the path worker — **present only while waiting** for
/// the `PathEvent` reply. `seq` is the request's sequence number: a reply
/// with an older one is stale (superseded by a newer click) and is dropped.
/// Java has no equivalent state — `CellPathFinding.findPath` runs
/// synchronously inside `moveToLocation`.
#[derive(Component, Debug, Clone, Copy)]
pub struct PathWait {
    pub seq: u64,
}

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

/// `AbstractAI.isFollowing()` — an attack-follow toward `target_object_id` is
/// registered (Java: the actor sits in `CreatureFollowTaskManager`'s
/// `ATTACK_FOLLOW_CREATURES` map, put there by `startFollow`, taken out by
/// `stopFollow`). Present only while the chase leg of an intent is running.
///
/// It carries the same payload Java's map value does — the follow range
/// recorded **at the moment the follow started**, already shrunk by 100 for a
/// target that was moving then. Java never refreshes it while the follow lives
/// (`maybeMoveToPawn` returns early before reaching `startFollow` again), so
/// neither does this.
///
/// The latch is what makes the 100-unit engage hysteresis in
/// `combat::maybe_move_to_pawn` possible: Java widens the range gate by 100
/// *only while following*.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Following {
    pub target_object_id: i32,
    pub offset: i32,
}

/// `AbstractAI._target` / `_clientMovingToPawnOffset` / `_moveToPawnTimeout` —
/// the throttle `moveToPawn` keeps so it neither re-paths nor re-broadcasts
/// `MoveToPawn` more than once a second toward the same pawn at the same
/// offset ("prevent possible extra calls to this function, also don't send
/// movetopawn packets too often"). `_clientMoving` is the `Movement` component
/// itself, so it is not duplicated here.
#[derive(Component, Debug, Clone, Copy)]
pub struct MoveToPawnState {
    pub target_object_id: i32,
    pub offset: i32,
    /// Tick at which a re-path at the *same* offset is allowed again.
    pub timeout_tick: u64,
}

/// Java `Player._currentSkillWorldPosition` — the ground point stored by
/// `RequestExMagicSkillUseGround` (ex 0x41) that a `targetType GROUND` cast
/// aims at. **Never cleared, only overwritten** by the next ground cast
/// (Java's field has exactly one setter call site); the channeling tick
/// re-reads it, which is safe for the same reason.
#[derive(Component, Debug, Clone, Copy)]
pub struct GroundSkillTarget {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Java `Npc.setSummoner` + `EffectPoint.getActingPlayer()` — the player who
/// summoned this NPC (a symbol totem). Distinct from [`ServitorOf`]: a totem
/// has an owner but none of the servitor bookkeeping (no follow AI, no
/// PetInfo, no upkeep). `acting_player` hops through it, so the friend/foe
/// filter and PvP rules treat the seal's pulses as the owner's actions —
/// which is why a seal never debuffs its own owner or their party/clan.
#[derive(Component, Debug, Clone, Copy)]
pub struct SummonerRef(pub i32);

/// Dr. Chaos's paranoia timer (Java `_pissedOffTimer`, starts at 30). Lives on
/// the Dr. Chaos NPC (32033); lingering players drain it, and at ≤0 he becomes
/// the Gigantic Chaos Golem. (G23 slice 22, PLAN_G23_DR_CHAOS.md.)
#[derive(Component, Debug, Clone, Copy)]
pub struct DrChaosState {
    pub pissed_off: i32,
}

/// The Gigantic Chaos Golem's idle clock (Java `_lastAttackVsGolem`). Lives on
/// the golem NPC (25512); 30 minutes with no refresh despawns it back to Dr.
/// Chaos.
#[derive(Component, Debug, Clone, Copy)]
pub struct DrChaosGolem {
    pub last_attack_tick: u64,
}

/// A Beast Farm tamed beast (Java `TamedBeast`): the top of the feeding
/// chain — follows its tamer and lives on a spice clock.
#[derive(Component, Debug, Clone, Copy)]
pub struct TamedBeastOf {
    /// The tamer's object id.
    pub owner: i32,
    /// The spice *skill* (2188 golden / 2189 crystal) this beast eats.
    pub food_skill: i32,
    /// Java `_remainingTime` in ticks: starts at 20 min, -60 s per duration
    /// check, +20 s per feeding, capped at 20 min. ≤ 0 → despawn.
    pub remaining_ticks: i32,
}

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

/// Known skills (skill_id → level), loaded from `character_skills` (or the
/// class's autoGet initial set at creation). Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct SkillBook(pub HashMap<i32, i32>);

/// The enchant sub-level per skill id (0/absent = unenchanted) — Java keeps
/// this on the `Skill` instance itself (`getSubLevel()`); the port's book is
/// (id → level), so the routes live in a parallel map. Persisted in the same
/// `character_skills` rows (`skill_sub_level`), banked per class index on a
/// subclass switch like the book. PLAN_G19_SKILL_ENCHANT.md.
#[derive(Component, Debug, Clone, Default)]
pub struct SkillEnchants(pub HashMap<i32, i32>);

/// The player's three worn henna dyes (Java `Player._henna[3]`), by slot →
/// dye id. Loaded from `character_hennas`, persisted in the store transaction.
/// The dyes' base-stat bonuses are folded into [`BaseStats`] (recomputed on
/// add/remove); this component holds only the slot assignments. Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct HennaSlots(pub [Option<i32>; 3]);

impl HennaSlots {
    /// Number of filled slots (Java `3 - getHennaEmptySlots()` counts these).
    pub fn worn(&self) -> usize {
        self.0.iter().filter(|s| s.is_some()).count()
    }

    /// The worn dye ids in slot order.
    pub fn dye_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.0.iter().filter_map(|s| *s)
    }
}

/// Clan skills currently granted to this member (skill_id → level), Java's
/// `Player.addSkill(clanSkill, false)` set. **Transient** — re-derived from the
/// clan on every login (see `game_loop::clans::apply_clan_skills`) and never
/// written to `character_skills` (Java passes `store=false`). Kept separate from
/// [`SkillBook`] both to preserve that no-persist contract and so leaving/
/// dispersing the clan strips exactly these. Folded into the `SkillList` packet
/// alongside the skill book. Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct ClanSkills(pub HashMap<i32, i32>);

/// The player's registered crafting recipes as recipe-*list* ids, split by
/// book (Java `Player._dwarvenRecipeBook` / `_commonRecipeBook`, keyed by
/// `RecipeList.getId()`). Loaded from `character_recipebook`, persisted in the
/// store transaction (the `type` column = dwarven/common, derived from
/// `RecipeData`). Player-only. Order is kept stable (Java uses a sorted map;
/// here insertion order — the wire packet carries a running 1-based slot index
/// the client keys buttons by, so consistency across resends is what matters).
#[derive(Component, Debug, Clone, Default)]
pub struct RecipeBook {
    pub dwarven: Vec<i32>,
    pub common: Vec<i32>,
}

impl RecipeBook {
    /// Whether either book holds this recipe-list id (Java `hasRecipeList`).
    pub fn contains(&self, list_id: i32) -> bool {
        self.dwarven.contains(&list_id) || self.common.contains(&list_id)
    }
}

/// The duel this player is currently in (`Player._isInDuel` → the duel id).
/// Present from the countdown until the duel ends.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct DuelRef(pub u32);

/// An outstanding duel challenge awaiting this player's answer
/// (`ExDuelAskStart` sent, `RequestDuelAnswerStart` pending).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PendingDuel {
    pub challenger: i32,
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

/// Abnormal visual effects a GM pinned on this creature with `//ave_abnormal`,
/// independent of any buff. Java has no such component — it calls
/// `startAbnormalVisualEffect` directly, which mutates the same
/// `EffectList._abnormalVisualEffects` set the buffs feed. This port keeps the
/// buff-derived set computed (a fold, never stored), so the manual ones need
/// somewhere of their own to live.
#[derive(Component, Debug, Clone, Default)]
pub struct AdminVisuals(pub Vec<i16>);

/// The per-character key/value store (Java `PlayerVariables`, table
/// `character_variables`). Java's `AbstractVariables` is a `StatSet` with typed
/// getters and a dirty flag that `storeMe` consults; here the map is plain and
/// the memory-first autosave flushes it wholesale with the rest of the
/// character, so no dirty tracking is needed.
///
/// Only the keys a ported subsystem reads live here today —
/// [`VITALITY_ITEMS_USED`]. The rest of Java's key set (instance origin/restore,
/// UI key mapping, ability points, auto-use settings, …) belongs to subsystems
/// that are not ported; they will land as their milestones do. Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct PlayerVariables(pub HashMap<String, String>);

/// Java `AutoPlaySettings` + the auto-attack half of `AutoUseSettings` — the
/// `.play` panel's state. Persisted through `PlayerVariables` at logout
/// (`AUTO_USE_SETTINGS`), so the panel survives a relog; whether the *loop*
/// restarts is `ResumeAutoPlay`'s call.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoPlaySettings {
    /// The loop is running.
    pub active: bool,
    /// `AutoUseSettings.getAutoActions().contains(2)` — the auto-attack box.
    /// Java calls the inverse `isMageCaster`: with it off the loop acquires a
    /// target but never swings.
    pub auto_attack: bool,
    /// `doPickup()` — walk to and take nearby loot.
    pub pickup: bool,
    /// `isRespectfulHunting()` — skip a mob already fighting somebody else.
    pub respectful_hunting: bool,
    /// `isShortRange()` — 600 units instead of 1400.
    pub short_range: bool,
    /// 0 any / 1 monster / 2 characters / 3 npc.
    pub next_target_mode: i32,
    /// The HP percentage the auto-potion half drinks at (slice 2).
    pub potion_percent: i32,
}

/// Java `AutoUseSettings` — what the three sub-pages choose: buffs to keep up,
/// attack skills to fire, supply items to use, and the one healing potion.
/// Persisted alongside [`AutoPlaySettings`] so the panel survives a relog.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct AutoUseSettings {
    /// Self-target skills, cast **even in town**.
    pub buffs: Vec<i32>,
    /// Offensive skills, cast at the current target outside a peace zone.
    pub skills: Vec<i32>,
    /// Shots, scrolls and the like, used outside a peace zone.
    pub supply_items: Vec<i32>,
    /// The single healing potion slot (`0` = none).
    pub potion_item: i32,
}

impl Default for AutoPlaySettings {
    fn default() -> Self {
        Self {
            active: false,
            auto_attack: true,
            pickup: false,
            respectful_hunting: false,
            short_range: false,
            next_target_mode: 0,
            potion_percent: 0,
        }
    }
}

/// `PlayerVariables.VITALITY_ITEMS_USED_VARIABLE_NAME` — how many
/// vitality-restoring items the character has consumed this week, capped by
/// `Config.VITALITY_MAX_ITEMS_ALLOWED` and reported by `ExVitalityEffectInfo`.
pub const VITALITY_ITEMS_USED: &str = "VITALITY_ITEMS_USED";

/// `PlayerVariables.UI_KEY_MAPPING` — the client's saved key layout, stored as
/// Java stores it: the raw bytes joined by tabs (`RequestSaveKeyMapping`'s
/// `SPLIT_VAR`), replayed verbatim by `ExUISetting`.
pub const UI_KEY_MAPPING: &str = "UI_KEY_MAPPING";

impl PlayerVariables {
    /// Java `AbstractVariables.getInt(key, default)` — a non-numeric or absent
    /// value yields the default.
    pub fn get_int(&self, key: &str, default: i32) -> i32 {
        self.0
            .get(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }

    /// Java `AbstractVariables.set(key, value)`.
    pub fn set_int(&mut self, key: &str, value: i32) {
        self.0.insert(key.to_string(), value.to_string());
    }
}

/// A player's active private *manufacture* store (Java `Player._manufactureItems`
/// + store title): the recipes they craft-for-hire and the adena fee each.
/// Present only while the store is open; not persisted (`StoreRecipeShopList =
/// False`). The store *type* byte (MANUFACTURE) lives on
/// [`Player::store_type`](crate::model::Player::store_type). `items` are
/// `(recipe_list_id, cost)`.
#[derive(Component, Debug, Clone, Default)]
pub struct ManufactureStore {
    pub items: Vec<(i32, i64)>,
    pub title: String,
}

/// The currently-applied grade-penalty levels (Java `Player._expertiseWeaponPenalty`
/// / `_expertiseArmorPenalty`, each 0-4). Cached so `refresh_expertise_penalty`
/// can no-op when nothing changed, and read by `EtcStatusUpdate`. Player-only.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct WeightPenalty {
    /// 0-4, the level of Java's `CommonSkill.WEIGHT_PENALTY` (4270) currently
    /// applied. The client draws its icon from the `EtcStatusUpdate` byte.
    pub level: i32,
    /// `Player.isOverloaded()` — carrying more than `getMaxLoad()`. Distinct
    /// from `level > 0`: the penalty ladder starts at 50% of the limit, so a
    /// character can be penalised without being overloaded.
    pub overloaded: bool,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ExpertisePenalty {
    pub weapon: i32,
    pub armor: i32,
}

/// Live cooldowns (Java `_reuseTimeStampsSkills` + `_disabledSkills`,
/// unified), keyed by `Skill::reuse_key()`. Checked lazily — no expiry tasks.
/// Persisted across relog via `character_skills_save` (Java `storeEffect`/
/// `restoreEffects`, reuse half; gated by `StoreSkillCooltime`) — see
/// `db::SkillReuseRow` + `PlayerData::restore_reuses`. Buff restore (the
/// `restore_type = 0` half) is still deferred.
#[derive(Component, Debug, Clone, Default)]
pub struct Reuses(pub HashMap<i32, crate::model::SkillReuse>);

/// Active buffs/debuffs (Java `EffectList`). Expiry is driven by the
/// `Scheduler` (`ScheduledTask::BuffExpire`), not by anything here.
#[derive(Component, Debug, Clone, Default)]
pub struct Buffs(pub Vec<crate::model::skill::ActiveBuff>);

/// Java `CreatureStat`'s two modifier maps — buffs/gear push entries here;
/// `recalculate_stats` folds them into `CombatStats`/`Speeds`.
#[derive(Component, Debug, Clone, Default)]
pub struct StatModifiers {
    pub add: HashMap<crate::model::stats::Stat, f64>,
    pub mul: HashMap<crate::model::stats::Stat, f64>,
    /// Admin fixed-value overrides (`//setparam` → Java
    /// `CreatureStat.addFixedValue`): when present, the stat's finalizer
    /// returns this value verbatim, ignoring base/buffs. Persists across buff
    /// recomputes (not cleared with `add`/`mul`); cleared by `//unsetparam`.
    pub fixed: HashMap<crate::model::stats::Stat, f64>,
    /// Java `CreatureStat._skillEvasionStat` — a flat % chance to dodge an
    /// incoming skill, keyed by the skill's `magicType` (0 = physical skills,
    /// which is the only bucket this dist's learnable sources use). A separate
    /// map rather than a `Stat` because Java keeps it that way: a buff that
    /// dodges physical skills must not dodge magic.
    pub skill_evasion: HashMap<i32, f64>,
    /// Java `CreatureStat._moveTypeStats` (`mergeMoveTypeValue`): flat
    /// contributions that only count in a particular locomotion state, from
    /// `StatByMoveType`. **Additive**, identity `0.0`.
    ///
    /// Deliberately *not* folded into `add`: Java reads this at finalize time
    /// against the creature's live move type, so the value swings as the player
    /// stands/walks/runs with no stat recompute anywhere.
    pub by_move_type: HashMap<(crate::model::stats::Stat, crate::model::stats::MoveType), f64>,
    /// Java `CreatureStat._positionTypeStats` (`mergePositionTypeValue`):
    /// contributions that only count when the attacker stands in a particular
    /// position relative to the target, from `CriticalDamagePosition`.
    /// **Multiplicative**, identity `1.0` — a different merge and a different
    /// identity from `by_move_type`, which is why Java keeps two maps and so
    /// does this.
    pub by_position: HashMap<(crate::model::stats::Stat, crate::model::movement::Position), f64>,
}

impl StatModifiers {
    /// Java `CreatureStat.getMoveTypeValue(stat, type)` — the flat term for
    /// this stat in the creature's *current* locomotion state (0 when there is
    /// no `StatByMoveType` contribution for that pairing).
    pub fn move_type_value(
        &self,
        stat: crate::model::stats::Stat,
        move_type: crate::model::stats::MoveType,
    ) -> f64 {
        self.by_move_type
            .get(&(stat, move_type))
            .copied()
            .unwrap_or(0.0)
    }

    /// Java `CreatureStat.getPositionTypeValue(stat, position)` — the
    /// multiplier for this stat at the given attacker position (**1.0**, not
    /// 0.0, when nothing contributes: this map multiplies).
    pub fn position_value(
        &self,
        stat: crate::model::stats::Stat,
        position: crate::model::movement::Position,
    ) -> f64 {
        self.by_position
            .get(&(stat, position))
            .copied()
            .unwrap_or(1.0)
    }
}

/// A summoned servitor's link to its owner — Java `Summon._owner` plus the
/// `Servitor` bookkeeping the `Summon` effect sets up.
///
/// Lives on the servitor NPC entity. The owner side is [`ServitorOf`]'s inverse
/// lookup (`Player.getServitors()`), which this port does by scanning rather
/// than caching a second index: a player has at most one servitor on this
/// dist, so there is nothing to iterate.
#[derive(Component, Debug, Clone, Copy)]
pub struct ServitorOf {
    pub owner_object_id: i32,
    /// Java `Servitor.setReferenceSkill` — the skill that summoned it, used to
    /// re-summon on login and to identify the servitor's own skill set.
    pub reference_skill: i32,
    /// Absolute tick the servitor expires at (Java's `lifeTime`, in seconds in
    /// the XML). `u64::MAX` for the `lifeTime <= 0` case, which Java maps to
    /// `Integer.MAX_VALUE` with the comment "Classic hack. Resummon upon
    /// entering game."
    pub expires_at_tick: u64,
    /// `lifeTime` as declared, for the `PetInfo` fed/max-fed pair (Java sends
    /// `getLifeTimeRemaining()` / `getLifeTime()` there for a servitor).
    pub life_time_secs: i32,
    /// Java `SummonAI._startFollow` / `Summon.getFollowStatus()` — whether the
    /// servitor trails its owner when it has nothing else to do. Toggled by the
    /// "hold" action; cleared when it is ordered to attack.
    pub following: bool,
    /// Java `Servitor._itemConsume` — the upkeep item the owner pays
    /// periodically (a gemstone on the golems). `0` = no upkeep.
    pub consume_item_id: i32,
    pub consume_item_count: i64,
    /// Absolute tick the next upkeep payment falls due; `u64::MAX` when there
    /// is no upkeep item.
    pub next_consume_tick: u64,
}

/// The pet-specific half of an owned summon. The **owner link, follow state and
/// AI all come from [`ServitorOf`]**, which a pet also carries — "owned summon"
/// is the same relationship whether it came from a skill or a collar, so pets
/// inherit follow/attack/leash for free. This holds only what a servitor has no
/// equivalent of.
#[derive(Component, Debug, Clone, Copy)]
pub struct PetOf {
    /// The **object id** of the collar that summoned it — a pet's identity in
    /// Java's `pets` table (`item_obj_id`), and why two collars of the same
    /// kind are two different pets.
    pub collar_object_id: i32,
    /// Java `Pet.getCurrentFed()` — the food bar.
    pub fed: i32,
    pub max_fed: i32,
    /// Java `PetStat.getLevel()`. A pet levels independently of its owner, so
    /// this is saved rather than derived — the point of the `pets` row.
    pub level: i32,
    /// Java `PetStat.getExp()` / `getSp()`.
    pub exp: i64,
    pub sp: i64,
    /// Java `Pet._expBeforeDeath` — the exp total *before* the death penalty,
    /// so a resurrection can hand back a percentage of what was lost. Zero
    /// when the pet has not died since it was last revived.
    ///
    /// Deliberately **not** persisted: Java holds it on the live instance
    /// only, so a pet that dies and logs out forfeits the restorable exp.
    pub exp_before_death: i64,
}

/// A **summon's** charged shots — Java's `Creature._chargedShots`.
///
/// Java keeps that field on `Creature`, so players and summons share it; this
/// port grew the player half first (`Player.is_charged_shot`) and only needs
/// the summon half now. Kept as a separate component rather than moved off
/// `Player`, because unifying them touches every player-shot call site for no
/// behavioural gain today.
/// `TODO(G29+)`: fold `Player`'s shot bits into this component.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ChargedShots {
    pub soulshot: bool,
    pub spiritshot: bool,
}

/// Marks a cubic's stats-only caster entity and links it back to its owner.
///
/// Java's `Cubic.getLevel()` is `return _owner.getLevel()` — a cubic borrows
/// its owner's **level** for accuracy and resist checks while using its own
/// template `power` for attack. Without this link the caster resolved to level
/// 1 and every cast was resisted, so the cubic did no damage at all.
#[derive(Component, Debug, Clone, Copy)]
pub struct CubicOf {
    pub owner_object_id: i32,
}

/// The owner's side of the summon link — Java's `Player._pet` / `_servitors`
/// fields.
///
/// The port originally derived this by sweeping the store for a matching
/// [`ServitorOf`], which needed `&mut World` (the ECS builds its `QueryState`
/// mutably) and so could not be read from the packet builders, which take
/// `&World`. Holding the reverse link is both faster and closer to Java, where
/// `getPet()` is a field read, not a world scan.
///
/// The ids are **validated on read** (`servitor_of`/`pet_of` check the entity
/// still exists), so a despawn path that forgets to clear this yields `None`
/// rather than a dangling reference.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SummonRef {
    pub servitor: Option<i32>,
    pub pet: Option<i32>,
}

/// Java `Player._fishing` (G32): the active fishing session. `cast_seq`
/// invalidates stale scheduled reel/cast tasks — a fresh cast (or a stop) bumps
/// it, so an in-flight `FishingReel`/`FishingCast` from a superseded cast
/// no-ops. The bait location is where the bob landed, echoed in the fishing
/// packets.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct FishingSession {
    pub is_fishing: bool,
    pub cast_seq: u64,
    pub bait_x: i32,
    pub bait_y: i32,
    pub bait_z: i32,
}

/// Every saved pet row belonging to a character, keyed by the **collar's object
/// id** — Java's `pets` primary key (`item_obj_id`).
///
/// Loaded with the character and written back with it, the memory-first model
/// again: Java re-reads the row inside `Pet.restore` on every summon, but this
/// port has the character's whole pet set in hand from login, so summoning is a
/// map lookup with no DB round-trip in the cast path.
///
/// A row here is the pet's state *as last stored*; a live pet's state lives on
/// [`PetOf`] and is flushed back into this map on unsummon and at save time.
#[derive(Component, Debug, Clone, Default)]
pub struct PlayerPets(pub HashMap<i32, crate::db::PetRow>);

/// The servitor this character had out at logout (`character_summons`).
///
/// At most one on this dist. Held on the owner because a servitor has no
/// persistent identity of its own — it is rebuilt by re-casting the skill.
#[derive(Component, Debug, Clone, Default)]
pub struct PlayerSummons(pub Vec<crate::db::SummonRow>);

/// Java `Creature._summonedNpcs` — the NPCs *this* NPC has spawned through a
/// script `addSpawn(summoner, …)`. Scripts read its size (`getSummonedNpcCount`)
/// to stop a talk/attack handler re-spawning the same guardian every time it is
/// triggered.
///
/// Only the parent's half of the link is kept. Java also back-links the child
/// (`npc.setSummoner`) so the child can unlink itself on decay; the port prunes
/// dead children when the count is read instead, which needs no despawn hook
/// and gives the same answer — a *corpse* still counts, exactly as in Java,
/// because `removeSummonedNpc` fires at `onDecay`, not at death.
#[derive(Component, Debug, Clone, Default)]
pub struct SummonedNpcs(pub Vec<i32>);

/// Java `CreatureStat._defenceTraits` / `_invulnerableTraits` — the per-trait
/// debuff resistances a `DefenceTrait` buff merges in, and the traits it makes
/// the bearer outright immune to.
///
/// Kept as its own component rather than as `Stat` entries because a trait
/// resistance is *per trait*, not a single scalar, and Java merges/unmerges it
/// by hand on effect start/exit rather than through the stat recalculation.
#[derive(Component, Debug, Clone, Default)]
pub struct DefenceTraits {
    /// trait → summed resistance (0.30 = 30 % harder to land).
    pub resist: std::collections::HashMap<crate::model::skill::TraitType, f64>,
    /// Traits the bearer cannot be affected by at all (Java's XML value ≥ 100).
    pub invulnerable: std::collections::HashSet<crate::model::skill::TraitType>,
}

/// Java `CreatureStat._attackTraitValues` / `_attackTraits` — the attacker-side
/// twin of [`DefenceTraits`], merged by the `AttackTrait` effect ("Detect
/// &lt;Category&gt; Weakness" 75/80/87/88/104, Eye of Hunter/Slayer 359/360).
///
/// **The table's identity is 1.0, not 0** (`Arrays.fill(_attackTraitValues, 1)`)
/// — the opposite of the defence table — because the pair is consumed as
/// `attackTrait − defenceTrait`. Presence in the map is Java's
/// `hasAttackTrait`, which several formulas gate on separately from the value.
#[derive(Component, Debug, Clone, Default)]
pub struct AttackTraits {
    /// trait → `1.0 + Σ(amount / 100)`.
    pub values: std::collections::HashMap<crate::model::skill::TraitType, f64>,
}

/// Java `CreatureStat._mpConsumeStat` / `_reuseStat` — the per-`magicType`
/// **multiplicative** rates that `MagicMpCost` and `Reuse` buffs merge in.
///
/// Both are keyed by the *effect's* `magicType` bucket (0 physical, 1 magic,
/// 3 dance) and consumed against the *cast skill's* own `magic_type`. Java
/// merges with `mul` on start and `div` on exit, which is why a stack of two
/// −10 % songs is 0.81 rather than 0.80 — and why the unmerge is exact even
/// out of order.
#[derive(Component, Debug, Clone, Default)]
pub struct SkillRateStats {
    /// magicType → MP-consume factor (0.70 = costs 30 % less).
    pub mp_consume: std::collections::HashMap<i32, f64>,
    /// magicType → reuse factor (0.80 = 20 % shorter cooldown).
    pub reuse: std::collections::HashMap<i32, f64>,
    /// The same two tables for **passive** skills, kept apart from the buff
    /// ones on purpose.
    ///
    /// Buff rates are merged and un-merged incrementally (`mul` on start,
    /// `div` on exit), which only stays consistent because every merge has
    /// exactly one matching un-merge. A passive has no such pair — it is
    /// simply true or not, and re-evaluated wholesale whenever the skill book
    /// or the worn gear changes. Folding passives into the shared tables would
    /// mean dividing out a factor that may never have been multiplied in,
    /// which corrupts the table rather than restoring it.
    ///
    /// Read multiplicatively with its buff twin, so a song's discount and
    /// Inner Rhythm's compound exactly as Java's stacked effects do.
    pub passive_mp_consume: std::collections::HashMap<i32, f64>,
    pub passive_reuse: std::collections::HashMap<i32, f64>,
}

/// Java `Player.setBlockActions(true)` during the 2.5 s sit-down animation.
/// A marker rather than a flag on `Player`: it is presence-based state with a
/// scheduled clear, exactly like `Casting`/`Movement`.
#[derive(Component, Debug, Clone, Copy)]
pub struct SitBlock;

/// Panel shortcuts (Java `Player._shortCuts`), keyed by
/// `slot + page * 12` — a `BTreeMap` so `ShortCutInit` order is stable.
/// Player-only; registry logic in `model/shortcut.rs`.
#[derive(Component, Debug, Clone, Default)]
pub struct Shortcuts(pub std::collections::BTreeMap<i32, crate::model::shortcut::Shortcut>);

/// Server-stored macros (Java `Player._macros`), insertion-ordered like
/// Java's `LinkedHashMap`. `next_id` is `MacroList._macroId` (starts at
/// 1000). Player-only; registry logic in `model/shortcut.rs`.
#[derive(Component, Debug, Clone)]
pub struct Macros {
    pub next_id: i32,
    pub entries: Vec<crate::model::shortcut::Macro>,
}

/// Currently targeted object id (Java `Creature._target`), player-only —
/// NPC targeting goes through the aggro list.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TargetRef(pub Option<i32>);

/// The multisell list the player currently has open (Java
/// `Player._currentMultiSell` / `setMultiSell`), player-only. Presence-based:
/// added when a `MultiSellList` is sent, read/validated by `MultiSellChoose`,
/// removed on a stale/forged choose. The multipliers still come off the list
/// itself (the community-board path uses the default 1.0), but the two fields
/// `PreparedMultisellListHolder` derives *from the NPC* are latched here, so the
/// exchange charges exactly the rate the window displayed.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ActiveMultisell {
    pub list_id: i32,
    /// Object id of the NPC the window was opened from (Java `_npcObjectId`),
    /// 0 for the npc-less community-board path. Tax is paid to its castle.
    pub npc_oid: i32,
    /// Java `PreparedMultisellListHolder.getTaxRate()` — already 0 for a list
    /// that doesn't `applyTaxes`, and for an NPC outside every tax zone.
    pub tax_rate: f64,
    /// The rows the window actually displayed, in order — Java's prepared
    /// `_entries` (+ the parallel `_itemInfos`). `MultiSellChoose`'s entry id
    /// indexes *this*, not the static list, which is what makes an
    /// inventory-only (`exc_multisell`) window addressable.
    pub rows: Vec<PreparedRow>,
}

/// One displayed multisell row: which entry of the static list it shows and,
/// for an inventory-only window, which of the player's item instances it was
/// paired with (Java `PreparedMultisellListHolder._itemInfos`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedRow {
    /// Index into `MultisellList.entries`.
    pub entry_index: usize,
    /// The paired inventory instance, `0` on a normal (non-inventory) window.
    pub item_object_id: i32,
    /// That instance's enchant level (0 when unpaired) — displayed in the
    /// window and echoed back by the client on the choose.
    pub enchant_level: i32,
}

/// GM-toggled state on a player (Java `Creature._isInvul`, `_isUndying`,
/// `Player.setInvisible`/`setSilenceMode`/`setDietMode`). Presence-based:
/// absent = every flag `false`, added on the first toggle or by the GM-startup
/// block at enter-world.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AdminFlags {
    /// `isInvul` — incoming damage is ignored entirely.
    pub invul: bool,
    /// `isUndying` — damage applies but HP never drops below 1 (no death).
    pub undying: bool,
    /// `isInvisible` — hidden from other players (`//hide`).
    pub hidden: bool,
    /// `isSilenceMode` — GM refuses incoming whispers/PMs. Set by
    /// `GMStartupSilence` and `//silence`; honored in `chat.rs`'s `Whisper` arm,
    /// which answers the sender `THAT_PERSON_IS_IN_MESSAGE_REFUSAL_MODE` and
    /// delivers nothing.
    pub silence: bool,
    /// `isInDietMode` — weight overload is ignored. Set by `GMStartupDietMode`
    /// and `//diet`; read by [`crate::game_loop::weight`], which reports penalty
    /// level 0 and "not overloaded" for a dieting GM no matter what they carry.
    pub diet: bool,
    /// `//para`'s `setBlockActions(true)` + `startParalyze()` — ORed into
    /// `abnormal::is_action_blocked`/`is_movement_disabled` beside the buff
    /// flags. Attachable to NPCs too (Java paralyzes any creature target).
    pub paralyzed: bool,
    /// `//settargetable`'s `setTargetable(false)` — `handle_action` refuses to
    /// select this creature.
    pub untargetable: bool,
}

/// The player's in-progress Lucky Lottery number picks (Java `Player._loto[5]`,
/// G26.5) — the five 1–20 numbers chosen through the Loto NPC dialog before a
/// ticket is bought. Transient; presence-based (added on first pick, reset to
/// zeros each time the buy window is (re)opened). `0` = an empty slot.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LotoPicks(pub [i32; 5]);

/// The player's in-progress Monster Race bet (Java `Player._raceTickets[2]`,
/// G26.5): slot 0 = the chosen lane (1–8), slot 1 = the price tier (1–8) picked
/// through the RaceManager dialog before the ticket is bought. `0` = unset.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RaceTicket(pub [i32; 2]);

/// Object id of the last NPC this player clicked/talked to (Java
/// `Player._lastFolkNpc`, set by `NpcAction.action`). Bare (non-`npc_`-
/// prefixed) HTML bypasses like `Quest ClanMaster 9000-02.htm` resolve their
/// NPC through this — Java uses the `validateHtmlAction` origin id there,
/// which we don't port (see `game_loop/bypass.rs`); the distance re-check at
/// use time is the guard either way. Player-only.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastFolkNpc(pub i32);

/// Zone membership (Java `Creature._zones` narrowed to the loaded
/// `ZoneKind`s) + the revalidation bookkeeping `Creature`/`Player` keep
/// alongside it. Player-only for now: the peace gate only ever checks
/// playables, and NPC water/no-restart behavior has no consumer yet.
#[derive(Component, Debug, Clone, Copy)]
pub struct ZoneFlags {
    /// OR of `ZoneKind::bit()`s the player currently stands in.
    pub mask: u8,
    /// `Creature._lastZoneValidateLocation` — revalidation is skipped while
    /// within 100 units of the last validated spot (movement calls it every
    /// tick).
    pub last_validate: (i32, i32, i32),
    /// `Player._lastCompassZone` (`ExSetCompassZoneCode` value last sent).
    /// Starts at 0 like Java's field default — 0 is not a valid zone code
    /// (they run 0x08–0x0F), so the first revalidate always pushes the real
    /// code. The client needs that initial push: without a valid code it
    /// treats the zone as unknown and refuses to open the world map.
    pub last_compass: i32,
    /// Whether the player currently stands in an *active* siege zone (its
    /// castle's siege in progress). Tracked so `revalidateZone` can fire
    /// `SiegeZone.onEnter`/`onExit` (combat-zone messages + the leave-flag) on
    /// the transition — a `SiegeZone` is only a combat zone while active, which
    /// the plain zone mask (membership only) can't express.
    pub in_active_siege: bool,
    /// Whether the last `ExAutoFishAvailable` we sent was YES — so the fishing
    /// availability packet only fires on a real transition (G32). FishingZone
    /// has no membership bit, so this can't ride the plain mask either.
    pub fishing_available: bool,
    /// Which TvT headquarters peace zone the player was last inside — `1` blue
    /// (`colosseum_peace1`), `2` red (`colosseum_peace2`), `0` neither. The
    /// event's `onEnterZone`/`onExitZone` hooks are edge-triggered off this;
    /// the zone mask is per *kind*, so a named-zone transition needs its own
    /// field.
    pub tvt_hq_zone: u8,
}

impl Default for ZoneFlags {
    fn default() -> Self {
        // A fresh player has no validated location yet — `i32::MIN` keeps the
        // first revalidate from being skipped by the distance filter.
        Self {
            mask: 0,
            last_validate: (i32::MIN, i32::MIN, i32::MIN),
            last_compass: 0,
            in_active_siege: false,
            tvt_hq_zone: 0,
            fishing_available: false,
        }
    }
}

/// Java `Player._taskWater` — the drowning clock, present exactly while
/// `Player.isInWater()` is true. Java holds a `ScheduledFuture` whose initial
/// delay is the breath gauge and whose period is 1 s; the port keeps the tick
/// the next damage beat is due on and lets
/// [`game_loop::water`](crate::game_loop::water) sweep it, so "cancel the
/// task" is just removing the component.
#[derive(Component, Debug, Clone, Copy)]
pub struct WaterTask {
    /// `world.tick` the next 1 s drowning beat lands on. Seeded to
    /// `now + breath`, so nothing happens at all until the gauge runs out.
    pub next_damage_tick: u64,
}

impl ZoneFlags {
    pub fn contains(&self, kind: crate::data::zone_data::ZoneKind) -> bool {
        self.mask & kind.bit() != 0
    }
}

/// Last position/heading the client reported via `ValidatePosition`
/// (Java `Player._clientX/_clientY/_clientZ/_clientHeading`).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ClientPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

/// STR/DEX/CON/INT/WIT/MEN (player-only for now — NPC base stats stay on the
/// template until something buffs them). Inputs to the stat finalizers and
/// the regen bonuses.
#[derive(Component, Debug, Clone, Copy)]
pub struct BaseStats {
    pub str_: i32,
    pub dex: i32,
    pub con: i32,
    pub int_: i32,
    pub wit: i32,
    pub men: i32,
}

/// Party membership — **present only while in a party**; the value keys
/// `World.parties`. The party's member list is the authority on membership,
/// this is the O(1) back-pointer (Java `Player._party`).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartyRef(pub u32);

/// What a `PendingRequest` is asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    /// An `AskJoinParty` is on the target's screen; answering joins this party.
    PartyInvite { party_id: u32 },
    /// A `FriendAddRequest` is on the target's screen.
    FriendInvite,
    /// An `AskJoinPledge` is on the target's screen; answering joins the
    /// inviter's clan. `pledge_type` rides along (Java keeps it on the stored
    /// `RequestJoinPledge` packet) — only 0 (main pledge) is accepted until
    /// sub-units land (G18 slice 6).
    ClanInvite { clan_id: i32, pledge_type: i32 },
    /// An `AskJoinAlly` is on the target clan leader's screen; accepting puts
    /// their whole clan into `ally_id`'s alliance.
    AllyInvite { ally_id: i32 },
    /// An `ExAskJoinPartyRoom` is on the target's screen; accepting puts them
    /// into the inviter's party matching room (G30).
    PartyRoomInvite { room_id: i32 },
    /// An `ExAskJoinMPCC` is on the target party leader's screen; accepting
    /// puts their party into the requestor's command channel (created on
    /// accept if the requestor's party isn't in one yet — Java re-derives
    /// everything from the requestor, so no channel id rides along).
    CommandChannelInvite,
}

/// Display mirror of "this player is in a party matching room" (G30), for the
/// `UserInfo`/`CharInfo` CLAN-block byte Java reads off
/// `Player.isInMatchingRoom()`. The **authority is `World.matching_rooms`** —
/// this component exists only because the packet builders take a component
/// view, and it is written in exactly one place
/// (`game_loop::party_room`'s join/leave helpers).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InMatchingRoom;

/// The one outstanding transaction-request slot — **present only while a
/// request is in flight**, on *both* sides (Java splits this across
/// `Player._requests`, `_activeRequester` and `_requestExpireTime`; one slot
/// covers them because a busy player answers "C1 is on another task" either
/// way). Cleared by the answer, the `RequestTimeout` task (seq-guarded), or
/// either side leaving the world.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingRequest {
    pub kind: RequestKind,
    /// The other side (target for the requestor, requestor for the target).
    pub other: i32,
    /// True on the side that must answer (got the Ask/FriendAddRequest).
    pub answerer: bool,
    pub seq: u64,
}

/// Friend-list snapshot (`Player._friendList` + the name/level/class data
/// Java pulls from `CharInfoTable`), loaded with the character. Online
/// status is always read live from `World`, never from here.
#[derive(Component, Debug, Clone, Default)]
pub struct Friends(pub Vec<crate::character::FriendInfo>);

/// Quest progress (Java `Player._quests`), keyed by quest name — the same
/// key the `character_quests` rows and the `Quest <Name> …` bypasses use.
/// Loaded with the character; mutated only through the quest engine
/// (`game_loop/quests.rs`), which mirrors every change to the DB.
#[derive(Component, Debug, Clone, Default)]
pub struct Quests(pub std::collections::HashMap<String, crate::model::quest::QuestState>);

/// Live quest-timer generations, keyed by `(quest name, timer name)` — the
/// cancellation side of `ScheduledTask::QuestTimer` (a fired task whose seq
/// no longer matches is stale). Starting a timer bumps the seq; so does
/// cancelling (Java's `QuestTimer.cancel`). Not persisted, like Java.
#[derive(Component, Debug, Clone, Default)]
pub struct QuestTimerSeqs(pub std::collections::HashMap<(&'static str, String), u64>);

/// `AdminDebug`'s per-GM visualizer state (`//debug doors|geodata|movement`):
/// which draw loops are on, the anchor the last frame was drawn from (redraw
/// after moving > 15 units, like Java's `PLAYER_*_LOCATIONS`), the door ids
/// currently drawn (`PLAYER_SHOWN_DOORS`), and the last movement line state.
#[derive(Component, Debug, Clone, Default)]
pub struct DebugDraw {
    pub doors: bool,
    pub geo: bool,
    pub movement: bool,
    pub shown_doors: Vec<i32>,
    pub door_anchor: (i32, i32, i32),
    pub geo_anchor: (i32, i32, i32),
    pub move_anchor: (i32, i32, i32),
    pub last_dest: Option<(i32, i32, i32)>,
    pub last_path: Option<Vec<(i32, i32, i32)>>,
}

/// Marks an HQ flag planted by **skill 326 "Build Advanced Headquarters"**
/// (Java `SiegeFlag._isAdvanced`). Same NPC as the basic camp (35062); the
/// flag only changes how much damage the thing takes.
///
/// **Deliberate deviation from Java** — see `docs/CUSTOM_DIST_DEVIATIONS.md`.
/// `SiegeFlagStatus.reduceHp` reads:
///
/// ```text
/// if (isAdvancedHeadquarter()) super.reduceHp(value / 2, …);
/// super.reduceHp(value, …);
/// ```
///
/// with no `else` and no `return`, so upstream an advanced HQ takes
/// `value/2 + value` — **1.5× damage**, making the noble-only skill strictly
/// worse than the basic one. This port halves, which is what the skill's name,
/// its `autoGet` place in the noble tree, and the obvious intent of that `if`
/// all say it should do.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvancedHeadquarter;
