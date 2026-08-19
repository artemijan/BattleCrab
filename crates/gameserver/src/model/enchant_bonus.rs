//! `IStatFunction.calcEnchantedItemBonus` and its three tables — the extra
//! P.Atk / M.Atk / P.Def / M.Def an **enchanted** piece of gear contributes, and
//! `ShotsBonusFinalizer`'s enchant-scaled soulshot/spiritshot multiplier.
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

use crate::data::item_data::{
    CrystalType, SLOT_HAIR, SLOT_HAIR2, SLOT_HAIRALL, SLOT_LR_HAND, WeaponType,
};

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
/// The R arm is unreachable on this dist (no item is R-grade or above), which
/// is also why `blessedBonus` — the only thing `isBlessed()` feeds — never
/// leaves 1.0 and is not carried here. Every grade Interlude ships takes the
/// default arm, so the defence bonus is **grade-independent**: a +4 D-grade
/// helmet gains exactly what a +4 S-grade one does.
pub fn enchant_def_bonus(crystal: CrystalType, enchant: i32) -> f64 {
    match crystal.plus() {
        CrystalType::R => 2.0 * enchant as f64 + 6.0 * 0.max(enchant - 3) as f64,
        _ => enchant as f64 + 3.0 * 0.max(enchant - 3) as f64,
    }
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
pub fn enchant_m_atk_bonus(crystal: CrystalType, enchant: i32) -> f64 {
    let over = 0.max(enchant - 3) as f64;
    let enchant = enchant as f64;
    match crystal.plus() {
        CrystalType::R => 5.0 * enchant + 10.0 * over,
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
    body_part: i32,
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
            if two_hand {
                if ranged {
                    12.0 * e + 24.0 * over
                } else {
                    7.0 * e + 14.0 * over
                }
            } else {
                6.0 * e + 12.0 * over
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
pub fn enchant_bonus_applies(body_part: i32, declares_stat: bool, is_defence_stat: bool) -> bool {
    if body_part == SLOT_HAIR || body_part == SLOT_HAIR2 || body_part == SLOT_HAIRALL {
        is_defence_stat
    } else {
        declares_stat
    }
}

/// `ShotsBonusFinalizer`:
///
/// ```java
/// double baseValue = 1;
/// final Player player = creature.getActingPlayer();
/// if (player != null)
/// {
///     final Item weapon = player.getActiveWeaponInstance();
///     if ((weapon != null) && weapon.isEnchanted())
///     {
///         baseValue += (weapon.getEnchantLevel() * 0.3) / 100;
///     }
///     if (player.getActiveRubyJewel() != null) baseValue += player.getActiveRubyJewel().getBonus();
/// }
/// return Stat.defaultValue(creature, stat, baseValue);
/// ```
///
/// `Stat.SHOTS_BONUS` multiplies **every** soulshot/spiritshot term in the game
/// — `ssmod` in the auto-attack and the six physical-skill handlers, `mAtkMul`
/// in `calcMagicDam`/`calcManaDam`/`Heal`/`HpCpHeal`. A +10 weapon therefore
/// buys 3 % on top of the flat ×2, and nothing else on this dist moves it: the
/// ruby brooch jewel is post-Interlude, and no skill or item declares a
/// `shotBonus` modifier, so `Stat.defaultValue`'s mul/add pair is the identity.
///
/// `getActingPlayer()` is the **owner** for a servitor, so a summon's shots ride
/// its master's weapon enchant; it is null for a plain NPC, which keeps them at
/// a flat 1.
pub fn shots_bonus(weapon_enchant_level: i32) -> f64 {
    let mut base = 1.0;
    if weapon_enchant_level > 0 {
        base += (weapon_enchant_level as f64 * 0.3) / 100.0;
    }
    base
}
