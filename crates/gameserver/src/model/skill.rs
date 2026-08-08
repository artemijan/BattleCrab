//! Port of `model/skill/Skill.java` — scoped to the fields G6's cast pipeline
//! actually reads (targeting/timing/costs/abnormal info), plus the effect
//! list. Full `Skill.java` has ~40 more fields (traits, elements, fan/affect
//! shapes, …) — added when combat (G9) or AoE/PvP targeting need them.

use crate::model::stats::{Stat, StatModifierType};

/// Java `SkillOperateType`, scoped to what the cast pipeline dispatches on.
/// Everything else (`A3`, `DA*`, …) reads as `Other` and isn't castable yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Java `enums/TraitType` — the tag a debuff carries (`<trait>`) and the tag a
/// resistance buff raises (`DefenceTrait`'s params). The numeric group is
/// Java's `_type`, and it decides how `calcGeneralTraitBonus` treats the pair:
///
/// - **3** (`SHOCK`, `HOLD`, `SLEEP`, `POISON`, `DERANGEMENT`, `PARALYZE`,
///   `BLEED`, `DEATH`, …) — a plain resistance: the target's defence applies
///   with no attacker-side requirement. **This is the group the dist uses**:
///   304 skills carry `<trait>SHOCK</trait>`, 194 `DERANGEMENT`, and the
///   learnable Stun/Mental/Poison resistances defend exactly these.
/// - **2** (the `*_WEAKNESS` family) — needs the *attacker* to carry a matching
///   `AttackTrait` as well, which is why "Detect Beast Weakness" is inert on
///   this dist (nothing gives a monster the paired `DefenceTrait`).
/// - **1** (weapon types) and **0** (`NONE`) — never resisted this way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TraitType {
    #[default]
    None,
    /// Group 3 — the resistable debuff traits this dist actually uses.
    Poison,
    Hold,
    Bleed,
    Sleep,
    Shock,
    Derangement,
    Paralyze,
    Death,
    Boss,
    CriticalPoison,
    RootPhysically,
    RootMagically,
    TurnStone,
    Gust,
    PhysicalBlockade,
    Target,
    PhysicalWeakness,
    MagicalWeakness,
    Knockback,
    Knockdown,
    Pull,
    Hate,
    Aggression,
    Airbind,
    Disarm,
    Deport,
    Changebody,
    Zone,
    Psychic,
    /// Group 2 — the creature-category weaknesses (attacker-gated).
    Weakness(WeaknessTrait),
    /// Group 1 — the weapon types (and `ETC`). Distinct values so a bearer's
    /// SWORD and DAGGER resistances can't fold into one bucket, which
    /// `calcWeaponTraitBonus` reads to soften hits from that weapon type
    /// (Deflect Arrow's BOW, Provoke's negative POLE).
    Weapon(WeaponTrait),
    /// Anything unrecognised — treated as group 1, i.e. never resisted.
    Other,
}

/// The `*_WEAKNESS` members of `TraitType`, kept as one variant so the enum
/// stays small; they all share group 2 and the same attacker-gated rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaknessTrait {
    Bug,
    Animal,
    Plant,
    Beast,
    Dragon,
    Giant,
    Construct,
    Valakas,
    Anesthesia,
    Demonic,
    Divine,
    Elemental,
    Fairy,
    Human,
    Humanoid,
    Undead,
    Embryo,
    Spirit,
}

/// Java's group-1 `TraitType` members: the weapon types a `DefenceTrait` can
/// name, plus `ETC` (which 61 skills declare as their `<trait>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponTrait {
    Sword,
    Blunt,
    Dagger,
    Pole,
    Fist,
    Bow,
    Etc,
    Dual,
    DualFist,
    Rapier,
    Crossbow,
    AncientSword,
    DualDagger,
    DualBlunt,
    TwoHandCrossbow,
}

impl TraitType {
    /// Java's `TraitType.getAllWeakness()` — the group-2 members, in Java's own
    /// order (the product is commutative, but the list is the authority on
    /// *which* traits count as a weakness).
    pub const ALL_WEAKNESS: [TraitType; 18] = {
        use WeaknessTrait as W;
        [
            TraitType::Weakness(W::Bug),
            TraitType::Weakness(W::Animal),
            TraitType::Weakness(W::Plant),
            TraitType::Weakness(W::Beast),
            TraitType::Weakness(W::Dragon),
            TraitType::Weakness(W::Giant),
            TraitType::Weakness(W::Construct),
            TraitType::Weakness(W::Valakas),
            TraitType::Weakness(W::Anesthesia),
            TraitType::Weakness(W::Demonic),
            TraitType::Weakness(W::Divine),
            TraitType::Weakness(W::Elemental),
            TraitType::Weakness(W::Fairy),
            TraitType::Weakness(W::Human),
            TraitType::Weakness(W::Humanoid),
            TraitType::Weakness(W::Undead),
            TraitType::Weakness(W::Embryo),
            TraitType::Weakness(W::Spirit),
        ]
    };

    /// Java `WeaponType.getTraitType()` — every weapon type *is* a trait, which
    /// is what `calcWeaponTraitBonus` looks the target's defence up by. The
    /// types Java maps to `NONE` (fishing rod, flag) stay `None`.
    pub fn of_weapon(weapon: crate::data::item_data::WeaponType) -> Self {
        use crate::data::item_data::WeaponType as W;
        use WeaponTrait as P;
        match weapon {
            W::Sword => Self::Weapon(P::Sword),
            W::Blunt => Self::Weapon(P::Blunt),
            W::Dagger => Self::Weapon(P::Dagger),
            W::Pole => Self::Weapon(P::Pole),
            W::DualFist => Self::Weapon(P::DualFist),
            W::Bow => Self::Weapon(P::Bow),
            W::Dual => Self::Weapon(P::Dual),
            W::DualBlunt => Self::Weapon(P::DualBlunt),
            W::Fist => Self::Weapon(P::Fist),
            W::Rapier => Self::Weapon(P::Rapier),
            W::Crossbow => Self::Weapon(P::Crossbow),
            W::AncientSword => Self::Weapon(P::AncientSword),
            W::DualDagger => Self::Weapon(P::DualDagger),
            W::TwoHandCrossbow => Self::Weapon(P::TwoHandCrossbow),
            // Java's `NONE`/`FISHINGROD`/`FLAG` map to `TraitType.NONE`, and
            // the port folds bare-handed into `WeaponType::None` too — an
            // unarmed swing carries no weapon trait, so nothing defends it.
            W::None | W::FishingRod => Self::None,
        }
    }

    /// Java's `TraitType._type`.
    pub fn group(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Weakness(_) => 2,
            Self::Weapon(_) | Self::Other => 1,
            _ => 3,
        }
    }

    /// Parse a `<trait>` tag or a `DefenceTrait` param name. Unknown names fall
    /// to [`TraitType::Other`] (group 1) rather than being dropped, so a later
    /// chronicle's trait can't silently read as `None` and skip the gate.
    pub fn from_xml(name: &str) -> Self {
        use WeaknessTrait as W;
        use WeaponTrait as P;
        match name {
            "NONE" => Self::None,
            "POISON" => Self::Poison,
            "HOLD" => Self::Hold,
            "BLEED" => Self::Bleed,
            "SLEEP" => Self::Sleep,
            "SHOCK" => Self::Shock,
            "DERANGEMENT" => Self::Derangement,
            "PARALYZE" => Self::Paralyze,
            "DEATH" => Self::Death,
            "BOSS" => Self::Boss,
            "CRITICAL_POISON" => Self::CriticalPoison,
            "ROOT_PHYSICALLY" => Self::RootPhysically,
            "ROOT_MAGICALLY" => Self::RootMagically,
            "TURN_STONE" => Self::TurnStone,
            "GUST" => Self::Gust,
            "PHYSICAL_BLOCKADE" => Self::PhysicalBlockade,
            "TARGET" => Self::Target,
            "PHYSICAL_WEAKNESS" => Self::PhysicalWeakness,
            "MAGICAL_WEAKNESS" => Self::MagicalWeakness,
            "KNOCKBACK" => Self::Knockback,
            "KNOCKDOWN" => Self::Knockdown,
            "PULL" => Self::Pull,
            "HATE" => Self::Hate,
            "AGGRESSION" => Self::Aggression,
            "AIRBIND" => Self::Airbind,
            "DISARM" => Self::Disarm,
            "DEPORT" => Self::Deport,
            "CHANGEBODY" => Self::Changebody,
            "ZONE" => Self::Zone,
            "PSYCHIC" => Self::Psychic,
            "BUG_WEAKNESS" => Self::Weakness(W::Bug),
            "ANIMAL_WEAKNESS" => Self::Weakness(W::Animal),
            "PLANT_WEAKNESS" => Self::Weakness(W::Plant),
            "BEAST_WEAKNESS" => Self::Weakness(W::Beast),
            "DRAGON_WEAKNESS" => Self::Weakness(W::Dragon),
            "GIANT_WEAKNESS" => Self::Weakness(W::Giant),
            "CONSTRUCT_WEAKNESS" => Self::Weakness(W::Construct),
            "VALAKAS" => Self::Weakness(W::Valakas),
            "ANESTHESIA" => Self::Weakness(W::Anesthesia),
            "DEMONIC_WEAKNESS" => Self::Weakness(W::Demonic),
            "DIVINE_WEAKNESS" => Self::Weakness(W::Divine),
            "ELEMENTAL_WEAKNESS" => Self::Weakness(W::Elemental),
            "FAIRY_WEAKNESS" => Self::Weakness(W::Fairy),
            "HUMAN_WEAKNESS" => Self::Weakness(W::Human),
            "HUMANOID_WEAKNESS" => Self::Weakness(W::Humanoid),
            "UNDEAD_WEAKNESS" => Self::Weakness(W::Undead),
            "EMBRYO_WEAKNESS" => Self::Weakness(W::Embryo),
            "SPIRIT_WEAKNESS" => Self::Weakness(W::Spirit),
            "SWORD" => Self::Weapon(P::Sword),
            "BLUNT" => Self::Weapon(P::Blunt),
            "DAGGER" => Self::Weapon(P::Dagger),
            "POLE" => Self::Weapon(P::Pole),
            "FIST" => Self::Weapon(P::Fist),
            "BOW" => Self::Weapon(P::Bow),
            "ETC" => Self::Weapon(P::Etc),
            "DUAL" => Self::Weapon(P::Dual),
            "DUALFIST" => Self::Weapon(P::DualFist),
            "RAPIER" => Self::Weapon(P::Rapier),
            "CROSSBOW" => Self::Weapon(P::Crossbow),
            "ANCIENTSWORD" => Self::Weapon(P::AncientSword),
            "DUALDAGGER" => Self::Weapon(P::DualDagger),
            "DUALBLUNT" => Self::Weapon(P::DualBlunt),
            "TWOHANDCROSSBOW" => Self::Weapon(P::TwoHandCrossbow),
            _ => Self::Other,
        }
    }
}

/// Java `AffectObject` (`handlers/targethandlers/affectobject/*`) — the
/// friend/foe filter applied to each candidate an [`AffectScope`] sweeps up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// The Rust counterpart of Java's `AbstractStatAddEffect`/
/// `AbstractStatPercentEffect` — one generic type instead of the 63 one-line
/// subclasses Java has (each just names a `Stat` and a fixed mode).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatModifierEffect {
    pub stat: Stat,
    pub mode: StatModifierType,
    pub amount: f64,
    /// `ConditionUsingItemType` armor mask from the effect's `<armorType>`
    /// list (OR of `ArmorType::mask_bit`s), or `0` when the effect has no such
    /// condition and always applies. Only meaningful for passive skills whose
    /// contribution depends on the worn armor (Spellcraft 163, Magician's
    /// Movement 118); active-buff effects leave it `0`.
    pub armor_condition: u8,
    /// OR of `WeaponType::mask_bit`s from the effect's `<weaponType>` list, or
    /// `0` when the effect has no such condition and always applies. Gates the
    /// effect on the *equipped weapon* (e.g. Weapon Mastery 249's
    /// `-30% MagicalAttackSpeed` applies only with a BOW/POLE in hand) — the
    /// weapon-side counterpart of `armor_condition`.
    pub weapon_condition: u32,
    /// A condition narrowing *when* this contribution counts —
    /// `StatByMoveType`'s locomotion state (Vital Force 148, Esprit 171,
    /// Acrobatic Move 225, Clear Mind 1297) or `CriticalDamagePosition`'s
    /// attacker position (Focus Death 355, Focus Power 357). `None` — every
    /// other effect — always counts.
    ///
    /// Java keeps these in maps *separate* from add/mul on `CreatureStat`
    /// (`_moveTypeStats`, `_positionTypeStats`) and reads them at finalize time
    /// against the creature's live state. Riding the qualifier along on this
    /// struct keeps the whole buff pipeline (landing, stacking, removal,
    /// passive folding) unchanged; the split happens in `apply_modifier`, which
    /// routes a qualified effect into the matching
    /// [`crate::model::components::StatModifiers`] map instead of `add`/`mul`.
    ///
    /// Each kind carries its own merge semantics — additive for move type,
    /// multiplicative for position — so `mode` is not consulted on either path.
    /// See [`crate::model::stats::StatQualifier`].
    pub qualifier: Option<crate::model::stats::StatQualifier>,
    /// `TwoHandedBluntBonus` / `TwoHandedSwordBonus`: the contribution counts
    /// only while the equipped weapon occupies **both hands**.
    ///
    /// Java expresses this as a second condition beside the weapon-type one —
    /// `ConditionUsingSlotType(SLOT_LR_HAND)` — so it is a separate axis from
    /// [`Self::weapon_condition`] rather than another mask bit: "a blunt" and
    /// "a two-handed weapon" are independent tests that both have to pass.
    pub two_handed: bool,
}

impl Default for StatModifierEffect {
    /// An unconditioned flat modifier. Exists so literals can use
    /// `..Default::default()` and stop breaking when a condition axis is added
    /// — the same reason [`Skill`] has one.
    fn default() -> Self {
        Self {
            stat: Stat::PhysicalAttack,
            mode: StatModifierType::Diff,
            amount: 0.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
        }
    }
}

/// One entry inside a `RestorationRandom` reward group (Java
/// `RestorationItemHolder`). `min_enchant`/`max_enchant` drive the grant-time
/// enchant roll (`game_loop::skills::effects::give_item_random`: when
/// `max_enchant > 0`, the created item gets `Rnd.get(min_enchant, max_enchant)`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RestorationItem {
    pub item_id: i32,
    pub count: i64,
    pub min_enchant: i32,
    pub max_enchant: i32,
}

/// One `<items><item chance="..">...</item></items>` roulette slice (Java
/// `ExtractableProductItem`): a chance-weighted set of items granted
/// together when this slice is picked. `chance` is the raw XML percentage
/// (0-100 space, slices summing to ~100), matching Java's
/// `100 * Rnd.nextDouble()` roulette roll — not pre-scaled like
/// `item_data::CapsuledItem::chance`.
/// `Escape.java`'s `escapeType`, i.e. the `TeleportWhereType` it hands to
/// `teleToLocation` — see [`SkillEffect::Escape`].
///
/// `FORTRESS` is deliberately absent: the two scrolls carrying it are fortress
/// content, which this chronicle has none of, so the effect drops as an
/// unhandled name rather than pretending to a destination that cannot exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeDest {
    /// `TOWN` — the enclosing map region's respawn point. 38 skills.
    Town,
    /// `CLANHALL` — `ClanHallData.getClanHallByClan(clan).getOwnerLocation()`,
    /// i.e. the hall's `<ownerRestartPoint>`. Scroll of Escape: Clan Hall
    /// (2040) and its blessed twin (2177).
    ClanHall,
    /// `CASTLE` — the owned castle's `getResidenceZone().getSpawnLoc()`, or its
    /// `getChaoticSpawnLoc()` when the player's reputation is negative. Java
    /// also accepts a *defender* standing on castle ground during a live siege,
    /// not only the owning clan.
    Castle,
}

#[derive(Debug, Clone)]
pub struct RestorationGroup {
    pub chance: f64,
    pub items: Vec<RestorationItem>,
}

/// A skill effect the pipeline knows how to apply. Java registers ~380 effect
/// handler scripts by name; here each supported kind is a variant —
/// `StatModifier` covers the whole `AbstractStatAddEffect`/
/// `AbstractStatPercentEffect` family, the instant kinds get one variant per
/// ported handler. Unregistered effect names are still dropped at load.
#[derive(Debug, Clone)]
pub enum SkillEffect {
    /// Continuous stat pump (goes into an `ActiveBuff` for `abnormal_time`).
    StatModifier(StatModifierEffect),
    /// `handlers/effecthandlers/BlockActions.java` — stun / sleep / paralyze:
    /// the target can neither act nor move for the buff's duration. Carries no
    /// stat modifier; the state lives in the [`effect_flag`] mask.
    ///
    /// `conditional` mirrors Java's `allowedSkills` whitelist (a non-empty list
    /// yields `CONDITIONAL_BLOCK_ACTIONS` instead). The whitelist contents are
    /// not modelled — `hasBlockActions()` treats both flags the same.
    /// SKIP(G19): every carrier on this dist lists the same ten ids
    /// (10279, 10517, 10025, 10776, 11770, 1904, 11264, 11093, 13314, 1912 —
    /// all post-Interlude), and none is reachable *by a player here*.
    ///
    /// **Re-verified 2026-08-07, and the earlier evidence was wrong.** It said
    /// "no item `<skills>` grant"; in fact **13314 is granted by ten items** —
    /// every Blessed Antharas' Earring variant (19463, 23637-8, 36241, 36409,
    /// 37667, 37732, 47484-6). What actually holds is one step further out:
    /// none of those ten items is obtainable, appearing in no NPC droplist, no
    /// buylist and no multisell, and all carry post-Interlude ids. The other
    /// nine skills have no skill-tree row, item grant or NPC template.
    ///
    /// So the conclusion stands — nobody can know a whitelisted skill, so
    /// nobody can be wrongly blocked from casting one — but it rests on item
    /// *reachability*, not on the absence of a grant. Re-check the droplists,
    /// not the `<skills>` blocks, if this is ever revisited.
    BlockActions {
        conditional: bool,
    },
    /// `handlers/effecthandlers/BlockChat.java` — the bot-report chat
    /// punishment (skill 6038). Starts a CHAT_BAN punishment on the bearer for
    /// the buff's life and carries [`effect_flag::CHAT_BLOCK`], which is what
    /// makes `Say2` use the "reported as an illegal program user" wording.
    BlockChat,
    /// `handlers/effecthandlers/BlockParty.java` — the party twin (skill
    /// 6039): a PARTY_BAN punishment for the buff's life.
    BlockParty,
    /// `handlers/effecthandlers/BlockAction.java` — blocks the listed action
    /// ids (`BotReportTable`'s negative constants; skills 6055/6056 block
    /// `-2`, trade). Two of the ids additionally start a punishment while the
    /// buff is up, and `checkCondition` is consulted by `TradeRequest`.
    BlockAction {
        blocked_actions: Vec<i32>,
    },
    /// `handlers/effecthandlers/Flag.java` — force the PvP flag on for the
    /// buff's life (bot-report skill 6040: "you can be attacked").
    PvpFlag,
    /// `handlers/effecthandlers/BlockAbnormalSlot.java` — while this buff is
    /// up, the listed abnormal types cannot land on the target at all. Backs
    /// the Prophecy family's mutual exclusion (Prophecy of Water 1355 blocks
    /// every `BUFF_SPECIAL_*` slot) and Heroic Miracle 395 (`INVINCIBILITY`).
    BlockAbnormalSlot {
        slots: Vec<String>,
    },
    /// `handlers/effecthandlers/Mute.java` — silence: magic skills refused.
    /// Landing it also aborts the victim's current cast; raid bosses are immune
    /// (`onStart`'s `isRaid()` bail).
    Mute,
    /// `handlers/effecthandlers/PhysicalMute.java` — the physical twin,
    /// refusing non-magic skills.
    PhysicalMute,
    /// `OpenDoor` (Unlock 27) — pick a lock. `chance` is per skill level
    /// (30/50/75, then 100 from level 4). Java refuses outright, with its own
    /// message, when the door is not `openMethod="BY_SKILL"` (unless the cast
    /// came from an *item*, which is what `is_item` records) or belongs to a
    /// fort; a roll that misses gets the softer "failed to unlock" message.
    OpenDoor {
        chance: i32,
        is_item: bool,
    },
    /// `OpenChest` (Unlock 27) — the treasure-box half of the same skill, and
    /// a parameterless effect whose entire behaviour is a **level check**:
    /// within 6 levels (5 above 77) the box pops open, dies without exp/sp and
    /// rolls its *own* drop list; outside that band it turns on you instead.
    OpenChest,
    /// `Bluff` (Blinding Blow 321, Bluff 358) — spin the target to face the
    /// **caster's** heading, so a rogue behind you leaves you facing away.
    /// Chance-rolled through `calcProbability`; raid bosses and their minions
    /// are immune.
    Bluff {
        chance: i32,
    },
    /// `Unsummon` (Erase 1395) — dismiss the target's servitor. `canStart`
    /// requires the *effected* to **be** a summon, so the skill is aimed at the
    /// pet, not its owner.
    Unsummon {
        chance: i32,
    },
    /// `DeathLink` (Curse Death Link 1159) — magic damage scaled by how close
    /// the **caster** is to death: `power × (2 − 2·curHp/maxHp)`, i.e. ×2 at
    /// 0 HP and ×0 at full. Casting it healthy does nothing at all.
    DeathLink {
        power: f64,
    },
    /// `CpHealPercent` — restore a share of the target's **max CP**, clamped
    /// by `MAX_RECOVERABLE_CP` (Victories of Pa'agrio 1414, Pa'agrio's Fist
    /// 1416).
    CpHealPercent {
        power: f64,
    },
    /// `HpByLevel` — a flat HP restore on the **effector**, i.e. the caster,
    /// not the target (Life Scavenge 46, Corpse Life Drain 1151: you drain a
    /// corpse to heal *yourself*). Reads `<power>`, not `<amount>`.
    HpByLevel {
        power: f64,
    },
    /// `Lucky` (194) — an **empty effect**: Java's handler carries only a
    /// `canStart` player guard and no mechanic at all. `Player.isLucky()` is
    /// `level <= 9 && isAffectedBySkill(194)`, so the buff's *presence* is the
    /// whole implementation; it exempts a newbie from the death exp penalty
    /// and from vitality consumption.
    Lucky,
    /// `DispelBySlotMyself` — strip the bearer's own buffs of the listed
    /// abnormal types (Flames of Invincibility 1427 clears `MAGICAL_STANCE`
    /// before it lands). Two differences from `DispelBySlot`: the list carries
    /// **no levels**, and an `irreplacableBuff` is **spared**.
    DispelBySlotMyself {
        dispel: Vec<String>,
    },
    /// `SkillEvasion` — a flat % chance to dodge an incoming skill of a given
    /// `magicType` (Ultimate Evasion 111, Evasion 446; both `magicType 0`, the
    /// physical-skill bucket). Java keeps a **per-magicType map**, not a single
    /// stat, so a buff that dodges physical skills does nothing against magic.
    SkillEvasion {
        magic_type: i32,
        amount: f64,
    },
    /// `SkillTurning` — Spell Turning (1412), despite the name an offensive
    /// `ENEMY_ONLY` instant that **breaks the target's cast** on a chance roll.
    /// Never fires on the caster themselves, and raid bosses are immune.
    SkillTurning {
        chance: i32,
        static_chance: bool,
    },
    /// `TargetMe` — force the effected **playable** to target the caster and
    /// **lock** it there for the buff's duration (Aggression 28, Aggression
    /// Aura 18). `TargetMeProbability` is the instant, chance-rolled variant
    /// with no lock (Vengeance 368).
    TargetMe,
    TargetMeProbability {
        chance: i32,
    },
    /// G34 S3 flag-only effects — each is a single [`effect_flag`] bit and
    /// nothing else; the constants say what reads them. They survive
    /// `apply_skill_effects`' empty-effects guard through `has_state_flag`.
    BuffBlock,
    PhysicalShieldAngleAll,
    Passive,
    Untargetable,
    DisableTargeting,
    PhysicalAttackMute,
    BlockResurrection,
    BlockEscape,
    AbnormalShield,
    /// `handlers/effecthandlers/DebuffBlock.java` — incoming debuffs fail while
    /// this is up.
    DebuffBlock,
    /// `handlers/effecthandlers/BlockControl.java` — the "out of control"
    /// state; blocks item use in this port.
    BlockControl,
    /// `handlers/effecthandlers/TargetCancel.java` — an instant, chance-rolled
    /// effect that drops the victim's target and aborts their attack and cast
    /// (Trick 11, Switch 12, Aura Flash 1417).
    TargetCancel {
        chance: i32,
    },
    /// The MP-restore family — four Java handlers that differ only in how they
    /// compute the amount, then share one apply path (dead/door/`isMpBlocked`
    /// gate, overheal clamp, `broadcastStatusUpdate`, and the self-vs-other
    /// system message).
    ///
    /// | variant | Java handler | amount |
    /// |---|---|---|
    /// | [`Self::ManaHeal`] | `ManaHeal` | flat `power`, then `MANA_CHARGE` |
    /// | [`Self::ManaHealByLevel`] | `ManaHealByLevel` | as above, then a level-gap penalty |
    /// | [`Self::ManaHealPercent`] | `ManaHealPercent` | `maxMp * power / 100` |
    /// | [`Self::MpRestore`] | `Mp` | flat, or `maxMp * amount / 100` in `PER` mode |
    ///
    /// Mortal Strike 410 is the one learnable `ManaHeal`; Recharge 1013,
    /// Servitor Recharge 1126 and Mass Recharge 1428 are the `ManaHealByLevel`
    /// ones; Pain of Sagittarius 417 and Body To Mind 1157 the `Mp` ones.
    ManaHeal {
        power: f64,
    },
    ManaHealByLevel {
        power: f64,
    },
    ManaHealPercent {
        power: f64,
    },

    /// `Feed` — restores a food bar (Java `effecthandlers/Feed`). Which of the
    /// three params applies is decided by *who is fed*: `normal` for a pet,
    /// and for a player `wyvern` while riding one, else `ride`. The same food
    /// item therefore serves a summoned strider and a ridden one.
    Feed {
        normal: i32,
        ride: i32,
        wyvern: i32,
    },

    /// `SummonCubic` — attaches a cubic to the caster (see `game_loop/cubic`).
    SummonCubic {
        cubic_id: i32,
        cubic_level: i32,
    },
    MpRestore {
        amount: f64,
        percent: bool,
    },
    /// `handlers/effecthandlers/MagicalAttackMp.java` — an MP drain (Mana Burn
    /// 1398, Mana Storm 1399, Aura Sink 1102, Seal of Gloom 1210). Damage is
    /// dealt to the target's **MP pool**, not HP, by its own `calcManaDam`
    /// formula. Mana Burn and Mana Storm carry only this effect, so both were
    /// dropped whole before it was ported.
    MagicalAttackMp {
        power: f64,
        critical: bool,
        critical_limit: f64,
    },
    /// `handlers/effecthandlers/TriggerSkillByAttack.java` — a chance to fire
    /// another skill when this creature lands a hit (Sword/Blunt Weapon Mastery
    /// 205, Dagger Mastery 209, Dance of Shadows 366).
    ///
    /// Java's handler takes 15 params; the fields here are the subset the
    /// reachable content actually uses. The rest keep Java's defaults and are
    /// documented as deferred in the slice plan — notably `triggerSkills` (a
    /// multi-skill ladder), `skillLevelScaleTo`, `min`/`maxAttackerLevel` and
    /// `attackerType`, none of which any learnable skill sets.
    TriggerSkillByAttack {
        /// The hit must deal at least this much damage (Java default 1).
        min_damage: i32,
        /// Percent chance, rolled per landed hit.
        chance: i32,
        /// The skill to cast, and at what level.
        skill_id: i32,
        skill_level: i32,
        /// `targetType`: `SELF` casts on the attacker, `MY_PARTY` on their
        /// party. Those are the only two the reachable content uses.
        on_party: bool,
        /// Java compares this for **equality** with the hit's own criticality —
        /// `isCritical=false` means the trigger fires only on *non*-crits, not
        /// "crits don't matter". Dance of Shadows 366 carries one of each.
        is_critical: bool,
        /// `allowWeapons` as a `WeaponType` mask (0 = ALL).
        allow_weapons: u32,
    },
    /// `handlers/effecthandlers/TriggerSkillByDamage.java` — the mirror of
    /// [`SkillEffect::TriggerSkillByAttack`]: it fires when the bearer
    /// **receives** a hit rather than lands one. Mirage (445) is the learnable
    /// carrier — take a hit from a *Playable* and there is an 80 % chance to
    /// cast Mirage (5144) back at the attacker.
    ///
    /// Two gates distinguish it from the attack-side twin and both are ported:
    /// `attackerType` (Mirage restricts to `Playable`, so mobs never set it
    /// off) and `hpPercent`, an *upper* bound — the trigger only arms once the
    /// bearer is at or below that share of HP. Damage-over-time ticks are
    /// excluded (`event.isDamageOverTime()`).
    ///
    /// `triggerSkills` (a multi-skill ladder), `skillLevelScaleTo` and the
    /// attacker level window keep Java's defaults; no learnable carrier sets
    /// them.
    TriggerSkillByDamage {
        min_damage: i32,
        chance: i32,
        skill_id: i32,
        skill_level: i32,
        /// `hpPercent` — an upper bound on the *bearer's* HP share (100 = no
        /// gate). Java compares `currentHpPercent > hpPercent` and bails.
        hp_percent: i32,
        /// `attackerType` narrowed to the one distinction the dist draws:
        /// `Playable` (Mirage) versus the default `Creature` (anything).
        attacker_playable_only: bool,
        /// `targetType`: `ENEMY` casts back at the attacker (Mirage), `SELF`
        /// on the bearer. Those are the two the dist uses.
        on_attacker: bool,
    },
    /// `handlers/effecthandlers/TriggerSkillByMagicType.java` — fires when the
    /// bearer **finishes casting** a skill whose `magicType` is in the list.
    /// Dance of Shadows (366) is the learnable carrier: any ordinary cast
    /// (types 0-4 and 22) fires Cancel Shadow Move (7097) on the party, which
    /// is how the dance's stealth ends the moment you act.
    ///
    /// Note the default `targetType` here is `TARGET`, not `SELF` as on the
    /// damage twin — and the resolution runs against the *triggering cast's*
    /// target, not the bearer.
    TriggerSkillByMagicType {
        /// `magicTypes`, a `;`-separated list. An empty list disables it.
        magic_types: Vec<i32>,
        chance: i32,
        skill_id: i32,
        skill_level: i32,
        /// `targetType`: `MY_PARTY` (Dance of Shadows) versus the default
        /// `TARGET`.
        on_party: bool,
    },
    /// `handlers/effecthandlers/Resurrection.java` — Resurrection 1016, Mass
    /// Resurrection 1254. Does not revive directly: it *proposes* a revive, and
    /// the dead player accepts through a `ConfirmDlg`. `power` is the percentage
    /// of XP lost on death that the revive restores (run through
    /// `calculateSkillResurrectRestorePercent` first); the three percentages are
    /// how much HP/MP/CP they come back with.
    Resurrection {
        power: i32,
        hp_percent: i32,
        mp_percent: i32,
        cp_percent: i32,
    },
    /// `handlers/effecthandlers/Summon.java` — summon a **servitor** (24
    /// learnable skills: Summon Dark Panther 283, Summon Kat the Cat 1111,
    /// Summon Shadow 1128, the golems, …).
    ///
    /// `npc_id` is per skill *level*, so each level summons a stronger
    /// template. `life_time` is in seconds and `<= 0` means "no expiry" (Java
    /// maps it to `Integer.MAX_VALUE` with the note "Classic hack. Resummon
    /// upon entering game.").
    Summon {
        npc_id: i32,
        life_time: i32,
        consume_item_id: i32,
        consume_item_count: i64,
    },
    /// `handlers/effecthandlers/SummonPet.java` — bring out the pet bound to
    /// the collar the player just used. Carries no params: the collar arrives
    /// through `Player.pending_pet_collar` (Java's `PetItemHolder`), and every
    /// stat comes from `PetData`.
    SummonPet,
    /// `handlers/effecthandlers/BlockMove.java` — `setImmobilized(true)` for
    /// the buff's duration (Ultimate Defense 110, Snipe 313, Vengeance 368).
    /// A pure state flag: the whole mechanic is `IMMOBILIZED` being read by the
    /// movement gate.
    BlockMove,
    /// `handlers/effecthandlers/ReflectSkill.java` — a percent chance to bounce
    /// an incoming **debuff** back at its caster (Riposte Stance 340, Physical
    /// Mirror 350, Magical Mirror 351). `magic` selects which of the two Java
    /// stats it pumps; the incoming skill's own `isMagic` decides which is read.
    ReflectSkill {
        magic: bool,
        amount: f64,
    },
    /// `handlers/effecthandlers/Confuse.java` — the victim turns on a random
    /// bystander (Madness 1105, Curse Discord 1163, Seal of Mirage 1213).
    /// Chance-gated by `calcProbability`. Madness and Curse Discord carry only
    /// this effect, so both were dropped whole before it was ported.
    Confuse {
        chance: i32,
    },
    /// `handlers/effecthandlers/RandomizeHate.java` — move the caster's hate
    /// onto a random bystander (Confusion 2, Switch 12). Confusion carries only
    /// this effect. Same chance gate as [`Self::Confuse`].
    RandomizeHate {
        chance: i32,
    },
    /// `handlers/effecthandlers/SilentMove.java` — stealth (Silent Move 221,
    /// Stealth 411, Dance of Shadows 366, Fake Death 60). A pure state flag:
    /// the Java handler has an empty constructor and nothing but
    /// `getEffectFlags`, and the whole mechanic is the aggro-scan gate.
    SilentMove,
    /// `handlers/effecthandlers/FakeDeath.java` — feign death (Fake Death 60).
    /// A state flag *plus* an MP upkeep on the same 5-tick cadence as
    /// `ManaDamOverTime`, which it shares the tick chain with.
    FakeDeath {
        power: f64,
        ticks: i32,
    },
    /// `handlers/effecthandlers/Fear.java` — forced flight (Horror 65, Banish
    /// Undead 405, Banish Seraph 450, Fear 1092, Curse Fear 1169, Word of Fear
    /// 1272, Mass Curse Fear 1381, Turn Undead 1400).
    ///
    /// Periodic: `onStart` shoves the victim 500 units directly away from the
    /// caster, then every tick repeats the shove along the victim's *current
    /// heading*, so they keep running in the direction they were first thrown.
    /// `ticks` is Java's hard-coded `getTicks() == 5` — a `Fear` element in
    /// this dist carries no params at all — kept as a field so the effect
    /// shares the DoT tick chain's cadence arithmetic rather than hard-coding
    /// an interval of its own.
    Fear {
        ticks: i32,
    },
    /// `handlers/effecthandlers/GetAgro.java` — forces the effected NPC to
    /// intend-attack the caster (Aggression 28, Aggression Aura 18, plus the
    /// aggro side-effect on the debuffs Judgment 401/Tribunal 400). No params.
    /// Java also pre-seeds nearby clan-mates with `addDamageHate(effector, 1,
    /// 200)`; this port leaves that to the already-ported
    /// `npc_ai::faction_call`, which pulls clan-mates in on its own once the
    /// taunted NPC is actually landing hits on the caster (at most one
    /// think-tick later than Java's immediate pre-seed) — an argued deviation;
    /// revisit only if that one-tick gap ever turns out to matter in play.
    GetAgro,
    /// `handlers/effecthandlers/AddHate.java` — a flat hate change with no
    /// damage. Positive `power` (Charm 15, Lure 51) adds hate for the caster;
    /// negative reduces it (unused on this dist, but Java supports it).
    AddHate {
        power: f64,
    },
    /// `handlers/effecthandlers/DeleteHate.java` — chance-rolled: wipes the
    /// target's *entire* aggro list and disengages its AI (Java
    /// `setWalking()` + `setIntention(ACTIVE)`). Eva's Serenade 1273, Peace
    /// 1075, Repose 1034.
    DeleteHate {
        chance: i32,
    },
    /// `handlers/effecthandlers/DeleteHateOfMe.java` — chance-rolled: zeroes
    /// just the caster's own aggro entry (`stopHating`), but — matching Java
    /// exactly — still disengages the target's AI wholesale (`setWalking()` +
    /// `setIntention(ACTIVE)`) even if other attackers remain in the list;
    /// the AI naturally re-picks the next-most-hated target on its following
    /// think tick if any hate remains. Bluff 358, Forget 1156, Trick 11.
    DeleteHateOfMe {
        chance: i32,
    },
    /// `handlers/effecthandlers/Root.java` — immobilised. Unlike a stun the
    /// target may still attack and cast.
    Root,
    /// `handlers/effecthandlers/MagicalAttack.java` — instant magic damage.
    MagicalAttack {
        power: f64,
    },
    /// `handlers/effecthandlers/SummonNpc.java`, narrowed to the `EffectPoint`
    /// branch (PLAN_G19_SYMBOLS.md): drop a totem NPC at the aimed ground
    /// point that pulses its template's `union_skill` every `skill_delay`
    /// seconds until `despawn_time`. The `Decoy` and default-spawn branches
    /// are not ported — Decoy's only carrier (525) is in no skill tree, and
    /// every reachable carrier is an `EffectPoint` symbol; `despawn_delay` is the effect's
    /// fallback when the template declares no `despawn_time`.
    SummonNpc {
        npc_id: i32,
        npc_count: i32,
        despawn_delay: i32,
    },
    /// `handlers/effecthandlers/PhysicalAttack.java` — instant physical skill
    /// damage (`77·((pAtk·pAtkMod)·levelMod + power) / (pDef·pDefMod)`, crit ×2,
    /// soulshot ×2). Also backs `PhysicalSoulAttack` (identical formula; its
    /// soul mAtk-style boost is ×1 until charges are modeled). The dagger-blow
    /// skills (`FatalBlow`/`Backstab`/`SoulBlow`) use a different `calcBlowDamage`
    /// formula and are NOT routed here.
    ///
    /// `ignore_shield_defence` is `<ignoreShieldDefence>` (55 skills on this
    /// dist declare it): when false, `calcShldUse` runs and a normal block adds
    /// the shield's `sDef` to the divisor while a perfect block cuts the hit to
    /// **1**.
    PhysicalAttack {
        power: f64,
        p_atk_mod: f64,
        p_def_mod: f64,
        critical_chance: f64,
        ignore_shield_defence: bool,
    },
    /// `handlers/effecthandlers/PhysicalAttackHpLink.java` — Fatal Counter
    /// (314) and Fatal Arrow (10905). Structurally identical to
    /// [`SkillEffect::PhysicalAttack`] — same fields, same formula, so the two
    /// share one arm — with one extra multiplier at the end:
    /// `−(curHp·2 / maxHp) + 2`, keyed on the **caster's** missing HP. At full
    /// health that is 0 and the shot does nothing; the skill's own description
    /// says "the power of the attack increases as your HP decreases".
    ///
    /// Two defaults differ from `PhysicalAttack` and both matter: Java's
    /// `criticalChance` default here is **0**, not 10 (and Fatal Counter
    /// declares none, so it never crits), and there is no
    /// `ignoreShieldDefence` param at all, so `calcShldUse` always runs.
    PhysicalAttackHpLink {
        power: f64,
        p_atk_mod: f64,
        p_def_mod: f64,
        critical_chance: f64,
        ignore_shield_defence: bool,
    },
    /// `handlers/effecthandlers/PolearmSingleTarget.java` — Focus Attack (317),
    /// a toggle that trades the polearm **sweep** for accuracy and crit damage.
    /// Java sets `PHYSICAL_POLEARM_TARGET_SINGLE` as a *fixed* value of 1 on
    /// start and removes it on exit; `generateAttackTargetData` skips the whole
    /// `ATTACK_COUNT_MAX` loop while it is above 0.
    ///
    /// Its two stat halves already landed through the registry, so without this
    /// one Focus Attack was a **pure bonus with no cost** — the trade it exists
    /// to offer was missing entirely.
    PolearmSingleTarget,
    /// `handlers/effecthandlers/Betray.java` — Betray (1380). `canStart`
    /// requires a **player** effector and a **summon** effected, so the skill
    /// is aimed at somebody else's servitor. It stamps `EffectFlag.BETRAYED`
    /// (the servitor stops obeying and becomes auto-attackable) and points its
    /// AI at its own owner; `onExit` returns it to idle.
    Betray,
    /// `handlers/effecthandlers/ImmobilePetBuff.java` — Servitor Empowerment
    /// (1299). `setImmobilized(true)` on the effected **summon**, but only when
    /// the effector is the summon itself or its owner, so you cannot root
    /// somebody else's pet with it. The port already has the flag
    /// (`IMMOBILIZED`); what is new is the ownership gate.
    ImmobilePetBuff,
    /// `handlers/effecthandlers/CallParty.java` — Chant of Gate (1429). Recall
    /// every *other* party member to the caster, each gated by CallPc's shared
    /// `checkSummonTargetStatus`. Unlike Summon Friend there is **no
    /// `ConfirmDlg`**: Java calls `teleToLocation` directly, so the members
    /// have no say in it.
    CallParty,
    /// `handlers/effecthandlers/ReduceDropPenalty.java` — Residence Death
    /// Fortune (610) and Noblesse Fortune (1325). Grants **two** stats per
    /// `type`: the exp-loss reduction and a death-penalty twin.
    ///
    /// Only the exp half has a consumer. `REDUCE_DEATH_PENALTY_BY_MOB`/`_PVP`/
    /// `_RAID` are merged by this handler and then read by **nothing in Java at
    /// all** — so Noblesse Fortune, whose only param is `deathPenalty -100`
    /// with `type RAID`, does *nothing whatever* on this dist. Ported as
    /// written: the exp stat is granted and consumed, the dead twin is not
    /// modelled, and the census stops counting the name.
    ///
    /// `type` selects which trio: `MOB` (the default), `PK` → the PvP stats,
    /// `RAID`.
    ReduceDropPenalty {
        /// The exp-loss multiplier as Java merges it: `amount/100 + 1`, so
        /// `-12` becomes ×0.88.
        exp_mul: f64,
        kind: ReduceDropKind,
    },
    /// `handlers/effecthandlers/ResurrectionSpecial.java` — Salvation (1410),
    /// Soul of the Phoenix (438). The auto-resurrect: a self-buff that does
    /// nothing while it is up and proposes a revive on **`onExit`**, which is
    /// what fires when death strips it. `power` is the share of lost XP the
    /// revive restores, as with the ordinary `Resurrection`.
    ResurrectionSpecial {
        power: i32,
        hp_percent: i32,
        mp_percent: i32,
        cp_percent: i32,
    },
    /// `handlers/effecthandlers/NightStatModify.java` — Shadow Sense (294),
    /// "increases Accuracy by 3 **at night**".
    ///
    /// Java's `pump` simply returns during the day, so the stat is granted or
    /// not depending on the clock, and a global `OnDayNightChange` listener
    /// re-pumps every bearer when it flips. This port does the same thing from
    /// the other end: the grant is *not* emitted by `stat_modifier_effects`
    /// (which has no clock), and `game_loop::night_stats` rewrites the landed
    /// buff's stored modifiers on every day/night change and when the buff
    /// lands. Same observable behaviour, and the stat hot path stays clean.
    ///
    /// Java also messages the bearer on each flip — but only if they *know*
    /// Shadow Sense, which is its own quirk: a character carrying the effect
    /// from some other source gets the stat and no message.
    NightStatModify {
        stat: Stat,
        amount: f64,
        mode: StatModifierType,
    },
    /// `handlers/effecthandlers/Teleport.java` — a jump to fixed coordinates:
    /// `teleToLocation(new Location(x, y, z), true, null)`.
    ///
    /// This is what every **destination** Scroll of Escape is: 107 reachable
    /// skills on this dist, and until G34 S6 the effect was unparsed, so every
    /// one of them loaded with an empty effect list and did nothing at all.
    /// The SoE scrolls key their destination off the skill **level** (skill
    /// 2213 alone carries 22 towns), which is why the coordinates are ordinary
    /// per-level values rather than constants.
    Teleport {
        x: i32,
        y: i32,
        z: i32,
    },
    /// `handlers/effecthandlers/Hp.java` — an instant HP change, `DIFF` (a flat
    /// amount) or `PER` (a share of **max** HP). Distinct from `Heal`: no
    /// `calcHeal` pipeline, no healing-stat scaling, no overheal message — it
    /// is the raw item effect behind Elixir of Life (2287) and the food/snack
    /// items, which parsed to *nothing* before this.
    Hp {
        amount: f64,
        percent: bool,
    },
    /// `handlers/effecthandlers/CallSkill.java` — cast another skill outright,
    /// no cast time and no cost. Java guards against the obvious infinite loop
    /// (a skill that calls itself at the same level returns immediately).
    CallSkill {
        skill_id: i32,
        skill_level: i32,
        chance: i32,
    },
    /// `handlers/effecthandlers/Heal.java` — instant HP restore.
    Heal {
        power: f64,
    },
    /// `handlers/effecthandlers/HealPercent.java` — instant HP restore as a
    /// `power`% share of the target's max HP (100 = full). Backs core priest
    /// heals — Miracle (1426), Benediction (1271), Restore Life (1258),
    /// Revival (181, self-res follow-up), Touch of Life (341, alongside its
    /// own `HealOverTime`/`HealEffect`) — none of which had ever restored HP
    /// on this port; the effect name wasn't recognized, so the buff/cast
    /// landed but the heal amount was always 0. Unlike [`SkillEffect::Heal`]
    /// this does **not** read the recipient's `HealEffect`/`HealEffectAdd`
    /// stats (Java's `HealPercent.instant` never touches them). A negative
    /// `power` (present elsewhere in the datapack, none of it learnable) is
    /// damage instead of healing, via the shared `apply_skill_damage` path —
    /// ported for parity even though no reachable skill exercises it today.
    /// Also skips a *positive*-power (heal) landing while
    /// `effected.isHpBlocked()` — the negative-power (damage) branch already
    /// gets this for free through the shared `apply_skill_damage` path.
    HealPercent {
        power: f64,
    },
    /// `handlers/effecthandlers/FocusMomentum.java` — the "Force" gain half of
    /// the Sonic/Force skill family (Sonic Focus 8, Focus Force 50, Sonic Rage
    /// 345, Raging Force 346, …): `+amount` charges (default 1), capped at
    /// `max_charges.min(8)` — Java's `MAX_MOMENTUM` stat is never set anywhere
    /// in this datapack, so `8` (the hardcoded fallback `FocusMomentum.java`
    /// itself passes to `getValue`) is the real cap on this build, not a
    /// simplification. Already at the cap: refused with SM 324 (no gain).
    /// Otherwise SM 323 (`"Your force has increased to level $s1"`) +
    /// `EtcStatusUpdate` (the charge-count icon).
    FocusMomentum {
        amount: i32,
        max_charges: i32,
    },
    /// `handlers/effecthandlers/EnergyAttack.java` — the "Force" *spend* half:
    /// instant physical damage (Double Sonic Slash 5, Sonic Blaster 6, Sonic
    /// Buster 9, Force Burst/Storm/Blaster 17/35/54, …), sharing
    /// [`SkillEffect::PhysicalAttack`]'s `77·((pAtk·levelMod) + power) /
    /// (pDef·pDefMod)` core (same simplifications: no weapon/general trait,
    /// weakness, attribute or PvP/PvE multiplier terms — none of those are
    /// modeled here either) times `energyChargesBoost = 1 + charge·0.1`, where
    /// `charge = min(charge_consume, player.charges)` is spent on landing.
    /// `charge_consume` is a **skill-level** `<chargeConsume>` tag, not an
    /// `<effect>` child — Java's effect constructors read the skill's whole
    /// merged param set, not just their own element's children.
    EnergyAttack {
        power: f64,
        critical_chance: f64,
        p_def_mod: f64,
        charge_consume: i32,
        ignore_shield_defence: bool,
    },
    /// Dagger blow skills (`FatalBlow`/`Backstab`/`SoulBlow`) — instant physical
    /// damage via `Formulas.calcBlowDamage`, gated by a `calcBlowSuccess` land
    /// roll (blows can miss). `critical_chance` is `Some` for FatalBlow/Backstab
    /// (rolls `calcCrit` to double the hit) and `None` for SoulBlow (whose
    /// charged-soul boost is ×1 until charges land). `backstab` requires the
    /// caster to be outside the target's front arc.
    /// SoulBlow's charged-soul boost is not ported: its only carrier here is
    /// skill 505, which appears in no skill tree.
    Blow {
        power: f64,
        chance_boost: f64,
        critical_chance: Option<f64>,
        backstab: bool,
    },
    /// `handlers/effecthandlers/Lethal.java` — the instant-kill/half-kill
    /// secondary effect riding alongside `Backstab`/`FatalBlow`/
    /// `PhysicalAttack` on Backstab (30), Lethal Blow (344), Deadly Blow
    /// (263), Critical Blow (409), Lethal Shot (343), … — previously dropped
    /// (the doc comment on [`SkillEffect::Blow`] above names it), so
    /// those skills' damage landed but the bonus kill chance never rolled.
    /// `full_lethal`/`half_lethal` are already 0-100 percentages (unlike
    /// Java's `AttackTrait` effect, this constructor doesn't `/100` these).
    /// A landed full-lethal sets HP (and CP, for a player) to 1; a half-kill
    /// sets a player's CP to 1 or halves a monster's HP. Java's
    /// `chanceMultiplier` (`calcAttributeBonus * calcGeneralTraitBonus`) scales
    /// both kill chances, and both halves are real. Raid bosses are
    /// immune (`isLethalable()`, mirroring the same raid-immunity check
    /// `Mute`'s cast-interrupt already has); `INSTANT_KILL_RESIST` isn't
    /// rolled at all — like `MAX_MOMENTUM`, no skill/item/npc in this
    /// datapack ever sets it, so Java's own roll against it is unconditionally
    /// lost and would never change the outcome.
    /// The three checks this arm was once missing are all present now, and
    /// two of the three had stopped being gaps before they were closed:
    /// `isHpBlocked()` was already consulted, and grand bosses were already
    /// immune because `is_raid()` matches the `GrandBoss` type name as well as
    /// `RaidBoss`. What actually needed doing was the door case and
    /// `calcCounterAttack`, which Java fires whether or not the kill landed.
    Lethal {
        full_lethal: f64,
        half_lethal: f64,
    },
    /// `handlers/effecthandlers/HpDrain.java` — magic damage (same
    /// `calcMagicDam` core as `MagicalAttack`) that also heals the caster by
    /// `percentage`% of the HP actually drained (CP absorbs first, clamped to
    /// the target's remaining HP). Backs Vampiric Touch/Claw.
    HpDrain {
        power: f64,
        percentage: f64,
    },
    /// `handlers/effecthandlers/DamOverTime.java` — a poison/bleed damage-over-
    /// time debuff. Lands as an `ActiveBuff` for `abnormalTime` and ticks every
    /// `ticks * EFFECT_TICK_RATIO` ms (Java `BuffInfo.scheduleEffects`) for
    /// `power * ticks * EFFECT_TICK_RATIO / 1000` damage per tick
    /// (`AbstractEffect.getTicksMultiplier`), stopping when the buff expires or
    /// the target dies. `can_kill == false` (the XML default) clamps each tick
    /// so it leaves the target at 1 HP. Backs Curse Poison (1168), Poison,
    /// Bleed, etc.
    DamOverTime {
        power: f64,
        ticks: i32,
        can_kill: bool,
    },
    /// `handlers/effecthandlers/Cp.java` — an instant CP change. `percent`
    /// selects Java's `PER` mode (a share of max CP) over `DIFF` (a flat
    /// amount). Braveheart 440 grants `+1000 DIFF`; Wrath 320 and Touch of
    /// Death 342 take CP away.
    Cp {
        amount: f64,
        percent: bool,
    },
    /// `handlers/effecthandlers/HealOverTime.java` — periodic HP change on the
    /// same tick chain as [`SkillEffect::DamOverTime`]. **`power` is routinely
    /// negative on this dist** (Fury Fists 222 `-12`, Arcane Wisdom 336 `-50`):
    /// those are toggles that *drain* HP for their upkeep, so this is not a
    /// heal-only effect despite the name.
    HealOverTime {
        power: f64,
        ticks: i32,
    },
    /// `handlers/effecthandlers/ManaDamOverTime.java` — periodic MP drain
    /// (positive `power` = MP removed). Silent Move 221 and friends are toggles
    /// paying MP upkeep; when a tick's drain exceeds current MP the toggle is
    /// switched off (Java returns `false`, which cancels a toggle).
    ManaDamOverTime {
        power: f64,
        ticks: i32,
    },
    /// `handlers/effecthandlers/Relax.java` — the Relax toggle (skill **226**,
    /// learnable at level 5 by Human and Orc Fighters, so this is early-game
    /// content rather than a curiosity).
    ///
    /// Sits the caster down on start, then drains `power` MP per tick while
    /// they stay seated. Java stops it — via the toggle-cancelling `false`
    /// return — on three conditions the plain MP-upkeep effects do not have:
    /// the holder stood up, their HP came back to full (with its own message),
    /// or the drain exceeds current MP.
    Relax {
        power: f64,
        ticks: i32,
    },
    /// `handlers/effecthandlers/ChameleonRest.java` — Chameleon Rest (296),
    /// which is [`SkillEffect::Relax`] with two differences that matter.
    ///
    /// It carries `SILENT_MOVE` **as well as** `RELAXING`, so resting under it
    /// also hides you from a monster's pre-emptive aggro (that is the whole
    /// point of the skill, per its own description). And it has *no* HP-full
    /// stop: Relax retires itself once there is nothing left to heal, while
    /// this one keeps running until you stand up or run out of MP — you are
    /// not resting to heal, you are resting to hide.
    ChameleonRest {
        power: f64,
        ticks: i32,
    },
    /// `handlers/effecthandlers/ManaHealOverTime.java` — the mirror of
    /// [`SkillEffect::ManaDamOverTime`]: positive `power` **restores** MP each
    /// tick, clamped to the recoverable ceiling. Force Meditation (441),
    /// Invocation (1430) and Soul Harmony (1480) on this dist.
    ///
    /// Java's guard is asymmetric and worth keeping: a *positive* power stops
    /// early when already at full MP, while a *negative* one (a drain wearing
    /// this handler) stops when the tick would take MP to 0 or below, and
    /// floors at 1 rather than 0 — a drain of this shape can never kill the MP
    /// pool outright.
    ManaHealOverTime {
        power: f64,
        ticks: i32,
    },
    /// `handlers/effecthandlers/RebalanceHP.java` — Balance Life (1043): pool
    /// the HP of every living party member (plus pets and servitors) in range,
    /// take the party's average HP **percentage**, and set everyone to it. It
    /// is a redistribution, not a heal: the total does not change, so it robs
    /// the healthy to save the dying.
    RebalanceHp,
    /// `handlers/effecthandlers/MpConsumePerLevel.java` — periodic MP drain
    /// for fighter-class toggles (Accuracy 256, Guard Stance 288, Vicious
    /// Stance 312, Parry/Riposte Stance 339/340, War Frenzy 424, Super Haste
    /// 7029, …): every learnable/reachable skill carrying this pairs it with
    /// a real `StatModifier` (e.g. Accuracy's own `+3 ACCUSTOM_COMBAT`), which
    /// already landed via `EFFECT_REGISTRY` — without this effect the toggle
    /// was a free buff with no MP upkeep at all. Ticks and toggle-off-on-
    /// insufficient-MP share `SkillEffect::ManaDamOverTime`'s tick-chain arm
    /// (`handle_dam_over_time_tick`): Java's formula is `power *
    /// getTicksMultiplier()` when the skill has no `abnormalTime` (every
    /// instance in this datapack — all 19 are toggles/`AU` skills with none
    /// set), identical to `ManaDamOverTime`'s. SKIP(G19): Java also has a
    /// level-scaled branch (`((level-1)/7.5) * base * abnormalTime`) for a
    /// skill *with* an `abnormalTime` — all 19 carriers in this datapack
    /// re-verified `abnormalTime`-less on 2026-08-06, so no cast can reach
    /// that branch; split it out of the shared arm if a carrier ever gains
    /// one.
    MpConsumePerLevel {
        power: f64,
        ticks: i32,
    },
    /// `handlers/effecthandlers/MagicalAttackRange.java` — the ranged nuke
    /// (Prominence 1230 family): `MagicalAttack`'s exact `calcMagicDam` core
    /// plus a shield term — a successful block adds `shldDef ·
    /// shieldDefPercent / 100` to mDef, a perfect block caps the hit at 1.
    MagicalAttackRange {
        power: f64,
        shield_def_percent: f64,
    },
    /// `handlers/effecthandlers/Restoration.java` — instant single-item
    /// grant. Backs item-use skills wrapping a fixed pack/box reward (e.g.
    /// spiritshot packs): the item's `<skills>` entry casts this, which is
    /// where the actual reward comes from.
    GiveItem {
        item_id: i32,
        item_count: i64,
        item_enchant_level: i32,
    },
    /// `handlers/effecthandlers/RestorationRandom.java` — one weighted
    /// roulette pick among reward groups (each group can grant multiple
    /// items at once). Used by "pick one of N" reward boxes.
    GiveItemRandom {
        groups: Vec<RestorationGroup>,
    },
    /// `handlers/effecthandlers/Escape.java` — the `/unstuck` skills
    /// (2099/2100) and the Scrolls of Escape: teleport the target to the
    /// destination its `escapeType` names.
    ///
    /// Java runs all of these through one `teleToLocation(TeleportWhereType)`
    /// call, so they share `MapRegionManager.getTeleToLocation`'s **fallthrough**
    /// as well: a residence destination the player has no claim on does not
    /// fail the cast, it lands them in town like a plain `TOWN` escape. See
    /// [`EscapeDest`].
    Escape {
        dest: EscapeDest,
    },
    /// `handlers/effecthandlers/DispelAll.java` — `effected.stopAllEffects()`,
    /// i.e. strip *every* abnormal, not the ranked subset `Dispel`/`DispelBySlot`
    /// pick from. Nothing on this dist teaches it; the reachable carrier is
    /// skill 4177 "Cancellation", cast by ~40 raid bosses (Pan Dryad, Verfa,
    /// Chertuba of Great Soul, …) as a `POINT_BLANK`/`NOT_FRIEND` sweep.
    ///
    /// `stopAllEffects` is `stopEffects(b -> !b.getSkill().isIrreplacableBuff())`,
    /// so an `irreplacableBuff` (Noblesse Blessing, the clan/pledge buffs) does
    /// survive a raid-boss cancel — but nothing else does, at any abnormal
    /// level, which is what makes this far blunter than `DispelBySlot`.
    DispelAll,
    /// `handlers/effecthandlers/Grow.java` — an NPC-only pair of hooks:
    /// `onStart` swaps the collision cylinder for the template's `grown` one
    /// and `onExit` puts the normal one back. Nothing else; the visible swell
    /// is the whole mechanic, and it rides buffs whose stat half already
    /// worked (Might 4028, Spirit Ogre 4091, Ultimate Buff 4318/4341,
    /// Berserker Spirit 4585 — the Orc Prefect and Grandis family).
    ///
    /// It is not cosmetic-only: the cylinder feeds every reach test, so a
    /// grown mob really does swing from further out.
    Grow,
    /// `handlers/effecthandlers/GiveSp.java` — a flat SP grant.
    ///
    /// Two Java quirks that read like bugs and are kept verbatim: the SP goes
    /// to the **effector**, not the effected (they are the same player for
    /// every carrier on this dist, all `SELF` item skills), and the guard
    /// requires *both* ends to be players with the effected not alike-dead.
    GiveSp {
        sp: i64,
    },
    /// `handlers/effecthandlers/TeleportToTarget.java` — the caster jumps to a
    /// point 25 units **behind** the target (the target's heading, flipped),
    /// with a `FlyToLocation(DUMMY)` so the client animates the dash.
    ///
    /// Carrier on this dist: skill 4671, the "Teleport" the Splendor mobs
    /// (21524/21531/21539) use to close on a fleeing player.
    TeleportToTarget,
    /// `handlers/effecthandlers/SetSkill.java` — grant a skill outright
    /// (`addSkill(skill, true)`, so it persists). The Ancient Book: Divine
    /// Inspiration family (skills 9214-9217 → Divine Inspiration 1405 levels
    /// 1-4) is the only reachable carrier.
    SetSkill {
        skill_id: i32,
        skill_level: i32,
    },
    /// `handlers/effecthandlers/CallPc.java` — drag the effected player to the
    /// effector.
    ///
    /// The handler has two halves and only the **NPC** one is ported. When the
    /// effector is a player it opens a Summon Friend `ConfirmDlg` (item cost,
    /// the store/combat/olympiad refusals, a 30 s answer window); when it is
    /// *not* a player — a monster — an `ENEMY`-targeted cast yanks the victim
    /// to the caster outright: abort their cast, abort their attack, stop their
    /// move, `FlyToLocation(DUMMY)`, `setLocation(effector)`.
    ///
    /// That NPC half is Porta's (20213) signature move: skill 4161 "Summon",
    /// `castRange=600`, `ENEMY`/`SINGLE`, 20 s reuse. Without the effect the
    /// skill still parsed, still bucketed into the AI's long-range list and
    /// still cast on cooldown — two seconds of animation that did nothing, so
    /// Porta read as an ordinary melee mob.
    ///
    /// `item_id`/`item_count` are the Summon Friend toll — Spirit Ore or a
    /// Summoning Crystal — and Java charges them to the **target**, not the
    /// caster. Both are 0 on the monster carriers.
    CallPc {
        item_id: i32,
        item_count: i64,
    },
    /// `handlers/effecthandlers/GiveRecommendation.java` — grant the target
    /// `amount` recommendations received (`rec_have`), capped at 255. Backs the
    /// "recommendation certificate" self-target skills.
    GiveRecommendation {
        amount: i32,
    },
    /// `handlers/effecthandlers/HeadquarterCreate.java` — the "Build
    /// Headquarters" siege skill (247): the caster (an attacker clan leader)
    /// plants an HQ flag (NPC 35062) in the siege zone as a respawn point.
    /// `isAdvanced` distinguishes skill **326 "Build Advanced Headquarters"**
    /// (`autoGet` in the noble tree) from the basic 247. Both plant the same
    /// NPC (35062); the advanced camp takes **half** incoming damage.
    ///
    /// That halving is a **deliberate deviation** — Java's
    /// `SiegeFlagStatus.reduceHp` omits an `else` and applies `value/2` *and*
    /// `value`, i.e. 1.5x, which would make the noble-only skill worse than
    /// the basic one. See `docs/CUSTOM_DIST_DEVIATIONS.md`.
    CreateHeadquarter {
        advanced: bool,
    },
    /// `handlers/effecthandlers/OpenCommonRecipeBook.java` /
    /// `OpenDwarfRecipeBook.java` — the "Common Craft" (1322) / "Dwarven Craft"
    /// (1321) ability skills: casting one opens the matching recipe window
    /// (`RecipeManager.requestBookOpen`). Refused while the caster runs a
    /// private store. Instant, self-target.
    OpenRecipeBook {
        dwarven: bool,
    },
    /// `handlers/effecthandlers/Spoil.java` — marks a live monster as spoiled
    /// (`Attackable.setSpoilerObjectId`) so its `<spoil>` list rolls into sweep
    /// loot on death. Gated by `calcSuccess` = `Formulas.calcMagicSuccess`, and
    /// wakes the mob's AI (`EVT_ATTACKED`). Instant.
    Spoil,
    /// `handlers/effecthandlers/Sweeper.java` — on a dead, spoiled corpse the
    /// caster owns (or is in the spoiler's looter party), hands out the loot
    /// rolled at death (`takeSweep`), solo or party-distributed. Instant.
    Sweeper,
    /// `handlers/effecthandlers/ConsumeBody.java` — decays the targeted corpse
    /// immediately (`Npc.endDecayTask`). Paired with `Sweeper` on skill 42 so
    /// the swept body vanishes at once. Instant.
    ConsumeBody,
    /// `handlers/effecthandlers/Sow.java` — the manor sow (skill 2097, cast via
    /// a Seed item). On a live `canBeSown` monster the caller has flagged with
    /// the seed, rolls `calcSuccess` and — on success — marks it seeded and
    /// stashes the crop it will yield on harvest (`Attackable.setSeeded`).
    /// Instant.
    Sow,
    /// `handlers/effecthandlers/Harvesting.java` — the manor harvest (skill
    /// 2098). On a dead, seeded corpse the caster sowed, rolls `calcSuccess` and
    /// hands over the stashed crop (`Attackable.takeHarvest`). Instant.
    Harvesting,
    /// `handlers/effecthandlers/DispelBySlot.java` — instant cleanse. Stops
    /// every active buff/debuff whose originating skill's `<abnormalType>` is in
    /// the dispel set, provided the listed level is negative (dispel all levels)
    /// or `>=` the buff skill's own `abnormalLevel`. Each `(abnormal_type, level)`
    /// pair comes from the `<dispel>` string (`"POISON,3"`), which is per-skill-
    /// level. Backs Cure Poison (1012), Cure Bleeding, etc. Java's special-cased
    /// `AbnormalType.TRANSFORM` branch is omitted — no transforms in scope yet.
    DispelBySlot {
        dispel: Vec<(String, i32)>,
    },
    /// `handlers/effecthandlers/DispelBySlotProbability.java` — the Bane family
    /// (Warrior Bane 1350, Mass Warrior Bane 1344, …): cleanse a set of
    /// abnormal types, but roll `rate`% **per buff** rather than stripping all
    /// of them. Unlike [`SkillEffect::DispelBySlot`] the spec carries no
    /// per-type level, so every level of a listed type is a candidate.
    DispelBySlotProbability {
        dispel: Vec<String>,
        rate: i32,
    },
    /// `handlers/effecthandlers/DispelByCategory.java` — the "Cancel" family
    /// (Cancellation 1056, Touch of Death 342: `BUFF`/`rate=25`/`max=5`;
    /// Cleanse 1409, Purification Field 1425: `DEBUFF`/`rate=100`/`max=10`).
    /// Unlike [`SkillEffect::DispelBySlot`]/[`SkillEffect::
    /// DispelBySlotProbability`] (a fixed abnormal-type list) this strips
    /// *whatever* is up, walking dances then buffs (`BUFF` slot) or debuffs
    /// (`DEBUFF` slot) in reverse cast order, up to `max`, each gated by
    /// `Formulas.calcCancelSuccess` (`BUFF`: `clamp(rate + (casterMagicLvl -
    /// buffMagicLvl)*2 + (buffAbnormalTime/120)*Stat.RESIST_DISPEL_BUFF, 25,
    /// 75)`, skipped entirely — treated as automatic — when `rate>=100`) or a
    /// flat `rate`% roll (`DEBUFF`). This is `Stat::ResistDispelBuff`'s only
    /// consumer, pumped by an earlier slice but unread until now. Java's
    /// `ALL` slot is dead code — no shipped skill uses it — and is a no-op
    /// here too. `isIrreplacableBuff()`/hero/GM/static-skill exclusions
    /// aren't modeled, matching `DispelBySlotProbability`'s own precedent.
    DispelByCategory {
        slot: DispelSlot,
        rate: i32,
        max: i32,
    },
    /// `handlers/effecthandlers/ProtectionBlessing.java` — the Newbie Helper's
    /// Blessing of Protection (5182): a chaotic (PK) character 10+ levels above
    /// the target cannot damage or be damaged by them. Carries no stat
    /// modifier, so it lands as an icon-only timed `ActiveBuff` (like a bare
    /// `DamOverTime`) — the `PK_PROTECT` abnormal + 7200 s duration are
    /// honored, and the immunity itself is `pvp::protection_blessing_blocks`,
    /// run from both the attack and bad-cast intention paths (Java
    /// `PlayableAI`).
    ProtectionBlessing,
    /// `handlers/effecthandlers/NoblesseBless.java` — Noblesse Blessing (1323):
    /// the target keeps its buffs through death, losing only this blessing.
    /// Carries no stat modifier; its whole mechanic is the
    /// [`effect_flag::NOBLESS_BLESSING`] bit read by `Playable.doDie`, so it
    /// lands as an icon-only timed `ActiveBuff` (kept off the empty-effects
    /// bail by `has_state_flag`).
    NoblesseBless,
    /// `handlers/effecthandlers/DefenceTrait.java` — raises the target's
    /// resistance to a set of `TraitType`s (Mental Shield's HOLD/SLEEP/
    /// DERANGEMENT, Stun Resistance's SHOCK, …) via `mergeDefenceTrait`. The
    /// per-trait resistances are not a single `Stat`, so they live in their own
    /// [`DefenceTraits`](crate::model::components::DefenceTraits) component,
    /// merged on buff start and unmerged on expiry, and are read by
    /// `calc_general_trait_bonus` in the debuff-landing roll.
    DefenceTrait {
        /// `(trait, resistance)` — Java divides the XML percent by 100, so
        /// `<SHOCK>30</SHOCK>` is 0.30. A value **≥ 1.0** is not a 100 %
        /// resistance but an *invulnerability* (Java branches to
        /// `mergeInvulnerableTrait`), which makes the debuff simply never land.
        traits: Vec<(TraitType, f64)>,
    },
    /// `handlers/effecthandlers/VampiricAttack.java` — Vampiric Rage: a chance to
    /// recover a % of melee damage dealt as HP (`ABSORB_DAMAGE_PERCENT` +
    /// `vampiricSum`). The melee HP-absorb path isn't modeled yet, so like
    /// `ProtectionBlessing` this carries no stat modifier and lands as an
    /// icon-only timed `ActiveBuff` (abnormal + duration honored).
    ///
    /// Java `pump`s two values: `ABSORB_DAMAGE_PERCENT += amount/100` and
    /// `vampiricSum += amount · chance`. The pair is what
    /// `VampiricChanceFinalizer` turns into a roll chance, so both ride the
    /// ordinary stat pipeline here (see [`Stat::VampiricSum`]).
    VampiricAttack {
        amount: f64,
        chance: f64,
    },
    /// `handlers/effecthandlers/AttackTrait.java` — the "Detect &lt;Category&gt;
    /// Weakness" family (Insect/Beast/Animal/Dragon/Plant 75/80/87/88/104, Eye
    /// of Hunter/Slayer 359/360): raises the caster's bonus damage against a
    /// set of creature-category `*_WEAKNESS` traits via `mergeAttackTrait`,
    /// the attacker-side counterpart of [`SkillEffect::DefenceTrait`]'s
    /// `mergeDefenceTrait`.
    ///
    /// Merges `amount / 100` onto the caster's [`AttackTraits`] table for each
    /// named trait, which `calcWeaknessBonus` / `calcAttackTraitBonus` read
    /// against the *target's* matching `DefenceTrait`.
    ///
    AttackTrait {
        traits: Vec<(TraitType, f64)>,
    },
    /// `handlers/effecthandlers/DamageBlock.java` — one `<effect>` instance
    /// per block kind (a skill carrying both writes two separate elements,
    /// e.g. Celestial Shield 1418's `BLOCK_HP` + `BLOCK_MP`). Carries no stat
    /// modifier; the whole mechanic is the [`effect_flag::HP_BLOCK`]/
    /// [`effect_flag::MP_BLOCK`] bits, folded into `Skill::effect_flags()`
    /// like `BlockActions`/`Root`/… — so it lands via `has_state_flag`, not
    /// `has_iconless_buff`. `HP_BLOCK` has a real consumer (`game_loop::
    /// combat::is_hp_blocked`, gating `player_receive_damage`/
    /// `npc_receive_damage`); `MP_BLOCK` doesn't, matching Java's own dead
    /// `isMpBlocked()`.
    DamageBlock {
        block_hp: bool,
        block_mp: bool,
    },
    /// `handlers/effecthandlers/MagicMpCost.java` — scales the bearer's
    /// MP-consume rate for **one** `magicType` bucket: `onStart` merges
    /// `amount/100 + 1` into `_mpConsumeStat` with `mul`, `onExit` with `div`,
    /// and `CreatureStat.getMpConsume` multiplies the skill's raw cost by the
    /// bucket for that skill's own `magicType`.
    ///
    /// The bucket is the effect's own `<magicType>` param (defaulting to 0),
    /// **not** the carrying skill's: Arcane Wisdom (336, −30) and Clarity
    /// (1397) discount magic (1), Zealot (420, −50) and Champion Song (364)
    /// discount physical (0), Inner Rhythm (428) discounts dances (3). A
    /// positive amount is a *penalty* — Magical Backfire (1396) triples magic
    /// cost at +200.
    MagicMpCost {
        magic_type: i32,
        amount: f64,
    },
    /// `handlers/effecthandlers/Reuse.java` — the same shape for skill reuse
    /// (`mergeReuseTypeValue` / `CreatureStat.getReuseTime`). Quick Recovery
    /// (164) and Song of Renewal (349) shorten physical cooldowns, Arcane
    /// Agility (338) magical ones; Seal of Suspension (1248) trebles them at
    /// +200. Static-reuse skills bypass it entirely — see [`Skill::static_reuse`].
    Reuse {
        magic_type: i32,
        amount: f64,
    },
    /// `handlers/effecthandlers/DamageShield.java` — `Stat.REFLECT_DAMAGE_PERCENT`:
    /// reflects `amount`% of received damage back at the attacker. Backs Song of
    /// Vengeance (305). The combat damage-reflect path isn't modeled yet, so this
    /// carries no stat modifier and lands as an icon-only timed `ActiveBuff`
    /// (abnormal + duration honored).
    ///
    /// A plain additive `REFLECT_DAMAGE_PERCENT` grant (Java's handler is an
    /// `AbstractStatEffect` in all but name), read off the **target** by
    /// `Creature.doAttack`.
    DamageShield {
        amount: f64,
    },
    /// `handlers/effecthandlers/Transformation.java` — polymorph the caster
    /// into `transformation_id` (Java `TransformData.getTransform`), backing
    /// the "Transform <Monster>" scroll family (541-558, 617-674: Grail
    /// Apostle, Unicorn, Doom Wraith, …). Self-target, `abnormalType
    /// TRANSFORM`, always `<targetType>SELF</targetType>`. Reuses the
    /// `//transform` admin runtime's state mutation
    /// ([`crate::game_loop::admin::transforms::apply_transform_state`]) —
    /// display id, collision, granted transform skills, recomputed speed —
    /// but not its broadcast, since the buff-landing path in
    /// `apply_continuous_effects` already sends `UserInfo`/`CharInfo`; only
    /// the transform-specific self packets (`ExUserInfoAbnormalVisualEffect`
    /// carrying the display id + refreshed `SkillList`) are added on top via
    /// [`crate::game_loop::admin::transforms::refresh_transform_visuals`].
    /// Reverts on `BuffExpire`/dispel/death like any other timed buff, via
    /// [`crate::game_loop::admin::transforms::remove_transform_state`].
    ///
    /// Java's `ConditionPlayerCanTransform` gates the *cast* (refused while
    /// already transformed, sitting, in water, mounted, alike-dead or
    /// cursed-weapon-equipped) — ported at the cast-condition check in
    /// `game_loop::skills::cast` for the already-transformed, in-water and
    /// cursed-weapon-equipped legs (mounted collapses into "already
    /// transformed" on this port, since a horse/bike mount is itself a
    /// transform), the registered-on-event leg (the TvT roster) and the
    /// **sitting** leg.
    Transform {
        transformation_id: i32,
    },
}

/// Java `EffectFlag` — the abnormal-state bitmask a creature carries while
/// certain effects are on it, consulted by the action gates
/// (`Creature.hasBlockActions`/`isRooted`/`isMovementDisabled`).
///
/// **Deviation:** Java caches the mask on `EffectList` and recomputes it on
/// every add/remove (`computeEffectFlags`). This port instead stamps each
/// [`ActiveBuff`] with the flags its skill contributes and ORs the live buff
/// list on read ([`crate::game_loop::abnormal::flags_of`]). Same answer, but
/// there is no cached value to go stale across the several places buffs are
/// added and removed.
///
/// Only the flags with a ported consumer are defined; the rest of Java's ~40
/// are added as their mechanics land.
pub mod effect_flag {
    /// `BLOCK_ACTIONS` — stun / sleep / paralyze: no attacking, casting or
    /// moving. Java also has `CONDITIONAL_BLOCK_ACTIONS` (a `BlockActions`
    /// carrying an `allowedSkills` whitelist); `hasBlockActions()` treats the
    /// two identically, and no whitelisted skill is obtainable on this dist
    /// (see the SKIP on `SkillEffect::BlockActions`), so both map here.
    pub const BLOCK_ACTIONS: u32 = 1 << 0;
    /// `ROOTED` — immobilised, but still able to attack and cast.
    pub const ROOTED: u32 = 1 << 1;
    /// `MUTED` — silenced: **magic** skills are refused (Seal of Silence 1246).
    pub const MUTED: u32 = 1 << 2;
    /// `PSYCHICAL_MUTED` (Java's spelling) — the physical twin: non-magic
    /// skills are refused (Shield Slam 353, Heroic Grandeur 1375).
    pub const PHYSICAL_MUTED: u32 = 1 << 3;
    /// `DEBUFF_BLOCK` — incoming debuffs fail outright (Mystic Immunity 1411,
    /// Celestial Shield 1418).
    pub const DEBUFF_BLOCK: u32 = 1 << 4;
    /// `BLOCK_CONTROL` — Java's "out of control" state (Horror 65, Curse Fear
    /// 1169, Turn Undead 1400). The only ported consumer is the item-use gate
    /// (`UseItem`'s `isControlBlocked()`); Java's broader summon/mob-control
    /// meaning needs G29.
    pub const BLOCK_CONTROL: u32 = 1 << 5;
    /// `NOBLESS_BLESSING` (Java's spelling) — Noblesse Blessing (1323): on
    /// death the creature keeps every other buff and loses only the blessing
    /// itself (`Playable.doDie`). Java's sibling [`RESURRECTION_SPECIAL`] has
    /// the same "keep your buffs" role there and landed with G34 S4.16 —
    /// `stop_effects_on_death` tests both flags together.
    pub const NOBLESS_BLESSING: u32 = 1 << 6;
    /// `HP_BLOCK` — incoming HP damage is refused outright (Celestial Shield
    /// 1418, Flames of Invincibility 1427, Dance of Medusa 367, Sonic/Force
    /// Barrier 442/443). `CreatureStatus.reduceHp`'s real gate: `if
    /// (creature.isHpBlocked() && !(isDOT || isHPConsumption)) return;` — a
    /// DoT tick or a skill's own HP cost still goes through.
    pub const HP_BLOCK: u32 = 1 << 7;
    /// `MP_BLOCK` — MP cannot be drained or restored while this is up.
    ///
    /// **Correction:** this was previously documented here as having no callers
    /// anywhere in Java. That grep covered `java/` only — every effect handler
    /// actually lives under `dist/game/data/scripts/handlers/effecthandlers/`,
    /// and **five** of them read `isMpBlocked()`: `MagicalAttackMp`, `Mp`,
    /// `ManaHeal`, `ManaHealByLevel`, `ManaHealPercent`. The flag is live, not
    /// dead code. `MagicalAttackMp`'s gate is ported
    /// (`game_loop::abnormal::is_mp_blocked`), and so is the whole MP-restore
    /// family (`ManaHeal`/`ManaHealByLevel`/`ManaHealPercent`/`Mp`) — the flag
    /// blocks restoration as well as drain.
    pub const MP_BLOCK: u32 = 1 << 8;
    /// `FEAR` — Java declares the flag on `Fear.getEffectFlags()`, but **no
    /// `isAfraid()` accessor exists and nothing reads the bit** (grepped the
    /// whole Java tree: the only hits are the `EffectFlag` declaration itself
    /// and two unrelated dist scripts with their own `isAfraid` fields). The
    /// entire fear mechanic is the forced movement in the handler, not a gate
    /// — a feared player is *not* stopped from walking or acting. Folded here
    /// for completeness, with no consumer, matching Java's own dead code the
    /// same way [`MP_BLOCK`] does.
    pub const FEAR: u32 = 1 << 9;
    /// `SILENT_MOVE` — stealth (Silent Move 221, Stealth 411, Dance of Shadows
    /// 366, and the `SilentMove` half of Fake Death 60). Read by
    /// `AttackableAI.isAggressiveTowards`: an aggressive monster simply does
    /// not notice a silent-moving playable. **Raid bosses see through it**, and
    /// so would an NPC with `canSeeThroughSilentMove()` — except
    /// `setSeeThroughSilentMove` has no callers anywhere in the Java tree, so
    /// that flag is always false (the `MP_BLOCK`/`MAX_MOMENTUM` pattern again).
    pub const SILENT_MOVE: u32 = 1 << 10;
    /// `FAKE_DEATH` — feign death (Fake Death 60). Folds into
    /// `Player.isAlikeDead()`, which is what takes the player out of every
    /// aggro scan; the client side is the `ChangeWaitType`/`Revive` pair.
    pub const FAKE_DEATH: u32 = 1 << 11;
    /// `CONFUSED` — declared by `Confuse.getEffectFlags()`, but **unreachable
    /// on this dist**: `Confuse.isInstant()` is true, so the effect is never
    /// added to a `BuffInfo`'s effect list, and none of the five skills that
    /// carry it declares an `<abnormalTime>` for a buff to live in anyway.
    /// Java's two readers (`AttackableAI`'s "attack the effect's target rather
    /// than the most-hated" branch and `Creature.onActionRequest`'s player
    /// gate) therefore never fire. Folded for completeness with no consumer —
    /// the same `FEAR`/`MP_BLOCK` pattern.
    pub const CONFUSED: u32 = 1 << 12;
    /// `IMMOBILIZED` — Java `Creature._isImmobilized`, set by `BlockMove`
    /// (Ultimate Defense 110, Snipe 313, Vengeance 368). Folded into
    /// `isMovementDisabled()` beside `ROOTED`: the creature is rooted in place
    /// but can still attack and cast, which is the point of these stances.
    ///
    /// This is the `_isImmobilized` term `game_loop::abnormal`'s module docs
    /// listed as having "no ported source".
    pub const IMMOBILIZED: u32 = 1 << 13;
    /// `BLOCK_RESURRECTION` — Java `Creature.isResurrectionBlocked()`, read by
    /// `Player.reviveRequest`. `BlockResurrection` has **no learnable source on
    /// this dist** (4 non-learnable skills carry it), so the gate is ported but
    /// nothing reachable trips it — the recurring "declared, unreachable here"
    /// shape.
    pub const BLOCK_RESURRECTION: u32 = 1 << 14;
    /// `CANNOT_ESCAPE` — Java `Creature.cannotEscape()`, read by the
    /// `OpCanEscape` skill condition (161 skills, 2 learnable: the two
    /// `/unstuck` escapes) and by the escape effects themselves. The flag's
    /// only source is the `BlockEscape` effect (Clan Escape Lock 19113), which
    /// is **not ported yet** — the gate is live and correct, nothing currently
    /// raises it.
    /// Sourced by the `BlockEscape` effect (Clan Escape Lock 19113).
    pub const CANNOT_ESCAPE: u32 = 1 << 15;
    /// `BUFF_BLOCK` — incoming **buffs** are refused; debuffs still land. Java
    /// `EffectList.add`: `if (isBuffBlocked() && !skill.isBad()) return;`, the
    /// exact mirror of [`DEBUFF_BLOCK`]. Source: `BuffBlock` (Dance of Medusa
    /// 367, plus 7 NPC skills).
    pub const BUFF_BLOCK: u32 = 1 << 16;
    /// `PHYSICAL_SHIELD_ANGLE_ALL` — the shield covers all 360°, not the usual
    /// 120° frontal arc, so a back attack can be blocked too. Java
    /// `Formulas.calcShldUse`: `degreeside = isAffected(…) ? 360 : 120`.
    /// Source: `PhysicalShieldAngleAll` (Aegis 316, Aegis Stance 318).
    pub const PHYSICAL_SHIELD_ANGLE_ALL: u32 = 1 << 17;
    /// `PASSIVE` — an aggressive monster stops being aggressive. Java
    /// `Monster.isAggressive()`: `getTemplate().isAggressive() &&
    /// !isAffected(EffectFlag.PASSIVE)`. Source: the `Passive` effect (Veil
    /// 106, Requiem 1049) — the "pacify the mob" utility line.
    pub const PASSIVE: u32 = 1 << 18;
    /// `UNTARGETABLE` — the bearer cannot be selected at all
    /// (`Creature.isTargetable()`). Source: `Untargetable` (2 items).
    pub const UNTARGETABLE: u32 = 1 << 19;
    /// `TARGETING_DISABLED` — the *bearer* cannot select anything, the
    /// caster-side twin of [`UNTARGETABLE`] (`Creature.isTargetingDisabled()`,
    /// read by `Action`/`AttackRequest`). Source: `DisableTargeting` (1 NPC).
    pub const TARGETING_DISABLED: u32 = 1 << 20;
    /// `PSYCHICAL_ATTACK_MUTED` (Java's spelling) — no **auto-attacking**,
    /// distinct from [`PHYSICAL_MUTED`], which refuses non-magic *skills*.
    /// Java folds it into `Creature.isAttackDisabled()` alongside
    /// `hasBlockActions()`. Source: `PhysicalAttackMute` (1 pet skill).
    pub const PSYCHICAL_ATTACK_MUTED: u32 = 1 << 21;
    /// `ABNORMAL_SHIELD` — **dead in Java**. The `AbnormalShield` handler
    /// returns both this flag and `EffectType.ABNORMAL_SHIELD`, and *nothing in
    /// the entire tree reads either* (grepped `java/` and
    /// `dist/game/data/scripts/`). Its 2 item sources are therefore inert on
    /// Java too. Defined here for completeness with no consumer — the same
    /// shape as [`FEAR`] and [`CONFUSED`], and the reason to grep for readers
    /// before porting a gate rather than after.
    pub const ABNORMAL_SHIELD: u32 = 1 << 22;
    /// `RESURRECTION_SPECIAL` — Java `Playable.isResurrectSpecialAffected()`,
    /// read in exactly one place, `Playable.doDie`: the holder stops *only*
    /// this effect and keeps every other buff through death, the same deal
    /// `NOBLESS_BLESSING` gets. Losing it is what fires the revive proposal.
    pub const RESURRECTION_SPECIAL: u32 = 1 << 24;
    /// `CHAT_BLOCK` — Java `EffectFlag.CHAT_BLOCK`, set by the `BlockChat`
    /// effect (bot-report punishment skill 6038). Read in exactly one place,
    /// `Say2`: a chat-banned player under *this* flag is told they were
    /// reported as an illegal-program user, instead of getting the ordinary
    /// prohibition notice. The block itself comes from the CHAT_BAN punishment
    /// the effect starts, not from the flag.
    pub const CHAT_BLOCK: u32 = 1 << 25;
    /// `BETRAYED` — Java `Summon.isBetrayed()`, with two consumers: the
    /// servitor **refuses its owner's commands** ("your servitor is
    /// unresponsive and will not obey any orders") and `PetSummonInfo` sets
    /// status bit `0x01`, which makes it auto-attackable — you have to kill
    /// your own summon. Set by Betray (1380).
    pub const BETRAYED: u32 = 1 << 23;
}

/// Java `NextActionType` — what `SkillCaster.finishSkill` queues after a cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NextAction {
    #[default]
    None,
    Attack,
    Cast,
}

/// `ReduceDropType` — which of `ReduceDropPenalty`'s three stat pairs to grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReduceDropKind {
    #[default]
    Mob,
    Pk,
    Raid,
}

/// Java `AbnormalVisualEffect` — the client-side *look* of an abnormal (the
/// stun swirl, the poison tint, the silence mark). Purely cosmetic: the
/// mechanics live in [`effect_flag`] and the effect handlers, while this is
/// what the client renders over the character.
///
/// The **full** enum in Java's own order, `(name, client id)`, generated from
/// the Java source. It used to stop at id 38, which silently dropped 102 of the
/// names the dist's own skills reference (`AURA_BUFF`, `ABSORB_SHIELD`, the
/// `*_CHANGE` grade glows…) — the skill parsed, but its visual never reached
/// the client. Order matters beyond lookup: `//ave_abnormal`'s menu pages
/// through `AbnormalVisualEffect.values()`, which is exactly this sequence.
/// `VP_KEEP` shares client id 29 with `VP_UP` in Java, comment and all.
pub const ABNORMAL_VISUAL_EFFECTS: &[(&str, i16)] = &[
    ("DOT_BLEEDING", 1),
    ("DOT_POISON", 2),
    ("DOT_FIRE", 3),
    ("DOT_WATER", 4),
    ("DOT_WIND", 5),
    ("DOT_SOIL", 6),
    ("STUN", 7),
    ("SLEEP", 8),
    ("SILENCE", 9),
    ("ROOT", 10),
    ("PARALYZE", 11),
    ("FLESH_STONE", 12),
    ("DOT_MP", 13),
    ("BIG_HEAD", 14),
    ("DOT_FIRE_AREA", 15),
    ("CHANGE_TEXTURE", 16),
    ("BIG_BODY", 17),
    ("FLOATING_ROOT", 18),
    ("DANCE_ROOT", 19),
    ("GHOST_STUN", 20),
    ("STEALTH", 21),
    ("SEIZURE1", 22),
    ("SEIZURE2", 23),
    ("MAGIC_SQUARE", 24),
    ("FREEZING", 25),
    ("SHAKE", 26),
    ("ULTIMATE_DEFENCE", 28),
    ("VP_UP", 29),
    // Java gives VP_KEEP the same 29 as VP_UP and flags that it does not know
    // the real id; carried as shipped.
    ("VP_KEEP", 29),
    ("REAL_TARGET", 30),
    ("DEATH_MARK", 31),
    ("TURN_FLEE", 32),
    ("INVINCIBILITY", 33),
    ("AIR_BATTLE_SLOW", 34),
    ("AIR_BATTLE_ROOT", 35),
    ("CHANGE_WP", 36),
    ("CHANGE_HAIR_G", 37), // Gold Afro
    ("CHANGE_HAIR_P", 38), // Pink Afro
    ("CHANGE_HAIR_B", 39), // Black Afro
    ("UNKNOWN_40", 40),
    ("STIGMA_OF_SILEN", 41),
    ("SPEED_DOWN", 42),
    ("FROZEN_PILLAR", 43),
    ("CHANGE_VES_S", 44),
    ("CHANGE_VES_C", 45),
    ("CHANGE_VES_D", 46),
    ("TIME_BOMB", 47),
    ("MP_SHIELD", 48),
    ("AIRBIND", 49),
    ("CHANGEBODY", 50),
    ("KNOCKDOWN", 51),
    ("NAVIT_ADVENT", 52),
    ("KNOCKBACK", 53),
    ("CHANGE_7ANNIVERSARY", 54),
    ("ON_SPOT_MOVEMENT", 55),
    ("DEPORT", 56),
    ("AURA_BUFF", 57),
    ("AURA_BUFF_SELF", 58),
    ("AURA_DEBUFF", 59),
    ("AURA_DEBUFF_SELF", 60),
    ("HURRICANE", 61),
    ("HURRICANE_SELF", 62),
    ("BLACK_MARK", 63),
    ("BR_SOUL_AVATAR", 64),
    ("CHANGE_GRADE_B", 65),
    ("BR_BEAM_SWORD_ONEHAND", 66),
    ("BR_BEAM_SWORD_DUAL", 67),
    ("NO_CHAT", 68),
    ("HERB_PA_UP", 69),
    ("HERB_MA_UP", 70),
    ("SEED_TALISMAN1", 71),
    ("SEED_TALISMAN2", 72),
    ("SEED_TALISMAN3", 73),
    ("SEED_TALISMAN4", 74),
    ("SEED_TALISMAN5", 75),
    ("SEED_TALISMAN6", 76),
    ("CURIOUS_HOUSE", 77),
    ("NGRADE_CHANGE", 78),
    ("DGRADE_CHANGE", 79),
    ("CGRADE_CHANGE", 80),
    ("BGRADE_CHANGE", 81),
    ("AGRADE_CHANGE", 82),
    ("SWEET_ICE_FLAKES", 83),
    ("FANTASY_ICE_FLAKES", 84),
    ("CHANGE_XMAS", 85),
    ("CARD_PC_DECO", 86),
    ("CHANGE_DINOS", 87),
    ("CHANGE_VALENTINE", 88),
    ("CHOCOLATE", 89),
    ("CANDY", 90),
    ("COOKIE", 91),
    ("STARS_0", 92),
    ("STARS_1", 93),
    ("STARS_2", 94),
    ("STARS_3", 95),
    ("STARS_4", 96),
    ("STARS_5", 97),
    ("DUELING", 98),
    ("FREEZING2", 99),
    ("CHANGE_YOGI", 100),
    ("YOGI", 101),
    ("MUSICAL_NOTE_YELLOW", 102),
    ("MUSICAL_NOTE_BLUE", 103),
    ("MUSICAL_NOTE_GREEN", 104),
    ("TENTH_ANNIVERSARY", 105),
    ("XMAS_SOCKS", 106),
    ("XMAS_TREE", 107),
    ("XMAS_SNOWMAN", 108),
    ("OTHELL_ROGUE_BLUFF", 109),
    ("HE_PROTECT", 110),
    ("SU_SUMCROSS", 111),
    ("WIND_STUN", 112),
    ("STORM_SIGN2", 113),
    ("STORM_SIGN1", 114),
    ("WIND_BLEND", 115),
    ("DECEPTIVE_BLINK", 116),
    ("WIND_HIDE", 117),
    ("PSY_POWER", 118),
    ("SQUALL", 119),
    ("WIND_ILLUSION", 120),
    ("SAYHA_FURY", 121),
    ("HIDE4", 123),
    ("PMENTAL_TRAIL", 124),
    ("HOLD_LIGHTING", 125),
    ("GRAVITY_SPACE_3", 126),
    ("SPACEREF", 127),
    ("HE_ASPECT", 128),
    ("RUNWAY_ARMOR1", 129),
    ("RUNWAY_ARMOR2", 130),
    ("RUNWAY_ARMOR3", 131),
    ("RUNWAY_ARMOR4", 132),
    ("RUNWAY_ARMOR5", 133),
    ("RUNWAY_ARMOR6", 134),
    ("RUNWAY_WEAPON1", 135),
    ("RUNWAY_WEAPON2", 136),
    ("PALADIN_PROTECTION", 141),
    ("SENTINEL_PROTECTION", 142),
    ("REAL_TARGET_2", 143),
    ("DIVINITY", 144),
    ("SHILLIEN_PROTECTION", 145),
    ("EVENT_STARS_0", 146),
    ("EVENT_STARS_1", 147),
    ("EVENT_STARS_2", 148),
    ("EVENT_STARS_3", 149),
    ("EVENT_STARS_4", 150),
    ("EVENT_STARS_5", 151),
    ("ABSORB_SHIELD", 152),
    ("PHOENIX_AURA", 153),
    ("REVENGE_AURA", 154),
    ("EVAS_AURA", 155),
    ("TEMPLAR_AURA", 156),
    ("LONG_BLOW", 157),
    ("WIDE_SWORD", 158),
    ("BIG_FIST", 159),
    ("SHADOW_STEP", 160),
    ("TORNADO", 161),
    ("SNOW_SLOW", 162),
    ("SNOW_HOLD", 163),
    ("TORNADO_SLOW", 165),
    ("ASTATINE_WATER", 166),
    ("BIG_BODY_COMBINATION_CAT_NPC", 167),
    ("BIG_BODY_COMBINATION_UNICORN_NPC", 168),
    ("BIG_BODY_COMBINATION_DEMON_NPC", 169),
    ("BIG_BODY_COMBINATION_CAT_PC", 170),
    ("BIG_BODY_COMBINATION_UNICORN_PC", 171),
    ("BIG_BODY_COMBINATION_DEMON_PC", 172),
    ("BIG_BODY_2", 173),
    ("BIG_BODY_3", 174),
    ("PIRATE_SUIT", 175),
    ("DARK_ASSASSIN_SUIT", 176),
    ("WHITE_ASSASSIN_SUIT", 177),
    ("AVE_DRAGON_ULTIMATE", 181),
    ("INFINITE_SHIELD1_AVE", 183),
    ("INFINITE_SHIELD2_AVE", 184),
    ("INFINITE_SHIELD3_AVE", 185),
    ("INFINITE_SHIELD4_AVE", 186),
    ("AVE_ABSORB2_SHIELD", 187),
    ("TALI_DECO_BAIUM", 190),
    ("CHANGESHAPE_TRANSFORM", 193),
    ("ANGRY_GOLEM_AVE", 194),
    ("WA_UNBREAKABLE_SONIC_AVE", 195),
    ("HEROIC_HOLY_AVE", 196),
    ("HEROIC_SILENCE_AVE", 197),
    ("HEROIC_FEAR_AVE_1", 198),
    ("HEROIC_FEAR_AVE_2", 199),
    ("AVE_BROOCH", 200),
    ("INFINITE_SHIELD4_AVE_2", 206),
    ("CHANGESHAPE_TRANSFORM_1", 207),
    ("CHANGESHAPE_TRANSFORM_2", 208),
    ("CHANGESHAPE_TRANSFORM_3", 209),
    ("CHANGESHAPE_TRANSFORM_4", 210),
    ("RO_COUNTER_TRASPIE", 215),
    ("RO_GHOST_REFLECT", 217),
    ("CHANGESHAPE_TRANSFORM_5", 218),
    ("ICE_ELEMENTALDESTROY", 219),
    ("DRAGON_ULTIMATE", 700),
    ("CHANGE_HALLOWEEN", 1000),
    ("BR_Y_1_ACCESSORY_R_RING", 10001),
    ("BR_Y_1_ACCESSORY_EARRING", 10002),
    ("BR_Y_1_ACCESSORY_NECKRACE", 10003),
    ("BR_Y_2_ACCESSORY_R_RING", 10004),
    ("BR_Y_2_ACCESSORY_EARRING", 10005),
    ("BR_Y_2_ACCESSORY_NECKRACE", 10006),
    ("BR_Y_3_ACCESSORY_R_RING", 10007),
    ("BR_Y_3_ACCESSORY_EARRING", 10008),
    ("BR_Y_3_ACCESSORY_NECKRACE", 10009),
];

/// Name → client id, or `None` for a name the enum doesn't have — which is
/// simply not shown, matching Java's `findByName` + warning.
pub fn abnormal_visual_client_id(name: &str) -> Option<i16> {
    ABNORMAL_VISUAL_EFFECTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|&(_, id)| id)
}

/// `AbnormalVisualEffect.STEALTH.getClientId()` — GM invisibility's translucent
/// glow, appended by Java whenever `isInvisible()`.
pub const STEALTH_CLIENT_ID: i16 = 21;

/// `dist/game/data/stats/skills/*.xml` → `Skill.java`, scoped to G6.
#[derive(Debug, Clone)]
pub struct Skill {
    pub id: i32,
    pub level: i32,
    pub name: String,
    /// `<icon>` — the client-side icon path (Java `Skill.getIcon()`, default
    /// `icon.skill0000`). Cosmetic: read by the shift-click NPC skill view.
    pub icon: String,
    pub operate_type: OperateType,
    /// Java `Skill.isContinuous()` — an effect that sits on the target for
    /// `abnormal_time` rather than resolving instantly. Drives the NPC AI's
    /// BUFF/DEBUFF bucketing and its "target already has this abnormal" skip.
    pub is_continuous: bool,
    pub target_type: TargetType,
    /// Java `<overHit>` — a killing blow from this skill grants bonus XP
    /// proportional to the *excess* damage (`Attackable.calculateOverhitExp`).
    /// 59 learnable skills carry it (Triple Slash, Power Strike, Sonic Storm…).
    pub over_hit: bool,
    /// Java `<abnormalVisualEffect>` as resolved client ids — what the client
    /// draws on anyone carrying this skill's abnormal. Cosmetic only.
    pub abnormal_visuals: Vec<i16>,
    /// Java `toggleGroupId` — toggles sharing a group are mutually exclusive:
    /// switching one on stops the others (`stopAllTogglesOfGroup`). 0 = no
    /// group.
    pub toggle_group_id: i32,
    /// Java `affectScope` — how the primary target expands into the affected
    /// set (`Skill.forEachTargetAffected`). Defaults to `SINGLE`.
    pub affect_scope: AffectScope,
    /// `<trait>` — the debuff's own `TraitType`, matched against the target's
    /// `DefenceTrait` resistances when it tries to land. `NONE` for most
    /// skills; the dist's stuns declare `SHOCK`, its fear/confuse
    /// `DERANGEMENT`, and so on.
    pub trait_type: TraitType,
    /// Java `affectObject` — the friend/foe filter each swept-up candidate must
    /// pass. Defaults to `ALL` (Java's "no handler" = no filtering).
    pub affect_object: AffectObject,
    /// Java `affectRange` — the radius the scope sweeps (0 = no sweep).
    pub affect_range: i32,
    /// Java `_affectLimit` `[min, max]` from `<affectLimit>min-max</affectLimit>`.
    /// Read through [`Skill::affect_limit`], which reproduces Java's roll.
    pub affect_limit: (i32, i32),
    /// Java `_fanRange` from `<fanRange>` — `unk;startDegree;fanAffectRange;
    /// fanAffectAngle`, the geometry behind the FAN/SQUARE/RING_RANGE scopes.
    /// `[1]` rotates the arc/rect off the caster's heading, `[2]` is the fan
    /// radius / rect length / ring inner radius, `[3]` the fan's full angle /
    /// rect width. `[0]` is never read (non-zero exactly once in the dist).
    /// Level-valued in the XML (one SQUARE breath declares six tuples).
    pub fan_range: [i32; 4],
    /// Java `isMagic`: 0 physical, 1 magic, 2 static, 3 dance/song, 4 trigger.
    /// Drives cast-time scaling (`calc_skill_time_factor`) and crit rolls.
    pub magic_type: i32,
    /// Java `magicLevel` — the skill's own level for magic-hit math. Feeds
    /// `Formulas.calcMagicSuccess` when `CalculateMagicSuccessBySkillMagicLevel`
    /// is on (the dist default), used by the Spoil landing roll.
    pub magic_level: i32,
    /// Java `activateRate` (default -1) — a debuff's base landing rate before the
    /// level/resist math in `Formulas.calcEffectSuccess`. `-1` means the effect
    /// always lands (no resist roll). Feeds `formulas::calc_effect_land_rate`.
    pub activate_rate: i32,
    /// Java `lvlBonusRate` (default 0) — how steeply the caster/target level gap
    /// swings the debuff landing rate; multiplies the level term in
    /// `calc_effect_land_rate`.
    pub lvl_bonus_rate: i32,
    /// Java `effectPoint` — negative marks an offensive ("bad") skill.
    pub effect_point: i32,
    pub cast_range: i32,
    pub effect_range: i32,
    /// Milliseconds from cast start to the skill "landing" (Java `hitTime`),
    /// before casting-speed scaling.
    pub hit_time: i32,
    /// Java `<nextAction>` — what the caster does once the cast finishes.
    /// `SkillCaster.finishSkill`: with `ATTACK` (339 skills on this dist) the
    /// caster resumes attacking the target, with `CAST` (11) it repeats the
    /// skill; `NONE` just fires `EVT_FINISH_CASTING`. Java gates both on the
    /// AI having no queued intention, a real target that is not the caster and
    /// is auto-attackable, and — for `ATTACK` only — shift not being held.
    ///
    /// This is why a Power Strike leaves you swinging rather than standing
    /// still: without it every offensive skill ends combat.
    pub next_action: NextAction,
    /// Java `<abnormalResists>` — abnormal types this skill makes its caster
    /// immune to **while it is casting** (`Formulas.calcEffectSuccess`:
    /// `target.isCastingNow(s -> s.getSkill().getAbnormalResists().contains(
    /// skill.getAbnormalType()))`). 176 skills declare one; the long list on
    /// 146 of them is the "uninterruptible ritual" set.
    pub abnormal_resists: Vec<String>,
    /// Java `hitCancelTime` (seconds) — the launch→finish phase length input;
    /// almost always 0, floored to 500 ms by `calc_skill_cancel_time`.
    pub hit_cancel_time: f64,
    /// Extra server-side cooldown after `finishSkill` (Java `coolTime`).
    pub cool_time: i32,
    /// Reuse delay in ms (Java `reuseDelay`) — enforced server-side via
    /// `Player.reuses` and shown client-side via the `MagicSkillUse` fields.
    pub reuse_delay: i32,
    /// Java `reuseDelayGroup` (default -1): skills sharing a positive group id
    /// share one cooldown. Sent raw in `MagicSkillUse`/`SkillList` — the
    /// client treats 0 as "every skill", so ungrouped must stay -1.
    pub reuse_delay_group: i32,
    /// `<staticReuse>` (Java `Skill._staticReuse`, default false; **1297
    /// skills on this dist set it**). A static-reuse skill's cooldown is its
    /// raw `reuse_delay` — `CreatureStat.getReuseTime` returns before applying
    /// the per-magic-type reuse rate — so no [`SkillEffect::Reuse`] buff can
    /// shorten it.
    pub static_reuse: bool,
    pub mp_consume: i32,
    pub mp_initial_consume: i32,
    pub hp_consume: i32,
    /// `<withoutAction>` (Java `Skill._withoutAction`, default false). An
    /// item skill flagged this way is fired instantly by
    /// `ItemSkillsTemplate` (the `SkillCaster.triggerCast` branch) instead of
    /// going through `useMagic`'s cast bar. Only four skills in the whole
    /// dist set it, none in the Interlude ranges, but the flag is half of
    /// Java's instant/cast decision so it is parsed rather than assumed.
    pub without_action: bool,
    /// Java `isSuicideAttack` — its only consumer is `NpcData.parse`, which
    /// routes the skill into the AI's SUICIDE bucket (cast below 30 % HP).
    pub is_suicide_attack: bool,
    /// `<itemConsumeId>`/`<itemConsumeCount>` (Java `Skill.getItemConsumeId`
    /// / `getItemConsumeCount`, 0 = none) — the "reagent" the skill spends.
    /// Read by `ItemSkillsTemplate.checkConsume` to decide whether the item
    /// handler is the one that destroys the item.
    pub item_consume_id: i32,
    pub item_consume_count: i32,
    /// Seconds a landed buff/debuff lasts (Java `abnormalTime`); 0 for
    /// instant/non-buff skills.
    pub abnormal_time: i32,
    pub abnormal_level: i32,
    /// Raw `<abnormalType>` XML text (Java `AbnormalType` has ~500 entries —
    /// only resolved to a client id, via `abnormal_type_client_id`, for the
    /// handful `AbnormalStatusUpdate` actually needs so far).
    pub abnormal_type: String,
    /// Java `Skill.canBeDispelled()` (`<canBeDispelled>`, default true) — whether
    /// the client's alt+click buff-cancel (`RequestDispel`) is allowed to strip it.
    pub can_be_dispelled: bool,
    /// Java `Skill.isDebuff()` (`<isDebuff>`, default false). A debuff can't be
    /// self-dispelled via alt+click even when `can_be_dispelled` is set.
    pub is_debuff: bool,
    /// Java `Skill.isSharedWithSummon()` (`<isSharedWithSummon>`, **default
    /// true**) — a continuous, non-debuff buff landing on a player is re-applied
    /// to each of their servitors (`Skill.applyEffects`'s "buff sharing"
    /// branch). The default being `true` is the load-bearing part: only three
    /// skills in the whole datapack declare the tag at all, so parsing this like
    /// a normal `false`-default flag would silently stop sharing every buff in
    /// the game.
    pub shared_with_summon: bool,
    /// Java `Skill.isStayAfterDeath()` (`<stayAfterDeath>`, default false) — the
    /// buff survives its holder's death (`EffectList
    /// .stopAllEffectsExceptThoseThatLastThroughDeath`).
    ///
    /// Java's getter is `_stayAfterDeath || _irreplacableBuff ||
    /// _isNecessaryToggle` — **one getter over three tags** — and all three are
    /// folded into this field at parse (G34 S3). `<irreplacableBuff>` alone is
    /// on 30 learnable skills, so reading only `<stayAfterDeath>` stripped the
    /// clan/pledge and noblesse buffs on every death.
    pub stay_after_death: bool,
    /// Java `Skill.isRemovedOnDamage()` (`<removedOnDamage>`, default false) —
    /// the buff drops the moment its holder takes damage
    /// (`CreatureStatus.reduceHp` → `EffectList.stopEffectsOnDamage`). This is
    /// what makes **sleep** a one-hit crowd control: 36 skills carry the tag on
    /// this dist and most of them are `SLEEP`, the rest `HIDE`,
    /// `FORCE_MEDITATION` and a few transforms. Without it a slept player stays
    /// action-blocked while the mob beats on them.
    pub removed_on_damage: bool,
    pub effects: Vec<SkillEffect>,
    /// Java `SkillOperateType.isSelfContinuous()` — true for `A3` alone.
    /// Read only by [`ActiveBuff::displayed`]; the effects themselves behave
    /// exactly like any other active skill's.
    pub self_continuous: bool,
    /// Java `EffectScope.SELF` (`<selfEffects>`) — applied to the **caster**,
    /// as a separate `applyEffects(caster, caster, …)` after the target loop.
    /// Blinding Blow 321, Sonic Rage 345, Raging Force 346, Vengeance 368,
    /// Evade Shot 369, Critical Blow 409 all put a real self-buff here, and the
    /// parser used to read only `<effects>` — so none of them landed.
    pub self_effects: Vec<SkillEffect>,
    /// Java `EffectScope.PVE` / `PVP` (`<pveEffects>`/`<pvpEffects>`) — applied
    /// to the same target as `effects`, but only for the matching matchup:
    /// `effector.isPlayable() && effected.isAttackable()` → PVE, else
    /// `effector.isPlayable() && effected.isPlayable()` → PVP, else neither.
    pub pve_effects: Vec<SkillEffect>,
    pub pvp_effects: Vec<SkillEffect>,
    /// Java `EffectScope.CHANNELING` (`<channelingEffects>`) — applied by the
    /// `SkillChannelizer` tick to each swept target while a `CA1` cast runs
    /// (Volcano's `MagicalAttack power=500`), never at cast finish.
    pub channeling_effects: Vec<SkillEffect>,
    /// Java `EffectScope.END` (`<endEffects>`) — applied when the buff comes
    /// **off**, as the last thing `EffectList` does on removal. Anchor (1170)
    /// is the learnable carrier: its first stage holds the body rigid, and the
    /// end-effect fires skill 6091 for the paralysis its own description
    /// promises. Without it Anchor did half its job.
    pub end_effects: Vec<SkillEffect>,
    /// Java `mpPerChanneling` — MP drained per channeling tick, **defaulting
    /// to `mpConsume`** (`set.getInt("mpPerChanneling", _mpConsume)`), so a
    /// channeling skill without the tag still drains. Running dry aborts the
    /// cast with SM 140.
    pub mp_per_channeling: i32,
    /// Java `Skill.getChannelingSkillId()` (`<channelingSkillId>`) — the skill a
    /// channeler *applies to its targets* while the cast is held, as opposed to
    /// `channeling_effects` which it applies directly.
    ///
    /// The distinction matters because the applied **level is the number of
    /// distinct channelers** aimed at that target (capped at the channeled
    /// skill's max level), which is the whole point of the mechanic: two
    /// Warcryers holding Battle Stance 426 on the same ally stack it to Battle
    /// Force 5104 level 2. `0` when the skill channels effects instead.
    pub channeling_skill_id: i32,
    /// Java `channelingTickInterval` in ms (XML seconds × 1000; Java defaults
    /// the raw value to 2000 s — dead for non-channeling skills, and every
    /// channeler on this dist declares it).
    pub channeling_tick_ms: i32,
    /// Java `channelingStart` in ms — delay before the first tick.
    pub channeling_start_ms: i32,
    /// Java `<attributeType>`/`<attributeValue>` — the skill's element and its
    /// flat attack contribution (Volcano is FIRE 20). Feeds
    /// `Formulas.calcAttributeBonus`'s attack side; `None` = no element, and
    /// the attacker's strongest POWER stat elects the element instead.
    pub attribute_type: Option<crate::model::stats::Element>,
    pub attribute_value: i32,
    /// The enchant sub-level this instance was built for (0 = unenchanted;
    /// 1001–3020 = an enchant-route step — PLAN_G19_SKILL_ENCHANT.md).
    pub sub_level: i32,
    /// Java `Skill._conditionLists` — the parsed `<conditions>` /
    /// `<targetConditions>` / `<passiveConditions>` blocks
    /// (`SkillConditionScope.GENERAL` / `TARGET` / `PASSIVE`).
    ///
    /// **GENERAL and TARGET are both checked at cast**, in that order, by
    /// `Skill.checkCondition` — the split exists for the datapack's benefit,
    /// not the engine's, and Java evaluates them back to back. PASSIVE is read
    /// by `Player.isSkillActive`-style gating instead: a passive skill whose
    /// conditions fail contributes no stat modifiers.
    ///
    /// A condition name this port doesn't implement is **not** in these lists
    /// — it is recorded by `SkillGaps` instead and the skill behaves as if it
    /// weren't declared, which is what the port did for every condition before
    /// G34 S1. See PLAN_G34_SKILL_PARITY.md §S1.
    /// Java `<basicProperty>` (390 learnable skills declare one). See
    /// [`BasicProperty`] — this is what ties a debuff into the stun-lock
    /// resistance chain.
    pub basic_property: BasicProperty,
    pub conditions: Vec<SkillCondition>,
    pub target_conditions: Vec<SkillCondition>,
    pub passive_conditions: Vec<SkillCondition>,
}

/// Java `BasicProperty` — the "mesmerizing debuff" family a skill belongs to.
///
/// Quoting Java's own enum docs (from Juji): **PHYSICAL** is Stun, Paralyze,
/// Knockback, Knock Down, Hold, Disarm, Petrify; **MAGIC** is Sleep, Mutate,
/// Fear, Aerial Yoke, Silence. Everything else is `NONE`.
///
/// Two independent mechanics read it, and conflating them is how the port
/// missed both (see [`crate::game_loop::basic_property`]):
///
/// 1. `Formulas.getAbnormalResist` — a *stat* lookup
///    (`ABNORMAL_RESIST_PHYSICAL` / `_MAGICAL`), subtracted inside `baseMod`.
/// 2. `Formulas.getBasicPropertyResistBonus` — the **accrual chain**, a
///    multiplier applied *after* the min/max clamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BasicProperty {
    #[default]
    None,
    Physical,
    Magic,
}

impl BasicProperty {
    pub fn from_xml(name: &str) -> Self {
        match name {
            "PHYSICAL" => Self::Physical,
            "MAGIC" => Self::Magic,
            _ => Self::None,
        }
    }
}

/// Java `SkillConditionPercentType` — the comparison a `Remain*Per` condition
/// makes. `MORE` is `current >= amount`, `LESS` is `current <= amount`; both
/// are inclusive, which matters for the skills that gate on exactly 100 %.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercentType {
    More,
    Less,
}

impl PercentType {
    pub fn test(self, current: i32, amount: i32) -> bool {
        match self {
            Self::More => current >= amount,
            Self::Less => current <= amount,
        }
    }

    pub fn from_xml(name: &str) -> Self {
        match name {
            "LESS" => Self::Less,
            _ => Self::More,
        }
    }
}

/// Java `SkillConditionAffectType` — whose state a condition reads. Java's
/// `BOTH` is declared but **no handler branches on it**: every `switch` in the
/// condition handlers covers `CASTER` and `TARGET` and falls through to
/// `return false` otherwise, so a `BOTH` condition refuses the cast outright.
/// Ported as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AffectType {
    Both,
    #[default]
    Caster,
    Target,
}

impl AffectType {
    pub fn from_xml(name: &str) -> Self {
        match name {
            "TARGET" => Self::Target,
            "BOTH" => Self::Both,
            _ => Self::Caster,
        }
    }
}

/// Which vital a `Remain*Per` condition reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vital {
    Hp,
    Mp,
    Cp,
}

/// Java `MountType`, as far as the mount conditions need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountKind {
    Strider,
    Wyvern,
}

/// One parsed `<condition name="…">` — Java's `ISkillCondition` implementations
/// (`handlers/skillconditionhandlers/*`), as a closed enum rather than 121
/// one-method classes.
///
/// Only the conditions with a source on this dist are here. The evaluator lives
/// in `game_loop::skills::conditions`; a variant added here without a match arm
/// there will not compile, which is the point.
#[derive(Debug, Clone, PartialEq)]
pub enum SkillCondition {
    /// `EquipWeapon` — the *equipped* weapon's type must be in the mask.
    /// Java tests `weapon.getItemMask() & mask`, so a skill listing several
    /// types accepts any of them.
    EquipWeapon { mask: u32 },
    /// `EquipShield` — the secondary slot holds an `ArmorType.SHIELD`.
    EquipShield,
    /// `Op1hWeapon` / `Op2hWeapon` — the equipped weapon is of a listed type
    /// **and** its body part is (or is not) `SLOT_LR_HAND`. Java returns on the
    /// first type match rather than continuing, so a weapon of a listed type
    /// held in the wrong number of hands fails rather than falling through.
    HandedWeapon { mask: u32, two_handed: bool },
    /// `OpEncumbered` — *free* inventory slots and weight, each as a
    /// percentage, must both be at least the declared amount. Java's
    /// `calcPercent` is `100 - current*100/max`, i.e. the headroom, and it uses
    /// the **non-quest** inventory size.
    Encumbered {
        weight_percent: i32,
        slots_percent: i32,
    },
    /// `RemainHpPer` / `RemainMpPer` / `RemainCpPer`.
    RemainVital {
        vital: Vital,
        amount: i32,
        percent: PercentType,
        affect: AffectType,
    },
    /// `EnergySaved` — the caster holds at least `amount` force charges.
    EnergySaved { amount: i32 },
    /// `OpEnergyMax` — the *inverse*: refuses once the caster is **at** the
    /// cap, with its own "force has reached maximum capacity" message. This is
    /// what stops a charge skill from being cast at full charges.
    EnergyMax { amount: i32 },
    /// `TargetRace` — `Creature.getRace()`, so it reads the NPC template's race
    /// for a monster and the character race for a player.
    TargetRace { race: crate::enums::Race },
    /// `TargetMyParty` — target is a player in the caster's party. With no
    /// party, only `includeMe` self-targeting passes; with one, `includeMe`
    /// decides whether the caster may pick themselves.
    TargetMyParty { include_me: bool },
    /// `ConsumeBody` — a *spawned dead* monster or summon corpse.
    ConsumeBody,
    /// `OpCanEscape` — `!caster.cannotEscape()` (the `CANNOT_ESCAPE` flag).
    CanEscape,
    /// `OpResurrection` — the target is a dead, un-blocked, not-already-asked
    /// player (or the caster themselves, which always passes).
    Resurrection,
    /// `OpUnlock` — the target is a door or a chest.
    Unlock,
    /// `OpTargetPc` — the target is a player.
    TargetPc,
    /// `OpCallPc` — Summon Friend's caster-side gate.
    CallPc,
    /// `CanTransform` — the transform scroll family's gate. Replaces the
    /// ad-hoc block that used to sit inline in `cast.rs`.
    CanTransform,
    /// `CanSummon` — servitor summoning.
    CanSummon,
    /// `CanSummonCubic`.
    CanSummonCubic,
    /// `CanSummonSiegeGolem`.
    CanSummonSiegeGolem,
    /// `CanUseInBattlefield` **and** `OpSiegeHammer` — two Java classes with
    /// one body: the caster is inside a `SIEGE` zone.
    InsideSiegeZone,
    /// `OpSocialClass` — clan leader always passes; otherwise the pledge type
    /// must be at least `social_class`. `-1` means "leader only".
    SocialClass { social_class: i32 },
    /// `BuildCamp` — the outpost/headquarters gate.
    BuildCamp,
    /// `OpSkillAcquire` — the *target* has (or hasn't) learned a skill.
    SkillAcquire { skill_id: i32, has_learned: bool },
    /// `OpStrider` / `OpWyvern` — the caster is riding that mount.
    Mounted { kind: MountKind },
    /// `NotInUnderwater` — the caster is not in a `WATER` zone.
    NotInUnderwater,
    /// `CheckLevel` — a level band, on caster or target.
    CheckLevel {
        min: i32,
        max: i32,
        affect: AffectType,
    },
    /// `CheckSex`.
    CheckSex { is_female: bool },
    /// `OpExistNpc` — the symbol/totem family's "is one of these already
    /// nearby" gate. Folded in from the inline block that used to sit in
    /// `cast.rs` ahead of target resolution; Java runs it here with the rest.
    ExistNpc(OpExistNpcCondition),
    /// `OpHome` — the caster's clan owns a residence of this type. Backs the
    /// two blessed Scrolls of Escape, which refuse the cast outright rather
    /// than falling through to town the way the unblessed ones do.
    Home { residence: ResidenceType },
    /// `OpTargetDoor` — the target is a **door** whose id is listed. The Four
    /// Sepulchers keys (2235/2236/2237) use it so a key cannot be burned on
    /// the wrong door.
    TargetDoor { door_ids: Vec<i32> },
    /// `OpTargetNpc` — the target is an NPC (or a door) whose id is listed.
    ///
    /// Java re-reads `caster.getTarget()` for a player caster instead of using
    /// the resolved target it was handed — for a `SELF`-targeted skill like
    /// Nectar (2005) those differ, and it is the *selection* that counts.
    TargetNpc { npc_ids: Vec<i32> },
    /// `OpCompanion` — the target is a pet, or a servitor of the caster.
    Companion { kind: CompanionKind },
    /// `OpAlignment` — caster's or target's karma standing. `LAWFUL` is
    /// `reputation >= 0`, `CHAOTIC` is `reputation < 0`.
    Alignment { affect: AffectType, chaotic: bool },
    /// `OpSkill` — the caster **knows** (or does not know) exactly this skill
    /// at exactly this level.
    ///
    /// Distinct from [`SkillCondition::SkillAcquire`] (`OpSkillAcquire`), which
    /// asks the *target*: this one reads the caster's own skill list, and its
    /// negative form is "not at that level" rather than "absent" — so an
    /// Ancient Book stays usable while the player is below the level it grants.
    SkillKnown {
        skill_id: i32,
        skill_level: i32,
        has_learned: bool,
    },
}

/// `enums/ResidenceType` — [`SkillCondition::Home`]'s parameter. `FORTRESS` is
/// listed because the dist declares it (one skill); this chronicle has no
/// fortresses, so it can never pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidenceType {
    Castle,
    ClanHall,
    Fortress,
}

/// `enums/SkillConditionCompanionType` — [`SkillCondition::Companion`]'s kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionKind {
    /// `PET` — `target.isPet()`: a collar pet, not a summoner's servitor.
    Pet,
    /// `MY_SUMMON` — a servitor **belonging to the caster**.
    MySummon,
}

/// `OpExistNpcSkillCondition`'s parsed form — see
/// [`SkillCondition::ExistNpc`]. The cast is allowed only if NPCs from
/// `npc_ids` within `range` of the **caster** exist (`is_around`) / don't
/// exist (`!is_around`); the symbol skills use it to stop a re-cast next to a
/// live seal.
#[derive(Debug, Clone, PartialEq)]
pub struct OpExistNpcCondition {
    pub npc_ids: Vec<i32>,
    pub range: i32,
    pub is_around: bool,
}
impl Default for Skill {
    /// A blank skill: no effects, no costs, single-target, instant.
    ///
    /// Exists so struct literals can use `..Default::default()` and stop
    /// breaking every time a field is added — adding `magic_critical_rate` once
    /// churned 15 test files and was backed out partly for that reason. Only
    /// the non-zero defaults below need thought; the rest are Java's own
    /// zero/absent values.
    fn default() -> Self {
        Self {
            trait_type: TraitType::None,
            static_reuse: false,
            id: 0,
            level: 1,
            name: String::new(),
            // Java's own `getString("icon", …)` default.
            icon: String::from("icon.skill0000"),
            operate_type: OperateType::Active,
            is_continuous: false,
            target_type: TargetType::Self_,
            over_hit: false,
            abnormal_visuals: Vec::new(),
            toggle_group_id: 0,
            affect_scope: AffectScope::Single,
            affect_object: AffectObject::All,
            affect_range: 0,
            affect_limit: (0, 0),
            fan_range: [0; 4],
            magic_type: 0,
            magic_level: 0,
            // Java's "no declared rate", which several gates test for
            // explicitly (a skill with -1 always lands and is never reflected).
            activate_rate: -1,
            lvl_bonus_rate: 0,
            effect_point: 0,
            cast_range: 0,
            effect_range: 0,
            hit_time: 0,
            next_action: NextAction::None,
            abnormal_resists: Vec::new(),
            hit_cancel_time: 0.0,
            cool_time: 0,
            reuse_delay: 0,
            // Java's "no group" sentinel.
            reuse_delay_group: -1,
            mp_consume: 0,
            mp_initial_consume: 0,
            hp_consume: 0,
            without_action: false,
            is_suicide_attack: false,
            item_consume_id: 0,
            item_consume_count: 0,
            abnormal_time: 0,
            abnormal_level: 0,
            abnormal_type: "NONE".to_string(),
            can_be_dispelled: true,
            is_debuff: false,
            shared_with_summon: true,
            stay_after_death: false,
            removed_on_damage: false,
            effects: Vec::new(),
            self_continuous: false,
            self_effects: Vec::new(),
            pve_effects: Vec::new(),
            pvp_effects: Vec::new(),
            channeling_effects: Vec::new(),
            end_effects: Vec::new(),
            mp_per_channeling: 0,
            channeling_skill_id: 0,
            channeling_tick_ms: 0,
            channeling_start_ms: 0,
            basic_property: BasicProperty::default(),
            conditions: Vec::new(),
            target_conditions: Vec::new(),
            passive_conditions: Vec::new(),
            attribute_type: None,
            attribute_value: 0,
            sub_level: 0,
        }
    }
}

/// Which count-cap pool a landed buff occupies (Java `SkillBuffType`, trimmed
/// to the pools the caps use). `Uncapped` folds Java's DEBUFF/TOGGLE/TRIGGER/
/// passive types — none are limited by `MaxBuffAmount`/`MaxDanceAmount`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffSlot {
    /// A good buff — counted against `MaxBuffAmount`.
    Buff,
    /// A dance/song (`isMagic == 3`) — counted against `MaxDanceAmount`.
    Dance,
    /// Debuff / toggle / passive — not slot-limited here.
    Uncapped,
}

/// Java `DispelSlotType` (`<effect name="DispelByCategory"><slot>…`) — which
/// pool [`SkillEffect::DispelByCategory`] steals from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispelSlot {
    Buff,
    Debuff,
    /// Dead in Java too — no shipped skill's `<slot>` is `ALL`.
    All,
}

impl Skill {
    /// Java `Skill.isBad()`: `effectPoint < 0` (aggro/debuff/damage skills).
    pub fn is_bad(&self) -> bool {
        self.effect_point < 0
    }

    /// Java `Skill.getAffectLimit()` — the per-cast cap on how many targets a
    /// scope may sweep up, rolled fresh each cast, or 0 for "no cap".
    ///
    /// The roll is `min + Rnd.get(max)`, **not** `Rnd.get(min..=max)`: Java
    /// passes the *max* as the exclusive bound of a 0-based roll, so the
    /// dist's common `5-12` yields 5..=16, and `10-10` yields 10..=19. That
    /// reads like a datapack authoring assumption more than an intent, but it
    /// is what the live server does, so it is reproduced exactly. `roll` takes
    /// the exclusive bound, matching `World::roll`.
    pub fn affect_limit(&self, roll: impl FnOnce(i32) -> i32) -> i32 {
        let (min, max) = self.affect_limit;
        if min > 0 || max > 0 {
            min + if max > 0 { roll(max) } else { 0 }
        } else {
            0
        }
    }

    /// Java `Skill.isStatic()` — `isMagic == 2`. A static skill's cast time and
    /// reuse are fixed (no attack-speed scaling, no reuse-rate buff).
    pub fn is_static(&self) -> bool {
        self.magic_type == 2
    }

    /// Java `Skill.isDance()` — `isMagic == 3`, the dance/song pool.
    pub fn is_dance(&self) -> bool {
        self.magic_type == 3
    }

    /// Java `Skill.getBuffType()` collapsed to the [`BuffSlot`] pools: a
    /// passive/toggle or a debuff is `Uncapped`, a dance/song (`isMagic == 3`)
    /// is `Dance`, everything else is a `Buff`.
    pub fn buff_slot(&self) -> BuffSlot {
        if matches!(
            self.operate_type,
            OperateType::Passive | OperateType::Toggle
        ) || self.is_bad()
        {
            BuffSlot::Uncapped
        } else if self.magic_type == 3 {
            BuffSlot::Dance
        } else {
            BuffSlot::Buff
        }
    }

    /// The id a reuse is tracked and broadcast under: the shared
    /// `reuseDelayGroup` when one is set, else the skill's own id. Java's
    /// `Skill._reuseHashCode` minus the level/sub-level dimensions —
    /// `Player.reuses` is keyed per skill, not per level.
    pub fn reuse_key(&self) -> i32 {
        if self.reuse_delay_group > 0 {
            self.reuse_delay_group
        } else {
            self.id
        }
    }

    /// The continuous stat-pump subset of `effects` — what lands as an
    /// `ActiveBuff` (instant effects never enter a buff).
    /// Java `Skill.hasEffectType(EffectType.HATE)` — whether any of this
    /// skill's effects is an aggro-management one (`DeleteHate`,
    /// `DeleteHateOfMe`, `DeleteTopAgro`).
    ///
    /// `hasEffectType` scans **every** effect scope (`_effectLists.values()`),
    /// not just `<effects>`, so this does too. The one gate that reads it is
    /// `SkillCaster.callSkill`'s `EVT_ATTACKED` notify: a skill that exists to
    /// *shed* aggro must not wake the mob it was cast at. The hate *addition*
    /// beside it (`addDamageHate(caster, 0, -effectPoint)`) is **not** gated —
    /// only the AI wake is.
    ///
    /// `DeleteTopAgro` has no port variant: its sole carrier is Mischief
    /// (10526), an off-chronicle skill no class learns.
    pub fn has_hate_effect(&self) -> bool {
        [
            &self.effects,
            &self.self_effects,
            &self.pve_effects,
            &self.pvp_effects,
            &self.channeling_effects,
        ]
        .into_iter()
        .flatten()
        .any(|e| {
            matches!(
                e,
                SkillEffect::DeleteHate { .. } | SkillEffect::DeleteHateOfMe { .. }
            )
        })
    }

    /// OR of the [`effect_flag`] bits this skill's effects contribute — Java's
    /// `AbstractEffect.getEffectFlags()` summed over the effect list.
    pub fn effect_flags(&self) -> u32 {
        self.effects.iter().fold(0, |acc, e| {
            acc | match e {
                // Java splits these into BLOCK_ACTIONS vs
                // CONDITIONAL_BLOCK_ACTIONS, but `hasBlockActions()` ORs them,
                // so a single bit is behaviourally identical here.
                SkillEffect::BlockActions { .. } => effect_flag::BLOCK_ACTIONS,
                SkillEffect::Root => effect_flag::ROOTED,
                SkillEffect::Mute => effect_flag::MUTED,
                SkillEffect::PhysicalMute => effect_flag::PHYSICAL_MUTED,
                SkillEffect::DebuffBlock => effect_flag::DEBUFF_BLOCK,
                SkillEffect::BlockControl => effect_flag::BLOCK_CONTROL,
                SkillEffect::Fear { .. } => effect_flag::FEAR,
                SkillEffect::Confuse { .. } => effect_flag::CONFUSED,
                SkillEffect::BlockMove | SkillEffect::ImmobilePetBuff => effect_flag::IMMOBILIZED,
                SkillEffect::Betray => effect_flag::BETRAYED,
                SkillEffect::BlockChat => effect_flag::CHAT_BLOCK,
                SkillEffect::ResurrectionSpecial { .. } => effect_flag::RESURRECTION_SPECIAL,
                SkillEffect::SilentMove => effect_flag::SILENT_MOVE,
                // `ChameleonRest.getEffectFlags()` returns SILENT_MOVE **and**
                // RELAXING. The stealth half is what the skill is for — resting
                // under it hides you from a monster's pre-emptive aggro — and
                // it is the half with a consumer here; `RELAXING` is read in
                // Java only by `Player.standUp`, which this port expresses
                // through `sit_stand::stop_relaxing` instead.
                SkillEffect::ChameleonRest { .. } => effect_flag::SILENT_MOVE,
                SkillEffect::FakeDeath { .. } => effect_flag::FAKE_DEATH,
                SkillEffect::NoblesseBless => effect_flag::NOBLESS_BLESSING,
                // G34 S3 — flag-only effects: the whole mechanic is the bit,
                // so `apply_skill_effects`' empty-effects guard keeps them
                // alive via `has_state_flag` and nothing else is needed.
                SkillEffect::BuffBlock => effect_flag::BUFF_BLOCK,
                SkillEffect::PhysicalShieldAngleAll => effect_flag::PHYSICAL_SHIELD_ANGLE_ALL,
                SkillEffect::Passive => effect_flag::PASSIVE,
                SkillEffect::Untargetable => effect_flag::UNTARGETABLE,
                SkillEffect::DisableTargeting => effect_flag::TARGETING_DISABLED,
                SkillEffect::PhysicalAttackMute => effect_flag::PSYCHICAL_ATTACK_MUTED,
                SkillEffect::BlockResurrection => effect_flag::BLOCK_RESURRECTION,
                SkillEffect::BlockEscape => effect_flag::CANNOT_ESCAPE,
                SkillEffect::AbnormalShield => effect_flag::ABNORMAL_SHIELD,
                SkillEffect::DamageBlock { block_hp, block_mp } => {
                    (if *block_hp { effect_flag::HP_BLOCK } else { 0 })
                        | (if *block_mp { effect_flag::MP_BLOCK } else { 0 })
                }
                _ => 0,
            }
        })
    }

    /// The abnormal types this skill blocks while active — Java
    /// `EffectList.addBlockedAbnormalTypes` on effect start.
    pub fn blocked_abnormals(&self) -> Vec<String> {
        self.effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::BlockAbnormalSlot { slots } => Some(slots.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    pub fn stat_modifier_effects(&self) -> Vec<StatModifierEffect> {
        let one = |stat, amount| StatModifierEffect {
            stat,
            mode: StatModifierType::Diff,
            amount,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
        };
        self.effects
            .iter()
            .flat_map(|e| match e {
                SkillEffect::StatModifier(m) => vec![*m],
                // `VampiricAttack.pump` grants **two** values, which is why this
                // is a `flat_map`: the absorb percentage (Java stores
                // `amount / 100`) and the `amount · chance` term the chance
                // finalizer divides back out.
                // `PolearmSingleTarget.onStart` is `addFixedValue(stat, 1.0)`
                // and `onExit` removes it. Expressed as an ordinary additive 1
                // so it rides the buff lifecycle that already merges and
                // unmerges every other stat grant — nothing else on this dist
                // touches the stat, so `fixed` and `add` are indistinguishable
                // at the one read site (`> 0`).
                SkillEffect::PolearmSingleTarget => {
                    vec![one(Stat::PhysicalPolearmTargetSingle, 1.0)]
                }
                // `ReduceDropPenalty.pump` merges a **mul**, not a diff — the
                // parser has already turned `amount` into `amount/100 + 1`.
                SkillEffect::ReduceDropPenalty { exp_mul, kind } => vec![StatModifierEffect {
                    stat: match kind {
                        ReduceDropKind::Mob => Stat::ReduceExpLostByMob,
                        ReduceDropKind::Pk => Stat::ReduceExpLostByPvp,
                        ReduceDropKind::Raid => Stat::ReduceExpLostByRaid,
                    },
                    mode: StatModifierType::Per,
                    amount: (exp_mul - 1.0) * 100.0,
                    ..Default::default()
                }],
                SkillEffect::VampiricAttack { amount, chance } => vec![
                    one(Stat::AbsorbDamagePercent, amount / 100.0),
                    one(Stat::VampiricSum, amount * chance),
                ],
                // `ReflectSkill.pump` is `mergeAdd(stat, amount)` — an ordinary
                // additive stat contribution that happens to have its own
                // handler class in Java rather than being an
                // `AbstractStatEffect`. Expressed here as the equivalent
                // `StatModifierEffect` so it rides the existing buff/passive
                // pipeline instead of needing its own plumbing.
                // `DamageShield`/`VampiricAttack` are the same shape: Java
                // handlers that only `pump` additive stats.
                SkillEffect::DamageShield { amount } => {
                    vec![one(Stat::ReflectDamagePercent, *amount)]
                }
                SkillEffect::ReflectSkill { magic, amount } => vec![one(
                    if *magic {
                        Stat::ReflectSkillMagic
                    } else {
                        Stat::ReflectSkillPhysic
                    },
                    *amount,
                )],
                _ => Vec::new(),
            })
            .collect()
    }
}

/// `AbnormalType.getClientId()` — the id the client groups a buff icon by, as
/// written into `AbnormalStatusUpdate` and `ExAbnormalStatusUpdateFromTarget`.
///
/// Generated from Java's `AbnormalType` enum: it declares 535 constants,
/// of which **309** carry a real id and the rest are `-1` (`NONE`). Only the
/// 309 are listed; everything else — unknown names included — falls through
/// to `-1`, which is exactly Java's default for an unmapped type.
///
/// This used to map **two** names, so all but two buffs told the client `NONE`.
/// Kept in sync by `abnormal_type_client_ids_match_java`.
pub fn abnormal_type_client_id(name: &str) -> i32 {
    match name {
        "AB_HAWK_EYE" => 0,
        "ALL_ATTACK_DOWN" => 1,
        "ALL_ATTACK_UP" => 2,
        "ALL_SPEED_DOWN" => 3,
        "ALL_SPEED_UP" => 4,
        "ANTARAS_DEBUFF" => 5,
        "ARMOR_EARTH" => 6,
        "ARMOR_FIRE" => 7,
        "ARMOR_HOLY" => 8,
        "ARMOR_UNHOLY" => 9,
        "ARMOR_WATER" => 10,
        "ARMOR_WIND" => 11,
        "ATTACK_SPEED_UP_BOW" => 12,
        "ATTACK_TIME_DOWN" => 13,
        "ATTACK_TIME_UP" => 14,
        "AVOID_DOWN" => 15,
        "AVOID_UP" => 16,
        "AVOID_UP_SPECIAL" => 17,
        "BERSERKER" => 18,
        "BIG_BODY" => 19,
        "BIG_HEAD" => 20,
        "BLEEDING" => 21,
        "BOW_RANGE_UP" => 22,
        "BUFF_QUEEN_OF_CAT" => 23,
        "BUFF_UNICORN_SERAPHIM" => 24,
        "CANCEL_PROB_DOWN" => 25,
        "CASTING_TIME_DOWN" => 26,
        "CASTING_TIME_UP" => 27,
        "CHEAP_MAGIC" => 28,
        "CRITICAL_DMG_DOWN" => 29,
        "CRITICAL_DMG_UP" => 30,
        "CRITICAL_PROB_DOWN" => 31,
        "CRITICAL_PROB_UP" => 32,
        "DANCE_OF_AQUA_GUARD" => 33,
        "DANCE_OF_CONCENTRATION" => 34,
        "DANCE_OF_EARTH_GUARD" => 35,
        "DANCE_OF_FIRE" => 36,
        "DANCE_OF_FURY" => 37,
        "DANCE_OF_INSPIRATION" => 38,
        "DANCE_OF_LIGHT" => 39,
        "DANCE_OF_MYSTIC" => 40,
        "DANCE_OF_PROTECTION" => 41,
        "DANCE_OF_SHADOW" => 42,
        "DANCE_OF_SIREN" => 43,
        "DANCE_OF_VAMPIRE" => 44,
        "DANCE_OF_WARRIOR" => 45,
        "DEBUFF_NIGHTSHADE" => 46,
        "DEBUFF_SHIELD" => 47,
        "DECREASE_WEIGHT_PENALTY" => 48,
        "DERANGEMENT" => 49,
        "DETECT_WEAKNESS" => 50,
        "DMG_SHIELD" => 51,
        "DOT_ATTR" => 52,
        "DOT_MP" => 53,
        "DRAGON_BREATH" => 54,
        "DUELIST_SPIRIT" => 55,
        "FATAL_POISON" => 56,
        "FISHING_MASTERY_DOWN" => 57,
        "FLY_AWAY" => 58,
        "FOCUS_DAGGER" => 59,
        "HEAL_EFFECT_DOWN" => 60,
        "HEAL_EFFECT_UP" => 61,
        "HERO_BUFF" => 62,
        "HERO_DEBUFF" => 63,
        "HIT_DOWN" => 64,
        "HIT_UP" => 65,
        "HOLY_ATTACK" => 66,
        "HP_RECOVER" => 67,
        "HP_REGEN_DOWN" => 68,
        "HP_REGEN_UP" => 69,
        "LIFE_FORCE_ORC" => 70,
        "LIFE_FORCE_OTHERS" => 71,
        "MAGIC_CRITICAL_UP" => 72,
        "MAJESTY" => 73,
        "MAX_BREATH_UP" => 74,
        "MAX_HP_DOWN" => 75,
        "MAX_HP_UP" => 76,
        "MAX_MP_UP" => 77,
        "MA_DOWN" => 78,
        "MA_UP" => 79,
        "MA_UP_HERB" => 80,
        "MD_DOWN" => 81,
        "MD_UP" => 82,
        "MD_UP_ATTR" => 83,
        "MIGHT_MORTAL" => 84,
        "MP_COST_DOWN" => 85,
        "MP_COST_UP" => 86,
        "MP_RECOVER" => 87,
        "MP_REGEN_UP" => 88,
        "MULTI_BUFF" => 89,
        "MULTI_DEBUFF" => 90,
        "PARALYZE" => 91,
        "PA_DOWN" => 92,
        "PA_PD_UP" => 93,
        "PA_UP" => 94,
        "PA_UP_HERB" => 95,
        "PA_UP_SPECIAL" => 96,
        "PD_DOWN" => 97,
        "PD_UP" => 98,
        "PD_UP_BOW" => 99,
        "PD_UP_SPECIAL" => 100,
        "PINCH" => 101,
        "POISON" => 102,
        "POLEARM_ATTACK" => 103,
        "POSSESSION" => 104,
        "PRESERVE_ABNORMAL" => 105,
        "PUBLIC_SLOT" => 106,
        "RAGE_MIGHT" => 107,
        "REDUCE_DROP_PENALTY" => 108,
        "REFLECT_ABNORMAL" => 109,
        "RESIST_BLEEDING" => 110,
        "RESIST_DEBUFF_DISPEL" => 111,
        "RESIST_DERANGEMENT" => 112,
        "RESIST_HOLY_UNHOLY" => 113,
        "RESIST_POISON" => 114,
        "RESIST_SHOCK" => 115,
        "RESIST_SPIRITLESS" => 116,
        "REUSE_DELAY_DOWN" => 117,
        "REUSE_DELAY_UP" => 118,
        "ROOT_PHYSICALLY" => 119,
        "ROOT_MAGICALLY" => 120,
        "SHIELD_DEFENCE_UP" => 121,
        "SHIELD_PROB_UP" => 122,
        "SILENCE" => 123,
        "SILENCE_ALL" => 124,
        "SILENCE_PHYSICAL" => 125,
        "SLEEP" => 126,
        "SNIPE" => 127,
        "SONG_OF_CHAMPION" => 128,
        "SONG_OF_EARTH" => 129,
        "SONG_OF_FLAME_GUARD" => 130,
        "SONG_OF_HUNTER" => 131,
        "SONG_OF_INVOCATION" => 132,
        "SONG_OF_LIFE" => 133,
        "SONG_OF_MEDITATION" => 134,
        "SONG_OF_RENEWAL" => 135,
        "SONG_OF_STORM_GUARD" => 136,
        "SONG_OF_VENGEANCE" => 137,
        "SONG_OF_VITALITY" => 138,
        "SONG_OF_WARDING" => 139,
        "SONG_OF_WATER" => 140,
        "SONG_OF_WIND" => 141,
        "SPA_DISEASE_A" => 142,
        "SPA_DISEASE_B" => 143,
        "SPA_DISEASE_C" => 144,
        "SPA_DISEASE_D" => 145,
        "SPEED_DOWN" => 146,
        "SPEED_UP" => 147,
        "SPEED_UP_SPECIAL" => 148,
        "SSQ_TOWN_BLESSING" => 149,
        "SSQ_TOWN_CURSE" => 150,
        "STEALTH" => 151,
        "STUN" => 152,
        "THRILL_FIGHT" => 153,
        "TOUCH_OF_DEATH" => 154,
        "TOUCH_OF_LIFE" => 155,
        "TURN_FLEE" => 156,
        "TURN_PASSIVE" => 157,
        "TURN_STONE" => 158,
        "ULTIMATE_BUFF" => 159,
        "ULTIMATE_DEBUFF" => 160,
        "VALAKAS_ITEM" => 161,
        "VAMPIRIC_ATTACK" => 162,
        "WATCHER_GAZE" => 163,
        "RESURRECTION_SPECIAL" => 164,
        "COUNTER_SKILL" => 165,
        "AVOID_SKILL" => 166,
        "CP_UP" => 167,
        "CP_DOWN" => 168,
        "CP_REGEN_UP" => 169,
        "CP_REGEN_DOWN" => 170,
        "INVINCIBILITY" => 171,
        "ABNORMAL_INVINCIBILITY" => 172,
        "PHYSICAL_STANCE" => 173,
        "MAGICAL_STANCE" => 174,
        "COMBINATION" => 175,
        "ANESTHESIA" => 176,
        "CRITICAL_POISON" => 177,
        "SEIZURE_PENALTY" => 178,
        "ABNORMAL_ITEM" => 179,
        "SEIZURE_A" => 180,
        "SEIZURE_B" => 181,
        "SEIZURE_C" => 182,
        "FORCE_MEDITATION" => 183,
        "MIRAGE" => 184,
        "POTION_OF_GENESIS" => 185,
        "PVP_DMG_UP" => 186,
        "PVP_DMG_DOWN" => 187,
        "IRON_SHIELD" => 188,
        "TRANSFER_DAMAGE" => 189,
        "SONG_OF_ELEMENTAL" => 190,
        "DANCE_OF_ALIGNMENT" => 191,
        "ARCHER_SPECIAL" => 192,
        "SPOIL_BOMB" => 193,
        "FIRE_DOT" => 194,
        "WATER_DOT" => 195,
        "WIND_DOT" => 196,
        "EARTH_DOT" => 197,
        "HEAL_POWER_UP" => 198,
        "RECHARGE_UP" => 199,
        "NORMAL_ATTACK_BLOCK" => 200,
        "DISARM" => 201,
        "DEATH_MARK" => 202,
        "KAMAEL_SPECIAL" => 203,
        "TRANSFORM" => 204,
        "DARK_SEED" => 205,
        "REAL_TARGET" => 206,
        "FREEZING" => 207,
        "TIME_CHECK" => 208,
        "MA_MD_UP" => 209,
        "DEATH_CLACK" => 210,
        "HOT_GROUND" => 211,
        "EVIL_BLOOD" => 212,
        "ALL_REGEN_UP" => 213,
        "ALL_REGEN_DOWN" => 214,
        "IRON_SHIELD_I" => 215,
        "ARCHER_SPECIAL_I" => 216,
        "T_CRT_RATE_UP" => 217,
        "T_CRT_RATE_DOWN" => 218,
        "T_CRT_DMG_UP" => 219,
        "T_CRT_DMG_DOWN" => 220,
        "INSTINCT" => 221,
        "OBLIVION" => 222,
        "WEAK_CONSTITUTION" => 223,
        "THIN_SKIN" => 224,
        "ENERVATION" => 225,
        "SPITE" => 226,
        "MENTAL_IMPOVERISH" => 227,
        "ATTRIBUTE_POTION" => 228,
        "TALISMAN" => 229,
        "MULTI_DEBUFF_FIRE" => 230,
        "MULTI_DEBUFF_WATER" => 231,
        "MULTI_DEBUFF_WIND" => 232,
        "MULTI_DEBUFF_EARTH" => 233,
        "MULTI_DEBUFF_HOLY" => 234,
        "MULTI_DEBUFF_UNHOLY" => 235,
        "LIFE_FORCE_KAMAEL" => 236,
        "MA_UP_SPECIAL" => 237,
        "PK_PROTECT" => 238,
        "MAXIMUM_ABILITY" => 239,
        "TARGET_LOCK" => 240,
        "PROTECTION" => 241,
        "WILL" => 242,
        "SEED_OF_KNIGHT" => 243,
        "EXPOSE_WEAK_POINT" => 244,
        "FORCE_OF_DESTRUCTION" => 245,
        "ELEMENTAL_ARMOR" => 246,
        "SUMMON_CONDITION" => 247,
        "IMPROVE_PA_PD_UP" => 248,
        "IMPROVE_MA_MD_UP" => 249,
        "IMPROVE_HP_MP_UP" => 250,
        "IMPROVE_CRT_RATE_DMG_UP" => 251,
        "IMPROVE_SHIELD_RATE_DEFENCE_UP" => 252,
        "IMPROVE_SPEED_AVOID_UP" => 253,
        "LIMIT" => 254,
        "MULTI_DEBUFF_SOUL" => 255,
        "CURSE_LIFE_FLOW" => 256,
        "BETRAYAL_MARK" => 257,
        "TRANSFORM_HANGOVER" => 258,
        "TRANSFORM_SCRIFICE" => 259,
        "SONG_OF_WINDSTORM" => 260,
        "DANCE_OF_BLADESTORM" => 261,
        "IMPROVE_VAMPIRIC_HASTE" => 262,
        "WEAPON_MASTERY" => 263,
        "APELLA" => 264,
        "TRANSFORM_SCRIFICE_P" => 265,
        "SUB_TRIGGER_HASTE" => 266,
        "SUB_TRIGGER_DEFENCE" => 267,
        "SUB_TRIGGER_CRT_RATE_UP" => 268,
        "SUB_TRIGGER_SPIRIT" => 269,
        "MIRAGE_TRAP" => 270,
        "DEATH_PENALTY" => 271,
        "ENTRY_FOR_GAME" => 272,
        "BLOOD_CONSTRACT" => 273,
        "DWARF_BUFF" => 274,
        "EVASION_BUFF" => 275,
        "SOUL_SHIELD" => 276,
        "BR_UTHANKA_BUFF" => 277,
        "FIELD_RAID_BUFF1" => 278,
        "PD_UP_DMAGIC" => 279,
        "PREMIUM_BUFF" => 280,
        "RUNWAY_ARMOR" => 281,
        "RUNWAY_WEAPON" => 282,
        "G_EV_BUFF1" => 283,
        "MAX" => 284,
        "AIRBIND" => 365,
        "KNOCKDOWN" => 367,
        "EARTHWORM_DEBUFF" => 424,
        "SYNERGY_SIGEL" => 433,
        "SYNERGY_TIR" => 434,
        "SYNERGY_OTHEL" => 435,
        "SYNERGY_YR" => 436,
        "SYNERGY_FEOH" => 437,
        "SYNERGY_IS" => 438,
        "SYNERGY_WYNN" => 439,
        "SYNERGY_EOLH" => 440,
        "AGATHION_SONG_DANCE" => 444,
        "SYNERGY_PARTY_BUF" => 465,
        "POTION_OF_PROTECTION" => 552,
        "SYNERGY_LENKER" => 589,
        "SYNERGY_SEER" => 590,
        "INSIDE_POSITION" => 593,
        "STEEL_MIND" => 596,
        "SIGEL_SHIELD" => 597,
        "HELLBOUND_BUFF" => 598,
        "MAPHR_AURA" => 599,
        "SAYHA_AURA" => 600,
        "EVAS_DEBUFF" => 601,
        "RIGHT_SIDESTEP" => 602,
        _ => -1,
    }
}

/// A landed buff/debuff on a `Player` (Java `BuffInfo`, trimmed to what G6
/// needs: which stats it's modifying and when it wears off — the "when" is
/// tracked by the `Scheduler`, not stored here).
#[derive(Debug, Clone)]
pub struct ActiveBuff {
    pub skill_id: i32,
    pub skill_level: i32,
    pub abnormal_type_client_id: i32,
    /// Java `Skill.getAbnormalType()` ("NONE" when unset) — the stacking key:
    /// effects of the same abnormal type don't stack (`EffectList.addActive`).
    pub abnormal_type: String,
    /// Java `Skill.getAbnormalLevel()` — decides which of two same-type buffs
    /// wins (the higher level overrides; a lower one is refused).
    pub abnormal_level: i32,
    /// Which slot pool this occupies for the count caps (`MaxBuffAmount` /
    /// `MaxDanceAmount`); debuffs/toggles/passives are `Uncapped`.
    pub slot: BuffSlot,
    /// Absolute tick the buff expires at (for `AbnormalStatusUpdate`'s
    /// remaining-time field).
    pub expires_at_tick: u64,
    /// Java `BuffInfo.isDisplayedForEffected()`:
    /// `!isSelfContinuous() || (effected == effector) || !hasEffects(SELF)`.
    ///
    /// An `A3` skill that also carries `<selfEffects>` hides its row from
    /// anyone who is not the caster — Blinding Blow 321, Vengeance 368, Evade
    /// Shot 369, Critical Blow 409, Aura Flare 1231 and Hurricane Shackle 1996
    /// on this dist. The victim feels the debuff but is never shown an icon
    /// for it. Stamped at creation because the effector is not stored.
    pub displayed: bool,
    /// True for entries that stand in for a passive skill's stat pump (the
    /// grade-penalty skills 6209/6213) rather than a timed buff. They drive
    /// stats through the same modifier maps but are hidden from
    /// `AbnormalStatusUpdate` (Java passive skills never show an abnormal icon)
    /// and never get a `BuffExpire` schedule.
    pub passive: bool,
    /// OR of the [`effect_flag`] bits this buff's skill contributes (0 for the
    /// overwhelming majority). Stamped at creation so the creature's live mask
    /// is a fold over its buff list — see [`effect_flag`].
    pub effect_flags: u32,
    /// Client ids of the visual effects this buff shows while up. Stamped at
    /// creation and folded over the buff list when a packet needs the creature's
    /// current look — same pattern as `effect_flags`.
    pub abnormal_visuals: Vec<i16>,
    /// Abnormal types this buff *blocks* from landing while it is up
    /// (`BlockAbnormalSlot`). Empty for almost every buff; stamped at creation
    /// and folded on read, the same way `effect_flags` is.
    pub blocked_abnormals: Vec<String>,
    pub effects: Vec<StatModifierEffect>,
}

impl ActiveBuff {
    /// A synthetic **passive stat pump**: displayed, but with no abnormal state
    /// of its own, `Uncapped`, and never scheduled to expire.
    ///
    /// The shape the grade-penalty, weight-penalty, clan-skill and
    /// passive-skill folds all want — they are stat contributions wearing a
    /// buff's clothes so that `remove_buff` can take them off again, not
    /// abnormals the client should stack or display an icon for.
    ///
    /// Augment options build a *similar* buff by hand in `game_loop::options`
    /// with `expires_at_tick: 0` and an empty abnormal type; that difference is
    /// untested, so it is deliberately not folded in here.
    pub fn passive_pump(skill_id: i32, skill_level: i32, effects: Vec<StatModifierEffect>) -> Self {
        Self {
            displayed: true,
            skill_id,
            skill_level,
            abnormal_type_client_id: -1,
            abnormal_type: "NONE".to_string(),
            abnormal_level: 0,
            slot: BuffSlot::Uncapped,
            expires_at_tick: u64::MAX,
            passive: true,
            effect_flags: 0,
            blocked_abnormals: Vec::new(),
            abnormal_visuals: Vec::new(),
            effects,
        }
    }
}

// The debuff landing-chance formula is unit-tested in `formulas.rs`
// (`effect_land_rate_clamps_and_special_cases`); the caster-facing chance line
// and the resist roll have end-to-end tests in `game_loop::tests::skills_tests`
// (`single_target_debuff_lands_and_reports_chance` /
// `single_target_debuff_resisted_leaves_target_and_reports`).

#[cfg(test)]
mod abnormal_type_tests {
    use super::abnormal_type_client_id;

    /// The table is generated from Java's `AbnormalType` enum, so it is checked
    /// against that enum rather than against a copy of itself.
    ///
    /// Parsing the reference at test time is the point: a hand-maintained list
    /// drifts silently, and this one had drifted all the way down to **two**
    /// entries while every other buff told the client `NONE`.
    ///
    /// Skipped (not failed) when the Java tree is not beside this repo, so the
    /// suite still runs standalone.
    #[test]
    fn abnormal_type_client_ids_match_java() {
        // Walk up from this crate looking for a sibling `interlude_classic`.
        // A fixed `../../../` breaks inside a git worktree (this repo's own
        // workflow), where it resolves under `.claude/worktrees/` — and the
        // skip-if-absent branch then hides the breakage: the test passed even
        // with a deliberately wrong entry until this was fixed.
        const REL: &str = "java/org/l2jmobius/gameserver/model/skill/AbnormalType.java";
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .map(|dir| dir.join("interlude_classic").join(REL))
            .find_map(|p| std::fs::read_to_string(p).ok());
        let Some(src) = src else {
            eprintln!("skipping: no sibling interlude_classic checkout found");
            return;
        };
        let body = &src[src.find("NONE(-1)").expect("the enum starts at NONE")
            ..src
                .find("private final int _clientId")
                .expect("the field follows")];

        let mut checked = 0usize;
        let mut with_id = 0usize;
        for line in body.lines() {
            let l = line.trim();
            let Some(open) = l.find('(') else { continue };
            let name = &l[..open];
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            {
                continue;
            }
            let Some(close) = l[open..].find(')') else {
                continue;
            };
            let Ok(expected) = l[open + 1..open + close].parse::<i32>() else {
                continue;
            };
            assert_eq!(
                abnormal_type_client_id(name),
                expected,
                "{name} disagrees with Java's AbnormalType"
            );
            checked += 1;
            if expected != -1 {
                with_id += 1;
            }
        }
        assert!(
            checked > 500 && with_id > 300,
            "sanity: parsed {checked} constants, {with_id} with a real id — \
             a parse that silently matched nothing would pass every assertion above"
        );
        assert_eq!(
            abnormal_type_client_id("NOT_A_REAL_ABNORMAL"),
            -1,
            "an unmapped name falls through to NONE, as Java's default does"
        );
    }
}
