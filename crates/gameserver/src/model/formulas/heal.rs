//! `calcHeal` — the healed amount, and the caster kinds that scale it.

/// Which of `Heal.instant`'s three caster tests the effector answers to. Java
/// asks `isPlayer() && isMageClass()`, `isSummon()` and `isNpc()` in that order
/// and the arms differ in *both* their `mAtkMul` and their static bonus, so a
/// single "is it a player" flag cannot stand in for the choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealCaster {
    /// `isPlayer() && getClassId().isMage()`.
    PlayerMage,
    /// A player whose class is not a mage class.
    PlayerFighter,
    /// `isSummon()` — and note this one takes the shot branch **with no shot
    /// charged**, which is the only arm that does.
    Summon,
    /// A plain NPC: `isNpc()` and not a summon.
    Npc,
}

/// `handlers/effecthandlers/Heal.java` `instant()`, the amount half.
/// `HEAL_EFFECT`/`HEAL_EFFECT_ADD` and the healing-skill config multiplier are
/// the caller's, as is the overheal clamp; a magic crit triples the result.
///
/// ```java
/// double staticShotBonus = 0;
/// double mAtkMul = 1;
/// final double shotsBonus = effector.getStat().getValue(Stat.SHOTS_BONUS);
/// if (((sps || bss) && (effector.isPlayer() && effector.getActingPlayer().isMageClass())) || effector.isSummon())
/// {
///     staticShotBonus = skill.getMpConsume();
///     mAtkMul = bss ? 4 * shotsBonus : 2 * shotsBonus;
///     staticShotBonus *= bss ? 2.4 : 1.0;
/// }
/// else if ((sps || bss) && effector.isNpc())
/// {
///     staticShotBonus = 2.4 * skill.getMpConsume();
///     mAtkMul = 4 * shotsBonus;
/// }
/// else
/// {
///     if (weaponInst != null) mAtkMul = S84 ? 4 : S80 ? 2 : 1;
///     mAtkMul = bss ? mAtkMul * 4 : mAtkMul + 1;
/// }
/// amount += staticShotBonus + Math.sqrt(mAtkMul * effector.getMAtk());
/// ```
///
/// Three things separate the arms, and all three were collapsed here before:
///
/// * the **NPC** arm reaches `4 · shotsBonus` on *plain* spiritshots, where the
///   others need blessed ones, and pays `2.4 × mpConsume` unconditionally —
///   Java's comment calls it "always blessed spiritshots";
/// * the **summon** arm fires with **no shot charged at all**, so a servitor's
///   heal always takes the static bonus;
/// * the `else` arm reaches the same 4/2 by the *grade* road (no Interlude
///   weapon is S80/S84, so its `mAtkMul` starts at 1) and takes **no**
///   `shotsBonus` — which is what made it look interchangeable with the mage
///   arm for as long as `SHOTS_BONUS` was hard-coded to 1.
pub fn calc_heal(
    power: f64,
    m_atk: f64,
    mcrit: bool,
    sps: bool,
    bss: bool,
    mp_consume: i32,
    caster: HealCaster,
    shots_bonus: f64,
) -> f64 {
    let shot = sps || bss;
    let (static_bonus, m_atk_mul) =
        if (shot && caster == HealCaster::PlayerMage) || caster == HealCaster::Summon {
            (
                mp_consume as f64 * if bss { 2.4 } else { 1.0 },
                if bss { 4.0 } else { 2.0 } * shots_bonus,
            )
        } else if shot && caster == HealCaster::Npc {
            (2.4 * mp_consume as f64, 4.0 * shots_bonus)
        } else {
            // `mAtkMul` starts at the weapon's crystal grade, which is 1 for
            // everything this chronicle ships (S80/S84 are post-Interlude), so
            // `bss ? 1 * 4 : 1 + 1`.
            (0.0, if bss { 4.0 } else { 2.0 })
        };
    (power + static_bonus + (m_atk_mul * m_atk).sqrt()) * if mcrit { 3.0 } else { 1.0 }
}

// ---------------------------------------------------------------------------
// Physical auto-attack formulas (G9). Dropped terms, all identity for the
// actors that exist (unarmed/starting-gear players without parsed item
// `<stats>`, plain monsters): soulshots (`ss = false`, `SHOTS_BONUS`),
// shield defence (`calcShldUse` needs the un-parsed shield `sDef` stat —
// always 0/no-block), ranged/dual weapon branches, `ATTACK_COUNT_MAX`
// polearm sweeps, trait/attribute/pvp-pve multipliers, `CRITICAL_DAMAGE`/
// `CRITICAL_DAMAGE_ADD` stats (base 1/0), `HitConditionBonus`'s night/rain
// terms (no game clock/weather).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Heal: power 83, mAtk 50 → 83 + √100 = 93; crit triples.
    #[test]
    fn heal_matches_java_formula() {
        use HealCaster::{Npc, PlayerFighter, PlayerMage, Summon};
        assert!(
            (calc_heal(83.0, 50.0, false, false, false, 0, PlayerMage, 1.0) - 93.0).abs() < 1e-9
        );
        assert!(
            (calc_heal(83.0, 50.0, true, false, false, 0, PlayerMage, 1.0) - 279.0).abs() < 1e-9
        );
        // Spiritshot on a mage caster adds the MP-consume static bonus (sqrt
        // term unchanged at ×2): 83 + 40 + √100 = 133.
        assert!(
            (calc_heal(83.0, 50.0, false, true, false, 40, PlayerMage, 1.0) - 133.0).abs() < 1e-9
        );
        // Blessed spiritshot: sqrt term ×4 (√200) and static ×2.4: 83 + 96 + √200.
        assert!(
            (calc_heal(83.0, 50.0, false, false, true, 40, PlayerMage, 1.0)
                - (83.0 + 96.0 + 200.0_f64.sqrt()))
            .abs()
                < 1e-9
        );

        // The three arms Java keeps apart, on the same inputs (plain
        // spiritshot, mpConsume 40, `SHOTS_BONUS` 1.03):
        // - a **fighter** falls through to the grade arm: no static bonus, ×2,
        //   and no shots bonus at all;
        let grade_arm = 83.0 + (2.0 * 50.0f64).sqrt();
        assert!(
            (calc_heal(83.0, 50.0, false, true, false, 40, PlayerFighter, 1.03) - grade_arm).abs()
                < 1e-9
        );
        // - an **NPC** gets `2.4 × mpConsume` and ×4 even on a *plain* shot;
        assert!(
            (calc_heal(83.0, 50.0, false, true, false, 40, Npc, 1.03)
                - (83.0 + 96.0 + (4.0 * 1.03 * 50.0f64).sqrt()))
            .abs()
                < 1e-9
        );
        // - a **summon** takes the mage arm **with no shot charged**.
        assert!(
            (calc_heal(83.0, 50.0, false, false, false, 40, Summon, 1.03)
                - (83.0 + 40.0 + (2.0 * 1.03 * 50.0f64).sqrt()))
            .abs()
                < 1e-9
        );
    }
}
