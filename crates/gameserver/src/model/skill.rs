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
    /// `SUMMON`: the caster's own summon (Java `targethandlers/Summon.java`).
    ///
    /// **Servitors only.** Java is
    /// `if (isPlayer() && hasSummon()) return getAnyServitor(); return getPet();`
    /// — and `getAnyServitor()` is null when the player has only a *pet*, so a
    /// pet owner casting "Servitor Heal" targets nothing. That reads like a bug
    /// but is thematically right: these are the Summoner's servitor kit, and a
    /// Wolf is not a servitor. Ported as written.
    Summon,
    Other,
}

/// Java `AffectScope` (`handlers/targethandlers/affectscope/*`) — how the
/// primary target expands into the set the skill actually lands on.
///
/// Ported: the four radius/group scopes that cover the dist's non-single
/// skills — `RANGE` (820 skills), `POINT_BLANK` (785), `PARTY` (272), `PLEDGE`
/// (44) — and the geometric family (plan: PLAN_G19_GEOMETRIC_SCOPES.md) —
/// `FAN`/`FAN_PB` (163+16, 5 learnable), `SQUARE`/`SQUARE_PB` (35+17),
/// `RING_RANGE` (18). Still unported (`TODO(G19)`), reading as
/// [`AffectScope::Other`] and falling back to single-target:
/// `SUMMON_EXCEPT_MASTER` (22, needs G29), `BALAKAS_SCOPE`/`WYVERN_SCOPE`
/// (boss/wyvern scripting), `RANGE_SORT_BY_HP` (4), the `DEAD_*` family
/// (mass-res fan-out), `PARTY_PLEDGE` (5), `STATIC_OBJECT_SCOPE`.
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
    ManaHeal { power: f64 },
    ManaHealByLevel { power: f64 },
    ManaHealPercent { power: f64 },

    /// `Feed` — restores a pet's food bar (Java `effecthandlers/Feed`). The
    /// `ride`/`wyvern` params feed a *mounted player's* bar instead; mounts are
    /// not ported, so only `normal` is carried.
    /// TODO(G29): apply `ride`/`wyvern` when mounts land.
    Feed { normal: i32 },

    /// `SummonCubic` — attaches a cubic to the caster (see `game_loop/cubic`).
    SummonCubic { cubic_id: i32, cubic_level: i32 },
    MpRestore { amount: f64, percent: bool },
    /// `handlers/effecthandlers/MagicalAttackMp.java` — an MP drain (Mana Burn
    /// 1398, Mana Storm 1399, Aura Sink 1102, Seal of Gloom 1210). Damage is
    /// dealt to the target's **MP pool**, not HP, by its own `calcManaDam`
    /// formula. Mana Burn and Mana Storm carry only this effect, so both were
    /// dropped whole before it was ported.
    MagicalAttackMp { power: f64, critical: bool, critical_limit: f64 },
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
    /// `handlers/effecthandlers/Resurrection.java` — Resurrection 1016, Mass
    /// Resurrection 1254. Does not revive directly: it *proposes* a revive, and
    /// the dead player accepts through a `ConfirmDlg`. `power` is the percentage
    /// of XP lost on death that the revive restores (run through
    /// `calculateSkillResurrectRestorePercent` first); the three percentages are
    /// how much HP/MP/CP they come back with.
    Resurrection { power: i32, hp_percent: i32, mp_percent: i32, cp_percent: i32 },
    /// `handlers/effecthandlers/Summon.java` — summon a **servitor** (24
    /// learnable skills: Summon Dark Panther 283, Summon Kat the Cat 1111,
    /// Summon Shadow 1128, the golems, …).
    ///
    /// `npc_id` is per skill *level*, so each level summons a stronger
    /// template. `life_time` is in seconds and `<= 0` means "no expiry" (Java
    /// maps it to `Integer.MAX_VALUE` with the note "Classic hack. Resummon
    /// upon entering game.").
    Summon { npc_id: i32, life_time: i32, consume_item_id: i32, consume_item_count: i64 },
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
    ReflectSkill { magic: bool, amount: f64 },
    /// `handlers/effecthandlers/Confuse.java` — the victim turns on a random
    /// bystander (Madness 1105, Curse Discord 1163, Seal of Mirage 1213).
    /// Chance-gated by `calcProbability`. Madness and Curse Discord carry only
    /// this effect, so both were dropped whole before it was ported.
    Confuse { chance: i32 },
    /// `handlers/effecthandlers/RandomizeHate.java` — move the caster's hate
    /// onto a random bystander (Confusion 2, Switch 12). Confusion carries only
    /// this effect. Same chance gate as [`Self::Confuse`].
    RandomizeHate { chance: i32 },
    /// `handlers/effecthandlers/SilentMove.java` — stealth (Silent Move 221,
    /// Stealth 411, Dance of Shadows 366, Fake Death 60). A pure state flag:
    /// the Java handler has an empty constructor and nothing but
    /// `getEffectFlags`, and the whole mechanic is the aggro-scan gate.
    SilentMove,
    /// `handlers/effecthandlers/FakeDeath.java` — feign death (Fake Death 60).
    /// A state flag *plus* an MP upkeep on the same 5-tick cadence as
    /// `ManaDamOverTime`, which it shares the tick chain with.
    FakeDeath { power: f64, ticks: i32 },
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
    Fear { ticks: i32 },
    /// `handlers/effecthandlers/GetAgro.java` — forces the effected NPC to
    /// intend-attack the caster (Aggression 28, Aggression Aura 18, plus the
    /// aggro side-effect on the debuffs Judgment 401/Tribunal 400). No params.
    /// Java also pre-seeds nearby clan-mates with `addDamageHate(effector, 1,
    /// 200)`; this port leaves that to the already-ported
    /// `npc_ai::faction_call`, which pulls clan-mates in on its own once the
    /// taunted NPC is actually landing hits on the caster (at most one
    /// think-tick later than Java's immediate pre-seed) — `TODO(G21+)` if
    /// that one-tick gap ever turns out to matter.
    GetAgro,
    /// `handlers/effecthandlers/AddHate.java` — a flat hate change with no
    /// damage. Positive `power` (Charm 15, Lure 51) adds hate for the caster;
    /// negative reduces it (unused on this dist, but Java supports it).
    AddHate { power: f64 },
    /// `handlers/effecthandlers/DeleteHate.java` — chance-rolled: wipes the
    /// target's *entire* aggro list and disengages its AI (Java
    /// `setWalking()` + `setIntention(ACTIVE)`). Eva's Serenade 1273, Peace
    /// 1075, Repose 1034.
    DeleteHate { chance: i32 },
    /// `handlers/effecthandlers/DeleteHateOfMe.java` — chance-rolled: zeroes
    /// just the caster's own aggro entry (`stopHating`), but — matching Java
    /// exactly — still disengages the target's AI wholesale (`setWalking()` +
    /// `setIntention(ACTIVE)`) even if other attackers remain in the list;
    /// the AI naturally re-picks the next-most-hated target on its following
    /// think tick if any hate remains. Bluff 358, Forget 1156, Trick 11.
    DeleteHateOfMe { chance: i32 },
    /// `handlers/effecthandlers/Root.java` — immobilised. Unlike a stun the
    /// target may still attack and cast.
    Root,
    /// `handlers/effecthandlers/MagicalAttack.java` — instant magic damage.
    MagicalAttack { power: f64 },
    /// `handlers/effecthandlers/SummonNpc.java`, narrowed to the `EffectPoint`
    /// branch (PLAN_G19_SYMBOLS.md): drop a totem NPC at the aimed ground
    /// point that pulses its template's `union_skill` every `skill_delay`
    /// seconds until `despawn_time`. The `Decoy` and default-spawn branches
    /// are TODO(G19) (no learnable carriers); `despawn_delay` is the effect's
    /// fallback when the template declares no `despawn_time`.
    SummonNpc { npc_id: i32, npc_count: i32, despawn_delay: i32 },
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
    /// Also skips a *positive*-power (heal) landing while
    /// `effected.isHpBlocked()` — the negative-power (damage) branch already
    /// gets this for free through the shared `apply_skill_damage` path.
    HealPercent { power: f64 },
    /// `handlers/effecthandlers/FocusMomentum.java` — the "Force" gain half of
    /// the Sonic/Force skill family (Sonic Focus 8, Focus Force 50, Sonic Rage
    /// 345, Raging Force 346, …): `+amount` charges (default 1), capped at
    /// `max_charges.min(8)` — Java's `MAX_MOMENTUM` stat is never set anywhere
    /// in this datapack, so `8` (the hardcoded fallback `FocusMomentum.java`
    /// itself passes to `getValue`) is the real cap on this build, not a
    /// simplification. Already at the cap: refused with SM 324 (no gain).
    /// Otherwise SM 323 (`"Your force has increased to level $s1"`) +
    /// `EtcStatusUpdate` (the charge-count icon).
    FocusMomentum { amount: i32, max_charges: i32 },
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
    /// TODO(G20): shield-block `pDef` add / perfect-block-to-1-damage, same
    /// gap `PhysicalAttack` already has — not modeled for either.
    EnergyAttack { power: f64, critical_chance: f64, p_def_mod: f64, charge_consume: i32 },
    /// Dagger blow skills (`FatalBlow`/`Backstab`/`SoulBlow`) — instant physical
    /// damage via `Formulas.calcBlowDamage`, gated by a `calcBlowSuccess` land
    /// roll (blows can miss). `critical_chance` is `Some` for FatalBlow/Backstab
    /// (rolls `calcCrit` to double the hit) and `None` for SoulBlow (whose
    /// charged-soul boost is ×1 until charges land). `backstab` requires the
    /// caster to be outside the target's front arc.
    /// TODO(G20): SoulBlow charged-soul boost.
    Blow { power: f64, chance_boost: f64, critical_chance: Option<f64>, backstab: bool },
    /// `handlers/effecthandlers/Lethal.java` — the instant-kill/half-kill
    /// secondary effect riding alongside `Backstab`/`FatalBlow`/
    /// `PhysicalAttack` on Backstab (30), Lethal Blow (344), Deadly Blow
    /// (263), Critical Blow (409), Lethal Shot (343), … — previously dropped
    /// (the doc-comment TODO on [`SkillEffect::Blow`] above named it), so
    /// those skills' damage landed but the bonus kill chance never rolled.
    /// `full_lethal`/`half_lethal` are already 0-100 percentages (unlike
    /// Java's `AttackTrait` effect, this constructor doesn't `/100` these).
    /// A landed full-lethal sets HP (and CP, for a player) to 1; a half-kill
    /// sets a player's CP to 1 or halves a monster's HP. Java's
    /// `chanceMultiplier` (attribute/general-trait bonus) is 1.0 here — no
    /// trait/attribute math is modeled anywhere on this port. Raid bosses are
    /// immune (`isLethalable()`, mirroring the same raid-immunity check
    /// `Mute`'s cast-interrupt already has); `INSTANT_KILL_RESIST` isn't
    /// rolled at all — like `MAX_MOMENTUM`, no skill/item/npc in this
    /// datapack ever sets it, so Java's own roll against it is unconditionally
    /// lost and would never change the outcome.
    /// TODO(G19): `isHpBlocked()` (this port's `DamageBlock` gap, same as
    /// `HealPercent`'s); `calcCounterAttack`'s reflect-on-lethal (no counter
    /// mechanic modeled yet); grand-boss/door lethal-immunity (only the raid
    /// case is covered).
    Lethal { full_lethal: f64, half_lethal: f64 },
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
    DispelByCategory { slot: DispelSlot, rate: i32, max: i32 },
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
    /// `handlers/effecthandlers/AttackTrait.java` — the "Detect &lt;Category&gt;
    /// Weakness" family (Insect/Beast/Animal/Dragon/Plant 75/80/87/88/104, Eye
    /// of Hunter/Slayer 359/360): raises the caster's bonus damage against a
    /// set of creature-category `*_WEAKNESS` traits via `mergeAttackTrait`,
    /// the attacker-side counterpart of [`SkillEffect::DefenceTrait`]'s
    /// `mergeDefenceTrait`.
    ///
    /// Unlike every other icon-only effect on this port, this one turns out
    /// to be **functionally inert in the real Java server too, not just
    /// unported here**: `Formulas.calcWeaknessBonus` only applies a
    /// `*_WEAKNESS` bonus when the *target* also carries a matching
    /// `DefenceTrait` (`target.getStat().hasDefenceTrait(trait)`), and
    /// nothing in this datapack — no NPC template, no skill, no Java
    /// call site outside `CreatureStat`'s own definition — ever calls
    /// `mergeDefenceTrait` for any monster. So even on the reference server,
    /// landing "Detect Beast Weakness" changes nothing observable; a
    /// completely faithful port is exactly as inert. Carries no stat
    /// modifier and no state of its own (unlike `DefenceTrait`/
    /// `VampiricAttack`, there's nothing worth storing if nothing would ever
    /// read it), so it lands as an icon-only timed `ActiveBuff` like its
    /// siblings.
    /// TODO: if NPC-side `DefenceTrait`/creature-category resistance data
    /// ever lands, this needs an actual per-creature attack-trait
    /// accumulator (Java: `mergeAttackTrait`/`removeAttackTrait`, additive
    /// per `TraitType`) and a real multiplier in `calcWeaknessBonus`'s
    /// callers — until then there is nothing to wire it to.
    AttackTrait,
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
    DamageBlock { block_hp: bool, block_mp: bool },
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
    /// Java `mpPerChanneling` — MP drained per channeling tick, **defaulting
    /// to `mpConsume`** (`set.getInt("mpPerChanneling", _mpConsume)`), so a
    /// channeling skill without the tag still drains. Running dry aborts the
    /// cast with SM 140.
    pub mp_per_channeling: i32,
    /// Java `channelingTickInterval` in ms (XML seconds × 1000; Java defaults
    /// the raw value to 2000 s — dead for non-channeling skills, and every
    /// channeler on this dist declares it).
    pub channeling_tick_ms: i32,
    /// Java `channelingStart` in ms — delay before the first tick.
    pub channeling_start_ms: i32,
    /// The `OpExistNpc` skill condition (`skillconditionhandlers/
    /// OpExistNpcSkillCondition.java`) — the first entry of a condition
    /// layer: the cast is allowed only if NPCs from `npc_ids` within `range`
    /// of the **caster** exist (`is_around`) / don't exist (`!is_around`).
    /// The symbol skills use it to stop you re-casting next to a live seal.
    pub op_exist_npc: Option<OpExistNpcCondition>,
    /// Java `<attributeType>`/`<attributeValue>` — the skill's element and its
    /// flat attack contribution (Volcano is FIRE 20). Feeds
    /// `Formulas.calcAttributeBonus`'s attack side; `None` = no element, and
    /// the attacker's strongest POWER stat elects the element instead.
    pub attribute_type: Option<crate::model::stats::Element>,
    pub attribute_value: i32,
}

/// See [`Skill::op_exist_npc`].
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
            id: 0,
            level: 1,
            name: String::new(),
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
            hit_cancel_time: 0.0,
            cool_time: 0,
            reuse_delay: 0,
            // Java's "no group" sentinel.
            reuse_delay_group: -1,
            mp_consume: 0,
            mp_initial_consume: 0,
            hp_consume: 0,
            without_action: false,
            item_consume_id: 0,
            item_consume_count: 0,
            abnormal_time: 0,
            abnormal_level: 0,
            abnormal_type: "NONE".to_string(),
            can_be_dispelled: true,
            is_debuff: false,
            stay_after_death: false,
            effects: Vec::new(),
            self_effects: Vec::new(),
            pve_effects: Vec::new(),
            pvp_effects: Vec::new(),
            channeling_effects: Vec::new(),
            mp_per_channeling: 0,
            channeling_tick_ms: 0,
            channeling_start_ms: 0,
            op_exist_npc: None,
            attribute_type: None,
            attribute_value: 0,
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
                SkillEffect::Fear { .. } => effect_flag::FEAR,
                SkillEffect::Confuse { .. } => effect_flag::CONFUSED,
                SkillEffect::BlockMove => effect_flag::IMMOBILIZED,
                SkillEffect::SilentMove => effect_flag::SILENT_MOVE,
                SkillEffect::FakeDeath { .. } => effect_flag::FAKE_DEATH,
                SkillEffect::NoblesseBless => effect_flag::NOBLESS_BLESSING,
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
        self.effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::StatModifier(m) => Some(*m),
                // `ReflectSkill.pump` is `mergeAdd(stat, amount)` — an ordinary
                // additive stat contribution that happens to have its own
                // handler class in Java rather than being an
                // `AbstractStatEffect`. Expressed here as the equivalent
                // `StatModifierEffect` so it rides the existing buff/passive
                // pipeline instead of needing its own plumbing.
                SkillEffect::ReflectSkill { magic, amount } => Some(StatModifierEffect {
                    stat: if *magic { Stat::ReflectSkillMagic } else { Stat::ReflectSkillPhysic },
                    mode: StatModifierType::Diff,
                    amount: *amount,
                    armor_condition: 0,
                    weapon_condition: 0,
                    qualifier: None,
                    two_handed: false,
                }),
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
