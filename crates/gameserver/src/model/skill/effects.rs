//! The `<effect name="…">` list a skill carries — Java's
//! `handlers/effecthandlers/*` as one closed enum, with the payload structs
//! the richer variants need.

use crate::model::stats::{Stat, StatModifierType};

use super::ReduceDropKind;
use super::traits::TraitType;

/// The Rust counterpart of Java's `AbstractStatAddEffect`/
/// `AbstractStatPercentEffect` — one generic type instead of the 63 one-line
/// subclasses Java has (each just names a `Stat` and a fixed mode).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// [`crate::model::components::stats::StatModifiers`] map instead of `add`/`mul`.
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
    /// `AbstractConditionalHpEffect`'s `<hpPercent>` — the contribution counts
    /// only while the *effected* creature's HP is at or below this percentage.
    /// `0` (the absent case) means unconditional.
    ///
    /// ```java
    /// public boolean canPump(Creature effector, Creature effected, Skill skill)
    /// {
    ///     return (_hpPercent <= 0) || (effected.getCurrentHpPercent() <= _hpPercent);
    /// }
    /// ```
    ///
    /// Java re-evaluates it on every stat recompute and registers an
    /// `ON_CREATURE_HP_CHANGE` listener that forces one whenever the predicate
    /// flips. Two learnable skills carry it here — **Final Frenzy (290)**
    /// (+P.Atk below 30 % HP) and **Final Fortress (291)** (+P.Def below 30 %).
    pub hp_percent: i32,
}

impl Default for StatModifierEffect {
    /// An unconditioned flat modifier. Exists so literals can use
    /// `..Default::default()` and stop breaking when a condition axis is added
    /// — the same reason [`super::Skill`] has one.
    fn default() -> Self {
        Self {
            stat: Stat::PhysicalAttack,
            mode: StatModifierType::Diff,
            amount: 0.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
            hp_percent: 0,
        }
    }
}

/// One entry inside a `RestorationRandom` reward group (Java
/// `RestorationItemHolder`). `min_enchant`/`max_enchant` drive the grant-time
/// enchant roll (`game_loop::skills::effects::give_item_random`: when
/// `max_enchant > 0`, the created item gets `Rnd.get(min_enchant, max_enchant)`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
/// `item_data::template::CapsuledItem::chance`.
/// `Escape.java`'s `escapeType`, i.e. the `TeleportWhereType` it hands to
/// `teleToLocation` — see [`SkillEffect::Escape`].
///
/// `FORTRESS` is deliberately absent: the two scrolls carrying it are fortress
/// content, which this chronicle has none of, so the effect drops as an
/// unhandled name rather than pretending to a destination that cannot exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RestorationGroup {
    pub chance: f64,
    pub items: Vec<RestorationItem>,
}

/// Which `PlayerAppearance` field an appearance potion writes
/// ([`SkillEffect::ChangeAppearance`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AppearancePart {
    /// `ChangeFace` — `setFace`.
    Face,
    /// `ChangeHairStyle` — `setHairStyle`.
    HairStyle,
    /// `ChangeHairColor` — `setHairColor`.
    HairColor,
}

/// A skill effect the pipeline knows how to apply. Java registers ~380 effect
/// handler scripts by name; here each supported kind is a variant —
/// `StatModifier` covers the whole `AbstractStatAddEffect`/
/// `AbstractStatPercentEffect` family, the instant kinds get one variant per
/// ported handler. Unregistered effect names are still dropped at load.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SkillEffect {
    /// Continuous stat pump (goes into an `ActiveBuff` for `abnormal_time`).
    StatModifier(StatModifierEffect),
    /// `handlers/effecthandlers/BlockActions.java` — stun / sleep / paralyze:
    /// the target can neither act nor move for the buff's duration. Carries no
    /// stat modifier; the state lives in the [`super::effect_flag`] mask.
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
    /// the buff's life and carries [`super::effect_flag::CHAT_BLOCK`], which is what
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
    /// G34 S3 flag-only effects — each is a single [`super::effect_flag`] bit and
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
    /// (which has no clock), and `game_loop::stats::night_stats` rewrites the landed
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
    /// `ChangeFace` / `ChangeHairStyle` / `ChangeHairColor` — the appearance
    /// potions (Facelifting, Hair Style Change, Dye), instant and
    /// players-only, each setting one `PlayerAppearance` field and
    /// re-broadcasting `UserInfo`. Three Java classes with the same body, so
    /// one variant names which field it writes.
    ChangeAppearance {
        part: AppearancePart,
        value: i32,
    },
    /// `handlers/effecthandlers/SendSystemMessageToClan.java` — an instant
    /// that broadcasts one `SystemMessageId` to the caster's clan. Clan Gate
    /// (3632) is the only carrier on this dist.
    SendSystemMessageToClan {
        message_id: i16,
    },
    /// `handlers/effecthandlers/Recovery.java` — **an empty instant**: the
    /// whole body of Java's `instant()` is commented out, so the effect exists
    /// and does nothing. Registered rather than dropped so the census stops
    /// reporting Scroll: Recovery (2286) as a skill that lost something —
    /// it did not; there was nothing to lose.
    Recovery,
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
    /// [`super::effect_flag::NOBLESS_BLESSING`] bit read by `Playable.doDie`, so it
    /// lands as an icon-only timed `ActiveBuff` (kept off the empty-effects
    /// bail by `has_state_flag`).
    NoblesseBless,
    /// `handlers/effecthandlers/DefenceTrait.java` — raises the target's
    /// resistance to a set of `TraitType`s (Mental Shield's HOLD/SLEEP/
    /// DERANGEMENT, Stun Resistance's SHOCK, …) via `mergeDefenceTrait`. The
    /// per-trait resistances are not a single `Stat`, so they live in their own
    /// [`DefenceTraits`](crate::model::components::stats::DefenceTraits) component,
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
    /// modifier; the whole mechanic is the [`super::effect_flag::HP_BLOCK`]/
    /// [`super::effect_flag::MP_BLOCK`] bits, folded into `Skill::effect_flags()`
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
    /// +200. Static-reuse skills bypass it entirely — see [`super::Skill::static_reuse`].
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

/// Java `DispelSlotType` (`<effect name="DispelByCategory"><slot>…`) — which
/// pool [`SkillEffect::DispelByCategory`] steals from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DispelSlot {
    Buff,
    Debuff,
    /// Dead in Java too — no shipped skill's `<slot>` is `ALL`.
    All,
}
