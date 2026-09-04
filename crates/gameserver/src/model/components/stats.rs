//! The cached stat block a per-tick sweep reads: vitals, speeds, combat
//! stats, and the modifier set they are finalized from.

use bevy_ecs::component::Component;
use std::collections::HashMap;

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
    /// `ShotsBonusFinalizer`'s result **minus one** — see [`Self::shots_bonus`].
    ///
    /// Stored as the increment rather than the finalized value so that
    /// `Default` (0.0) means "no bonus" instead of "shots deal nothing": this
    /// struct is built with `..Default::default()` in dozens of fixtures, and a
    /// derived 0.0 in the multiplier position would silently delete every
    /// soulshot's damage.
    pub shots_bonus_add: f64,
}

impl CombatStats {
    /// Java `CreatureStat.getAttackSpeedMultiplier` (`Formulas.calcAtkSpdMultiplier`):
    /// the client uses this to set the **attack-animation playback rate**, the
    /// haste counterpart of [`Speeds::client_move_multiplier`]. Java's formula
    /// `dexBonus × (weaponBaseAtkSpd / 333) × mul + add / 333` reduces exactly to
    /// `pAtkSpd / 333` (the finalized `p_atk_spd` is `weaponBase × dexBonus × mul
    /// + add`) whenever `mul ≥ 0.7` and there is no move-type term — the case for
    /// every player here. Sending a bare `1.0` (the old value) left the swing
    ///   animation at base cadence while Super Haste quadrupled the actual p_atk_spd.
    pub fn client_atk_speed_multiplier(&self) -> f64 {
        self.p_atk_spd as f64 / 333.0
    }

    /// `Stat.SHOTS_BONUS` as the damage formulas read it — Java's
    /// `ShotsBonusFinalizer` returns `1 + enchantLevel·0.003` for an enchanted
    /// weapon and a flat 1 otherwise, and every `ssmod`/`mAtkMul` in the game
    /// multiplies by it.
    pub fn shots_bonus(&self) -> f64 {
        1.0 + self.shots_bonus_add
    }
}

/// STR/DEX/CON/INT/WIT/MEN (player-only for now — NPC base stats stay on the
/// template until something buffs them). Inputs to the stat finalizers and
/// the regen bonuses.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseStats {
    pub str_: i32,
    pub dex: i32,
    pub con: i32,
    pub int_: i32,
    pub wit: i32,
    pub men: i32,
}

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

/// Java `Creature._basicPropertyResists` — the mesmerizing-debuff resistance
/// chain, one slot per [`crate::model::skill::BasicProperty`] (`PHYSICAL`, `MAGIC`), each holding
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
    pub resist: HashMap<crate::model::skill::traits::TraitType, f64>,
    /// Traits the bearer cannot be affected by at all (Java's XML value ≥ 100).
    pub invulnerable: std::collections::HashSet<crate::model::skill::traits::TraitType>,
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
    pub values: HashMap<crate::model::skill::traits::TraitType, f64>,
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
    pub mp_consume: HashMap<i32, f64>,
    /// magicType → reuse factor (0.80 = 20 % shorter cooldown).
    pub reuse: HashMap<i32, f64>,
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
    pub passive_mp_consume: HashMap<i32, f64>,
    pub passive_reuse: HashMap<i32, f64>,
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
