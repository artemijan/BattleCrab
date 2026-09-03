//! Formulas that scale a result by the level gap or the config multipliers
//! rather than by combat stats, plus `calcAtkBreak`.

/// `Attackable.calculateExpAndSp`'s level-gap multiplier (the
/// "4gameforum" table): full reward through +2 levels above the mob,
/// tapering to 5% at +10 and beyond.
pub fn exp_sp_level_gap_multiplier(attacker_level: i32, npc_level: i32) -> f64 {
    match attacker_level - npc_level {
        i32::MIN..=2 => 1.0,
        3 => 0.97,
        4 => 0.80,
        5 => 0.61,
        6 => 0.37,
        7 => 0.22,
        8 => 0.13,
        9 => 0.08,
        _ => 0.05,
    }
}

/// `Util.map(value, min, max, targetMin, targetMax)` with the clamping Java's
/// `constrain` performs first — used by the drop level-gap chances.
pub fn map_range(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    let value = value.clamp(from_min.min(from_max), from_min.max(from_max));
    (value - from_min) * (to_max - to_min) / (from_max - from_min) + to_min
}

/// `Formulas.calcAtkBreak`. `men_bonus` is the target's `BaseStat.MEN` bonus;
/// `roll` is `Rnd.get(100)`. `cancel_add`/`cancel_mul` are the target's
/// `Stat.ATTACK_CANCEL` modifiers (Java `getStat().getValue(ATTACK_CANCEL,
/// init)`), which buffs like Concentration lower. The raid/HP-blocked/
/// channeling guards still don't apply to players yet.
///
/// `applies` is Java's `init > 0` test — it starts at **0** and only reaches
/// 15 when a branch of `AltGameCancelByHit` matches what the target is doing
/// (`ALT_GAME_CANCEL_CAST` while casting abortably, `ALT_GAME_CANCEL_BOW`
/// while mid-shot with a bow). `init <= 0` returns `false` outright, so with
/// the key set to neither, damage interrupts nothing at all. The port used to
/// hardcode the 15, which meant a cast was always interruptible however the
/// key was set.
pub fn calc_atk_break(
    dmg: f64,
    men_bonus: f64,
    roll: i32,
    cancel_add: f64,
    cancel_mul: f64,
    applies: bool,
) -> bool {
    if !applies {
        return false;
    }
    let init = 15.0 + (13.0 * dmg).sqrt() - (men_bonus * 100.0 - 100.0);
    let rate = (init * cancel_mul + cancel_add).clamp(1.0, 99.0);
    (roll as f64) < rate
}

/// `Formulas.calculatePvpPveBonus` — a multiplier every damage formula ends
/// with, and one this port previously hard-coded to 1.0 in three places
/// (`calc_auto_attack_damage`, `calc_physical_skill_damage` and
/// `calc_magic_dam` all carry a comment saying "pvp-pve mods 1.0"). That was
/// true only while nothing granted the stats; **~1300 dist effects grant
/// them**, so it stopped being true the moment any of them landed.
///
/// The shape is a *difference of multipliers* rather than a product: each side
/// merges as a `mul` (`amount 5` → 1.05), and the result is
/// `max(0.05, 1 + (attackMul − defenceMul))`. Two +5 % buffs on opposite sides
/// therefore cancel exactly.
///
/// Which pair is read depends on the pairing and the delivery:
/// - **playable vs playable** → the `PVP_*` triple;
/// - **either side `Attackable`** → the `PVE_*` triple, times the
///   level-difference penalty below;
/// - anything else (two NPCs that are not attackable, a door) → 1.0.
///
/// and within each, `skill = None` (an auto-attack) reads the
/// `*_PHYSICAL_ATTACK_*` pair, a magic skill the `*_MAGICAL_SKILL_*` pair, and
/// a physical skill the `*_PHYSICAL_SKILL_*` one.
///
/// Two upstream slips are ported as written because changing them would be a
/// silent divergence, and both are inert on this dist:
/// - Java binds `targetPlayer = attacker.getActingPlayer()` — the *attacker*
///   again — and then uses it only to index class-balance config arrays, which
///   are empty here (`Custom/ClassBalance.ini` ships every multiplier blank),
///   so every class multiplier is 1.0 and the slip cannot be observed.
/// - the raid `*_DEFENCE` terms are read off the **attacker**, not the target.
///
/// Dragon weapons (`DRAGON_WEAPON_DEFENCE`) do not exist in Interlude.
#[allow(clippy::too_many_arguments)]
pub fn calculate_pvp_pve_bonus(
    attack_mul: f64,
    defence_mul: f64,
    raid_attack_mul: f64,
    raid_defence_mul: f64,
    pve_penalty: f64,
) -> f64 {
    (1.0 + ((attack_mul * raid_attack_mul) - (defence_mul * raid_defence_mul))) * pve_penalty
}

/// The PvE half's level-difference penalty (`Config.NPC_SKILL_DMG_PENALTY`,
/// `0.8, 0.6, 0.5, 0.42, 0.36, 0.32, 0.28, 0.25` on this dist — steeper than
/// Mobius' four-entry default, and previously unparsed here).
///
/// It bites only when the **target** is a non-raid NPC of at least
/// `MinNPCLevelForDmgPenalty` (78) that is 2+ levels above the attacking
/// player: `levelDiff − 1` indexes the table, and past its end the last entry
/// (a flat ×0.25) applies. Note the two sibling tables, `DmgPenaltyForLvL-`
/// and `CritDmgPenaltyForLvLDifferences`, are parsed by Java's `Config` and
/// then read by **nothing** — dead config on both sides.
pub fn npc_level_damage_penalty(
    table: &[f64],
    target_level: i32,
    attacker_level: i32,
    target_is_raid: bool,
    min_npc_level: i32,
) -> f64 {
    if target_is_raid || target_level < min_npc_level || (target_level - attacker_level) < 2 {
        return 1.0;
    }
    if table.is_empty() {
        return 1.0;
    }
    let idx = (target_level - attacker_level - 1) as usize;
    table[idx.min(table.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The level-gap XP table: full through +2, tapering to 5%.
    #[test]
    fn exp_gap_table() {
        assert_eq!(exp_sp_level_gap_multiplier(10, 20), 1.0);
        assert_eq!(exp_sp_level_gap_multiplier(12, 10), 1.0);
        assert_eq!(exp_sp_level_gap_multiplier(13, 10), 0.97);
        assert_eq!(exp_sp_level_gap_multiplier(19, 10), 0.08);
        assert_eq!(exp_sp_level_gap_multiplier(40, 10), 0.05);
    }

    /// `Util.map` with constrain: the drop level-gap scaling from 100% at
    /// −min to the floor at −max, clamped outside.
    #[test]
    fn map_range_matches_util_map() {
        assert!((map_range(-8.0, -15.0, -8.0, 10.0, 100.0) - 100.0).abs() < 1e-9);
        assert!((map_range(-15.0, -15.0, -8.0, 10.0, 100.0) - 10.0).abs() < 1e-9);
        assert!(
            (map_range(-20.0, -15.0, -8.0, 10.0, 100.0) - 10.0).abs() < 1e-9,
            "clamped below"
        );
        assert!(
            (map_range(3.0, -15.0, -8.0, 10.0, 100.0) - 100.0).abs() < 1e-9,
            "clamped above"
        );
    }

    /// Neutral MEN (bonus 1.0): rate = 15 + √(13·dmg), clamped to [1, 99].
    #[test]
    fn atk_break_rate_and_clamps() {
        // dmg 100 → 15 + √1300 ≈ 51.06: roll 51 breaks, 52 doesn't.
        assert!(calc_atk_break(100.0, 1.0, 51, 0.0, 1.0, true));
        assert!(!calc_atk_break(100.0, 1.0, 52, 0.0, 1.0, true));
        // Huge MEN bonus can't push the rate below 1%.
        assert!(calc_atk_break(0.0, 2.0, 0, 0.0, 1.0, true));
        assert!(!calc_atk_break(0.0, 2.0, 1, 0.0, 1.0, true));
        // Massive damage caps at 99% — roll 99 still survives.
        assert!(!calc_atk_break(1e9, 1.0, 99, 0.0, 1.0, true));
    }

    /// `Stat.ATTACK_CANCEL`: Concentration's -18 DIFF lowers the interrupt rate
    /// (≈51.06 → ≈33.06), so a roll of 40 now survives where it broke before.
    #[test]
    fn atk_break_honors_cancel_stat() {
        assert!(
            calc_atk_break(100.0, 1.0, 40, 0.0, 1.0, true),
            "no buff: 40 < 51.06 breaks"
        );
        assert!(
            !calc_atk_break(100.0, 1.0, 40, -18.0, 1.0, true),
            "Concentration: 40 > 33.06 survives"
        );
    }
}
