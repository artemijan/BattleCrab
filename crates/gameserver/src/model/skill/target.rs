//! Java `SkillTargetType` / `AffectScope` / `AffectObject` — how a cast picks
//! its first target and then sweeps up everyone else around it.

/// Java `SkillOperateType`, scoped to what the cast pipeline dispatches on.
/// Everything else (`A3`, `DA*`, …) reads as `Other` and isn't castable yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OperateType {
    /// `A1`/`A2`: an active, targeted or self-cast skill with a cast bar.
    Active,
    /// `P`: passive — never sent to `RequestMagicSkillUse`, no cast pipeline.
    Passive,
    /// `T`: toggle — out of scope for G6 (see plan's deferred list).
    Toggle,
    /// `CA1` (`SkillOperateType.isChanneling()`): an active cast whose payload
    /// is delivered by `channeling_effects` ticks while the cast bar runs
    /// (Volcano family — PLAN_G19_GROUND_CHANNELING.md). Cast time is
    /// **static** for these: Java skips `calcSkillTimeFactor` entirely
    /// (`_hitTime = max(hitTime − cancelTime, 0)`, `_cancelTime = 2866`).
    /// `CA5` doesn't occur on this dist's reachable content.
    Channeling,
    Other,
}

/// Java `TargetType`, scoped to the single-target types the cast pipeline
/// resolves (see `resolve_cast_target`) plus a catch-all so unhandled skills
/// still load instead of failing to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TargetType {
    /// `SELF`: always the caster.
    Self_,
    /// `TARGET`: the current target, friendly or not (self allowed).
    Target,
    /// `ENEMY`: an attackable target (force-use required against unflagged
    /// players — see `targethandlers/Enemy.java`).
    Enemy,
    /// `ENEMY_ONLY`: like `ENEMY` minus the "attack anything with ctrl"
    /// leniencies; identical to `Enemy` in a world with only players.
    EnemyOnly,
    /// `ENEMY_NOT` (`targethandlers/EnemyNot.java`) — "any friendly selected
    /// target": the exact inverse gate of `Enemy`/`EnemyOnly` (refused when
    /// `is_auto_attackable`, not when it isn't), self always allowed, **no**
    /// force-use override. Also exempt from the general "no dead targets"
    /// gate (Java: "works on dead targets or doors as well") — backs the
    /// priest heals that need to land on a fresh corpse ahead of a
    /// resurrection.
    EnemyNot,
    /// `NONE`: no selection involved — `targethandlers/None.java` returns the
    /// caster, so it behaves like `SELF` minus the peace-zone gate. This is
    /// what every toggle uses.
    None_,
    /// `PC_BODY`: a dead **player** corpse (Java `targethandlers/PcBody.java`)
    /// — what a resurrection targets. Like `NpcBody` this inverts the usual
    /// "no dead targets" gate, but requires a player rather than an NPC.
    PcBody,
    /// `NPC_BODY`: a dead NPC corpse (Java `targethandlers/NpcBody.java`) —
    /// used by corpse skills (Sweeper). Unlike the other types this requires
    /// the target to be **dead**, so the cast pipeline's "no dead targets"
    /// gate is inverted for it.
    NpcBody,
    /// `GROUND` (`targethandlers/Ground.java`): the cast is aimed at a world
    /// position stored by `RequestExMagicSkillUseGround` (ex 0x41), not at a
    /// creature — the handler validates the point (dontMove range, LOS,
    /// peace-zone effect clip for bad skills) and returns **the caster** as a
    /// sentinel; the POINT_BLANK sweep then centres on the stored point.
    /// Player-only: Java returns null for NPC casters, so NPC GROUND skills
    /// are inert on both sides.
    Ground,
    /// `OTHERS` (`targethandlers/Others.java`): the current selection, with
    /// one rule — it may not be **you**, and Java says so with its own message
    /// rather than the generic invalid-target one. Battle Stance (426), Spell
    /// Stance (427) and Summon Friend (1403) on this dist.
    Others,
    /// `DOOR_TREASURE` (`targethandlers/DoorTreasure.java`): whatever is
    /// currently selected, **if** it is a door or a chest — nothing else.
    /// Unlike every other type this runs no range, LOS, peace-zone or
    /// alive/dead gate of its own; the selection *is* the validation, which is
    /// what lets Unlock be cast on a closed door (not attackable) and on a
    /// chest (attackable) through the same path.
    DoorTreasure,
    /// `SUMMON`: the caster's own summon (Java `targethandlers/Summon.java`).
    ///
    /// **Servitors only.** Java is
    /// `if (isPlayer() && hasSummon()) return getAnyServitor(); return getPet();`
    /// — and `getAnyServitor()` is null when the player has only a *pet*, so a
    /// pet owner casting "Servitor Heal" targets nothing. That reads like a bug
    /// but is thematically right: these are the Summoner's servitor kit, and a
    /// Wolf is not a servitor. Ported as written.
    Summon,
    /// `OWNER_PET` (`targethandlers/OwnerPet.java`): `creature.getActingPlayer()`
    /// — cast *by* a servitor, it resolves to its **owner**.
    ///
    /// This one has to be a real variant rather than falling into `Other`,
    /// because `Summon.useMagic` special-cases it before target resolution
    /// even runs (`Summon.java`: `if (targetType == OWNER_PET) target = _owner`).
    /// Collapsed into `Other` it took the *owner's current selection* instead,
    /// so a Baby Kookaburra's Master Recharge (4025) fired at whatever mob the
    /// owner had clicked — or refused with "invalid target" when they had
    /// clicked nothing.
    OwnerPet,
    Other,
}

/// Java `AffectScope` (`handlers/targethandlers/affectscope/*`) — how the
/// primary target expands into the set the skill actually lands on.
///
/// Ported: the four radius/group scopes that cover the dist's non-single
/// skills — `RANGE` (820 skills), `POINT_BLANK` (785), `PARTY` (272), `PLEDGE`
/// (44) — and the geometric family (plan: PLAN_G19_GEOMETRIC_SCOPES.md) —
/// `FAN`/`FAN_PB` (163+16, 5 learnable), `SQUARE`/`SQUARE_PB` (35+17),
/// `RING_RANGE` (18) and the `DEAD_*` family. Reading as
/// [`AffectScope::Other`] and falling back to single-target — each verified to
/// have no reachable carrier on this dist (see `skills::affect`'s header):
/// `SUMMON_EXCEPT_MASTER` (22, off-chronicle), `BALAKAS_SCOPE`/`WYVERN_SCOPE`
/// (boss/wyvern scripting), `RANGE_SORT_BY_HP` (4), `PARTY_PLEDGE` (5) and
/// `STATIC_OBJECT_SCOPE` (2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AffectScope {
    /// `SINGLE` (and the `NONE`/absent default): only the primary target.
    Single,
    /// `RANGE`: everything within `affect_range` of the **target**.
    Range,
    /// `POINT_BLANK`: everything within `affect_range` of the **caster**.
    PointBlank,
    /// `PARTY`: the target's party (or the target alone when unpartied).
    Party,
    /// `PLEDGE`: the target's clan mates in range.
    Pledge,
    /// `FAN`: an arc of `fan_range[3]` degrees around the caster's heading
    /// (rotated by `fan_range[1]`), radius `fan_range[2]`. Can miss the
    /// primary target — a fan cast at someone behind you hits nobody.
    Fan,
    /// `FAN_PB`: FAN minus the corpse-target exemption and the primary
    /// target's affect-object bypass ("without taking target into account").
    FanPointBlank,
    /// `SQUARE`: a `fan_range[2]` × `fan_range[3]` rectangle extending from
    /// the caster along their heading.
    Square,
    /// `SQUARE_PB`: SQUARE, related as FAN_PB is to FAN.
    SquarePointBlank,
    /// `RING_RANGE`: an annulus around the **target** — inside `affect_range`
    /// but outside `fan_range[2]`. The epicenter target itself is never
    /// affected (that is the donut hole).
    RingRange,
    /// `DEAD_PLEDGE` / `DEAD_PARTY` / `DEAD_UNION`: the **corpses** of the
    /// target's clan / party / alliance within `affect_range` — the mass-
    /// resurrect fan-out. Mirror images of `Pledge`/`Party` with the liveness
    /// test inverted, and the only scope family where a *dead* candidate is
    /// the one that qualifies.
    DeadPledge,
    DeadParty,
    DeadUnion,
    /// Any scope not ported yet — treated as [`AffectScope::Single`].
    Other,
}

/// Java `AffectObject` (`handlers/targethandlers/affectobject/*`) — the
/// friend/foe filter applied to each candidate an [`AffectScope`] sweeps up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AffectObject {
    /// `ALL`: no filtering.
    All,
    /// `NOT_FRIEND` (1637 skills) / `NOT_FRIEND_PC`: everyone *except* the
    /// caster, their party and their clan — the offensive-AoE filter.
    NotFriend,
    /// `FRIEND` (463) / `FRIEND_PC`: only the caster's own side.
    Friend,
    /// `CLAN`: clan mates only.
    Clan,
    /// `UNDEAD_REAL_ENEMY` — the priest anti-undead auras (Sanctuary 97, Holy
    /// Aura 107, Repose 1034, Requiem 1049). Java: not yourself, `isUndead()`
    /// (an NPC whose template race is `UNDEAD` — a player never is), and
    /// `isAutoAttackable(caster)`.
    ///
    /// These are `SELF` + `POINT_BLANK` skills, so without the filter they
    /// sweep **everything** in range: friendly players and every non-undead
    /// mob alike. That made this the one live correctness bug on the affect
    /// axis rather than a missing nicety.
    UndeadRealEnemy,
    /// Unported filters (`INVISIBLE`, `HIDDEN_PLACE`, `WYVERN_OBJECT`,
    /// `OBJECT_DEAD_NPC_BODY`) — no filtering, like Java's null-handler path.
    /// None has a learnable source on this dist.
    Other,
}
