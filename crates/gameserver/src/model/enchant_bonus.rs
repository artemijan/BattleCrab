//! `IStatFunction.calcEnchantedItemBonus` and its three tables — the extra
//! P.Atk / M.Atk / P.Def / M.Def an **enchanted** piece of gear contributes —
//! `calcEnchantBodyPart`'s R-grade armour bonuses, and `ShotsBonusFinalizer`'s
//! enchant-scaled soulshot/spiritshot multiplier.
//!
//! These are not `Formulas` entries; they live on Java's `IStatFunction`
//! interface and are folded into `PAttackFinalizer`, `MAttackFinalizer`,
//! `PDefenseFinalizer` and `MDefenseFinalizer` before those apply their stat
//! bonus and level mod. `Stat.SHOTS_BONUS` has a finalizer of its own and is
//! read by every shot-carrying damage formula.
//!
//! **The shape that matters in all four tables** is `k·enchant + 2k·max(0,
//! enchant − 3)`: the first three enchant levels pay `k` each, every level past
//! +3 pays three times that. It is why +4 is the wall retail players talk about.

use crate::data::item_data::kinds::{CrystalType, WeaponType};
use crate::data::item_data::{SLOT_HAIR, SLOT_HAIR2, SLOT_HAIRALL, SLOT_LR_HAND};

/// `IStatFunction.calcEnchantDefBonus`:
///
/// ```java
/// switch (item.getTemplate().getCrystalTypePlus())
/// {
///     case R: return ((2 * blessedBonus * enchant) + (6 * blessedBonus * Math.max(0, enchant - 3)));
///     default: return enchant + (3 * Math.max(0, enchant - 3));
/// }
/// ```
///
/// `blessedBonus` is `item.isBlessed() ? 1.5 : 1` and only the R arm reads
/// it. Every other grade takes the default arm, so below R the defence bonus
/// is **grade-independent**: a +4 D-grade helmet gains exactly what a +4
/// S-grade one does.
pub fn enchant_def_bonus(crystal: CrystalType, blessed: bool, enchant: i32) -> f64 {
    match crystal.plus() {
        CrystalType::R => {
            let b = blessed_bonus(blessed);
            2.0 * b * enchant as f64 + 6.0 * b * 0.max(enchant - 3) as f64
        }
        _ => enchant as f64 + 3.0 * 0.max(enchant - 3) as f64,
    }
}

/// `calcEnchantedItemBonus`'s `item.isBlessed() ? 1.5 : 1`.
fn blessed_bonus(blessed: bool) -> f64 {
    if blessed { 1.5 } else { 1.0 }
}

/// `IStatFunction.calcEnchantMatkBonus`:
///
/// ```java
/// case R: return ((5 * blessedBonus * enchant) + (10 * blessedBonus * Math.max(0, enchant - 3)));
/// case S: return (4 * enchant) + (8 * Math.max(0, enchant - 3));
/// case A: case B: case C: return (3 * enchant) + (6 * Math.max(0, enchant - 3));
/// default: return (2 * enchant) + (4 * Math.max(0, enchant - 3));
/// ```
///
/// Unlike the defence table this one *is* graded — and note that `default`
/// catches both D-grade and no-grade, so a gradeless staff still gains 2 per
/// level.
pub fn enchant_m_atk_bonus(crystal: CrystalType, blessed: bool, enchant: i32) -> f64 {
    let over = 0.max(enchant - 3) as f64;
    let enchant = enchant as f64;
    match crystal.plus() {
        CrystalType::R => {
            let b = blessed_bonus(blessed);
            5.0 * b * enchant + 10.0 * b * over
        }
        CrystalType::S => 4.0 * enchant + 8.0 * over,
        CrystalType::A | CrystalType::B | CrystalType::C => 3.0 * enchant + 6.0 * over,
        _ => 2.0 * enchant + 4.0 * over,
    }
}

/// `IStatFunction.calcEnchantedPAtkBonus` — the widest of the three, because it
/// splits each grade three ways:
///
/// ```java
/// case S:
///     if ((bodyPart == SLOT_LR_HAND) && (itemType != WeaponType.POLE))
///     {
///         if (itemType.isRanged()) return (10 * enchant) + (20 * Math.max(0, enchant - 3));
///         return (6 * enchant) + (12 * Math.max(0, enchant - 3));
///     }
///     return (5 * enchant) + (10 * Math.max(0, enchant - 3));
/// // A: 8 / 5 / 4 · B and C: 6 / 4 / 3 · default: ranged 4, else 2 (no two-hand split)
/// ```
///
/// The `SLOT_LR_HAND && !POLE` test is "two-handed, but a polearm doesn't
/// count": a two-handed sword out-scales a one-handed one, a bow out-scales
/// both, and a polearm — which occupies the same slot — is paid as if it were
/// one-handed. The gradeless arm has no two-hand split at all, only ranged vs
/// not.
pub fn enchant_p_atk_bonus(
    crystal: CrystalType,
    blessed: bool,
    body_part: i64,
    weapon_type: WeaponType,
    enchant: i32,
) -> f64 {
    let over = 0.max(enchant - 3) as f64;
    let e = enchant as f64;
    let two_hand = body_part == SLOT_LR_HAND && weapon_type != WeaponType::Pole;
    let ranged = matches!(
        weapon_type,
        WeaponType::Bow | WeaponType::Crossbow | WeaponType::TwoHandCrossbow
    );
    match crystal.plus() {
        CrystalType::R => {
            let b = blessed_bonus(blessed);
            if two_hand {
                if ranged {
                    12.0 * b * e + 24.0 * b * over
                } else {
                    7.0 * b * e + 14.0 * b * over
                }
            } else {
                6.0 * b * e + 12.0 * b * over
            }
        }
        CrystalType::S => {
            if two_hand {
                if ranged {
                    10.0 * e + 20.0 * over
                } else {
                    6.0 * e + 12.0 * over
                }
            } else {
                5.0 * e + 10.0 * over
            }
        }
        CrystalType::A => {
            if two_hand {
                if ranged {
                    8.0 * e + 16.0 * over
                } else {
                    5.0 * e + 10.0 * over
                }
            } else {
                4.0 * e + 8.0 * over
            }
        }
        CrystalType::B | CrystalType::C => {
            if two_hand {
                if ranged {
                    6.0 * e + 12.0 * over
                } else {
                    4.0 * e + 8.0 * over
                }
            } else {
                3.0 * e + 6.0 * over
            }
        }
        _ => {
            if ranged {
                4.0 * e + 8.0 * over
            } else {
                2.0 * e + 4.0 * over
            }
        }
    }
}

/// `IStatFunction.calcEnchantedItemBonus`'s per-item gate:
///
/// ```java
/// final int bodypart = item.getBodyPart();
/// if ((bodypart == SLOT_HAIR) || (bodypart == SLOT_HAIR2) || (bodypart == SLOT_HAIRALL))
/// {
///     if ((stat != Stat.PHYSICAL_DEFENCE) && (stat != Stat.MAGICAL_DEFENCE)) continue;
/// }
/// else if (item.getStats(stat, 0) <= 0)
/// {
///     continue;
/// }
/// ```
///
/// A **hair accessory counts for both defences whether or not it declares
/// one** — Java's own comment there says the client shows pDef while the scroll
/// promises mDef, and it resolves the disagreement by paying both. Everything
/// else has to already carry a positive value for the stat being finalized,
/// which is what keeps an enchanted necklace out of the P.Atk sum.
pub fn enchant_bonus_applies(body_part: i64, declares_stat: bool, is_defence_stat: bool) -> bool {
    if body_part == SLOT_HAIR || body_part == SLOT_HAIR2 || body_part == SLOT_HAIRALL {
        is_defence_stat
    } else {
        declares_stat
    }
}

/// The per-level rate of one `calcEnchantBodyPartBonus` override — `(plain,
/// blessed)`, each paid as `rate·max(e − 3, 0) + rate·max(e − 6, 0)`.
#[derive(Debug, Clone, Copy)]
pub struct BodyPartRate {
    pub plain: f64,
    pub blessed: f64,
}

/// `PAttackFinalizer` (chest): 2 / 3.
pub const BODY_PART_P_ATK: BodyPartRate = BodyPartRate {
    plain: 2.0,
    blessed: 3.0,
};
/// `MAttackFinalizer` (chest): 1.4 / 2.
pub const BODY_PART_M_ATK: BodyPartRate = BodyPartRate {
    plain: 1.4,
    blessed: 2.0,
};
/// `PCriticalRateFinalizer`, `MCritRateFinalizer`, `PSkillCritRateFinalizer`
/// (legs): 0.34 / 0.5.
pub const BODY_PART_CRIT: BodyPartRate = BodyPartRate {
    plain: 0.34,
    blessed: 0.5,
};
/// `PAccuracyFinalizer`, `MAccuracyFinalizer` (gloves) and
/// `PEvasionRateFinalizer`, `MEvasionRateFinalizer` (head): 0.2 / 0.3.
pub const BODY_PART_ACCURACY_EVASION: BodyPartRate = BodyPartRate {
    plain: 0.2,
    blessed: 0.3,
};
/// `SpeedFinalizer` (feet): 0.6 / 1.
pub const BODY_PART_SPEED: BodyPartRate = BodyPartRate {
    plain: 0.6,
    blessed: 1.0,
};

/// `IStatFunction.calcEnchantBodyPart` for one worn piece:
///
/// ```java
/// if ((item != null) && (item.getEnchantLevel() >= 4) && (item.getTemplate().getCrystalTypePlus() == CrystalType.R))
/// {
///     value += calcEnchantBodyPartBonus(item.getEnchantLevel(), item.getTemplate().isBlessed());
/// }
/// ```
///
/// R-grade armour enchanted to +4 or more adds a small flat amount to one stat
/// per slot. The finalizers add it to the base before their stat bonus, and
/// only for a player.
pub fn body_part_enchant_bonus(
    crystal: CrystalType,
    blessed: bool,
    enchant: i32,
    rate: BodyPartRate,
) -> f64 {
    if enchant < 4 || crystal.plus() != CrystalType::R {
        return 0.0;
    }
    let r = if blessed { rate.blessed } else { rate.plain };
    (r * 0.max(enchant - 3) as f64) + (r * 0.max(enchant - 6) as f64)
}

/// `ShotsBonusFinalizer`:
///
/// ```java
/// double baseValue = 1;
/// final Player player = creature.asPlayer();
/// if (player != null)
/// {
///     final Item weapon = player.getActiveWeaponInstance();
///     if ((weapon != null) && weapon.isEnchanted())
///     {
///         baseValue += (weapon.getEnchantLevel() * 0.7) / 100;
///     }
///     if (isMageCaster(player)) { if (player.getActiveShappireJewel() != null) baseValue += …getBonus(); }
///     else if (player.getActiveRubyJewel() != null) baseValue += …getBonus();
/// }
/// return Stat.defaultValue(creature, stat, baseValue);
/// ```
///
/// `Stat.SHOTS_BONUS` feeds every soulshot/spiritshot term in the game — the
/// additive `2 + SB/100` in the auto-attack, blow and physical-skill handlers,
/// `mAtkMul` in `calcMagicDam`/`calcManaDam`, and the multiplicative one in
/// `Heal`/`HpCpHeal`. The brooch jewels only reach it once item skills land,
/// and no skill or item declares a `shotBonus` modifier, so
/// `Stat.defaultValue`'s mul/add pair is the identity.
///
/// `asPlayer()` is null for a servitor, which keeps its shots at a flat 1.
pub fn shots_bonus(weapon_enchant_level: i32) -> f64 {
    let mut base = 1.0;
    if weapon_enchant_level > 0 {
        base += (weapon_enchant_level as f64 * 0.7) / 100.0;
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only the R arms read `blessedBonus`: `2·1.5·6 + 6·1.5·3` = 45 against
    /// a plain 30, while S keeps the shared default arm either way.
    #[test]
    fn blessed_scales_only_the_r_arms() {
        assert_eq!(enchant_def_bonus(CrystalType::R, false, 6), 30.0);
        assert_eq!(enchant_def_bonus(CrystalType::R95, true, 6), 45.0);
        assert_eq!(enchant_def_bonus(CrystalType::S, true, 6), 15.0);
        // M.Atk +4: 5·1.5·4 + 10·1.5·1 = 45.
        assert_eq!(enchant_m_atk_bonus(CrystalType::R, true, 4), 45.0);
        assert_eq!(enchant_m_atk_bonus(CrystalType::S, true, 4), 24.0);
        // Two-handed bow +5: 12·5 + 24·2 = 108, ×1.5 blessed.
        let bow = |blessed| {
            enchant_p_atk_bonus(CrystalType::R, blessed, SLOT_LR_HAND, WeaponType::Bow, 5)
        };
        assert_eq!(bow(false), 108.0);
        assert_eq!(bow(true), 162.0);
    }

    /// `calcEnchantBodyPart`: R-grade only, +4 and up, `r·(e−3) + r·max(e−6,
    /// 0)`.
    #[test]
    fn body_part_bonus_starts_at_plus_four_on_r_grade() {
        assert_eq!(
            body_part_enchant_bonus(CrystalType::R, false, 3, BODY_PART_P_ATK),
            0.0
        );
        assert_eq!(
            body_part_enchant_bonus(CrystalType::R, false, 4, BODY_PART_P_ATK),
            2.0
        );
        assert_eq!(
            body_part_enchant_bonus(CrystalType::S84, false, 10, BODY_PART_P_ATK),
            0.0
        );
        // Blessed legs +7: 0.5·4 + 0.5·1.
        assert_eq!(
            body_part_enchant_bonus(CrystalType::R, true, 7, BODY_PART_CRIT),
            2.5
        );
        // R99 boots +8: 0.6·5 + 0.6·2.
        assert!(
            (body_part_enchant_bonus(CrystalType::R99, false, 8, BODY_PART_SPEED) - 4.2).abs()
                < 1e-12
        );
    }

    /// `ShotsBonusFinalizer`: `1 + enchant·0.7/100`.
    #[test]
    fn shots_bonus_is_seven_tenths_of_a_percent_per_level() {
        assert_eq!(shots_bonus(0), 1.0);
        assert!((shots_bonus(10) - 1.07).abs() < 1e-12);
    }
}
