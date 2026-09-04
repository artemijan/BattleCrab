//! Servitors, pets, cubics and the boss-side spawned adds — both the link
//! from owner to summon and the summon's own state.

use bevy_ecs::component::Component;
use std::collections::HashMap;

/// Java `Npc.setSummoner` + `EffectPoint.getActingPlayer()` — the player who
/// summoned this NPC (a symbol totem). Distinct from [`ServitorOf`]: a totem
/// has an owner but none of the servitor bookkeeping (no follow AI, no
/// PetInfo, no upkeep). `acting_player` hops through it, so the friend/foe
/// filter and PvP rules treat the seal's pulses as the owner's actions —
/// which is why a seal never debuffs its own owner or their party/clan.
#[derive(Component, Debug, Clone, Copy)]
pub struct SummonerRef(pub i32);

/// A pet ordered to fetch a ground item (`RequestPetGetItem` →
/// `AI_INTENTION_PICK_UP`). Java hangs the target on the AI's intention; the
/// port's NPC intention enum has no pick-up arm, so the order rides as its own
/// component and the summon think checks it before the follow.
///
/// Dropped on arrival, on the item vanishing, and on any other order — which
/// is what `changeIntention` does there.
#[derive(Component, Debug, Clone, Copy)]
pub struct SummonPickup {
    pub item_object_id: i32,
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
    /// Java `SummonAI._isDefending` — the `ServitorMode` toggle (action
    /// 1103/1104). When set, being attacked makes the summon turn on its
    /// attacker; when clear, Java's `avoidAttack` has it sidestep instead.
    /// Defaults to **false**, matching `SummonAI`'s field initialiser: a fresh
    /// summon is in passive mode until the owner says otherwise.
    pub defending: bool,
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

/// Marks an NPC as part of the active Sailren wave encounter (its
/// velociraptors, pterosaur, trex, and Sailren himself). The wave mobs also
/// spawn in the open world, so the kill-chain only advances for tagged ones.
#[derive(Component, Debug, Clone, Copy)]
pub struct SailrenWaveMob;
