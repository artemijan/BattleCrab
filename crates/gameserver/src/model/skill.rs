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
    /// `handlers/effecthandlers/HpDrain.java` — magic damage (same
    /// `calcMagicDam` core as `MagicalAttack`) that also heals the caster by
    /// `percentage`% of the HP actually drained (CP absorbs first, clamped to
    /// the target's remaining HP). Backs Vampiric Touch/Claw.
    HpDrain { power: f64, percentage: f64 },
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
}

/// `dist/game/data/stats/skills/*.xml` → `Skill.java`, scoped to G6.
#[derive(Debug, Clone)]
pub struct Skill {
    pub id: i32,
    pub level: i32,
    pub name: String,
    pub operate_type: OperateType,
    pub target_type: TargetType,
    /// Java `isMagic`: 0 physical, 1 magic, 2 static, 3 dance/song, 4 trigger.
    /// Drives cast-time scaling (`calc_skill_time_factor`) and crit rolls.
    pub magic_type: i32,
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
    /// Seconds a landed buff/debuff lasts (Java `abnormalTime`); 0 for
    /// instant/non-buff skills.
    pub abnormal_time: i32,
    pub abnormal_level: i32,
    /// Raw `<abnormalType>` XML text (Java `AbnormalType` has ~500 entries —
    /// only resolved to a client id, via `abnormal_type_client_id`, for the
    /// handful `AbnormalStatusUpdate` actually needs so far).
    pub abnormal_type: String,
    /// Java `affectScope`, narrowed to the one distinction the cast pipeline
    /// needs so far: `true` for `SINGLE` (the default when absent), `false` for
    /// any area scope (`RANGE`/`SQUARE`/`FAN`/…). Used to gate the per-target
    /// debuff-percentage message to single-target debuffs only.
    pub single_target: bool,
    pub effects: Vec<SkillEffect>,
}

impl Skill {
    /// Java `Skill.isBad()`: `effectPoint < 0` (aggro/debuff/damage skills).
    pub fn is_bad(&self) -> bool {
        self.effect_point < 0
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
    pub fn stat_modifier_effects(&self) -> Vec<StatModifierEffect> {
        self.effects
            .iter()
            .filter_map(|e| match e {
                SkillEffect::StatModifier(m) => Some(*m),
                _ => None,
            })
            .collect()
    }

    /// A human-readable summary of a single-target debuff's percentage
    /// modifiers, e.g. `"Speed -20%"` or `"P. Atk. -23%, P. Def. -23%"` — for
    /// the caster-facing `S1_TEXT` feedback line. `None` unless this is a
    /// single-target bad skill carrying at least one `Per`-mode modifier (only
    /// percentage modifiers have a meaningful "%"; flat `Diff` mods are skipped).
    /// The four movement stats a `Speed` effect expands into collapse back to a
    /// single `"Speed"` entry, and identical `(label, amount)` pairs dedupe.
    pub fn debuff_percent_summary(&self) -> Option<String> {
        if !self.is_bad() || !self.single_target {
            return None;
        }
        let mut parts: Vec<String> = Vec::new();
        for m in &self.effects {
            let SkillEffect::StatModifier(m) = m else { continue };
            if m.mode != StatModifierType::Per {
                continue;
            }
            let Some(label) = stat_display_name(m.stat) else { continue };
            // `-20` → "-20%", `15` → "+15%".
            let entry = format!("{label} {:+}%", m.amount as i64);
            if !parts.contains(&entry) {
                parts.push(entry);
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    }
}

/// Short display label for a debuff-relevant stat. The four movement stats map
/// to a single `"Speed"` so a `Speed` effect reads as one line. `None` for
/// stats we don't surface in the debuff message (keeps the line focused on the
/// familiar combat stats rather than dumping every internal stat name).
fn stat_display_name(stat: Stat) -> Option<&'static str> {
    Some(match stat {
        Stat::RunSpeed | Stat::WalkSpeed | Stat::SwimRunSpeed | Stat::SwimWalkSpeed => "Speed",
        Stat::PhysicalAttack => "P. Atk.",
        Stat::PhysicalDefence => "P. Def.",
        Stat::MagicalAttack => "M. Atk.",
        Stat::MagicalDefence => "M. Def.",
        Stat::PhysicalAttackSpeed => "Atk. Spd.",
        Stat::MagicAttackSpeed => "Casting Spd.",
        Stat::CriticalRate => "Critical Rate",
        Stat::MagicCriticalRate => "Magic Crit. Rate",
        Stat::EvasionRate => "Evasion",
        Stat::MagicEvasionRate => "Magic Evasion",
        Stat::AccuracyCombat => "Accuracy",
        _ => return None,
    })
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
    /// Absolute tick the buff expires at (for `AbnormalStatusUpdate`'s
    /// remaining-time field).
    pub expires_at_tick: u64,
    /// True for entries that stand in for a passive skill's stat pump (the
    /// grade-penalty skills 6209/6213) rather than a timed buff. They drive
    /// stats through the same modifier maps but are hidden from
    /// `AbnormalStatusUpdate` (Java passive skills never show an abnormal icon)
    /// and never get a `BuffExpire` schedule.
    pub passive: bool,
    pub effects: Vec<StatModifierEffect>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal skill carrying `effects`, marked bad + single-target unless
    /// overridden — enough to exercise `debuff_percent_summary`.
    fn debuff_skill(name: &str, is_bad: bool, single: bool, effects: Vec<SkillEffect>) -> Skill {
        Skill {
            id: 1,
            level: 1,
            name: name.into(),
            operate_type: OperateType::Active,
            target_type: TargetType::EnemyOnly,
            magic_type: 1,
            effect_point: if is_bad { -100 } else { 100 },
            cast_range: 0,
            effect_range: 0,
            hit_time: 0,
            hit_cancel_time: 0.0,
            cool_time: 0,
            reuse_delay: 0,
            reuse_delay_group: -1,
            mp_consume: 0,
            mp_initial_consume: 0,
            hp_consume: 0,
            abnormal_time: 60,
            abnormal_level: 1,
            abnormal_type: "NONE".into(),
            single_target: single,
            effects,
        }
    }

    fn per(stat: Stat, amount: f64) -> SkillEffect {
        SkillEffect::StatModifier(StatModifierEffect {
            stat,
            mode: StatModifierType::Per,
            amount,
            armor_condition: 0,
            weapon_condition: 0,
        })
    }

    /// The four movement stats a `Speed` effect expands into collapse to one
    /// "Speed -20%" line (deduped).
    #[test]
    fn debuff_summary_collapses_speed() {
        let sk = debuff_skill("Decrease Speed", true, true, vec![
            per(Stat::RunSpeed, -20.0),
            per(Stat::WalkSpeed, -20.0),
            per(Stat::SwimRunSpeed, -20.0),
            per(Stat::SwimWalkSpeed, -20.0),
        ]);
        assert_eq!(sk.debuff_percent_summary().as_deref(), Some("Speed -20%"));
    }

    /// Multiple distinct stats list in order; a positive modifier shows `+`.
    #[test]
    fn debuff_summary_lists_multiple_stats_with_sign() {
        let sk = debuff_skill("Curse", true, true, vec![
            per(Stat::PhysicalAttack, -23.0),
            per(Stat::CriticalRate, 15.0),
        ]);
        assert_eq!(sk.debuff_percent_summary().as_deref(), Some("P. Atk. -23%, Critical Rate +15%"));
    }

    /// Gates: a buff (not bad) and an area (non-single) skill get no % line, and
    /// a flat `Diff` modifier is skipped (only `Per` mods have a meaningful "%").
    #[test]
    fn debuff_summary_gates() {
        let effects = vec![per(Stat::PhysicalAttack, -20.0)];
        assert_eq!(debuff_skill("Buff", false, true, effects.clone()).debuff_percent_summary(), None);
        assert_eq!(debuff_skill("Area", true, false, effects.clone()).debuff_percent_summary(), None);
        let diff = vec![SkillEffect::StatModifier(StatModifierEffect {
            stat: Stat::PhysicalAttack,
            mode: StatModifierType::Diff,
            amount: -20.0,
            armor_condition: 0,
            weapon_condition: 0,
        })];
        assert_eq!(debuff_skill("Flat", true, true, diff).debuff_percent_summary(), None);
    }
}
