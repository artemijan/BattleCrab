//! XP/SP distribution for a party's damage share of one kill.

// ---------------------------------------------------------------------------
// Party rewards (`Party.distributeXpAndSp` + `distributeItem`/`distributeAdena`)
// ---------------------------------------------------------------------------

/// XP/SP for one party's damage share against one kill — the party branch of
/// `Attackable.calculateRewards` + `Party.distributeXpAndSp`.
/// `base_exp`/`base_sp` already carry `partyDmg/totalDamage × partyMul ×
/// level-gap` (the caller ports `calculateExpAndSp`); this adds the party
/// bonus ladder and splits by level².
use crate::world::World;
pub(crate) fn distribute_xp_and_sp(
    world: &mut World,
    rewarded: &[(i32, i32)], // (object_id, level), alive + in range
    top_level: i32,
    base_exp: f64,
    base_sp: f64,
    // The killed monster's template — needed for the per-member vitality
    // charge (`target.getVitalityPoints(...)` in Java's loop).
    target: &crate::data::npc_data::NpcTemplate,
    // Java `Attackable.useVitalityRate()` — false for a champion unless
    // `ChampionEnableVitality`. It gates three things at once: the bonus
    // multiplier inside `addExpAndSp`, the vitality charge, and the PA points.
    use_vitality_rate: bool,
) {
    let cfg = &world.cfg.character;
    let valid = crate::model::party::valid_members(
        rewarded,
        top_level,
        &cfg.party_xp_cutoff_method,
        cfg.party_xp_cutoff_level,
        cfg.party_xp_cutoff_percent,
    );
    let xp_reward =
        base_exp * crate::model::party::exp_sp_bonus(valid.len(), world.cfg.rates.rate_party_xp);
    let sp_reward =
        base_sp * crate::model::party::exp_sp_bonus(valid.len(), world.cfg.rates.rate_party_sp);
    let sq_level_sum: f64 = rewarded
        .iter()
        .filter(|(id, _)| valid.contains(id))
        .map(|&(_, l)| (l as f64) * (l as f64))
        .sum();
    if sq_level_sum <= 0.0 {
        return;
    }

    let highfive = cfg.party_xp_cutoff_method == "highfive";
    let (gaps, percents) = (
        cfg.party_xp_cutoff_gaps.clone(),
        cfg.party_xp_cutoff_gap_percents.clone(),
    );
    for &(member, level) in rewarded {
        if !valid.contains(&member) {
            continue; // Java: `member.addExpAndSp(0, 0)` — a no-op here.
        }
        let pre = (level as f64) * (level as f64) / sq_level_sum;
        let mut xp = xp_reward * pre;
        let mut sp = sp_reward * pre;
        // `calculateExpSpPartyCutoff`: premium rates first, then the cutoff.
        if crate::game_loop::admin::premium::has_premium_status(world, member) {
            xp *= world.cfg.premium.rate_xp;
            sp *= world.cfg.premium.rate_sp;
        }
        if highfive {
            match crate::model::party::highfive_cutoff_percent(top_level - level, &gaps, &percents)
            {
                Some(pct) => {
                    xp = xp * pct as f64 / 100.0;
                    sp = sp * pct as f64 / 100.0;
                }
                None => continue, // outside every gap range: nothing at all
            }
        }
        crate::game_loop::death::add_exp_and_sp(world, member, xp, sp, use_vitality_rate);
        // Java charges each rewarded member's vitality on the post-cutoff xp,
        // and awards that member's PA points from the same value — both inside
        // the same `if (useVitalityRate())`.
        if xp > 0.0 && use_vitality_rate {
            crate::game_loop::death::consume_kill_vitality(world, member, level, target, xp);
            crate::game_loop::character::pc_cafe::give_point(world, member, xp);
        }
    }
}
