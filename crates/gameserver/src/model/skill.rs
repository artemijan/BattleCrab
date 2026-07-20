//! Port of `model/skill/Skill.java` — scoped to the fields G6's cast pipeline
//! actually reads (targeting/timing/costs/abnormal info), plus the effect
//! list. Full `Skill.java` has ~40 more fields (traits, elements, fan/affect
//! shapes, …) — added when combat (G9) or AoE/PvP targeting need them.

use crate::model::stats::{Stat, StatModifierType};

/// Java `SkillOperateType`, scoped to what G6 dispatches on. Everything else
/// (`A2 static, `A3`, channeling, …) reads as `Other` and isn't castable yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperateType {
    /// `A1`/`A2`: an active, targeted or self-cast skill with a cast bar.
    Active,
    /// `P`: passive — never sent to `RequestMagicSkillUse`, no cast pipeline.
    Passive,
    /// `T`: toggle — out of scope for G6 (see plan's deferred list).
    Toggle,
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
    /// `NPC_BODY`: a dead NPC corpse (Java `targethandlers/NpcBody.java`) —
    /// used by corpse skills (Sweeper). Unlike the other types this requires
    /// the target to be **dead**, so the cast pipeline's "no dead targets"
    /// gate is inverted for it.
    NpcBody,
    Other,
}

/// Java `AffectScope` (`handlers/targethandlers/affectscope/*`) — how the
/// primary target expands into the set the skill actually lands on.
///
/// Ported: the four scopes that cover the dist's non-single skills —
/// `RANGE` (820 skills), `POINT_BLANK` (785), `PARTY` (272), `PLEDGE` (44).
/// The geometric cone/rectangle scopes (`FAN`/`FAN_PB` 179, `SQUARE`/`SQUARE_PB`
/// 52, `RING_RANGE` 18) and the niche ones (`SUMMON_EXCEPT_MASTER`,
/// `BALAKAS_SCOPE`, `RANGE_SORT_BY_HP`, the `DEAD_*` family, `WYVERN_SCOPE`,
/// `STATIC_OBJECT_SCOPE`, `PARTY_PLEDGE`) read as [`AffectScope::Other`] and
/// fall back to single-target — see the TODO(G19) in `skills::affect`.
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
    /// Any scope not ported yet — treated as [`AffectScope::Single`].
    Other,
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
    /// Unported filters (`INVISIBLE`, `UNDEAD_REAL_ENEMY`, `HIDDEN_PLACE`,
    /// `WYVERN_OBJECT`, `OBJECT_DEAD_NPC_BODY`) — no filtering, like Java's
    /// null-handler path.
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
    /// not modelled — `hasBlockActions()` treats both flags the same, so the
    /// only skills wrongly blocked are the whitelisted ones. TODO(G19).
    BlockActions { conditional: bool },
    /// `handlers/effecthandlers/BlockAbnormalSlot.java` — while this buff is
    /// up, the listed abnormal types cannot land on the target at all. Backs
    /// the Prophecy family's mutual exclusion (Prophecy of Water 1355 blocks
    /// every `BUFF_SPECIAL_*` slot) and Heroic Miracle 395 (`INVINCIBILITY`).
    BlockAbnormalSlot { slots: Vec<String> },
    /// `handlers/effecthandlers/Mute.java` — silence: magic skills refused.
    /// Landing it also aborts the victim's current cast; raid bosses are immune
    /// (`onStart`'s `isRaid()` bail).
    Mute,
    /// `handlers/effecthandlers/PhysicalMute.java` — the physical twin,
    /// refusing non-magic skills.
    PhysicalMute,
    /// `handlers/effecthandlers/DebuffBlock.java` — incoming debuffs fail while
    /// this is up.
    DebuffBlock,
    /// `handlers/effecthandlers/BlockControl.java` — the "out of control"
    /// state; blocks item use in this port.
    BlockControl,
    /// `handlers/effecthandlers/TargetCancel.java` — an instant, chance-rolled
    /// effect that drops the victim's target and aborts their attack and cast
    /// (Trick 11, Switch 12, Aura Flash 1417).
    TargetCancel { chance: i32 },
    /// `handlers/effecthandlers/Root.java` — immobilised. Unlike a stun the
    /// target may still attack and cast.
    Root,
    /// `handlers/effecthandlers/MagicalAttack.java` — instant magic damage.
    MagicalAttack { power: f64 },
    /// `handlers/effecthandlers/PhysicalAttack.java` — instant physical skill
    /// damage (`77·((pAtk·pAtkMod)·levelMod + power) / (pDef·pDefMod)`, crit ×2,
    /// soulshot ×2). Also backs `PhysicalSoulAttack` (identical formula; its
    /// soul mAtk-style boost is ×1 until charges are modeled). The dagger-blow
    /// skills (`FatalBlow`/`Backstab`/`SoulBlow`) use a different `calcBlowDamage`
    /// formula and are NOT routed here.
    /// TODO(G20): ranged (bow) weaponMod 70 branch; shield-block `pDef` add.
    PhysicalAttack { power: f64, p_atk_mod: f64, p_def_mod: f64, critical_chance: f64 },
    /// `handlers/effecthandlers/Heal.java` — instant HP restore.
    Heal { power: f64 },
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
    /// TODO(G19): Java also skips this while `effected.isHpBlocked()`
    /// (`DamageBlock`'s `BLOCK_HP` flag) — not gated, since that effect isn't
    /// ported yet either.
    HealPercent { power: f64 },
    /// Dagger blow skills (`FatalBlow`/`Backstab`/`SoulBlow`) — instant physical
    /// damage via `Formulas.calcBlowDamage`, gated by a `calcBlowSuccess` land
    /// roll (blows can miss). `critical_chance` is `Some` for FatalBlow/Backstab
    /// (rolls `calcCrit` to double the hit) and `None` for SoulBlow (whose
    /// charged-soul boost is ×1 until charges land). `backstab` requires the
    /// caster to be outside the target's front arc.
    /// TODO(G20): SoulBlow charged-soul boost; the accompanying `Lethal`
    /// instant-kill effect is still dropped.
    Blow { power: f64, chance_boost: f64, critical_chance: Option<f64>, backstab: bool },
    /// `handlers/effecthandlers/HpDrain.java` — magic damage (same
    /// `calcMagicDam` core as `MagicalAttack`) that also heals the caster by
    /// `percentage`% of the HP actually drained (CP absorbs first, clamped to
    /// the target's remaining HP). Backs Vampiric Touch/Claw.
    HpDrain { power: f64, percentage: f64 },
    /// `handlers/effecthandlers/DamOverTime.java` — a poison/bleed damage-over-
    /// time debuff. Lands as an `ActiveBuff` for `abnormalTime` and ticks every
    /// `ticks * EFFECT_TICK_RATIO` ms (Java `BuffInfo.scheduleEffects`) for
    /// `power * ticks * EFFECT_TICK_RATIO / 1000` damage per tick
    /// (`AbstractEffect.getTicksMultiplier`), stopping when the buff expires or
    /// the target dies. `can_kill == false` (the XML default) clamps each tick
    /// so it leaves the target at 1 HP. Backs Curse Poison (1168), Poison,
    /// Bleed, etc.
    DamOverTime { power: f64, ticks: i32, can_kill: bool },
    /// `handlers/effecthandlers/Cp.java` — an instant CP change. `percent`
    /// selects Java's `PER` mode (a share of max CP) over `DIFF` (a flat
    /// amount). Braveheart 440 grants `+1000 DIFF`; Wrath 320 and Touch of
    /// Death 342 take CP away.
    Cp { amount: f64, percent: bool },
    /// `handlers/effecthandlers/HealOverTime.java` — periodic HP change on the
    /// same tick chain as [`SkillEffect::DamOverTime`]. **`power` is routinely
    /// negative on this dist** (Fury Fists 222 `-12`, Arcane Wisdom 336 `-50`):
    /// those are toggles that *drain* HP for their upkeep, so this is not a
    /// heal-only effect despite the name.
    HealOverTime { power: f64, ticks: i32 },
    /// `handlers/effecthandlers/ManaDamOverTime.java` — periodic MP drain
    /// (positive `power` = MP removed). Silent Move 221 and friends are toggles
    /// paying MP upkeep; when a tick's drain exceeds current MP the toggle is
    /// switched off (Java returns `false`, which cancels a toggle).
    ManaDamOverTime { power: f64, ticks: i32 },
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
    /// set), identical to `ManaDamOverTime`'s. TODO(G19): Java also has a
    /// level-scaled branch (`((level-1)/7.5) * base * abnormalTime`) for a
    /// skill *with* an `abnormalTime` — unexercised by any skill in this
    /// datapack, so not ported; split out of the shared arm if one ever needs
    /// it.
    MpConsumePerLevel { power: f64, ticks: i32 },
    /// `handlers/effecthandlers/Restoration.java` — instant single-item
    /// grant. Backs item-use skills wrapping a fixed pack/box reward (e.g.
    /// spiritshot packs): the item's `<skills>` entry casts this, which is
    /// where the actual reward comes from.
    GiveItem { item_id: i32, item_count: i64, item_enchant_level: i32 },
    /// `handlers/effecthandlers/RestorationRandom.java` — one weighted
    /// roulette pick among reward groups (each group can grant multiple
    /// items at once). Used by "pick one of N" reward boxes.
    GiveItemRandom { groups: Vec<RestorationGroup> },
    /// `handlers/effecthandlers/Escape.java`, `escapeType=TOWN` only — the
    /// `/unstuck` skills (2099/2100) and scrolls of escape: teleport the
    /// target to its map-region town respawn on landing. The CASTLE/CLANHALL/
    /// FORTRESS variants wait for their residence systems (G24).
    EscapeToTown,
    /// `handlers/effecthandlers/GiveRecommendation.java` — grant the target
    /// `amount` recommendations received (`rec_have`), capped at 255. Backs the
    /// "recommendation certificate" self-target skills.
    GiveRecommendation { amount: i32 },
    /// `handlers/effecthandlers/HeadquarterCreate.java` — the "Build
    /// Headquarters" siege skill (247): the caster (an attacker clan leader)
    /// plants an HQ flag (NPC 35062) in the siege zone as a respawn point.
    /// (`isAdvanced` — the advanced HQ's extra abilities — is collapsed for now,
    /// TODO(G24).)
    CreateHeadquarter,
    /// `handlers/effecthandlers/OpenCommonRecipeBook.java` /
    /// `OpenDwarfRecipeBook.java` — the "Common Craft" (1322) / "Dwarven Craft"
    /// (1321) ability skills: casting one opens the matching recipe window
    /// (`RecipeManager.requestBookOpen`). Refused while the caster runs a
    /// private store. Instant, self-target.
    OpenRecipeBook { dwarven: bool },
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
    /// `handlers/effecthandlers/DispelBySlot.java` — instant cleanse. Stops
    /// every active buff/debuff whose originating skill's `<abnormalType>` is in
    /// the dispel set, provided the listed level is negative (dispel all levels)
    /// or `>=` the buff skill's own `abnormalLevel`. Each `(abnormal_type, level)`
    /// pair comes from the `<dispel>` string (`"POISON,3"`), which is per-skill-
    /// level. Backs Cure Poison (1012), Cure Bleeding, etc. Java's special-cased
    /// `AbnormalType.TRANSFORM` branch is omitted — no transforms in scope yet.
    DispelBySlot { dispel: Vec<(String, i32)> },
    /// `handlers/effecthandlers/DispelBySlotProbability.java` — the Bane family
    /// (Warrior Bane 1350, Mass Warrior Bane 1344, …): cleanse a set of
    /// abnormal types, but roll `rate`% **per buff** rather than stripping all
    /// of them. Unlike [`SkillEffect::DispelBySlot`] the spec carries no
    /// per-type level, so every level of a listed type is a candidate.
    DispelBySlotProbability { dispel: Vec<String>, rate: i32 },
    /// `handlers/effecthandlers/ProtectionBlessing.java` — the Newbie Helper's
    /// Blessing of Protection (5182): a chaotic (PK) character 10+ levels above
    /// the target cannot damage or be damaged by them. Carries no stat
    /// modifier, so it lands as an icon-only timed `ActiveBuff` (like a bare
    /// `DamOverTime`) — the `PK_PROTECT` abnormal + 7200 s duration are honored,
    /// but the actual damage-immunity check is deferred to the PvP milestone.
    /// TODO(G-pvp): gate PK damage on this buff in the combat/flag path.
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
    /// per-trait resistances aren't a single `Stat` and the trait-defense math
    /// isn't modeled yet, so this carries no stat modifier and lands as an
    /// icon-only timed `ActiveBuff` (like `ProtectionBlessing`) — the abnormal +
    /// duration are honored so the buff shows and expires correctly.
    /// TODO(G16): apply the trait-defense resistances in the debuff land math.
    DefenceTrait,
    /// `handlers/effecthandlers/VampiricAttack.java` — Vampiric Rage: a chance to
    /// recover a % of melee damage dealt as HP (`ABSORB_DAMAGE_PERCENT` +
    /// `vampiricSum`). The melee HP-absorb path isn't modeled yet, so like
    /// `ProtectionBlessing` this carries no stat modifier and lands as an
    /// icon-only timed `ActiveBuff` (abnormal + duration honored).
    /// TODO(G20): honor the actual HP absorb-on-hit in the melee combat path.
    VampiricAttack,
    /// `handlers/effecthandlers/AttackAttribute.java` — adds `amount` to the
    /// target's `<attribute>_POWER` attack-element stat (`mergeAdd`). Backs the
    /// elemental dance/song buffs (Dance of Light 277 → HOLY, …). Attribute-based
    /// attack math isn't modeled yet, so like `VampiricAttack` this carries no
    /// stat modifier and lands as an icon-only timed `ActiveBuff` (abnormal +
    /// duration honored) so the buff shows and expires.
    /// TODO(G16): apply the attack-element power in the elemental damage math.
    AttackAttribute,
    /// `handlers/effecthandlers/MagicMpCost.java` — multiplies the target's
    /// MP-consume rate for a given `magicType` (`mergeMpConsumeTypeValue`, factor
    /// `amount/100 + 1`). Backs the MP-cost-reduction songs (Song of Champion
    /// 8547, Song of Renewal 349). The cast MP-consume path reads `skill.mp_consume`
    /// raw (no per-type stat multiplier), so this carries no stat modifier and
    /// lands as an icon-only timed `ActiveBuff` (abnormal + duration honored).
    /// TODO(G16): route MP consume through a per-magic-type consume-rate stat.
    MagicMpCost,
    /// `handlers/effecthandlers/Reuse.java` — multiplies the target's skill-reuse
    /// rate for a given `magicType` (`mergeReuseTypeValue`, factor `amount/100 + 1`).
    /// Backs the reuse-reduction buffs (Song of Champion 8547, Song of Renewal 349,
    /// Gift of Seraphim 4703). The reuse path uses `skill.reuse_delay` raw (no
    /// per-type stat multiplier), so this carries no stat modifier and lands as an
    /// icon-only timed `ActiveBuff` (abnormal + duration honored).
    /// TODO(G16): route reuse through a per-magic-type reuse-rate stat.
    Reuse,
    /// `handlers/effecthandlers/DamageShield.java` — `Stat.REFLECT_DAMAGE_PERCENT`:
    /// reflects `amount`% of received damage back at the attacker. Backs Song of
    /// Vengeance (305). The combat damage-reflect path isn't modeled yet, so this
    /// carries no stat modifier and lands as an icon-only timed `ActiveBuff`
    /// (abnormal + duration honored).
    /// TODO(G20): reflect `amount`% of received damage in the combat path.
    DamageShield,
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
    /// transform); TODO(G19): the sitting and registered-on-event legs have
    /// no modeled state on this port yet.
    Transform { transformation_id: i32 },
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
    /// two identically, and the whitelist itself is a TODO(G19), so both map
    /// here.
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
    /// itself (`Playable.doDie`). Java's sibling `RESURRECTION_SPECIAL` has the
    /// same "keep your buffs" role there, but its self-resurrect mechanic isn't
    /// ported, so the flag has no source yet.
    /// TODO(G22): add RESURRECTION_SPECIAL alongside the self-res effect.
    pub const NOBLESS_BLESSING: u32 = 1 << 6;
}

/// Java `AbnormalVisualEffect` — the client-side *look* of an abnormal (the
/// stun swirl, the poison tint, the silence mark). Purely cosmetic: the
/// mechanics live in [`effect_flag`] and the effect handlers, while this is
/// what the client renders over the character.
///
/// Only the ids the dist's skills actually reference are mapped; an unknown
/// name yields `None` and is simply not shown (Java would fail the enum lookup
/// and log). `VP_KEEP` shares client id 29 with `VP_UP` in Java, comment and
/// all.
pub fn abnormal_visual_client_id(name: &str) -> Option<i16> {
    Some(match name {
        "DOT_BLEEDING" => 1,
        "DOT_POISON" => 2,
        "DOT_FIRE" => 3,
        "DOT_WATER" => 4,
        "DOT_WIND" => 5,
        "DOT_SOIL" => 6,
        "STUN" => 7,
        "SLEEP" => 8,
        "SILENCE" => 9,
        "ROOT" => 10,
        "PARALYZE" => 11,
        "FLESH_STONE" => 12,
        "DOT_MP" => 13,
        "BIG_HEAD" => 14,
        "DOT_FIRE_AREA" => 15,
        "CHANGE_TEXTURE" => 16,
        "BIG_BODY" => 17,
        "FLOATING_ROOT" => 18,
        "DANCE_ROOT" => 19,
        "GHOST_STUN" => 20,
        "STEALTH" => 21,
        "SEIZURE1" => 22,
        "SEIZURE2" => 23,
        "MAGIC_SQUARE" => 24,
        "FREEZING" => 25,
        "SHAKE" => 26,
        "ULTIMATE_DEFENCE" => 28,
        "VP_UP" | "VP_KEEP" => 29,
        "REAL_TARGET" => 30,
        "DEATH_MARK" => 31,
        "TURN_FLEE" => 32,
        "INVINCIBILITY" => 33,
        "AIR_BATTLE_SLOW" => 34,
        "AIR_BATTLE_ROOT" => 35,
        "CHANGE_WP" => 36,
        "CHANGE_HAIR_G" => 37,
        "CHANGE_HAIR_P" => 38,
        _ => return None,
    })
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
    /// Java `affectObject` — the friend/foe filter each swept-up candidate must
    /// pass. Defaults to `ALL` (Java's "no handler" = no filtering).
    pub affect_object: AffectObject,
    /// Java `affectRange` — the radius the scope sweeps (0 = no sweep).
    pub affect_range: i32,
    /// Java `_affectLimit` `[min, max]` from `<affectLimit>min-max</affectLimit>`.
    /// Read through [`Skill::affect_limit`], which reproduces Java's roll.
    pub affect_limit: (i32, i32),
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
    /// Java `Skill.isStayAfterDeath()` (`<stayAfterDeath>`, default false) — the
    /// buff survives its holder's death (`EffectList
    /// .stopAllEffectsExceptThoseThatLastThroughDeath`). Java ORs
    /// `irreplacableBuff` and `isNecessaryToggle` into the same getter; neither
    /// tag is parsed here yet, so this is the plain `<stayAfterDeath>` value.
    /// TODO: fold in `<irreplacableBuff>`/`<isNecessaryToggle>` when parsed.
    pub stay_after_death: bool,
    pub effects: Vec<SkillEffect>,
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

    /// Java `Skill.getBuffType()` collapsed to the [`BuffSlot`] pools: a
    /// passive/toggle or a debuff is `Uncapped`, a dance/song (`isMagic == 3`)
    /// is `Dance`, everything else is a `Buff`.
    pub fn buff_slot(&self) -> BuffSlot {
        if matches!(self.operate_type, OperateType::Passive | OperateType::Toggle) || self.is_bad() {
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
                SkillEffect::NoblesseBless => effect_flag::NOBLESS_BLESSING,
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
        self.effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::StatModifier(m) => Some(*m),
                _ => None,
            })
            .collect()
    }
}

/// `AbnormalType.getClientId()`, scoped to the types skills registered in
/// `EFFECT_REGISTRY` actually use. Unknown/unregistered types map to `NONE`
/// (`-1`), same as Java's default. TODO: grow alongside `EFFECT_REGISTRY`.
pub fn abnormal_type_client_id(name: &str) -> i32 {
    match name {
        "PA_UP" => 94,
        "PD_UP" => 98,
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

// The debuff landing-chance formula is unit-tested in `formulas.rs`
// (`effect_land_rate_clamps_and_special_cases`); the caster-facing chance line
// and the resist roll have end-to-end tests in `game_loop::tests::skills_tests`
// (`single_target_debuff_lands_and_reports_chance` /
// `single_target_debuff_resisted_leaves_target_and_reports`).
