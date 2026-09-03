//! Java `ISkillCondition` (`handlers/skillconditionhandlers/*`) — the
//! `<condition name="…">` gates a cast has to pass, as a closed enum.

use super::{AffectType, MountKind, PercentType, Vital};

/// One parsed `<condition name="…">` — Java's `ISkillCondition` implementations
/// (`handlers/skillconditionhandlers/*`), as a closed enum rather than 121
/// one-method classes.
///
/// Only the conditions with a source on this dist are here. The evaluator lives
/// in `game_loop::skills::conditions`; a variant added here without a match arm
/// there will not compile, which is the point.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SkillCondition {
    /// `EquipWeapon` — the *equipped* weapon's type must be in the mask.
    /// Java tests `weapon.getItemMask() & mask`, so a skill listing several
    /// types accepts any of them.
    EquipWeapon { mask: u32 },
    /// `EquipShield` — the secondary slot holds an `ArmorType.SHIELD`.
    EquipShield,
    /// `CanUntransform` — may this caster drop their transform? The only leg
    /// that ever refuses on this dist is the altitude one: a **flying-mounted**
    /// player (a wyvern rider) must be standing over a `LandingZone`, which is
    /// what those 69 zones exist for.
    CanUntransform,
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
    /// `CanSummonPet` — the **pet** summon gate (collars), which is a
    /// different chain from `CanSummon`'s servitor one: already has a pet,
    /// mid-trade or private store, in combat, mounted, observing or
    /// teleporting. Each of the first three answers with its own line.
    CanSummonPet,
    /// `OpMainjob` — the caster must be on their **base** class. The summon
    /// spellbooks and Lyn Draco carry it.
    OpMainjob,
    /// `CannotUseInTransform` — refused while transformed; with a
    /// `transformId`, refused only while wearing *that* transformation.
    CannotUseInTransform { transform_id: i32 },
    /// `OpPledge` — the caster's clan must be at least this level.
    OpPledge { level: i32 },
    /// `OpCheckResidence` — the caster's clan owns (`is_within`) or does not
    /// own one of these clan halls.
    OpCheckResidence {
        residence_ids: Vec<i32>,
        is_within: bool,
    },
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ResidenceType {
    Castle,
    ClanHall,
    Fortress,
}

/// `enums/SkillConditionCompanionType` — [`SkillCondition::Companion`]'s kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OpExistNpcCondition {
    pub npc_ids: Vec<i32>,
    pub range: i32,
    pub is_around: bool,
}
